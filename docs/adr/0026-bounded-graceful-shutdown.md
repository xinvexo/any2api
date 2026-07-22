# ADR-0026: 有界优雅停机与进程任务生命周期

- 状态：Accepted
- 日期：2026-07-22
- 决策者：maintainer

## 背景

当前服务只把 Ctrl-C 交给 Axum graceful shutdown。Axum 会停止 accept，但会无限等待现有连接；静默 SSE 可以永久占用 Credential Permit 和实例锁。配置发布、密码轮换、健康唤醒和 RequestTelemetry Writer 又各自直接 `tokio::spawn`，其中 Writer 超时后丢弃 JoinHandle 会变成脱管任务。文件日志 Guard 还被间接持有，最终 flush 顺序不确定。

项目不恢复任何运行态，但仍必须在当前进程退出前有界地释放请求、上游连接、SQLite Writer、文件日志和实例锁。

## 决策

- Runtime 提供唯一的进程级 `ProcessLifecycle`，状态为 `Running / Draining / Forced`，内部持有请求 TaskTracker、后台 TaskTracker、draining 通知与 forced 取消令牌。App 驱动状态，Server 和 Runtime 只消费稳定 API。
- Server 最外层 middleware 追踪完整 HTTP 生命周期。活动 Guard 在 Handler 完成后转移到包装响应 Body；强制取消时 Drop Handler future 或内部 Body，复用现有 QueueTicket、PreparedAttempt、GuardedBody 与 Permit 的 RAII 取消链。
- Axum graceful future 在收到 Ctrl-C 或 Unix SIGTERM 时立即完成，从而立刻停止 accept；宽限计时器独立等待 server future，禁止为了“等待宽限期”继续接受新连接。
- 配置发布和管理员密码轮换继续脱离客户端取消，但通过后台 TaskTracker 注册；Forced 后丢弃尚未完成的异步 future。Argon2 使用同一 Tracker 的 `spawn_blocking` 注册，外层 future 被 Drop 后仍保持计数，直到不可取消的 blocking closure 真正返回。健康唤醒任务同时监听 draining 通知，无需等待远期 cooldown deadline。
- RequestTelemetry Worker 由同一 TaskTracker spawn，但保留专用 sender/JoinHandle 以执行“停止生产 → 排空 → 有界等待”。超时后 abort 并再次 await，保证 API 返回时 Writer 已退出。
- 二进制入口显式构建 Tokio Runtime，并在 Runtime 外先取得实例锁。Composition Root 保留文件日志根 `Arc`；HTTP、后台任务和遥测结束后显式关闭 SQLite Pool，确认根 `Arc` 是最后一个文件日志所有者，再 Drop `WorkerGuard`。
- 正常收尾完成后使用 `Runtime::shutdown_timeout` 有界关闭 Tokio worker/blocking pool，随后释放实例锁。后台任务、SQLite 或文件日志所有权检查失败时进入致命退出：仍持有实例锁完成 runtime shutdown timeout，然后直接终止进程，由操作系统释放锁，禁止旧 blocking task 与新实例重叠运行。
- SettingRegistry 新增 `shutdown.request_grace_period=30_000ms` 与 `shutdown.finalize_timeout=5_000ms`。信号到达时从当前 PublishedSnapshot 一次性捕获；一次停机不混用后续 revision。
- 所有强制取消都只结束当前进程，不持久化或重放 in-flight、QueueTicket、会话、健康、冷却或重试状态。

## 取舍

- 不实现 Nginx 式多进程热升级、请求迁移或运行态快照；单节点新进程仍只从 SQLite 配置启动。
- 不自建第二套请求队列或在每个 Permit 上重复增加 shutdown 分支。最外层 future/Body Drop 已能触发集中 RAII 清理。
- 文件日志使用 `tracing-appender` 现有有界 best-effort Guard，不为个人单节点场景再实现日志线程协议。
- Tokio 已开始执行的 blocking closure 无法强制取消；Tracker 用于证明其是否结束，最终 runtime timeout 和致命进程退出负责给等待设置硬上限。
- 首版 Windows 处理 Ctrl-C；SIGTERM 只在 Unix 条件编译。Windows 服务控制事件以后按独立部署需求增加。

## 后果

- 正常短请求和流可以在宽限期内完成；超过宽限期的静默流会以连接/Body 错误结束，并归还全部运行时容量。
- 客户端在 Draining 竞态中已经进入的请求仍被追踪；Forced 后不保证获得完整协议错误 envelope，但不会拼接第二条上游流。
- 未提交的配置或密码事务在 Forced Drop 时回滚；已提交事务不会由停机回滚，下次启动直接读取 SQLite 当前值。
- 只有全部受管资源完成收尾才记录 `shutdown complete`。收尾超时返回致命结果，不走正常实例锁释放路径。

## 验证

- Lifecycle 单元测试覆盖状态单调转换、Draining 拒绝新 Guard、请求 Guard 持有到 Drop、异步任务 forced 收敛，以及 blocking JoinHandle 被 Drop 后仍保持追踪。
- Server 单元测试覆盖 Guard 随普通/静默响应 Body 存活，并验证 Forced 会 Drop 静默 Body、结束活动计数。
- RequestTelemetry 测试覆盖正常排空与超时 abort + join 后 Tracker 归零。
- App 测试使用可注入信号验证自然 drain、信号时读取最新 PublishedSnapshot 设置、后台任务 forced 收敛，以及 blocking 任务错过期限时明确返回收尾失败。
- 实例锁独占与 Drop 后重取由独立 App 单元测试覆盖；Windows release 二进制已使用真实 Ctrl-C 连续两次在同一端口和数据目录启动/停机，均以退出码 0 记录 `shutdown complete`。Unix SIGTERM 真实子进程回归留在后续跨平台 CI 加固项。
