# ADR-0164: 统一上游失败结果与候选恢复

- 状态：Accepted
- 日期：2026-08-17
- 决策者：maintainer
- 修订：ADR-0013、ADR-0093、ADR-0118、ADR-0120、ADR-0121、ADR-0128、ADR-0136

## 背景

现有 Runtime 把 HTTP 状态、流式拒绝、协议失败和 Transport 失败分别处理，又把
`RetrySafety` 同时当作上游执行证据和全部恢复动作的总开关。这导致两类错误行为：

- 普通 HTTP 5xx 虽然已经形成完整的上游失败响应，却因 `Ambiguous` 在未绑定请求中直接结束，候选池、
  请求内尝试历史和长期 CandidatePath health 都无法参与恢复；
- HTTP 200 下的 JSON 错误 envelope 或 SSE `error` / `response.failed` 只在少数过载、限流代码上进入
  重试路径，其余完整上游失败可能被当作成功、桥接本地错误或已经提交的失败流。

另一方面，成功响应头后的正文断流、首帧读取失败和请求写出后丢失响应同样可能是 `Ambiguous`，但它们
没有完整的上游失败事实，不能仅因下游尚未提交就自动换路。错误分类、重放证据、恢复能力和选路动作
必须成为四个独立事实。

现有 reselect 还把失败 Candidate 的 `Retry-After` 作为整个逻辑请求的全局等待。一个账号返回
`429 Retry-After: 30` 时，即使另一个账号健康，请求也要先等待 30 秒，错误扩大了提示的作用范围。

## 决策

1. Runtime 以统一的 Attempt outcome 处理上游结果。以下完整事实都形成 `UpstreamFailure`：
   - 已收到非 2xx HTTP 状态；
   - 完整 2xx buffered body 被对应上游协议精确识别为错误 envelope；
   - SSE 在任何客户端可见语义输出前出现协议精确识别的 `error`、`response.failed` 或等价失败终止事件。
   `UpstreamFailureOrigin` 保留 `HttpStatus`、`BufferedEnvelope`、`PreSemanticStreamEnvelope` 来源，恢复决策
   不再通过猜测状态码反推证据。
2. 非 2xx 错误正文必须保留“完整/不完整”事实。只有完整且在 64 KiB 上限内的正文参与 Provider
   结构化细化和最终透明返回；超时、断流或超限时仅使用 HTTP 状态基线并返回空正文。正文读取失败不得
   被伪装为完整错误 envelope，也不得抹掉已经收到的状态和安全 Header；不完整正文不进入
   alternate-path recovery，避免把响应读取失败误判为上游明确拒绝。
3. Protocol 只识别线协议中的显式失败 envelope，并携带原始、有界 JSON 证据；Provider Driver 继续负责
   将该证据分类为 `UpstreamErrorKind`、`RetrySafety`、`Retry-After` 和 attribution。协议审计已经证明的
   预执行拒绝可以把 safety 收窄为 `RejectedBeforeExecution`，但普通 5xx、通用 `server_error` 和未知
   failed envelope 仍保持 `Ambiguous`。Responses `incomplete` 是成功终局，不属于失败 envelope。
4. 以下情况不形成 `UpstreamFailure`：成功响应后的 Body/SSE Transport 中断、超时、非法 JSON/SSE、
   缺失终止事件、桥接编码失败、粘性提交失败和其他本地错误。它们保持各自的 Transport、InvalidResponse
   或 Local outcome；只有确有证据的安全 Transport 失败可重试，不能借 2xx 状态扩大重放权限。
5. `RetrySafety` 继续只记录请求是否有可证明的安全重放依据：`DefinitelyNotSent`、
   `RejectedBeforeExecution`、`Idempotent` 或 `Ambiguous`。Provider 和 Transport 不为提高成功率篡改
   该证据。未绑定请求对完整 `UpstreamFailure` 采用有界的 alternate-path recovery，是独立且明确接受的
   at-least-once 风险，不把 `Ambiguous` 改写为“未执行”。
6. Runtime 的恢复动作显式区分：
   - `RetrySamePath`：已绑定请求仅在 safety 允许且同路径可能恢复时等待后重试；
   - `Reselect`：有明确、稳定故障归因时硬排除对应作用域并立即重新选择；
   - `PreferAlternate`：记录失败作用域和 Candidate/Credential/Egress 尝试历史，优先探索其他 Candidate，
     但在没有更好路径时允许失败作用域于自身 not-before 到期后重新进入选择；
   - `Terminal`：不再发起上游 Attempt。
7. 未绑定且仍为 Pending 的请求中，除 `InvalidRequest` 外的完整上游失败均可在预算内恢复：认证、权限、
   额度、模型和操作等明确归因使用 `Reselect`；限流、临时错误、未知错误和其他仅能归因到当前组合路径的
   失败使用 `PreferAlternate`。这同时覆盖 HTTP 4xx/5xx、2xx JSON 错误和预语义 SSE 错误，不建立
   状态码特例列表。普通请求格式错误始终 `Terminal`。
8. 已绑定显式 Session 和 Continuation 永不切换 Credential、Target、上游模型或方言。安全的临时拒绝
   可以 `RetrySamePath`；`Ambiguous` 上游失败和不会因等待改变的认证、权限、额度、模型、操作失败终止。
   首次创建但尚未提交的 Session 仍属于未绑定请求，最终成功 Candidate 才提交绑定。
9. `PreferAlternate` 使用同一 Candidate 选择器。请求状态同时保存硬排除、exact Candidate 尝试历史、
   Credential/Egress 尝试历史和按 attribution scope 的 not-before。选择顺序优先未尝试 exact Candidate，
   再按新 Credential + 新 Egress、新 Credential + 旧 Egress、旧 Credential + 新 Egress、旧 Credential +
   旧 Egress 探索；不创建第二个 scheduler 或跨请求 blacklist。
   普通 fallback 的指数按 exact Candidate Attempt 数计算，避免同 Credential 的新 Target/Egress 继承旧
   路径退避；已审计的 precontent overload 仍按逻辑请求总 Attempt 数计算。
10. `Retry-After` 和指数 fallback 只决定失败作用域再次可选的 not-before，或固定路径
    `RetrySamePath` 的等待。健康备选立即可选，不继承其他 Candidate 的等待。所有可选路径都只被请求内
    not-before 挡住时，选择器等待最早到期时间后重新扫描；等待仍受同一 precommit deadline 约束，且在
    RPM 预留与 `in_flight` Guard 之前发生。等待复用统一有界 QueueTicket 和 scheduler epoch，但不读取
    `on_rate_limited` 的 wait/reject 策略；Ticket 已满或到期超出总预算时返回最后真实上游失败。
11. RetryBudget 的 Attempt/切换资格必须成为候选选择的前置过滤，禁止先消耗 RPM 再发现该 Credential
    已超出逻辑请求预算。真正开始的每个 Attempt 才消耗一次 RPM，失败后不归还。
12. 上游失败无论是否还能重试，都先按 attribution 更新现有 Credential、CredentialModel、Endpoint、
    Proxy、EgressPath 或 CandidatePath health。流已经提交后的显式失败事件不再切换，但仍以同一分类更新
    health 和 Attempt 最终结果，不能把 `HTTP 200 + failed event` 记为成功。
13. 中间失败只进入 Attempt 遥测。恢复成功时客户端只看到最终成功 Attempt；路径、预算或安全边界耗尽时，
    最终响应使用最后一次真实上游状态、允许 Header 和完整有界正文/错误帧。Transport、InvalidResponse 或
    Local failure 继续使用稳定的 any2api 本地错误，不把所有失败统一改写成网关 envelope。
14. 可靠性算法不再向普通管理面暴露 Attempt 数、Credential 切换数、同号重试数、退避、抖动、冷却和
    breaker 阈值等 17 个内部调优项。它们收敛为 Runtime 中集中、版本化的固定策略；用户只保留
    `retry.precommit_total_budget` 这一总时限，以及既有上游读取、流式、排队、会话等待和停机时限。
    Migration 0035 在移除 SettingKey 前拒绝仍保存上述内部调优 override 的数据库，禁止静默丢弃用户值。

## 后果

- 同一流程可以自然处理 `A -> HTTP 503`、`B -> HTTP 200 + error event`、`C -> success`，不再复制
  buffered、streaming 与状态码专用重试器。
- `429 Retry-After` 不再冻结整个路由池；提示仍严格约束返回同一失败作用域的最早时间。
- 单 Candidate 可以在预算和自身 not-before 允许时重试；已绑定请求、响应结果丢失和下游已提交请求不会
  因此扩大切换范围。
- 后提交失败不能挽救当前客户端请求，但会让后续请求看到正确的 Candidate 健康状态。
- 管理设置保留部署者能理解且确实需要调整的时限，不再把内部状态机参数转嫁给用户。

## 验证

- Domain/Provider/Protocol 测试覆盖 HTTP 与 2xx JSON/SSE 错误分类、精确 pre-execution safety 和错误
  envelope 桥接透传。
- Runtime 决策测试覆盖所有 `UpstreamErrorKind`、unbound alternate、bound same-path/terminal、Transport
  丢失响应 terminal，以及 Retry-After 只约束失败作用域。
- 选择测试覆盖 exact Candidate 优先级、混合 not-before/RPM/health blocker、预算前置过滤、单 Candidate
  到期回退和无 RPM 泄漏。
- 公共契约覆盖非流式与流式的 `503 -> success`、`200 error -> success`、多次混合失败后成功、耗尽时保留
  最后真实错误、新 Session 最终绑定成功 Candidate、既有 Session/Continuation 不切换，以及提交后错误
  不重试但更新 health。
- Migration 与设置契约覆盖旧内部 override 的显式拒绝、当前 SettingKey 集合和只保留总时限的可靠性响应。
