# ADR-0045：Grok OAuth 周订阅额度查询

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者
- 补充：ADR-0034、ADR-0041

## 背景

Grok OAuthAccount 已能登录、刷新 Token 并参与统一路由，但管理页只为 Codex 注册额度查询，因此 Grok 卡片既不显示额度，也不能手动刷新。xAI 的 CLI subscription 数据面提供只读 billing 接口；Sub2API 同时使用 billing 与推理响应头，但后者只有在真实推理发生且上游返回限额头时才存在。

管理面需要一个主动、无副作用且不会消耗推理额度的刷新入口。Codex 的 reset credit 是 ChatGPT 专属能力，不能套用到 Grok。

## 决策

1. Grok Driver 注册 `GET https://cli-chat-proxy.grok.com/v1/billing?format=credits`，注入当前 OAuth Bearer Token、`X-XAI-Token-Auth: xai-grok-cli`、Grok CLI 版本与 User-Agent；不发送测试推理请求。
2. 只解析 `config.currentPeriod`、`config.creditUsagePercent` 与周期起止时间，并投影为通用 OAuth quota 周窗口。百分比和时间必须有限、非负且结构有效；未知字段、产品明细和原始响应不进入管理 DTO。
3. 查询复用现有 OAuth quota Runtime：固定 DIRECT/全局代理、严格 SSRF、禁用重定向、有界响应体和读取超时；401 最多刷新 Token 一次并重试一次。
4. Grok quota 是只读临时快照，不写 SQLite、OAuth JSON、PublishedSnapshot、日志或浏览器持久化。不得根据本地请求日志推算上游额度。
5. Grok 不实现 `quota/reset`。Web 为 Codex 与 Grok 显示单账号及批量刷新，但只为 Codex 显示 reset credit 与重置按钮；Claude 继续不显示额度入口。
6. 通用 `OAuthQuotaQueryPlan` 的 reset-credit 查询改为可选：Codex 保持双 GET，Grok 只执行 billing GET。中央 Runtime 不增加 Provider `match`。

## 参考

- Sub2API xAI billing 与 quota probe：<https://github.com/Wei-Shaw/sub2api>
- CLIProxyAPI xAI OAuth 数据面：<https://github.com/router-for-me/CLIProxyAPI>

## 后果

- Grok OAuth 账号可主动显示并刷新真实周订阅 credits 使用率，不必先发起业务请求。
- xAI 未返回有效 billing 周期时查询失败并显示安全错误，不伪造“无限”或“剩余 100%”。
- 未来若需要展示响应头中的请求/Token 限额，应另行设计运行态观测模型，不能把被动头部快照混入本次 billing 契约。

## 验证

- Provider 测试覆盖请求 URL、CLI 身份头、Token 脱敏、周窗口解析与畸形响应拒绝。
- Runtime/HTTP 契约测试覆盖 Grok 查询、单请求执行、DIRECT 代理、401 刷新和无 reset credits。
- Web 测试覆盖 Grok 卡片额度、单账号刷新、批量刷新以及不显示重置按钮。
