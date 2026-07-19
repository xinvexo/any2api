# ADR-0007: 首版同协议 JSON 请求执行链

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer
- 后续：`ADR-0008` 已接入 `/v1/messages/count_tokens` 与独立辅助并发，本 ADR 记录的“仅认证门”是当时切片边界。

## 背景

Provider/Route/GatewayKey 配置切片、GatewayApiKey 认证和 Transport 的 DIRECT/HTTP/SOCKS5h 非流式执行切片已完成，但 Protocol/Provider 仍是空端口。需要一条可独立验证的最小数据面，确认 Route、Credential 并发、代理、上游路径和认证注入可以在不把业务逻辑塞进 Axum Handler 的前提下串起来；SettingRegistry、严格 SSRF 本地 DNS、代理健康和可靠性策略仍留在后续切片。

## 决策

- App Composition Root 静态注册 Codex/Claude Driver、OpenAI Responses/Anthropic Messages Adapter 和 ReqwestTransportManager；Runtime 只依赖各自公开 API。
- 首个执行切片支持同协议、非流式 JSON：`/v1/responses`、`/v1/responses/compact` 和 `/v1/messages`。`/v1/messages/count_tokens` 仍保留认证门，等辅助并发 Permit 完成后接入。
- ProtocolAdapter 只结构化读取 `model`、`stream` 和操作类型，保留未知 JSON 字段；出站时只替换 `model`，不把 Provider 路径或认证逻辑放进协议模块。
- ProviderDriver 根据结构化 `ProtocolOperation` 在 Endpoint Base URL 后追加固定路径，并通过持有的 `ConcurrencyPermit` 注入 Provider API Key。Driver 不创建 HTTP Client。
- Runtime 先在同一 `PublishedSnapshot` 中解析 Route、过滤 Endpoint/Credential/Proxy 和 Driver 能力，再调用原子 select-and-acquire。非流式响应 Body 完整读取结束后才释放 Permit。
- 相同比例的轮询游标按 `(ModelRouteId, FallbackTier)` 隔离，并由 `RuntimeRegistry` 跨连续配置代际复用；快照只持有对应绑定。Route/tier 删除后新快照立即移除游标，旧快照仍可完成已有请求，删除后重新加入则建立新游标代际。
- Runtime 请求执行按规划、单次 Attempt 和响应处理拆分。上游响应会移除认证字段、Cookie、固定 hop-by-hop Header 以及 `Connection` 动态指定的 Header，避免 Provider Secret 或连接级元数据返回给客户端。
- `/v1/*` 的 fallback 与方法错误和已知路由使用同一 GatewayApiKey 鉴权层；未知路径不会绕过 Access 阶段，已知路径的方法错误返回稳定 JSON 405。
- 首版暂不自动重试、排队、冷却、健康熔断、会话粘性或 SSE 事件转换；`stream=true` 明确返回协议兼容的 invalid request，而不是把流式请求降级为 JSON。
- 上游非 2xx 错误只返回协议兼容的脱敏错误 envelope；不透传上游原始错误正文。
- 当前切片仍把已分类的上游错误统一映射为脱敏 `502` envelope，尚未透传 `Retry-After` 或细分客户端状态码；这属于后续可靠性切片，不把错误正文直接暴露给客户端。
- DIRECT 模式已执行最终 DNS/IP 校验并固定本次连接地址；HTTP/SOCKS5 远端 DNS 仍按显式受信代理边界处理，严格本地 DNS 模式尚未进入本切片。请求级绝对 deadline 也仍待可靠性切片，因此当前结果是可验证的非流式 JSON vertical slice，不宣称首个正式可靠性版本已完成。

## 后果

- Codex Responses、Responses Compact 和 Claude Messages 可以通过真实 HTTP/代理链路验证路径、模型替换、Provider 认证头和 Gateway 认证隔离。
- 同一个 Route Target 的 `upstream_model` 在 Runtime 中保持独立，不会因为只按 Credential 选择而丢失 Target 身份。
- 不同 Route 或 fallback tier 不再共享全局轮询序号；无效 JSON、未知模型和没有 Route 的请求不会扰动其他 Route 的 Credential 选择。
- 由于没有 SSE/重试，流式客户端和可安全切换的故障恢复仍不能视为首版完成；后续必须增加 GuardedBody、SSE 分帧、CommitState 和 RetrySafety，而不是在当前 Handler 中打补丁。

## 验证

- Protocol/Provider 模块测试覆盖模型替换、未知字段保留、错误 envelope、固定路径和认证头脱敏；Registry 契约通过 App Composition Root 的唯一装配工厂枚举全部已注册 Adapter/Driver。
- Runtime 单元测试覆盖 Route/tier 游标隔离、连续发布复用和删除后新代际；HTTP 契约测试使用本地上游验证 Codex Responses、Responses Compact 和 Claude Messages 的真实请求路径、Provider 头、客户端认证头剥离、响应模型别名恢复、敏感响应头过滤、fallback 鉴权和 JSON 405。
