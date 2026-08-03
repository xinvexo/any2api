# ADR-0026: 有界优雅停机与进程任务生命周期

- 状态：Accepted
- 日期：2026-07-22
- 修订：2026-08-03
- 决策者：maintainer

## 背景

进程停机必须先停止 accept，再在有界时间内收敛现有连接、静默 SSE、配置发布、密码轮换、zstd 解压、应用更新、健康唤醒、RequestTelemetry Writer 和文件日志。任何未跟踪任务都可能越过实例锁生命周期或让最终 flush 顺序不确定。

项目不恢复任何运行态，但仍必须在当前进程退出前有界地释放请求、上游连接、SQLite Writer、文件日志和实例锁。

## 决策

- Runtime 提供唯一的进程级 `ProcessLifecycle`，状态为 `Running / Draining / Forced`，内部持有请求 TaskTracker、后台 TaskTracker、draining 通知与 forced 取消令牌。App 驱动状态，Server 和 Runtime 只消费稳定 API。
- Server 最外层 middleware 追踪完整 HTTP 生命周期。活动 Guard 在 Handler 完成后转移到包装响应 Body；强制取消时 Drop Handler future 或内部 Body，复用现有 QueueTicket、PreparedAttempt 与 GuardedBody 的 RAII 取消链。
- App 在宣布 listener 就绪前安装 Ctrl-C 与 Unix SIGTERM handler；安装失败属于启动失败，不能让已经可连接的
  socket 先暴露默认终止竞态。Axum graceful future 在收到信号时立即完成，从而立刻停止 accept；宽限计时器
  独立等待 server future，禁止为了“等待宽限期”继续接受新连接。自更新的启动确认还受 ADR-0089 约束。
- 配置发布、管理员密码轮换和已经接受的应用更新继续脱离客户端取消，但通过后台 TaskTracker 注册；Forced 后丢弃尚未完成的异步 future。应用更新不得使用脱管的 `tokio::spawn`，也不得在停机已开始后接受新安装；下载与 checksum 校验结束后，最终解包、冒烟、文件同步、previous/rename 提交及终态回调必须整体登记为同一 blocking closure。该登记调用本身不引入可取消等待，closure 自己持有临时目录和全部提交输入；外层更新 future 被 Forced Drop 后仍保持 Tracker 计数，完整替换成功后在 closure 内请求重启，预期失败也在 closure 内写入终态。Argon2 与取得专用并发许可后的 zstd 解压使用相同的 Tracker `spawn_blocking` 所有权规则。`SchedulerEpoch` 的健康唤醒是唯一、按需启动且可重排的后台 worker；它同时监听 draining 通知，进入 Draining 后退出并拒绝重新启动，无需等待任何远期 cooldown deadline。
- RequestTelemetry Worker 由同一 TaskTracker spawn，但保留专用 sender/JoinHandle 以执行“停止生产 → 排空 → 有界等待”。超时后 abort 并再次 await，保证 API 返回时 Writer 已退出。
- 二进制入口显式构建 Tokio Runtime，并在 Runtime 外先取得实例锁。文件日志把 ConfigPublisher 可持有的热更新
  控制句柄与唯一 `WorkerGuard` 分离；Composition Root 在写入最终停机事件后总能直接 Drop Guard，执行依赖
  提供的有界 best-effort flush，无需用 `Arc::try_unwrap` 猜测其他运行时所有权。
- 正常收尾完成后使用 `Runtime::shutdown_timeout` 有界关闭 Tokio worker/blocking pool，随后释放实例锁。活动
  请求、受管后台任务、RequestTelemetry 或 SQLite 未按预算收敛时进入致命退出：仍持有实例锁完成 runtime
  shutdown timeout，然后直接终止进程，由操作系统释放锁，禁止退出中 blocking task 与新实例重叠运行。
  文件日志队列/写入/flush 失败或控制句柄存活不是关键收尾失败，不阻断正常退出或已请求的自更新重启；
  详细决策见 ADR-0090。
- SettingRegistry 新增 `shutdown.request_grace_period=30_000ms` 与 `shutdown.finalize_timeout=5_000ms`。信号到达时从当前 PublishedSnapshot 一次性捕获；一次停机不混用后续 revision。
- 所有强制取消都只结束当前进程，不持久化或重放 in-flight、QueueTicket、会话、健康、冷却或重试状态。

## 取舍

- 不实现 Nginx 式多进程热升级、请求迁移或运行态快照；单节点新进程仍只从 SQLite 配置启动。
- 不自建第二套请求队列或在每个运行态 Guard 上重复增加 shutdown 分支。最外层 future/Body Drop 已能触发集中 RAII 清理。
- 文件日志使用 `tracing-appender` 现有有界 best-effort Guard，不为个人单节点场景再实现日志线程协议，也不把
  它无法报告的 flush 结果提升成进程正确性信号。
- Tokio 已登记的 blocking closure 无法依靠丢弃 JoinHandle 强制取消；Tracker 用于证明其是否结束，最终 runtime timeout 和致命进程退出负责给等待设置硬上限。更新提交选择这一语义是为了同时避免阻塞异步 worker 和半提交取消，而不是为普通 I/O 扩大不可取消区。
- Windows 处理 Ctrl-C；SIGTERM 只在 Unix 条件编译。不支持 Windows 服务控制事件。

## 后果

- 正常短请求和流可以在宽限期内完成；超过宽限期的静默流会以连接/Body 错误结束，并结算全部运行态 Guard。
- 客户端在 Draining 竞态中已经进入的请求仍被追踪；Forced 后不保证获得完整协议错误 envelope，但不会拼接第二条上游流。
- 未提交的配置或密码事务在 Forced Drop 时回滚；已提交事务不会由停机回滚，下次启动直接读取 SQLite 当前值。
- 只有全部关键受管资源完成收尾才记录 `shutdown complete`。关键收尾超时返回致命结果，不走正常实例锁
  释放路径；两种路径都在最后事件后 Drop 文件日志 Guard，使可刷新的诊断尽量落盘。

## 验证

- Lifecycle 单元测试覆盖状态单调转换、Draining 拒绝新 Guard、请求 Guard 持有到 Drop、配置/更新等异步任务 forced 收敛，以及 blocking JoinHandle 被 Drop 后仍保持追踪。App 更新执行器测试额外证明 Forced 只取消外层 future，已登记的更新提交仍在 blocking pool 完成且 Tracker 在此之前不会归零。
- Server 单元测试覆盖 Guard 随普通/静默响应 Body 存活，并验证 Forced 会 Drop 静默 Body、结束活动计数；zstd executor 测试覆盖并发上限、等待取消和已经开始的 blocking closure 在 Forced 后继续受管直至返回。
- RequestTelemetry 测试覆盖正常排空与超时 abort + join 后 Tracker 归零。
- App 测试使用可注入信号验证自然 drain、信号时读取最新 PublishedSnapshot 设置、后台任务 forced 收敛，以及 blocking 任务错过期限时明确返回收尾失败。
- 实例锁独占与 Drop 后重取由独立 App 单元测试覆盖；进程测试覆盖启动确认后立即发送 Unix SIGTERM 仍能
  有界退出，并保留正常停机的 `shutdown complete` 记录。
- 文件日志单元测试在热更新控制句柄仍存活时消费唯一 Guard，并证明已入队事件仍被刷新；这一路径不产生
  fatal `ShutdownOutcome`。
