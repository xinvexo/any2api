# ADR-0116：OAuth Token 刷新使用分阶段安全诊断

- 状态：Accepted
- 日期：2026-08-05
- 决策范围：Provider、Runtime、管理 API、Web OAuth 管理
- 替代：ADR-0036、ADR-0070 中把刷新失败折叠为通用认证错误的部分契约

## 背景

OAuthAccount 在额度查询或数据面收到 `401` 后会尝试刷新 Token，定时 Worker 也会在到期前刷新。旧实现把“没有 refresh token”“Refresh Endpoint 明确拒绝”“网络/代理失败”“响应解析失败”“发布失败”和“新 access token 仍被 401 拒绝”压缩为两个泛化错误。管理员因此无法判断是需要重新授权、出口暂时不可用，还是刷新链内部某一阶段失败；定时刷新失败又只存在于日志，已打开的管理页面看不到。

OAuth JSON 与 Token 是 Secret。改进诊断不能把原始请求、响应正文、URL、代理地址或底层错误字符串带入日志、管理响应和浏览器状态。

## 决策

Runtime 为每个 OAuthAccount 的当前 `token_version` 保存最近一次进程内刷新失败诊断。诊断只包含：

- 触发来源：`scheduled` 或 `authentication_failure`；
- 阶段：前置条件、请求构造、DNS、TCP、代理握手、TLS、请求写入、等待响应头、读取响应体、Token Endpoint、响应解析、Token 校验、Token 发布或刷新后认证复核；
- 代码内稳定原因；
- 可选的 HTTP 状态和 Transport 失败归因；
- 发生时间与 `reauthorization_required`；
- 对应的 `token_version`。

Provider 对结构化拒绝码作精确分类。通用永久拒绝至少识别 `invalid_grant`；Codex 还识别 `refresh_token_expired`、`refresh_token_reused` 和 `refresh_token_invalidated`。未知拒绝不按永久失效处理。

额度认证重试的管理错误固定为：

- `oauth_refresh_token_missing`：旧 access token 已被拒绝且没有 refresh token；
- `oauth_refresh_permanently_rejected`：Refresh Endpoint 返回声明过的永久拒绝；
- `oauth_refreshed_access_token_rejected`：Token 已成功换代，但同一认证操作重试仍为 `401`；
- `oauth_token_refresh_failed`：网络、代理、超时、5xx、未知拒绝、解析、校验或发布失败。

错误 envelope 与账号列表中的 `token_refresh_failure` 使用同一诊断结构。Web 在账号卡片展示阶段、原因、时间和处置建议。刷新诊断变化通过既有已认证 `/api/admin/oauth/quota-events` 的 `oauth_refresh_diagnostic_changed` 无 payload 事件使账号列表失效；额度变化继续使用同一端点的 `oauth_quota_changed` 事件。

诊断由 `token_version` 隔离。成功换代、重新授权、账号删除或配置发布使当前版本变化后，旧诊断不再返回；进程内状态按活动账号版本裁剪，不持久化到 SQLite。永久拒绝抑制、singleflight、CAS、批量发布和认证/路由健康代际保持原有语义。

“删除失效账号”只接受前三个错误且要求 `reauthorization_required=true`；`oauth_token_refresh_failed` 永不成为自动删除证据。

## 安全边界

诊断和普通结构化日志禁止包含 access token、refresh token、ID token、OAuth JSON、原始响应正文、请求或响应 Header、Endpoint URL、代理地址、账号原始 subject、Provider 错误字符串或 Transport 错误字符串。HTTP 状态、稳定枚举、已审计的 Provider 拒绝码、Transport 阶段/归因和 Unix 时间可以暴露。

## 后果

- 管理员可以区分重新授权、网络/代理、上游拒绝、解析、校验、发布和刷新后复核失败。
- 定时刷新失败无需主动点击额度刷新即可在页面出现。
- 诊断是当前进程观测，重启后清空；这与健康、冷却和刷新抑制的运行态边界一致。
- 新增分类需要 Provider、Runtime、Server 与 Web 共同维护稳定枚举映射，并由契约测试验证不会退化为泛化错误。
