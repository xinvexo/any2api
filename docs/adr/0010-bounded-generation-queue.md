# ADR-0010: 生成请求的有界 QueueTicket 与快照策略

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

## 背景

当前 Route tier 的候选可能都因 RPM 窗口用尽而暂时不可用。等待必须有上限、超时和取消语义，RPM 到期、健康变化或配置发布后等待者要重新执行完整选择，并避免“检查不可用”和“开始等待”之间丢失唤醒。同时，排队参数属于已发布配置，不能让持有已捕获 `PublishedSnapshot` 的请求在等待过程中读取到其他 revision 的策略。

## 决策

- RuntimeRegistry 持有跨配置 revision 复用的统一 `SchedulerEpoch` 与 `QueueCoordinator`；等待计数只在内存中存在，进程重启后清空。
- `QueuePolicy` 默认值固定为等待、30 秒超时、最多 128 个等待请求、主 tier RPM 用尽时默认不进入 fallback tier。
- `PublishedSnapshot` 按值捕获当前 `QueuePolicy`，同一请求的等待、超时、队列上限和默认 fallback 策略全部来自其持有的同一 revision。
- RuntimeRegistry 不保存可变 QueuePolicy；ConfigPublisher 从已校验的 SettingsConfiguration 编译策略，在提交后放入新快照再原子切换。
- RPM 用尽且策略为 `wait` 时取得 RAII `QueueTicket`。Ticket 在创建时订阅统一 epoch，并计入 `max_waiting_requests`；成功、超时、取消或错误均通过 Drop 归还名额。
- 等待循环先标记当前 epoch 已观察，再执行一次完整 select-and-reserve；若仍无可执行候选才等待 epoch 变化，避免 RPM 到期或状态变化发生在检查与休眠之间时丢失唤醒。
- 超时边界执行最后一次完整选择，避免容量与 timeout 同时发生时错误拒绝本可执行请求。
- `scheduler.fallback_on_rate_limit` 决定当前 tier RPM 用尽时是否继续检查下一 tier；禁止时在当前 tier 等待或拒绝。
- Count Tokens 使用同一 Credential RPM、QueueTicket 和 fallback 策略，不建立辅助并发路径。

## 备选方案

- 不使用固定 `tokio::Semaphore`：本地准入只由滚动 RPM 窗口决定，Semaphore 还会形成额外并发限制。
- 不为每个 Credential 建独立 FIFO：普通请求等待的是 Route 候选集合，而不是预先固定一个 Credential；独立队列容易造成容量闲置和队头阻塞。
- 不把 QueuePolicy 放进共享可变 Coordinator：已捕获快照会观察到其他 revision 的参数，甚至一个请求可能混用两组策略。
- 不在 QueueTicket 前先选择固定 Credential：RPM 预留失败后必须重新执行完整选择，不能携带未预留名额的过期选择结果。

## 后果

- RPM 暂时用尽时具有明确的等待、队列上限、超时和取消边界，到期或状态变化能唤醒重新竞争者。
- 热更新不会重置 waiting count，也不会让已开始请求跨 revision 改变队列策略。
- 固定会话等待在同一 QueueCoordinator 中具有高于普通未绑定请求的优先级，不另建绕过上限的等待链。
- scheduler 默认值可由 SQLite SettingRegistry 覆盖并经 ConfigPublisher 热更新。

## 验证

- 单元测试覆盖默认值和非法零值、Ticket 上限与 Drop、Reject、不存在候选、fallback 开关、RPM 到期后重选、取消归还、超时归还和超时边界最后一次选择。
- 并发测试覆盖 epoch 在复查与等待之间推进时不会丢失唤醒。
- 快照测试覆盖 QueueCoordinator/waiting count 跨快照复用、QueuePolicy 按 revision 捕获。
- Runtime、Workspace、Clippy、架构检查和 HTTP 契约测试作为提交门禁。
