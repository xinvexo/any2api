# ADR-0013: 提交前重试、错误分类与代际健康状态

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer
- 修订：ADR-0093 将 `408`、`425` 明确为执行前拒绝，并保留 5xx 与成功响应头后的 Body/SSE 失败为 `Ambiguous`
- 修订：ADR-0094 仅在健康 Guard 竞争失败且 Attempt 尚未开始时精确回滚 RPM 预留
- 修订：ADR-0095 将认证错误与账号/路由健康拆成独立代际
- 修订：2026-08-03 将指数退避限定为同一凭据的绑定重试，候选切换立即重选
- 修订：2026-08-03 将上游错误 kind 的默认 RetrySafety 收敛为 Domain 唯一映射

## 背景

提交前重试需要可靠地区分 Credential、模型、Endpoint 与 Proxy 故障，使用 `Retry-After`，并在下游尚未提交且执行安全性允许时切换候选。该内部分类只服务调度与健康状态，不能改变最终上游 HTTP 响应的公开语义。

可靠性实现不能把所有失败统一冷却 Credential，也不能为了成功率重复执行语义不明确的生成请求。健康状态还必须遵守配置代际隔离和进程重启清空的项目边界。

## 决策

- 在 domain 增加 `UpstreamErrorKind`、`UpstreamErrorClassification` 与 `RetryAfterHint`。Provider Driver 只分类上游 HTTP 响应；Transport 继续独立产生带阶段的 `TransportError + RetrySafety`。
- Codex、Claude 与 Grok 各自在独立错误模块解析自身受限错误 envelope；共享模块处理标准 HTTP 状态和 `Retry-After`，Claude 错误模块还启用官方 SDK 使用的 `retry-after-ms` 扩展，Runtime 不按 `ProviderKind` 分支。分类结果不得生成、替换或补充客户端可见的状态、类型或消息。
- `UpstreamErrorKind::default_retry_safety()` 是所有 Provider 共用的唯一 kind 默认表：认证、权限、额度、限流、模型和操作拒绝为 `RejectedBeforeExecution`，请求错误、临时错误和未知错误为 `Ambiguous`。HTTP `408`/`425` 等状态级未执行证据可以在共享分类器中覆盖默认值；Provider envelope 只在相容细化得到新的拒绝 kind 时使用其领域默认值，否则必须保留 HTTP 基线已有的更强证据。Provider 私有模块不得复制 safety match。
- 普通 HTTP 400 保持 `InvalidRequest`，但 Provider 从已声明 envelope 的精确 code/type 得到 `QuotaExhausted` 时允许做相容细化并按执行前拒绝处理。禁止扫描自然语言余额文案、任意嵌套 JSON 或模糊子串；最终上游状态、Header 与正文仍透明返回。具体 code 集合和兼容边界见 ADR-0086。
- `CredentialGenerationRuntime` 持有 authentication-version-scoped 认证错误，并引用 routing-generation-scoped 的权限、额度与模型健康。仅认证材料轮换会隔离 `auth_error` 并复用同一账号身份的其他健康；Secret/Endpoint 或账号路由身份变化时全部换代。完整边界见 ADR-0095。
- `RuntimeRegistry` 按 `(EndpointId, config_version)` 与 `(ProxyId, config_version)` 复用健康句柄。PublishedSnapshot 固定引用与自身配置版本一致的句柄；退役代际 Attempt 的迟到结果只能更新对应退役句柄。
- Endpoint 与 Proxy 使用相互独立的滑动窗口熔断器。进入 Open 后，到期只允许受 `breaker.half_open_max_probes` 限制的探测；探测成功关闭，失败重新打开。
- Transport 阶段与健康归因显式分离。Runtime 只根据 `TransportFailureScope` 更新 Endpoint/Proxy；reqwest 无法可靠区分的 CONNECT、SOCKS 或目标 TLS 故障使用 `Unattributed`，不污染共享健康状态，请求内只排除当前 Credential。
- Candidate 选择同时检查 Credential、模型、Endpoint 与 Proxy 的动态可用性，并原子预留 RPM 名额。HalfOpen 探测 Permit 与运行态 Guard 都由 `SelectedCandidate` 持有；健康状态在 Guard 结算前发布，随后统一推进 scheduler epoch。
- HalfOpen 探测名额在健康预检查后被并发请求抢占时，选择器按唯一令牌原子回滚尚未开始 Attempt 的 RPM 预留、结算已经取得的运行态 Guard、移除该候选并继续检查同 tier 其他候选。回滚边界与并发证明见 ADR-0094；一旦健康 Guard 获取成功，后续任何失败都继续保守计数。
- 冷却和 Open 到期通过 `SchedulerEpoch` 唯一的 keyed-slot worker 推进统一 epoch。每个实际健康状态边界只占一个可替换 slot，同一错误重复发生不会增加 Tokio task；slot 清除或运行态 Drop 时撤销。worker 保留较晚 deadline 并始终按最早值重排，到期批量推进一次 epoch。等待者继续直接监听本轮 `retry_at`，并使用现有 QueueTicket、超时、取消和最大等待数量，不为健康状态增加第二套队列。
- Public request 使用显式多 Attempt 循环。每次失败产生只供内部使用的类型化 `AttemptFailure`，先更新健康状态，再结算当前 Guard，最后由 `RetryBudget` 判断是否退避和重新选择。
- 指数退避按 `RoutingCredentialId` 的当前请求 Attempt 计数隔离。已绑定失败的下一 Attempt 必然仍使用原凭据，因此使用该凭据自身的 `base_delay × 2^n`并限制到 `max_delay`；未绑定失败会先排除当前 Credential/Endpoint/Proxy，下一候选立即重选，既不继承旧凭据的指数，也不预付新凭据的基础延迟。两类路径仍共享绝对提交前 deadline。
- 自动重试必须同时满足：CommitState 仍为 Pending、`RetrySafety::allows_automatic_retry()`、总尝试/切换/同 Credential/总耗时预算未耗尽、请求未取消。
- 已建立会话绑定的请求只能重新取得原 Credential，绝不跨 Credential。未绑定请求可以在提交前按 RetrySafety 与预算切换；首次创建通过 AffinityRegistry 的版本化 Creating 租约提交最终目标。
- 当前请求会临时排除已经确认失败的 Endpoint 或 Proxy，避免在全局熔断达到阈值前立即重复同一路径；该排除只存在于请求内存中。
- HTTP `408` 与 `425` 按标准语义确认当前请求未执行，使用 `Transient + RejectedBeforeExecution`；HTTP 5xx、响应体读取失败和 SSE 成功状态后的无效/中断属于 `Ambiguous`。完整证明边界见 ADR-0093。不提供 at-least-once 开关，也不因尚未向客户端输出就盲目重试。
- `Retry-After` 解析和运行时 deadline 使用可失败加法，并把外部延迟限制为 30 天，避免异常值溢出后立即解除冷却。Claude 的合法 `retry-after-ms` 优先于标准 Header 并使用毫秒精度；缺失、负数、非有限值或其他非法值回落到标准 `Retry-After`，同样受 30 天上限约束。
- 上游已经成功返回后发生的续接 ID 提取、egress 编码、公开模型恢复或粘性提交失败，仍先按健康成功结算并关闭 HalfOpen 探测，再结算运行态 Guard 并返回 any2api 本地错误。
- 最终收到的上游非 2xx 原样返回上游状态码、有界正文和允许的 Header；内部 `UpstreamErrorClassification` 只能决定重试、OAuth 刷新、健康、冷却与内部遥测，不得反向生成客户端错误正文或自定义类型。
- SQLite Attempt/RequestLog 消费同一类型化 Attempt 结果，但只展示上游原始消息或 any2api 自己产生的本地消息，不向管理 API 暴露内部分类。

## 设置

统一 SettingRegistry 注册十八项 `retry.*`、`cooldown.*` 与 `breaker.*` 设置。Duration 使用整数秒；抖动使用 `0..=100` 的整数百分比。所有设置按值编译进 PublishedSnapshot，已开始请求不在执行中途读取其他 revision。

## 后果

- 新 Provider 只实现自身内部错误分类，不修改中央重试器、调度器、健康状态机或客户端错误协议。
- 运行态健康与配置代际严格分离，进程重启后全部清空，不引入恢复、后台数据库状态或外部缓存。
- 对安全性不明确的上游执行结果宁可返回错误，也不默认重复生成内容。
- RequestLog/Attempt 直接消费现有类型化结果，不从客户端可见正文重新推断错误或重试原因。

## 验证

- Domain 单元测试穷举全部 `UpstreamErrorKind` 的默认 RetrySafety，并断言 `Transient`/`Unknown` 默认 `Ambiguous` 且禁止自动重试；Provider 测试覆盖 Codex/Claude 错误 envelope、400/429 的结构化额度 code、普通 400 不误判、模型错误、Count Tokens 404、标准 Retry-After 两种形式，以及 Claude `retry-after-ms` 的精度、优先级、回落和上限。
- Runtime 虚拟时间测试覆盖模型冷却、认证代际隔离、Endpoint/Proxy 熔断、HalfOpen 探测竞态、超大 Retry-After、成功后处理结算、重复 schedule 去重、更早/更晚 deadline 重排、多 slot 到期 epoch 唤醒、热更新代际隔离、同凭据指数退避以及候选切换零退避；并断言健康 worker 的后台任务数始终不超过一个。
- Public request 契约覆盖提交前切换、已绑定请求不切换、Ambiguous 不重试、Retry-After、总 Attempt 预算、SSE 首帧提交边界，以及最终上游状态/正文/Header 不被内部分类改写。
- Web 测试覆盖新增设置的契约解析、中文展示、保存具体覆盖值与无恢复默认入口。
