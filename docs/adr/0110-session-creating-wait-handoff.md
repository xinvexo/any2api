# ADR-0110: Session Creating 在候选等待前交还

- 状态：Accepted
- 日期：2026-08-04
- 决策者：maintainer
- 修订：ADR-0062 的并发创建边界

## 背景

普通显式 Session 首次请求原先在短锁内写入版本化 `Creating`，随后一直持有 Lease 完成普通候选选择、
RPM/健康等待和上游 Attempt。它能够阻止双创建，但把最长可达 `scheduler.queue_timeout` 的纯调度等待
错误地算进“已有创建者正在执行”：同 Session 请求只能旁路等待，最终可能收到
`session binding creation timed out`，而真实阻塞原因只是全部候选 RPM 用尽或暂时不健康。旧快照的等待者
也会在整个排队期阻止新快照请求竞争已经可用的候选。

直接在 Lease 外完成候选预留再争抢 `Creating` 虽然不会双发上游，但会让同 Session 并发请求同时推进
轮询游标、Selection 计数、Health Guard 和 RPM 预留，再由失败者回滚。这些可观察的伪选择和短时容量
占用没有必要。直接在排队前 Drop Lease、醒来后不重新取得 Lease，则会允许多个请求同时预留并执行
首次上游 Attempt，破坏固定会话语义。

## 决策

### 两阶段 Creating

Session 内部状态改为 `Creating { version, phase }`，其中 phase 只有：

- `Selecting`：当前请求拥有执行一次同步候选检查与 RPM 原子预留的权利；该阶段禁止跨任何 `await`；
- `Attempting`：当前请求已经取得唯一候选和 RPM 名额，Lease 继续覆盖上游 Attempt，直到绑定提交或
  RAII Drop。

`begin_session` 仍在同一个短 Mutex 临界区内原子返回 `Create`、`Wait(Selecting)`、
`Wait(Attempting)` 或 `Bound`。取得候选的请求必须在开始任何上游 I/O 前，用相同 version 在短锁内把
自己的 Lease 从 `Selecting` 提升为 `Attempting`；提升后推进统一 scheduler epoch。

### 等待前交还

创建者在每次同步候选检查前必须持有 `Selecting` Lease：

```text
begin_session -> Creating(Selecting)
  -> 同步检查健康并原子预留 RPM
     -> Acquired: CAS 提升为 Attempting -> 上游 Attempt
     -> RateLimited / TemporarilyUnavailable:
          Drop Selecting -> 推进 epoch -> 再进入统一 QueueTicket 等待
```

每次 epoch 或 `retry_at` 唤醒后都重新执行 `begin_session`，只有新一轮原子取得 `Selecting` 的请求才可
再次检查和预留候选。其他请求不会并发预留同一 Session 的首次 Attempt：

- 看到 `Wait(Selecting)` 时，只等待这个不会跨 `await` 的选择临界段结束；
- 看到 `Wait(Attempting)` 时，等待真实创建者提交或 Drop；
- 看到 `Bound` 时，立即转为固定目标选择。

同一请求在这些阶段之间复用一张全局 `QueueTicket` 和同一个 scheduler epoch 订阅，避免释放后重新入队
造成容量竞态，也不建立 Session 私有队列。请求取消会同时 Drop QueueTicket 和它仍持有的 Lease。

### 超时与错误归因

- 自己的同步检查确认 `RateLimited` 或 `TemporarilyUnavailable` 后，`Reject` 策略立即返回对应调度错误；
  `Wait` 策略在 `scheduler.queue_timeout` 内等待，期限边界再做一次完整检查，仍失败时返回实际 RPM 或
  健康错误及可用的 `Retry-After`。
- `Wait(Selecting)` 属于调度选择协调，不使用 `affinity.wait_timeout`，也不得伪装成绑定创建超时；它受
  同一 scheduler deadline 和 QueueTicket 上限约束。正常实现中该状态只存在于无 `await` 的短代码段。
- 只有 `Wait(Attempting)` 才启动并受 `affinity.wait_timeout` 约束；期限内仍有真实 Attempting 创建者时
  返回 `session binding creation timed out`。
- 两类绝对 deadline 各自在请求首次遇到对应原因时建立，epoch 唤醒和阶段往返都不得延长。外层请求
  总预算仍可更早取消整个选择流程。

活跃 `Attempting` Lease 继续不按 TTL 或 waiter timeout 强制回收；网络生命周期仍只由已有 Attempt、
Body Guard、取消和 RetrySafety 规则治理。

## 后果

- 纯 RPM/健康排队期间 Registry 不再保留长寿命 `Creating`，新快照请求可以参与下一次选择。
- 每个 Session 仍只有一个请求能跨入上游 Attempt，不需要并发预留后的补偿回滚，也不会制造虚假的
  Selection 计数或轮询游标推进。
- `Creating` 多一个只在 Runtime 内存存在的 phase；它不改变持久化模型、管理 DTO、绑定强度或固定
  目标语义。
- 等待实现需要在候选等待和绑定等待之间复用 QueueTicket，并分别维护调度与 Attempting 的绝对期限。

## 验证

- AffinityRegistry 测试覆盖 `Selecting -> Attempting` 的 version CAS、两阶段可见性、提交、Drop 与清理
  竞态。
- Runtime 选择测试覆盖 RPM 等待前已删除 `Selecting`、唤醒后只有一个请求重新取得选择权、取得 RPM
  后其他请求只看到 `Attempting`，以及取消不遗留 Lease/QueueTicket。
- 虚拟时间测试分别固定调度超时返回 RPM/健康错误、Attempting 超时返回绑定创建错误，并证明反复 epoch
  唤醒不会延长任一绝对 deadline。
