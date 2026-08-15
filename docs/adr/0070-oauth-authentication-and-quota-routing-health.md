# ADR-0070：OAuth 认证失效分类与额度路由健康

- 状态：Accepted
- 日期：2026-07-30
- 决策人：项目维护者
- 相关决策：ADR-0036、ADR-0045、ADR-0046、ADR-0106、ADR-0111、ADR-0137

## 背景

额度管理请求遇到 401 时会刷新 Token 后重试。原实现把刷新端点所有非 2xx 响应体丢弃，因此 OAuth 标准的 `invalid_grant` 与网络或 5xx 故障都被折叠成“认证无法确认”，管理面的“删除失效账号”无法删除已经明确失效的账号。

额度健康只接受 Provider 明确的账号级证据。Codex 的 `allowed=false`、`limit_reached=true`，以及 Grok 数据面明确耗尽并提供可验证 Token actual/limit 时，账号在短期冷却内不得重新进入路由候选；管理额度查询仍只执行已审计的只读端点。

## 决策

1. Provider Driver 对刷新端点的有界结构化错误 envelope 分类。前一 access token 已经被 401 拒绝后，账号没有 refresh token、刷新端点返回明确永久失效码，或新 access token 在同次认证操作中再次收到 401 时，分别返回 `oauth_refresh_token_missing`、`oauth_refresh_permanently_rejected` 或 `oauth_refreshed_access_token_rejected`，并设置 `reauthorization_required=true`。通用分类至少识别标准 `invalid_grant`；Codex 额外识别官方的 `refresh_token_expired`、`refresh_token_reused` 与 `refresh_token_invalidated`。禁止递归扫描任意 JSON 值或依赖自然语言消息。
2. 刷新网络错误、超时、5xx、响应超限、畸形或未知错误码仍返回 `oauth_account_authentication_unverified`。这些账号不得进入批量删除候选。
3. Runtime 按账号与 `token_version` 在进程内记住结构化永久拒绝；同一 Token 版本后续由定时 Worker、额度 401 或数据面 401 触发时直接复用永久结论，不再向刷新端点重放 refresh token。Token 版本变化或账号删除后记录失效，进程重启不恢复。
4. “删除失效账号”继续由前端复用逐账号实时诊断、重新读取安全元数据、核对 `token_version` 并串行调用现有 DELETE；不新增批量删除端点，不下发 OAuth JSON。
5. 额度查询或真实数据面只在明确、账号级的权威信号出现时更新当前 OAuthAccount `account_generation` 的路由健康：`allowed=false`、`limit_reached=true`、Provider 声明的额度耗尽诊断，或由真实耗尽 actual/limit 安全投影出的 Token `remaining=0`。未知值、单个模型/时间窗口达到 100%、Free 套餐标签和本地估算均不得排除账号；禁止为了制造该信号发送额外生成请求。
6. 明确耗尽的账号在上游 reset 时刻前不进入普通路由候选；没有可靠 reset 时刻时使用 `cooldown.permission_denied` 作为有界兜底探测间隔。到期只恢复一次正常候选资格，不从持久化展示快照恢复额度健康；下一次明确失败可重新建立状态。
7. 明确可用的后续额度查询、成功数据面请求或成功 Codex reset 清除当前账号路由 generation 的耗尽状态。建立、清除和到期复用统一 scheduler epoch 与 QueueTicket，不新增额度队列。
8. 额度健康按 `account_generation` 隔离并跨同账号 Token refresh 复用；`auth_error` 按 `token_version` 严格隔离。两者、等待和 reset 时刻都只存在于进程内，不写 SQLite、OAuth JSON、RequestLog、PublishedSnapshot 或浏览器存储，也不影响 GatewayApiKey。ADR-0111 允许独立持久化不参与路由恢复的最后成功安全展示快照；完整代际边界见 ADR-0095。
9. 数据面非 2xx 也可以通过 Provider 已声明 envelope 中的精确额度 code/type 建立同一 generation 的额度健康；HTTP 400 不再机械压回普通请求错误。该路径禁止按错误 message 推断，且不得改写客户端看到的原始上游响应。完整边界见 ADR-0086。

## 后果

- 管理员可以删除经过结构化证据确认失效的 OAuthAccount，同时短暂上游故障不会导致误删。
- 已明确耗尽的账号不再持续进入代理池；额度恢复后无需修改配置即可重新参与路由。
- Claude 当前只提供窗口使用率，没有全局可用信号，因此窗口达到 100% 仍只展示，不改变路由。

## 验证

- Provider 测试覆盖 `invalid_grant`、Codex 三个永久 refresh token 错误码、未知 4xx、5xx、畸形 envelope 与禁止递归扫描。
- Runtime 和 HTTP 契约测试覆盖无 refresh token、永久 refresh token 拒绝只请求一次、刷新后第二次 401、临时刷新失败，以及相应的 failed/unverified 错误码。
- Runtime 测试覆盖明确耗尽后候选不可用、reset 时刻到期恢复、明确可用快照提前清除和 scheduler epoch 唤醒。
- Web 测试覆盖额度耗尽状态标记与失效账号删除流程。
