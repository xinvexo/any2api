# ADR-0136: 预提交拒绝保真与请求级过载退避

- 状态：Accepted
- 日期：2026-08-11
- 决策者：maintainer

## 背景

ADR-0118/0121 允许 any2api 在任何语义输出前识别精确的上游 SSE 准入拒绝并内部重试。基线实现把生命周期帧和拒绝帧一起丢弃，只在 Runtime 保留 `Overloaded` 或 `RateLimited` 分类；一旦重试预算耗尽，最终响应被重新构造为本地 `upstream stream was rejected before content` 502。这样客户端丢失了上游实际返回的 `server_is_overloaded`、`overloaded_error` 或 `rate_limit_error`，RequestAttempt 也只记录本地通用描述。

ADR-0128 的 fallback 指数又按当前失败 Credential 计算。连续切换多个 Credential 都收到同一个明确过载拒绝时，每个新 Credential 都重新使用第一次失败的短等待，导致单个逻辑请求在共享上游故障期间形成紧邻 Attempt。

## 决策

1. Protocol 对受审计拒绝同时产出通用重试类别和精确稳定码。稳定码只能来自已经精确匹配的声明字段：OpenAI `server_is_overloaded`、Anthropic `overloaded_error` 或 `rate_limit_error`；不保存任意上游字符串，也不按自然语言推断。
2. Runtime 在预提交阶段继续丢弃可重试 Attempt 的生命周期控制帧，但单独保留该 Attempt 已经按入口协议编码的最终拒绝帧、上游成功 HTTP 状态和安全响应 Header。保留内容受既有单帧和预提交字节预算约束。
3. 后续 Attempt 成功时，所有先前拒绝帧继续丢弃。若最大 Attempt/切换次数、候选选择、等待预算或其他既有边界使重试终止，最终响应使用最后一次真实拒绝的 HTTP 状态、Header 和协议拒绝帧；禁止改写为 any2api 本地 502 或另造 JSON envelope。
4. 同方言直通时拒绝帧沿既有 wire-preserving 编码路径返回；跨协议时返回 ProtocolBridge 已经转换到入口方言的帧。Runtime 不解析或重建 Provider JSON。
5. RequestAttempt 与最终 RequestLog 使用协议确认的精确稳定码作为安全错误信息，并保留对应 `Upstream` 或 `RateLimited` 分类。`HTTP 200 + error_class` 继续按失败统计，不能因状态为 2xx 显示为成功。
6. 明确 precontent overload 的指数 fallback 使用本逻辑请求已注册总 Attempt 数，切换 Credential 不重置指数。其他 Transport/HTTP/认证/额度/限流失败以及 Anthropic `rate_limit_error` 继续按当前失败 Credential 计算，Retry-After 规则不变。
7. 过载拒绝仍只在当前请求排除失败 ExactCandidate，并对 Endpoint、Proxy 和 EgressPath 健康保持 neutral。本决策不新增 Provider 全局熔断、跨请求冷却或固定并发限制。

本决策局部取代 ADR-0118 的“预算耗尽时返回本地上游错误”条款，并为 ADR-0128 的按 Credential fallback 增加仅限精确 precontent overload 的请求级例外。

## 后果

- 客户端最终看到上游实际声明的 `server_is_overloaded` 等协议错误，而不是来源不明的代理 502 文案。
- any2api 仍可在安全边界内换号恢复请求；只有最终无法恢复时才交付最后拒绝帧。
- 同一逻辑请求连续遇到共享过载时会逐步退避，不再因换号反复回到最短等待。
- 健康状态仍不把单次账号路径证据扩张为整个 Endpoint 故障。

## 验证

- Protocol 测试覆盖每种受审计拒绝的类别和精确稳定码，未知值不产生元数据。
- Runtime stream 测试覆盖生命周期帧不进入保留结果、最终拒绝帧逐字节保留，以及语义输出后的相同错误仍按已提交流处理。
- Runtime 最终失败测试覆盖真实状态、帧、稳定码和失败分类；公开契约测试覆盖中间拒绝后成功不泄漏帧，以及终局 HTTP 200 SSE 返回真实码。
- RetryBudget 测试覆盖两个不同 Credential 连续过载得到递增 fallback，而普通失败切换后仍从该 Credential 的计数开始。
