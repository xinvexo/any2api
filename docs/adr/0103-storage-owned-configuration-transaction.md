# ADR-0103: Storage 独占配置事务能力

- 状态：Accepted
- 日期：2026-08-03
- 修订：2026-08-04（Commit 确认丢失与 phase-two I/O fail-fast）
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
6. 配置发布的取消边界覆盖整个“Storage 事务调用 → Runtime Binding → ArcSwap”阶段，而不只覆盖
   `commit().await`。当前固定的 sqlx 0.8.6 SQLite Driver 在专用 Worker 上执行 `COMMIT`，并通过只有接收方
   确认后发送方才完成的 rendezvous oneshot 返回结果；如果调用 Future 在命令发出后被 Drop，Worker 仍可能
   完成 SQLite 提交，然后识别确认接收方已经消失。这不是一个返回给 Repository 的 `sqlx::Error`，而是调用栈
   已被取消。因此 ConfigPublisher 必须继续用进程级 critical task 脱离 HTTP waiter；客户端取消只能丢弃等待
   结果，不能取消发布任务。只有生命周期已经单调进入 `Forced`、HTTP 不再接收请求且进程正在退出时才允许取消
   critical task；该状态不能恢复为旧快照继续服务。
7. SQLite 3.46.0 WAL 在 commit frame 已写入后仍调用 VFS `SQLITE_FCNTL_COMMIT_PHASETWO`。SQLite 对该 opcode
   的定义明确说明此时事务已经提交；把主数据库文件的 VFS `xFileControl` 故障注入为 `SQLITE_IOERR`，能够稳定
   复现“新 revision 对其他连接可见，但 `transaction.commit().await` 返回错误”。因此 Storage 在 Commit 阶段
   单独分类结果：SQLite extended code 的 primary code 为 `SQLITE_IOERR`，或 sqlx Worker 无法返回数据库错误的
   其他通信失败，统一转为 `IndeterminateConfigurationCommit`。其中可能包含实际在 phase one 发生、尚未提交的
   I/O 失败；为保证一致性允许保守误杀，不尝试在故障连接上判定或修复。
8. ConfigPublisher 不得把 `IndeterminateConfigurationCommit` 转为可返回的管理 API 错误。它必须使受管 critical
   task 失败，由 `publish_task` 立即终止进程；下次启动从 SQLite 重建快照。延迟外键或 commit hook 拒绝产生的
   `SQLITE_CONSTRAINT` 已由故障注入证明回滚，仍按普通 `StorageError::Database` 返回。禁止在错误后查询 revision
   再猜测是否切换：查询本身可能失败，而且无法证明所有配置表与预编译候选对应同一个已提交结果。

## 备选方案

- 保留活事务句柄并依赖注释/代码评审：无法从类型和 API 阻止未来跨网络 `await`，不采用。
- 给活事务句柄增加超时：只能在错误已经发生后回滚，还会把正常的大候选编译与超时策略耦合，不采用。
- Commit 后再编译：缩短写锁，但编译失败时 SQLite revision 已推进而 PublishedSnapshot 未切换，破坏原子发布。
- Storage 保存事务到内部 ID map，Runtime 只持有 token：隐藏了具体类型，却仍允许 Runtime 无限延长写锁，并
  增加清理状态，不采用。
- 让 Storage 依赖 Runtime 的具体快照类型：违反 crate 依赖方向；泛型同步回调保留了边界反转。

## 后果

- SQLite 写事务的完整生命周期只能在 Storage 的一个 async 调用栈内观察；Runtime API 不再具备事务能力。
- Runtime 的候选编译仍发生在 Commit 前；明确约束拒绝继续回滚且不切快照，提交结果不确定则进程退出。
- 事务 Repository 需要两个固定泛型参数，但 ConfigPublisher 内部将其固定为
  `PreparedPublishedSnapshot` 与 `ConfigPublishError`，不会扩散到管理 API。
- HTTP waiter 取消不会缩短 SQLite 写锁持有期；这是保证提交确认与快照切换连续完成的必要所有权边界。Forced
  停机仍有界取消尚未完成的异步事务，可能留下只能由进程重启重新加载的已提交 revision，但不会继续对外服务。
- Commit I/O 错误不再总是普通可恢复错误；即使故障实际发生在提交点之前，进程也会为避免旧快照继续服务而
  保守退出。约束、冲突等具有确定未提交语义的错误不受影响。

## 验证

- Storage 模块测试覆盖 no-op 不调用编译器、同步接受后提交、同步拒绝后回滚、Commit 失败不返回接受值，以及
  真实 sqlx SQLite `COMMIT` waiter 被取消后仍可能落盘的确认丢失窗口。
- Runtime 原子性测试继续覆盖候选编译失败、Commit 失败、成功发布、revision watch 与 scheduler epoch，并在
  真实 SQLite 已提交、Repository 暂停返回的窗口取消 HTTP waiter，确认 detached critical task 最终仍切换
  同一 revision 的快照。
- 外部 VFS fault fixture 在 `SQLITE_FCNTL_COMMIT_PHASETWO` 返回 `SQLITE_IOERR`，同时断言观察连接读到新值；
  Storage 分类回归固定 primary IOERR/Worker 确认丢失映射为 `IndeterminateConfigurationCommit`，Runtime 直接
  发布阶段回归断言该类型不会降级为普通错误，而是触发 critical task 的 fatal panic。
- 架构检查拒绝 ConfigPublisher 之外的生产 `transact_configuration` 调用，并确认公开 API 不再导出
  `PreparedConfiguration`、`ConfigurationCommit` 或 `sqlx::Transaction`。

## 依据

- [SQLite File Control Opcodes](https://www.sqlite.org/c3ref/c_fcntl_begin_atomic_write.html#sqlitefcntlcommitphasetwo)
  定义 `SQLITE_FCNTL_COMMIT_PHASETWO` 在事务提交后、数据库解锁前发出。
- [SQLite Commit Hook](https://www.sqlite.org/c3ref/commit_hook.html) 定义非零 hook 结果把 Commit 转成 Rollback。
- [sqlx 0.8.6 SQLite Worker](https://github.com/launchbadge/sqlx/blob/v0.8.6/sqlx-sqlite/src/connection/worker.rs)
  明确处理 “COMMIT was processed but not acknowledged”，并用接收方确认的 rendezvous oneshot 交付结果。
