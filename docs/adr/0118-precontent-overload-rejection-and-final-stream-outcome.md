# ADR-0118: 首个语义输出前的明确过载拒绝与最终流结果

- 状态：Accepted
- 日期：2026-08-05
- 决策者：maintainer

## 背景

部分 Responses 上游会先以 HTTP 200 发送 `response.created`、`response.in_progress` 等生命周期事件，随后在没有任何内容、推理、工具调用或其他语义输出时发送带精确 `server_is_overloaded` code 的失败事件。当前 Runtime 在首个生命周期事件处提交下游响应，因而只能把失败事件转发并结束 Body；客户端看到的是 200 后断流并自行重连，RequestLog 和历史统计又可能只按 2xx 把这次真实失败显示为成功。

“下游尚未提交”本身仍不能证明生成请求未执行，ADR-0093 的保守边界继续有效。本决策只承认 Provider 协议明确声明的过载拒绝，并要求失败前不存在任何语义事件；它不是自然语言错误猜测，也不是通用的成功状态后重试。

## 决策

- ProtocolAdapter 为 SSE 事件提供两个与具体 Provider 解耦的旁路信号：该事件是否是可暂缓提交的生命周期控制事件，以及失败事件是否命中受审计的精确 `Overloaded` 拒绝代码。Runtime 不按 Provider、JSON 路径或错误文案分支。
- OpenAI Responses 只从已声明错误 envelope 的精确 `error.code = "server_is_overloaded"` 识别该拒绝；Anthropic Messages 只从精确 `error.type = "overloaded_error"` 识别同类拒绝。缺字段、未知 code/type、自然语言“overloaded”或任意递归搜索都不匹配。
- 只有 `response.created`、`response.in_progress`、协议 keepalive/ping 等明确列出的生命周期事件可以在预提交阶段继续缓冲。内容 delta、reasoning、tool/function call、output item、未知事件以及任何其他可能承载语义的事件立即锁定 Attempt。未知事件默认不透明且不可暂缓。
- 若在锁定前收到精确过载失败事件，Runtime 丢弃当前 Attempt 尚未下发的控制帧和该失败帧，把它作为窄化的 `RejectedBeforeExecution` 证据交回既有重试循环。未绑定请求排除当前凭据后立即重选；已绑定请求仍只能重试原凭据。总尝试次数、切换次数、绝对 deadline、取消与 RPM 规则全部复用现有预算。
- 该错误证明代理、连接与 Endpoint 已经成功返回协议响应，因此不会把共享 Endpoint 或 Proxy 标记为故障；当前请求仍排除失败凭据，Attempt 记录为上游失败。若重试预算耗尽，Runtime 在下游仍未提交时返回入口协议的本地上游错误，而不是伪造 200 成功流。
- 续接 ID、桥状态和会话绑定只在真正锁定 Attempt 时提交。被丢弃的控制事件不得留下指向失败凭据或上游 Response ID 的绑定。
- 预提交缓冲继续受现有字节和时间预算约束。只有控制事件而预算到期时，Runtime 仍按既有预提交超时失败并丢弃尚未下发的控制帧，不得在 deadline 后写入续接绑定，也不能为了等待可重试失败而无限延迟首帧。
- 一旦任何语义事件出现、下游响应头/字节已经提交，或者错误不是精确允许项，现有 at-most-once 边界立即恢复：失败事件按协议转发或 Body 终止，不切换凭据、不拼接第二条流。
- 这是针对明确上游准入拒绝的产品级窄化判断，不是对所有 5xx 或成功响应头后失败的重新分类。即使没有客户端可见语义输出，上游仍可能消耗少量不可观察计算；我们接受该有限代价以消除明确过载造成的客户端重连，但不接受工具调用、内容或未知事件后的重复执行。
- RequestLog 的管理响应派生粗粒度 `success | failed | cancelled`：只有 2xx 且最终 `error_class` 为空才成功。Attempt 采用同一粗粒度展示，不暴露内部 `RetrySafety`、错误分类或精确 Attempt outcome。Gateway Key、Provider API Key、OAuthAccount 与系统总览的成功/失败聚合使用相同最终结果语义。

## 后果

- Codex/Responses 上游在首内容前明确返回 `server_is_overloaded` 时，可以由 any2api 内部换账号完成请求，客户端不会先收到失败 Attempt 的生命周期事件。
- 已经开始输出的流仍绝不自动切换，避免两条生成流拼接或重复工具调用。
- HTTP 200 只描述响应头已经建立，不再被管理面误当作完整流成功；断流和协议失败可在请求日志中直接识别。
- Protocol 增加少量安全元数据，Runtime 只消费通用信号，不引入按 Provider 增长的中央 `match`。

## 验证

- Protocol 测试覆盖精确允许 code/type、未知值和自然语言不误判，并覆盖生命周期事件与语义事件的提交属性。
- Runtime 测试覆盖控制帧后过载拒绝仍处于 Pending、丢弃失败 Attempt 的全部帧、续接绑定不泄漏、重选后只向客户端交付成功 Attempt，以及内容出现后的同类错误不得重试。
- RequestLog DTO、Web 展示与 Storage 聚合测试覆盖 `HTTP 200 + stream_error` 显示和统计为失败，取消保持独立状态，普通 2xx 完整流仍为成功。
