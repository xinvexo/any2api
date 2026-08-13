# ADR-0133：拒绝可证明重复的 OAuth 导入身份

- 状态：Accepted
- 日期：2026-08-11
- 决策人：maintainer
- 修订：ADR-0044、ADR-0087、ADR-0147

## 背景

交互式 OAuth 登录已经按 Provider account ID、无 ID 时的规范化邮箱识别同一官方账号，并在唯一命中时重新授权原记录。Provider 专用 JSON 导入则遵循 ADR-0044 的“输入多少条就创建多少条”语义，既不检查同批输入，也不检查当前 PublishedSnapshot。

因此，同一个外部账号文件可以被重复导入，或在同一批中出现多次，并被编译为多个独立 `RoutingCredential`。这些记录拥有不同本地 ID、RPM、健康、连接池和粘性，却发送相同官方身份与 Token；调度器会把一次上游账号误当成多个候选，后台 Token/额度操作也会重复执行。这是认证身份和路由身份不一致，而不是标签冲突。

导入不得根据不可靠相似性覆盖既有配置。只有能够从当前规范 Token 材料证明重复时才拒绝；不能证明相同的账号继续保持独立。

## 决策

1. 导入仍只创建，不覆盖、合并或重新授权既有 OAuthAccount。发现重复时整个 HTTP 导入批次返回 `409 oauth_account_identity_conflict`，SQLite revision、Runtime 和 PublishedSnapshot 均不变化。
2. 每个规范化 `OAuthTokenMaterial` 在进入发布任务前产生短期、不可显示的导入身份键：
   - 对应 Provider Driver 投影的稳定主体身份；Codex 按 ADR-0147 使用工作区与成员主体的组合，绝不单独使用工作区 ID；
   - Driver 无法投影主体但有非空邮箱时，使用 `ProviderKind + trim/lowercase email` 回落；
   - 任何账号：额外加入 `ProviderKind + token kind + domain-separated SHA-256(exact access/refresh/ID token)` 短期键；任一完全相同的认证 Token 都能证明本次导入重复使用同一凭据。
3. Driver 主体身份优先于邮箱。已有强主体身份的账号不因邮箱相同而与邮箱回落账号冲突；不同 Token 不会被猜测为相同，但即使某一侧缺失稳定身份，完全相同的 access、refresh 或 ID token 仍属于可证明的重复凭据。
4. Token digest 按 access、refresh、ID token 分别计算，包含 Provider、Token kind、长度边界和固定 domain separation；不持久化、不进入 DTO、错误、普通日志、`Debug` 或浏览器状态。它只用于同一发布临界区内的集合比较。
5. `ConfigPublisher` 在取得串行发布锁并读取最新 PublishedSnapshot 后，同时比较全部待导入身份、当前已编译账号身份和同批其他输入。这样并发登录/导入不能在预检查与提交之间插入重复账号。
6. 历史上已经存在的重复记录不在启动期自动删除、合并或改写，也不由生产代码执行数据迁移。交互式重新授权继续对多重命中 fail closed；管理员可以在运行实例中明确删除重复记录。
7. 无身份且 Token 不同的输入继续允许。标签唯一化仍只解决展示名称，不参与认证身份判断。

## 后果

- 重复上传同一账号文件不再扩大路由候选、后台刷新和额度查询数量。
- 导入保持 all-or-nothing 与单 revision 发布，不需要 Schema 或 Migration。
- Token 轮换不能通过重复导入静默覆盖原账号；应使用交互式重新授权，或由管理员明确删除旧导入记录后再导入。
- 无法证明相同的账号不会因邮箱猜测或标签相似被误合并。

## 验证

- Runtime 测试覆盖同批稳定身份重复、与当前账号重复、无稳定身份但完整 Token 相同，以及无稳定身份且 Token 不同。
- 所有冲突均断言 SQLite 与 PublishedSnapshot revision、账号数量和 Runtime reconcile epoch 不变。
- HTTP 契约断言返回 `409 oauth_account_identity_conflict`，响应不包含邮箱、account ID、Token 或 digest。
- 原多文件、多 Provider、标签去重、边界限制与 canonical document 测试继续通过。
