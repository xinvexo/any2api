# ADR-0103: Storage 独占配置事务能力

- 状态：Accepted
- 日期：2026-08-03
- 决策者：maintainer

## 背景

配置发布必须在 SQLite Commit 前用 Runtime 的 Provider/Protocol Registry 完成整份候选能力校验和
`PreparedPublishedSnapshot` 预编译，因此候选不能先提交再交给 Runtime。此前 Storage 返回
`PreparedConfiguration`，其中私有持有 `Transaction<'static, Sqlite>`；Runtime 取得候选后同步编译，再调用
`ConfigurationCommit::finish` 或 `rollback`。当前编译路径是纯 CPU 且没有 `await`，实际行为正确，但 API 让
活 SQLite 写事务离开 Storage，并允许未来调用者在持有写锁时任意等待网络、Timer 或其他异步资源。

仅把 `Transaction` 字段设为私有不能约束锁的持有期；静态架构检查只能限制谁调用，不能证明该调用点未来没有
在验收前后增加异步等待。另一方面，把快照编译移到 Commit 后会重新引入“数据库已提交但运行快照无法构造”的
原子性缺口。

## 决策

1. `ConfigurationRepository` 只保留已提交配置读取；新增泛型、对象安全的
   `ConfigurationTransactionRepository<Accepted, Rejected>` 作为 mutation 事务端口。
2. 事务端口接收类型擦除的同步
   `FnOnce(StoredConfiguration) -> Result<Accepted, Rejected>` 候选编译器。Storage 自行执行
   `BEGIN IMMEDIATE`、mutation、影响面回读与一致性核对，然后在仍持有事务时同步调用编译器：
   - no-op 不调用编译器，Storage Rollback 后只返回 `NoChange`；
   - 编译器拒绝候选时，Storage Rollback 成功后返回类型化拒绝；
   - 编译器接受候选时，Storage Commit 成功后才返回 `Accepted` 值；Commit 失败则丢弃该值并返回
     `StorageError`。
3. 同步回调的返回类型不能是 Future，因而候选验收链无法直接插入 `.await`。回调只允许确定性的 CPU/内存
   校验与本地预编译；网络、Timer、DNS、文件 I/O 或 Provider 探测必须在进入事务前完成，或作为另一次明确
   工作流处理。
4. 删除公开 `PreparedConfiguration` 与 `ConfigurationCommit`，不保留兼容构造器或双轨 API。Runtime 从事务
   调用返回后只持有已提交的 `PreparedPublishedSnapshot`，再执行无失败 Runtime reconcile 和单次 ArcSwap；
   它不能 Commit、Rollback、延长或泄漏 SQLite 写锁。
5. `ConfigPublisher` 是该事务端口唯一生产调用者。测试若需直接提交候选，必须使用显式测试夹具并提供同步
   identity 编译器；`xtask architecture-check` 检查新的调用名并拒绝其他生产调用点。

## 备选方案

- 保留活事务句柄并依赖注释/代码评审：无法从类型和 API 阻止未来跨网络 `await`，不采用。
- 给活事务句柄增加超时：只能在错误已经发生后回滚，还会把正常的大候选编译与超时策略耦合，不采用。
- Commit 后再编译：缩短写锁，但编译失败时 SQLite revision 已推进而 PublishedSnapshot 未切换，破坏原子发布。
- Storage 保存事务到内部 ID map，Runtime 只持有 token：隐藏了具体类型，却仍允许 Runtime 无限延长写锁，并
  增加清理状态，不采用。
- 让 Storage 依赖 Runtime 的具体快照类型：违反 crate 依赖方向；泛型同步回调保留了边界反转。

## 后果

- SQLite 写事务的完整生命周期只能在 Storage 的一个 async 调用栈内观察；Runtime API 不再具备事务能力。
- Runtime 的候选编译仍发生在 Commit 前，原有失败回滚与 Commit 失败不切快照语义不变。
- 事务 Repository 需要两个固定泛型参数，但 ConfigPublisher 内部将其固定为
  `PreparedPublishedSnapshot` 与 `ConfigPublishError`，不会扩散到管理 API。

## 验证

- Storage 模块测试覆盖 no-op 不调用编译器、同步接受后提交、同步拒绝后回滚和 Commit 失败不返回接受值。
- Runtime 原子性测试继续覆盖候选编译失败、Commit 失败、成功发布、revision watch 与 scheduler epoch。
- 架构检查拒绝 ConfigPublisher 之外的生产 `transact_configuration` 调用，并确认公开 API 不再导出
  `PreparedConfiguration`、`ConfigurationCommit` 或 `sqlx::Transaction`。
