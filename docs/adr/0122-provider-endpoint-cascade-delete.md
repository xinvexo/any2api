# ADR-0122: Provider Endpoint 确认后的级联删除

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer
- 修订：ADR-0003

## 背景

管理 Web 的 Endpoint 删除确认框明确告知操作员绑定的 API Key 会一起删除，但旧实现仍以
`ProviderEndpointInUse` 拒绝删除。这让确认后的行为与实际结果矛盾，也迫使操作员重复删除本来已经明确
授权移除的子配置。Endpoint 是 Credential 的所有者边界；其模型选择还会物化为共享的 Model Route/Target，
不能只删除父行而留下悬挂引用。

## 决策

1. 管理员在 Endpoint 删除确认框中确认后，Storage 在同一个 `BEGIN IMMEDIATE` 配置事务内删除该 Endpoint
   及其全部 `ProviderCredential`、API Key Secret、`provider_credential_models` 行。
2. 删除 Endpoint 前先按当前 Credential 集合重建候选 Model Route，并完成差异同步。只由被删除 Endpoint
   提供的 Route/Target 删除；其他 Endpoint 仍提供的 Route/Target 保留。随后按现有规则裁剪失效的
   `models.allowed` allowlist，`"all"` 与 OAuthAccount 模型不受影响。
3. 删除成功后统一递增配置 revision，并通过既有 readback/PublishedSnapshot 发布完整候选；任何校验、物化、
   SQL 或发布失败都回滚整个事务。历史 RequestLog/RequestAttempt 只按现有 `ON DELETE SET NULL` 保留，
   不删除历史遥测。
4. 不改变 Proxy 的引用保护，也不允许客户端 GatewayApiKey 影响级联范围。Endpoint 删除不再返回
   `provider_endpoint_in_use`；未找到 Endpoint 和 revision 冲突仍保持原有管理契约。

## 后果

- 一次确认即可清理 Endpoint 及其 API Key/模型路由，Web 不再出现“确认会删除、实际不能删除”的冲突。
- 共享模型路由会保留其他 Endpoint 的 Target，单一 Endpoint 的模型会从路由和 allowlist 中同时消失。
- 级联删除 API Key Secret 是显式、不可逆的管理操作，因此确认文案必须同时说明 API Key、模型权限与对应路由目标会被移除。
- SQLite 现有 `RESTRICT` 外键继续作为最后一道一致性保护；应用按正确的子表、物化路由、父表顺序删除，
  不需要改写历史 Migration 或引入运行时双轨 Schema。

## 验证

- Storage 回归覆盖 Endpoint 删除同时移除 Credential、Secret、模型选择和仅由该 Endpoint 提供的 Route/Target，
  以及共享模型仍保留其他 Endpoint Target。
- 管理 API/前端契约覆盖删除成功响应和新的确认文案；Provider Endpoint 不再映射
  `provider_endpoint_in_use`。
