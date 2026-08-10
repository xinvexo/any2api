# ADR-0087：交互式 OAuth 同账号重新授权

- 状态：Accepted
- 日期：2026-08-03
- 决策人：项目维护者
- 修订：ADR-0033、ADR-0078、ADR-0133

## 背景

交互式 OAuth 登录原先每次生成新的 `OAuthAccountId`，并用 Token 邮箱作为默认标签。同一账号 Token 被吊销或管理员主动重新登录时，第二次发布通常先撞到 `(provider_kind, label_key)` 唯一约束而失败。仅给标签追加 `(2)` 虽能让发布成功，却会把同一上游账号变成两条独立路由凭据，重复维护 RPM、健康、粘性、模型和管理状态。

本地 `OAuthAccountId` 已被 RequestLog、会话绑定、配置编辑和运行态句柄引用，因此同一上游身份的认证恢复应保持该 ID。与此同时，不能只按邮箱覆盖：Codex/Grok 可以提供更稳定的 account ID，同一邮箱也可能对应不同上游主体；导入路径还可能已经产生重复身份记录。

## 决策

1. 交互式 Token 交换完成后生成内部稳定身份投影：Provider account ID 存在时使用其精确值；只有 Token 没有 account ID 时才使用 trim 后大小写规范化的邮箱。身份包含 Provider，不能跨 Provider 匹配。
2. 配置发布锁内读取当前 PublishedSnapshot，并用当前已编译 Token 材料匹配。account ID 身份只匹配同 account ID；邮箱身份只匹配同样没有 account ID 且邮箱相同的账号，禁止用邮箱覆盖一个已经有稳定 account ID 的记录。
3. 恰好匹配一条时重新授权原 `OAuthAccount`：保留 `OAuthAccountId`、label、`requests_per_minute`、enabled 与 `account_generation`；替换 OAuth JSON、safe email、expiry，并只递增 `token_version`。写入使用锁内读取的 `token_version` CAS；新的认证健康与旧 Token 隔离，账号级额度/权限/模型健康继续复用，边界见 ADR-0095。
4. 重新授权不自动扩大管理员选择的模型。新模型目录与原选择取交集；失效模型在同一事务中移除，只有交集变化时才递增 `config_version`。交集可以为空，继续表示不开放该账号的任何模型。
5. 没有身份匹配时创建新账号，并在同一发布锁内生成 Provider 内唯一标签。无 account ID 且无邮箱的 Token 没有稳定身份，每次登录都按新账号处理。
6. 匹配到多条记录时返回 `409 oauth_account_identity_conflict`，不任意挑选、不覆盖、不新增。管理员需先删除重复记录再重新登录。
7. Provider JSON 导入继续保持原子、只创建且不静默覆盖现有账号；ADR-0133 之后，可由 Provider account ID、双方无 ID 时的规范化邮箱，或同一 Provider 下任一 access/refresh/ID Token 完全相同证明为重复的输入会整批拒绝。历史重复记录仍会使之后的交互式重新授权明确冲突。
8. 新建与重新授权都必须执行完整候选配置校验，成功后提交 SQLite、reconcile Runtime 并切换一次 PublishedSnapshot，之后才能返回安全元数据。响应、日志和浏览器状态不得包含身份原值之外的 Token 材料；内部身份值不进入 Debug 或错误消息。

## 后果

- 被吊销或过期的账号可以通过正常登录恢复，不需要先删除本地配置。
- 管理员设置、历史账号 ID 与仍有效的模型选择保持稳定，不会因为认证恢复产生重复候选或模型权限扩张。
- 当数据无法唯一证明目标账号时显式失败，避免覆盖错误主体。
- 不需要新增数据库列或迁移；身份从当前已验证并编译的 Provider Token 文档中短暂投影。

## 验证

- 身份单元测试覆盖 account ID 优先、邮箱仅在双方无 ID 时回落、Provider 隔离和无身份情况。
- Storage 测试覆盖重新授权的 token-version CAS、文档替换、配置字段保留、模型交集持久化和重开数据库后一致性。
- HTTP 契约连续完成两次同账号登录，中间修改 label、RPM、enabled 和模型；第二次返回相同 `OAuthAccountId`，只保留一条记录并写入新 Token，旧配置保持不变。
- 契约覆盖同邮箱但不同稳定 account ID 不被覆盖，以及多条同身份记录返回明确冲突。
