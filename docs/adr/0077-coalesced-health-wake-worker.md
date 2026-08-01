# ADR-0077: 合并健康 deadline 的单一 scheduler wake worker

- 状态：Accepted
- 日期：2026-07-31
- 决策者：maintainer

## 背景

Credential/模型冷却、OAuth 额度耗尽、Endpoint transient 和 Endpoint/Proxy Circuit Open 都会在未来某个
时刻重新具备候选资格。旧实现每次记录这些状态都调用一次 `spawn_until_draining(sleep_until(...))`；连续
上游失败会创建任意数量、最长存活 30 天的 Tokio task。TaskTracker 虽能在停机时取消它们，却不能约束
运行期间的任务数量，重复失败和 deadline 延长也会留下已经失效的 timer。

QueueTicket 已经在每轮完整选择后直接等待候选返回的最早 `retry_at`，同时监听 scheduler epoch。因此健康
timer 的职责是把共享状态变化广播给其他等待者，而不是建立第二套队列或替代请求自己的 deadline。

## 决策

- 每个 `SchedulerEpoch` 最多拥有一个健康唤醒 worker。worker 在首个有效 slot 被 schedule 时按需启动，
  通过进程级 `ProcessLifecycle` 的后台 TaskTracker 跟踪；没有健康 deadline 时不启动第二个 worker。
- 健康运行态不按错误事件创建 timer，而是为每个实际可用性边界持有稳定 keyed slot：Credential 级冷却、
  每个模型冷却、额度耗尽、Endpoint transient 以及每个 Circuit Open 各一个。同一 slot 再次 schedule 只
  原子替换其 deadline；清除状态或运行态 Drop 会撤销对应 slot。
- worker 保存 `slot -> deadline` 与按 deadline 排序的反向索引。新 schedule 比当前最早值更早时立即重排
  sleep；改到更晚时旧 deadline 被删除，禁止提前推进；不同 slot 的较晚 deadline 必须保留，不能因为只
  记住全局最早值而丢失后续边界。
- deadline 到期时，worker 在短锁内批量移除所有已经到期的 slot，锁外只推进一次 scheduler epoch，然后
  继续等待剩余最早值。重复 schedule 和同一时刻的多个状态只产生一次广播。
- QueueTicket 继续在每一轮直接等待其选择结果中的最早 `retry_at`，并在 deadline 或 epoch 任一先到时重新
  完整选择。这条请求侧 timer 是无丢失唤醒的最终保证；worker 的批量合并不会固定 Credential，也不会改变
  QueueTicket 上限、超时、取消、RPM 或 affinity 语义。
- worker 同时监听 `Draining`。进入 Draining 后立即退出；此后 schedule 是 no-op，不允许重新创建后台
  task。进程重启创建全新的 SchedulerEpoch、slot 和 worker，不持久化任何 deadline。

## 备选方案

- 每次错误 spawn 一个 sleep task：实现短，但任务数与故障次数成正比，重复和被延长的 deadline 无法撤销。
- 只保存一个全局最早 deadline：任务数有界，但最早值到期后无法知道此前被覆盖的较晚边界；即使请求侧
  timer 能避免等待丢失，也不能完整兑现健康到期广播语义。
- 每个 Credential/Endpoint 各自启动 worker：比逐事件 task 更少，但任务数仍随配置规模增长，且重复实现
  重排和停机逻辑。
- 固定周期轮询全部健康状态：任务数有界，但引入无故扫描、恢复延迟和新的轮询间隔常量。

## 后果

- 健康定时后台任务数在 Running 期间严格为 `0..=1`，与错误频率、配置规模和最长 Retry-After 无关。
- slot 数量只随当前存在的健康状态边界增长，不随同一边界的重复失败增长；状态清除和运行态释放会回收
  slot 索引。
- scheduler epoch 仍是合并广播，等待者仍按自身已捕获 revision 和 `retry_at` 执行完整选择。

## 验证

- Tokio paused-time 测试覆盖并发重复 schedule、同 slot 更早重排、同 slot 更晚替换、不同 slot 的较晚
  deadline 保留、同一时刻批量唤醒和 slot Drop 撤销。
- 生命周期测试在所有上述场景断言 `background_task_count <= 1`，并验证 Draining 使 worker 退出、后续
  schedule 不再启动任务。
- Runtime 定向测试、严格 Clippy、fmt 与架构检查作为提交门禁。
