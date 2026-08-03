# ADR-0095：拆分认证健康与路由身份健康代际

- 状态：Accepted
- 日期：2026-08-03
- 决策人：项目维护者
- 修订：ADR-0013、ADR-0033、ADR-0070、ADR-0087

## 背景

`CredentialGenerationRuntime` 原先把认证错误、账号额度耗尽、Credential 级权限冷却和模型冷却放在同一个对象中，并在 `routing_generation` 或 `authentication_version` 任一变化时整体重建。OAuth 定时刷新和同身份重新授权会同时增加 `token_version` 与 `account_generation`，因此一次仅替换 Token 的正常操作也会无条件清空额度与冷却状态，令已知不可用账号立刻重新进入候选池。

直接跨 Token 版本复用整个健康对象同样不安全。旧快照中使用旧 Token 的迟到 401 会把共享的 `auth_error` 写入新快照，导致已经刷新成功的新 Token 被旧 Attempt 错误停用。

API Key 轮换与 OAuth Token 轮换的身份语义不同：API Key 替换可能代表另一上游主体，必须重置全部 Credential 健康；OAuth refresh 和已确认同一 Provider 身份的重新授权只替换该账号的认证材料，账号级额度与权限身份没有改变。

## 决策

1. `CredentialGenerationRuntime` 同时携带两个显式版本：`routing_generation` 表示可共享账号/路由健康的身份代际，`authentication_version` 表示当前认证材料代际。
2. Credential 健康拆为两部分：
   - authentication health 只保存 `auth_error`，严格属于单个 `authentication_version`；
   - routing health 保存 Credential 级权限冷却、模型冷却和权威额度耗尽状态，属于单个 `routing_generation`。
3. Runtime reconcile 遵循三种情况：两个版本都相同时复用完整 generation；只有认证版本变化时创建新的 authentication health 并复用原 routing health；路由 generation 变化时两部分都新建。
4. 旧 PublishedSnapshot 继续持有旧 authentication health。旧 Token Attempt 的迟到认证失败只能写入旧对象，不能污染新 Token；同一 `routing_generation` 的旧 Attempt 对额度、权限和模型冷却的更新继续作用于共享 routing health，因为这些信号属于同一上游账号身份。
5. OAuth refresh 和已确认同一 Provider 身份的交互式重新授权只增加 `token_version`，不增加 `account_generation`。重新授权造成的模型集合变化仍按既有规则增加 `config_version`，但不改变账号健康身份。
6. OAuthAccount 从 disabled 重新 enabled 时增加 `account_generation` 并重建两类健康，保留管理员显式重新启用作为恢复边界。删除后重新创建是新的稳定 ID，也自然获得全新 Runtime Handle。
7. Provider API Key 轮换继续同时增加 `secret_version` 与 `credential_generation`；Endpoint URL/协议身份变化和重新启用继续增加 `credential_generation`，因此不会把旧 Key 或旧路由身份的健康带入新配置。
8. 两类健康仍只在进程内存在，共用 `SchedulerEpoch` 和已有有界等待机制；不增加持久化字段、后台队列、恢复逻辑或兼容分支。

## 后果

- 正常 OAuth Token 保活不再让已知额度耗尽或权限受限的账号无意义冷启动。
- 新 Token 不继承旧 Token 的认证失败，也不会被退役快照的迟到 401 污染。
- 同一账号的迟到额度信号仍能约束当前路由，API Key/Endpoint 身份替换则保持完整隔离。
- 现有 SQLite Schema 足以表达两个版本，不需要 Migration。

## 验证

- Domain 测试断言 refresh 与同身份 reauthorization 只增加 `token_version`，重新启用仍增加 `account_generation`。
- Runtime 测试断言仅认证材料换代时额度/模型冷却被复用、认证错误被清空，且旧 generation 的迟到认证失败不能污染新 generation。
- Runtime 测试断言 `routing_generation` 变化后认证与路由健康均为全新状态。
- OAuth refresh、Storage 与 HTTP 契约继续验证 token-version CAS、单次配置发布及持久化版本一致性。
