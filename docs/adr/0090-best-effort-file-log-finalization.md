# ADR-0090：文件日志控制面与 best-effort 收尾

- 状态：Accepted
- 日期：2026-08-03
- 决策人：项目维护者
- 修订：ADR-0021、ADR-0026、ADR-0065

## 背景

旧实现把文件日志策略、动态级别和 `tracing_appender::WorkerGuard` 放在同一个 `Arc<FileLogging>` 中。
ConfigPublisher 通过整快照 reconciler 间接持有该 Arc；停机时 `Arc::try_unwrap` 失败就把
`"file logging still has active runtime owners"` 转成 fatal `ShutdownOutcome`。自更新已经替换磁盘二进制并
请求重启后，这个结构性所有权检查可以使旧进程直接退出而不执行新程序。

该检查不能证明 flush 失败，也不能定位关键任务泄漏。文件日志从设计上使用 lossy 有界队列，写线程会吞掉
I/O error 并继续工作；`WorkerGuard::drop` 只在内部以固定超时发送 shutdown/等待 flush，不返回可供应用判断的
结果。相比之下，活动请求、ProcessLifecycle 后台任务、RequestTelemetry 和 SQLite Pool 都有明确的完成协议，
它们未收敛才可能与新进程共享运行资源或丢失已经承诺的持久化边界。

## 决策

1. 文件日志拆成两个所有权对象：可克隆 `FileLoggingControl` 只包含热更新所需的策略锁和级别原子值；不可克隆
   `FileLogging` 由 Composition Root 独占，并持有唯一 `WorkerGuard`。ConfigPublisher/AppSnapshotReconciler
   只能持有前者，不能延长 Guard 生命周期。
2. 停机先按既有顺序停止 HTTP、受管后台任务和 RequestTelemetry，并关闭 SQLite。结果确定后写入最终事件：
   关键收尾成功记录 `shutdown complete`；失败记录结构化 error。随后两条路径都消费 `FileLogging`，直接
   Drop Guard，让依赖在自身有界等待内尽量刷新最终事件。
3. 文件日志队列丢弃、分段写入/清理失败、Guard best-effort flush 不完整以及控制句柄仍存活，都只降低本地
   诊断完整性；它们不产生 fatal `ShutdownOutcome`，不改变 HTTP server result，也不阻断已经请求的自更新
   restart。日志初始化失败仍是启动失败，因为此时应用尚未对外就绪。
4. 以下边界继续 fatal：活动请求在强制阶段后未结束、ProcessLifecycle 受管后台任务未收敛、RequestTelemetry
   无法完成其关闭协议、SQLite Pool 未在预算内关闭。HTTP server 自身错误继续通过原有 result 返回，并继续
   阻止 `result.is_ok()` 所要求的自更新 exec；本决策不弱化这些资源生命周期。
5. 本决策不顺带解决写线程运行期间 I/O error 的可见性与自愈；该问题由 `REV-APP-002` 独立实现。这里仅移除
   一个无法代表 I/O 成败、却会放大成进程终止的错误分类。

## 后果

- 后续新增日志策略消费者不会因多一个控制句柄而静默取消重启。
- WorkerGuard 的销毁时点由 Composition Root 明确拥有，不依赖 Router、Publisher 或后台任务 Arc 的精确
  Drop 顺序。
- 文件日志仍可能按既有 lossy/best-effort 契约丢行；控制台诊断和关键持久化收尾语义不因此伪装成更强保证。

## 验证

- 文件日志模块测试保留一个 `FileLoggingControl` clone，消费 `FileLogging` 后仍观察到已入队事件被 Guard
  刷新，证明控制句柄不再拥有日志线程。
- App 正常 SIGTERM 进程测试继续验证 `shutdown complete` 与零退出；Shutdown finalization 故障测试继续
  返回 fatal，证明关键资源边界未放宽。
- 自更新进程决策测试验证非关键日志收尾不能覆盖 `restart_requested`；相关 fmt、clippy、test 与架构门禁
  全部通过。
