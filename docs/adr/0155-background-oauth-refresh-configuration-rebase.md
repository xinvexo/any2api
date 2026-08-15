# ADR-0155: 管理配置跨自动 OAuth 刷新透明 Rebase

- 状态：Accepted
- 日期：2026-08-16
- 决策者：maintainer

## 背景

配置发布已经由 `ConfigPublisher` 的单一异步互斥锁串行。管理请求在另一个发布进行时会等待锁，但取得锁后仍用
页面提交的全局 `expected_revision` 做严格比较。定时或认证失败触发的 OAuth Token 刷新也必须通过 SQLite
事务和完整 `PublishedSnapshot` 发布，因此它会推进同一个全局 revision。页面读取 Provider Endpoint 或
Credential 后，只要后台刷新过任意无关 OAuthAccount，后续保存就会立即返回 `revision_conflict`；账号越多，
误冲突越频繁。

直接忽略全局 revision 会允许两个管理员写入互相覆盖；浏览器看到 409 后盲目刷新并重试也无法区分无关后台
换代与真实管理并发。把 OAuth Token 排除在 PublishedSnapshot revision 之外则会破坏 Token CAS、认证代际与
数据库/运行快照的一致发布边界。

## 决策

1. 每次成功配置发布标记为 `Operator` 或 `AutomaticOAuthRefresh`。OAuth 登录、重新授权和 JSON 导入属于
   `Operator`；只有定时、额度链或认证失败触发的 Token 换代属于 `AutomaticOAuthRefresh`。
2. `SnapshotStore` 在内存中维护最近一次成功 `Operator` 发布的 revision。启动时把水位初始化为启动快照
   revision；自动刷新不推进该水位，成功的管理发布在快照切换时推进它。
3. 管理 mutation 仍先等待唯一发布锁。取得锁后：
   - 当前 revision 等于 `expected_revision` 时按原路径发布；
   - 当前 revision 更大，且 `expected_revision` 不早于管理员水位时，错过的发布只可能是自动 Token 刷新，
     ConfigPublisher 在当前完整快照上重新执行命令校验，并以当前 revision 开启一次配置事务；
   - 当前 revision 更小，或 `expected_revision` 早于管理员水位时返回 `RevisionConflict`。
4. Rebase 只替换事务的全局 revision 前置条件，不修改 mutation 内容，不循环重试，也不绕过聚合级 CAS。
   Endpoint、Credential、OAuthAccount、Gateway Key 的 `config_version`/`token_version` 仍由 Storage 在最新
   事务视图中核对；目标对象发生真实变化时继续返回对应的类型化冲突。
5. 管理员水位不持久化。进程重启后以加载到的 revision 作为新水位，因此重启前页面携带的旧 revision
   fail-closed，必须重新读取配置。正在执行的数据面请求继续持有其原 PublishedSnapshot Arc，不参与发布等待。

## 备选方案

- 浏览器收到 409 后自动 refetch 并重放：会把另一位管理员或另一个标签页的真实修改当成后台刷新，存在覆盖风险。
- 所有管理写入无条件采用当前 revision：同样丢失全局乐观并发保护。
- 为 OAuth Token 使用独立、不推进快照的 revision：需要拆分数据库与运行快照提交语义，并会削弱当前原子发布边界。
- 持久化逐 revision 发布来源历史：单节点进程内只需判断当前生命周期内是否出现过管理发布；重启后 fail-closed
  更简单，也不增加 Schema 和迁移。

## 后果

- 后台 Token 刷新可以先完成，排队中的管理保存随后基于最新快照继续，不再产生无关的 409。
- 两项基于同一旧页面的管理修改仍只有第一项成功；第二项看到管理员水位推进后返回 `RevisionConflict`。
- 管理 mutation 携带的聚合级前置版本不再匹配时，版本检查仍会拒绝提交，不会用透明 rebase 绕过 CAS。
- 水位只负责并发分类，不是第二份配置真相，也不改变 SQLite revision 或公开响应结构。

## 验证

- Runtime 发布测试先执行管理员 OAuth 激活，再执行自动 Token 刷新，随后用刷新前的 expected revision 提交
  Provider mutation，确认发布成功且 revision 单调推进。
- 并发管理员发布测试继续确认两个相同 expected revision 的 mutation 只有一个成功，另一个返回
  `RevisionConflict`。
- 现有 OAuth 登录、导入、批量刷新与提交原子性回归继续通过，确认来源标记没有改变这些发布边界。
