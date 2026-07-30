# ADR-0046：Claude OAuth 上游额度查询

- 状态：Accepted
- 日期：2026-07-26
- 决策人：项目维护者
- 补充：ADR-0034、ADR-0036
- 部分修订：ADR-0070

## 背景

Claude OAuthAccount 已能登录、刷新 Token 并参与统一路由，但管理面没有 Anthropic 上游额度读取能力。Token refresh 只更新认证材料，不会读取订阅使用率，因此不能把“Token 已刷新”当作“额度已刷新”。

Anthropic 为 Claude Code OAuth 提供 `GET https://api.anthropic.com/api/oauth/usage`。响应除通用 5 小时和 7 天窗口外，还可能包含 Sonnet 7 天与 `seven_day_overage_included` 模型窗口。现有通用 DTO 固定为 `primary_window`/`secondary_window`，最多容纳两个窗口，会静默丢失真实上游数据；Claude 响应也不提供可安全映射为全局 `allowed`/`limit_reached` 的字段。

## 决策

1. Claude Driver 固定构造 `GET https://api.anthropic.com/api/oauth/usage`，注入当前 OAuth Bearer Token、`Accept: application/json, text/plain, */*`、`Content-Type: application/json`、`anthropic-beta: oauth-2025-04-20` 与固定 Claude Code User-Agent。URL、身份头和窗口映射不能由客户端输入改变。
2. Provider 只构造请求并解析响应，不执行网络。Runtime 复用现有 OAuth quota 编排、账号固定 DIRECT/全局代理、严格 SSRF、禁重定向、有界响应体和读取超时；401 最多触发一次 Token refresh，并用新版本 Token 完整重建计划后重试一次。
3. 只解析 `five_hour`、`seven_day`、`seven_day_sonnet` 和 `seven_day_overage_included`。使用率必须是有限非负数，重置时间必须是有效 RFC 3339；缺失或 `null` 的可选窗口保持缺失，出现但畸形的窗口使整次查询失败。原始响应、未知字段和 Token 不进入 DTO、日志或持久化。
4. 通用 `OAuthQuotaRateLimit` 改为带稳定 `id` 的窗口列表，并把 `allowed`、`limit_reached` 改为可空观测。Codex 与 Grok 同步投影到该模型；Claude 不推断上游未提供的全局可用状态。该项目尚无需要兼容的正式内部/API 契约，不保留固定主/次槽位的双轨结构。
5. Claude 额度是只读瞬时快照，不写 SQLite、OAuth Provider JSON、PublishedSnapshot、RequestLog、浏览器存储或文件，也不参与 RPM 或粘性。Claude 当前窗口没有全局可用状态，因此单个窗口 100% 不得影响路由；若未来 Provider 契约明确返回全局耗尽信号，则按 ADR-0070 只更新当前认证 generation 的临时健康。Claude 不实现 quota reset。
6. Web 为 Claude 显示单账号“刷新额度”和 Provider 级“刷新全部额度”，复用既有账号级内存 Query cache、最多 6 个并发与 all-settled 汇总。所有返回窗口均展示；只有 Codex 显示 reset credit 与重置按钮。

## 备选方案

- 只展示 5 小时与普通 7 天窗口：拒绝。它会丢弃 Anthropic 已明确返回的 Sonnet/Fable 窗口，且会把错误的固定双槽模型继续扩散。
- 把 Claude 字段追加成 `additional_windows`：拒绝。新项目没有兼容负担，双轨模型会让 Provider、DTO 和 Web 长期维护两套窗口语义。
- 从本地请求日志估算使用率：拒绝。多客户端、上游侧消耗和窗口规则不可见，结果不是权威额度。
- 定时后台刷新并持久化快照：拒绝。当前需求是管理员显式读取，持久化会引入过期与恢复语义。

## 后果

每次 Claude 刷新产生一个只读 Anthropic 请求；批量刷新最多按账号各产生一个请求且同时在途不超过 6。通用管理响应从固定主/次字段切换为窗口数组，Codex、Grok 与 Claude 使用同一展示和缓存路径。Provider 新增更多额度窗口时不再受两个槽位限制。

## 验证

- Provider 测试覆盖固定 URL、身份头、Debug 脱敏、四类窗口、缺失可选窗口及畸形百分比/时间拒绝。
- Runtime 测试覆盖 Claude DIRECT/全局代理、单次请求、无 reset credits 和 401 后单次 Token refresh 重试。
- HTTP 契约测试覆盖管理鉴权、四窗口 DTO、`no-store` 与 Token/原始字段脱敏。
- Web 测试覆盖 Claude 单账号刷新、四窗口标签、批量刷新、无重置按钮及通用窗口数组解析。
