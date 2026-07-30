# ADR-0070：OAuth 认证失效分类与额度路由健康

- 状态：Accepted
- 日期：2026-07-30
- 决策人：项目维护者
- 修订：ADR-0036、ADR-0045、ADR-0046、ADR-0060

## 背景

额度管理请求遇到 401 时会刷新 Token 后重试。原实现把刷新端点所有非 2xx 响应体丢弃，因此 OAuth 标准的 `invalid_grant` 与网络或 5xx 故障都被折叠成“认证无法确认”，管理面的“删除失效账号”无法删除已经明确失效的账号。

额度结果原本只用于展示。即使 Codex 返回 `allowed=false`、`limit_reached=true`，或 Grok 返回权威 Token 剩余量为零，同一账号在短期冷却后仍会重新进入路由候选，持续产生已知无效请求。

## 决策

1. Provider Driver 对刷新端点的有界结构化错误 envelope 分类。前一 access token 已经被 401 拒绝后，下列情形返回 `oauth_account_authentication_failed`：刷新成功后再次 401；账号没有 refresh token；刷新端点返回明确永久失效码，首批至少识别标准 `invalid_grant`。禁止递归扫描任意 JSON 值或依赖自然语言消息。
2. 刷新网络错误、超时、5xx、响应超限、畸形或未知错误码仍返回 `oauth_account_authentication_unverified`。这些账号不得进入批量删除候选。
3. “删除失效账号”继续由前端复用逐账号实时诊断、重新读取安全元数据、核对 `token_version` 并串行调用现有 DELETE；不新增批量删除端点，不下发 OAuth JSON。
4. 额度查询只在明确、账号级的权威信号出现时更新当前 OAuthAccount 认证 generation 的内存健康：`allowed=false`、`limit_reached=true`、Provider 声明的额度耗尽诊断或权威 Token `remaining=0`。未知值、单个模型/时间窗口达到 100% 和本地估算均不得排除账号。
5. 明确耗尽的账号在上游 reset 时刻前不进入普通路由候选；没有可靠 reset 时刻时使用 `cooldown.permission_denied` 作为有界兜底探测间隔。到期只恢复一次正常候选资格，不恢复或持久化额度状态；下一次明确失败可重新建立状态。
6. 明确可用的后续额度查询、成功数据面请求或成功 Codex reset 清除当前 generation 的耗尽状态。建立、清除和到期复用统一 scheduler epoch 与 QueueTicket，不新增额度队列。
7. 额度健康、认证健康、等待和 reset 时刻都只存在于进程内并按认证 generation 隔离。它们不写 SQLite、OAuth JSON、RequestLog、PublishedSnapshot 或浏览器存储，也不影响 GatewayApiKey。

## 后果

- 管理员可以删除经过结构化证据确认失效的 OAuthAccount，同时短暂上游故障不会导致误删。
- 已明确耗尽的账号不再持续进入代理池；额度恢复后无需修改配置即可重新参与路由。
- Claude 当前只提供窗口使用率，没有全局可用信号，因此窗口达到 100% 仍只展示，不改变路由。

## 验证

- Provider 测试覆盖 `invalid_grant`、未知 4xx、5xx、畸形 envelope 与禁止递归扫描。
- Runtime 和 HTTP 契约测试覆盖无 refresh token、`invalid_grant`、刷新后第二次 401、临时刷新失败，以及相应的 failed/unverified 错误码。
- Runtime 测试覆盖明确耗尽后候选不可用、reset 时刻到期恢复、明确可用快照提前清除和 scheduler epoch 唤醒。
- Web 测试覆盖额度耗尽状态标记与失效账号删除流程。
