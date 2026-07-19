# ADR-0010: 生成请求的有界 QueueTicket 与快照策略

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

## 背景

生成请求原本在当前 Route tier 全部满载时立即返回本地并发错误。架构要求等待必须有上限、超时和取消语义，任何 Credential 释放容量后等待者重新执行完整选择，并避免“检查满载”和“开始等待”之间丢失唤醒。同时，排队参数属于已发布配置，不能让持有旧 `PublishedSnapshot` 的请求在等待过程中读取到新 revision 的策略。

本切片只处理普通生成请求。硬粘性、软粘性优先级、冷却到期定时器、熔断恢复和 SettingRegistry/Web 配置仍未实现；Count Tokens 继续使用独立辅助并发并立即拒绝。

## 决策

- RuntimeRegistry 持有跨配置 revision 复用的统一 `SchedulerEpoch` 与 `QueueCoordinator`；等待计数只在内存中存在，进程重启后清空。
- `QueuePolicy` 默认值固定为等待、30 秒超时、最多 128 个等待请求、主 tier 满载时默认不进入 fallback tier。
- `PublishedSnapshot` 按值捕获当前 `QueuePolicy`，同一请求的等待、超时、队列上限和默认 fallback 策略全部来自其持有的同一 revision。
- RuntimeRegistry 只保存启动时注入的默认 QueuePolicy，不提供独立的可变策略钩子；未来 SettingRegistry 发布器必须从已校验候选配置编译策略，并在提交后显式传入新快照再原子切换。
- 满载且策略为 `wait` 时取得 RAII `QueueTicket`。Ticket 在创建时订阅统一 epoch，并计入 `max_waiting_requests`；成功、超时、取消或错误均通过 Drop 归还名额。
- 等待循环先标记当前 epoch 已观察，再执行一次完整 select-and-acquire；若仍满载才等待 epoch 变化，避免容量释放发生在检查与休眠之间时丢失唤醒。
- 超时边界执行最后一次完整选择，避免容量与 timeout 同时发生时错误拒绝本可执行请求。
- Route 的显式 `fallback_on_saturation` 覆盖全局默认。允许 fallback 时，当前 tier 满载可继续检查下一 tier；禁止时在当前 tier 等待或拒绝。
- 辅助 Count Tokens 满载仍立即返回本地 429，不取得 QueueTicket，也不因为生成请求的 fallback 策略溢出。

## 备选方案

- 不使用固定 `tokio::Semaphore`：它难以动态缩容，也不能表达跨 Credential 的重新完整选择。
- 不为每个 Credential 建独立 FIFO：普通请求等待的是 Route 候选集合，而不是预先固定一个 Credential；独立队列容易造成容量闲置和队头阻塞。
- 不把 QueuePolicy 放进共享可变 Coordinator：旧快照会观察到新 revision 的参数，甚至一个请求可能混用两组策略。
- 不在 QueueTicket 前先选择固定 Credential：CAS 失败后必须重新执行完整选择，不能携带无 Permit 的过期选择结果。

## 后果

- 生成请求满载时具有明确的等待、队列上限、超时和取消边界，Permit 释放能唤醒所有重新竞争者。
- 热更新不会重置 waiting count，也不会让旧请求跨 revision 改变队列策略。
- 当前等待者没有硬粘性/strict 优先级；加入会话粘性时必须扩展 QueueCoordinator 的等待类别，而不能另建绕过上限的等待链。
- 当前默认值可由 Composition Root 注入；SQLite SettingRegistry、Web 默认/覆盖/生效值仍需后续切片接线。

## 验证

- 单元测试覆盖默认值和非法零值、Ticket 上限与 Drop、Reject、不存在候选、fallback 开关、队列满载、Permit 释放后重选、取消归还、超时归还和超时边界最后一次选择。
- 并发测试覆盖 epoch 在复查与等待之间推进时不会丢失唤醒。
- 快照测试覆盖 QueueCoordinator/waiting count 跨快照复用、QueuePolicy 按 revision 捕获。
- Runtime、Workspace、Clippy、架构检查和 HTTP 契约测试作为提交门禁。
