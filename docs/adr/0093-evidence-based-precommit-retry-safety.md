# ADR-0093: 以可证明的未执行证据判定提交前重试安全性

- 状态：Accepted
- 日期：2026-08-03
- 决策者：maintainer
- 修订：ADR-0164 保留 5xx 和通用 2xx failed envelope 的 `Ambiguous` 证据，但允许未绑定 Pending
  请求以独立的 `PreferAlternate` 路由动作执行有界故障恢复

## 背景

Runtime 会在下游仍为 `Pending` 时重试安全的上游失败，但“尚未向客户端提交”只能证明客户端还没看到结果，不能证明非幂等的生成请求没有被上游执行。尤其是 SSE：`Transport::execute` 成功返回上游 HTTP 响应头之后，Runtime 才预读首个事件；此时发生首事件超时、读错、EOF 或协议错误，均可能只是已执行请求的响应丢失。

当前数据面没有跨 Codex、Claude 与 Grok 都可靠的幂等键契约，也不能通过响应是否到达反推请求是否执行。若把这些失败按 at-least-once 自动重试，可能重复生成、重复调用工具或重复产生其他上游副作用。

另一方面，部分标准 HTTP 状态本身明确表达服务器没有执行当前请求。完全忽略这些证据会让已经存在的 `RejectedBeforeExecution` 重试路径无法用于普通临时错误。

## 决策

- 下游 `CommitState::Pending` 是自动重试的必要条件，不是执行安全证据。自动重试仍必须同时满足 `RetrySafety::allows_automatic_retry()`、失败种类、预算、取消和会话绑定约束。
- DNS、建连、代理握手或 TLS 阶段确认请求体尚未发送的 Transport 失败继续使用 `DefinitelyNotSent`。
- HTTP `408 Request Timeout` 使用 `Transient + RejectedBeforeExecution`。RFC 9110 §15.5.9 定义该响应表示服务器没有收到完整请求，因此可以重新发送。
- HTTP `425 Too Early` 使用 `Transient + RejectedBeforeExecution`。RFC 8470 §5.2 定义服务器拒绝承担重放风险并要求客户端自动重试时不得再次使用 early data；当前 Transport 未启用 TLS 0-RTT/early data，Runtime 重试仍经过普通请求发送路径。
- HTTP 5xx 继续使用 `Transient + Ambiguous`。5xx 只说明服务器处理请求时失败，不能证明非幂等 POST 没有生效；即使下游尚未提交，也不得自动重放。
- HTTP `421 Misdirected Request` 暂不提升为自动重试安全状态。RFC 9110 §15.5.20 对非幂等方法的重试要求使用不同连接，而当前池化 Transport 没有向 Runtime 提供“废弃本次连接并新建连接”的可证明控制边界；在该能力出现前保持 `Unknown + Ambiguous`。
- 成功收到上游 HTTP 响应头后，buffered body 读取失败，以及首个 SSE 事件前的超时、超限、EOF、Transport 错误或协议错误，全部保持 `Ambiguous`。`GuardedBody::prime` 不把这些错误重新包装成可重试 Transport 失败，也不启动第二条流。
- 收到首个可提交 SSE 事件后，既有下游 Commit 边界继续永久禁止切换上游。本 ADR 不增加 at-least-once 设置，也不推导新的 `Idempotent` 操作。
- `408`、`425` 和 5xx 的最终 Attempt 仍遵循透明上游错误契约；内部分类只影响是否重试、健康状态和遥测。被安全重试掉的响应对客户端不可见。

## 依据

- [RFC 9110 §9.2.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-9.2.2) 要求客户端不要自动重试非幂等请求，除非已知请求语义幂等，或能确认原请求从未被应用。
- [RFC 9110 §15.5.9](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5.9) 给出 `408` 的未收到完整请求语义。
- [RFC 8470 §5.2](https://www.rfc-editor.org/rfc/rfc8470.html#section-5.2) 给出 `425` 的拒绝处理和禁止以 early data 重试的语义。
- [RFC 9110 §15.5.20](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5.20) 对 `421` 的非幂等重试附加不同连接条件。

## 后果

- 标准明确拒绝的 `408` 与 `425` 可以在 Pending 和既有重试预算内切换失败路径，减少无谓失败。
- 首事件前尚未向客户端输出不再被误当成“上游未执行”；生成请求保持 at-most-once 优先。
- 5xx 仍会计入 Endpoint 临时健康状态，但不会因 `Transient` 名称而自动重放。
- 将来若要支持 `421` 或真正的 `Idempotent`，必须先提供不同连接或可靠幂等键的端到端证明，再单独修订本决策。

## 验证

- Provider 单元测试枚举 `408`、`425` 为 `Transient + RejectedBeforeExecution`，并枚举代表性 5xx 为 `Transient + Ambiguous`；`421` 保持 `Unknown + Ambiguous`。
- Public reliability 契约验证流式请求收到 `425` 后会在下游提交前切换路径并成功，Attempt 遥测保留安全性分类。
- 契约验证 5xx 生成请求只发送一次并透明返回原响应。
- 契约验证成功响应头后的首个 SSE Body 读取失败只发送一次，并记录 `Ambiguous`。
