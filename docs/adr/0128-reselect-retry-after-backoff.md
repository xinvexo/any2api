# ADR-0128: Credential reselect 遵守 Retry-After 与语义退避

- 状态：Accepted
- 日期：2026-08-10
- 修订：2026-08-10（补充真实 Runtime 换号连接实验）
- 决策者：maintainer
- 相关决策：ADR-0136

## 背景

基线 Runtime 只给已绑定的同 Credential retry 使用指数退避；未绑定请求一旦决定 `Reselect` 就固定使用零延迟。默认 `retry.base_delay` 又是 0，因此即使上游已经返回标准 Retry-After，Runtime 也可能立即用另一个 Credential 发起紧邻 Attempt。Transport 的数据面共享/隔离由当前 affinity 模式决定，但请求级的服务端退避语义必须独立成立。

健康冷却与当前请求等待不是同一件事。前者按证据作用于 Credential、模型、Endpoint 或 EgressPath，避免后续请求再次选择失败候选；后者约束已经决定自动重试的这个逻辑请求，不能因为换候选就绕过服务端明确给出的 not-before。

## 决策

1. `RetryDecision` 的 `RetrySamePath`、`Reselect` 与 `OAuthRefresh` 都携带同一个 request-local delay。只有 `Terminal` 没有 delay。
2. fallback 继续按当前失败 Credential 在本请求中的已注册 Attempt 数做指数增长：第一次失败使用 `retry.base_delay`，之后乘二并受 `retry.max_delay` 限制，再应用 `retry.jitter_ratio`。它不因切换到新 Credential 而预先增加新 Credential 的 Attempt 计数。
3. `AttemptFailure::Upstream` 的 Retry-After hint 从决策时的当前时间换算为有界 delay。最终 delay 是 jitter 后 fallback 与 Retry-After 的较大值；jitter 不作用于 hint，因此永远不能把服务端 not-before 向前移动。Transport 与 Anthropic precontent `rate_limit_error` 没有 hint，只使用当前 Credential 的 fallback；精确 precontent overload 按 ADR-0136 使用本逻辑请求的总 Attempt 数计算 fallback。
4. 未绑定 `Reselect` 先按既有证据排除失败路径，再等待 delay，之后才重新选择。健康冷却仍保持原作用域；新 Credential 不继承旧 Credential 的健康状态，但同一逻辑请求遵守统一等待。
5. OAuth refresh 可以在 retry-not-before 窗口内执行；刷新耗时从 delay 中扣除。刷新成功后的同 Credential 数据面 retry，或刷新失败后的 Credential reselect，都不得早于原 retry-not-before。
6. 如果所需 delay 大于等于当前剩余 precommit budget，则本 Attempt 标记为 Terminal，并立即返回当前真实上游/Transport 失败。不得先睡到 deadline 再生成本地 504，也不得截短 Retry-After 后尝试。
7. 默认 `retry.base_delay` 从 0 秒改为 1 秒，`retry.max_delay=2` 与 `retry.jitter_ratio=20` 保持不变。管理员仍可显式配置 0 来关闭无 hint fallback；这不能绕过非零 Retry-After。
8. 不新增随机 sleep、Provider 分支或 Credential 间共享冷却。现有 jitter 只用于常规并发去同步，不声称模拟官方客户端，也不作为 Transport 隔离替代品。

## 后果

- 默认配置下的安全 reselect 不再形成零间隔 Credential 切换。
- 明确 Retry-After 对同路径、换号与 OAuth 修复路径具有一致的 request-local 下限。
- 很长的 Retry-After 不会占满请求预算并变成本地超时，而是保留当前上游错误作为最终响应。
- Candidate 健康状态与请求等待仍然分离，不会把账号级提示错误扩张为全局 Provider 停顿。

## 验证

- 纯决策测试覆盖 unbound reselect、bound same-path 与 OAuth refresh 都携带 delay，且 Retry-After 大于 fallback 时精确胜出。
- Tokio 虚拟时间测试覆盖 OAuth refresh 耗时抵扣、delay 未到不开始下一 Attempt、delay 放不进预算时立即 Terminal。
- 完整 Runtime loopback 实验让首个 Credential 返回 `429 + Retry-After: 1`、第二个 Credential 返回成功；上游捕获证明 Authorization 已切换、两次请求来自不同物理 TCP peer，且第二次到达不早于一秒。
- 既有 RetrySafety、最大 Attempt/switch、会话绑定和 transport isolation 测试保持通过。
