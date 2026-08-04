# ADR-0105: 以 PublishedSnapshot reconcile 约束 Gateway 使用状态

- 状态：Accepted
- 日期：2026-08-04
- 决策者：maintainer

## 背景

`RequestTelemetry` 为 Gateway API Key 保存两份进程内状态：立即供管理列表覆盖 SQLite 的最新
`live_last_used_at`，以及把持久化频率限制为每 Key 每分钟一次的 `last_enqueued_at`。旧实现只在请求认证成功时
插入或更新，没有 Key 删除后的淘汰入口。Release 手动基准模拟 100,000 个不同 Key 依次创建、使用、删除后，
两张 map 都保留 100,000 项、capacity 均为 114,688，插入阶段约 108.426 ms；条目数随历史 Key 总量而不是
当前配置增长，问题成立。

现有 Commit 后回调名为 `LoggingSettingsReconciler`，只接收 revision 与 `LoggingSettings`。把 Gateway Key ID
额外塞入该接口会让名称和契约失真，并且以后每增加一个有界派生状态都要继续扩张参数。另一方面，直接在删除
Handler 调用 telemetry 会遗漏批量导入、未来 mutation 与进程内其他发布入口，形成第二套配置生命周期。

配置切换还存在一个很短但真实的顺序窗口：ArcSwap 必须先让新快照可见，随后无失败 reconcile 才更新共享派生
状态。旧快照请求也允许继续执行。因此仅在删除发布时 `retain` 一次不够；一个已经用旧快照认证的请求可能在
reconcile 后迟到，再把已删除 ID 插回 map。

## 决策

1. 将 Commit 后回调重命名为 `PublishedSnapshotReconciler`，唯一方法接收刚通过 ArcSwap 发布的完整
   `PublishedSnapshot`。ConfigPublisher 仍在快照可见后、scheduler epoch 推进前同步调用。该接口只允许短时
   CPU/内存状态更新，不执行 I/O、不等待、不返回 `Result`；发生 panic 继续适用配置发布的进程 fail-fast。
2. `RequestTelemetry` 在同一次 reconcile 中：
   - 按快照 revision/LoggingSettings 推进 Writer 与清理策略；
   - 收集当前 Gateway API Key ID；
   - 从 `live_last_used_at` 与 `last_enqueued_at` 淘汰不存在的 ID，并 `shrink_to_fit` 释放历史峰值容量；
   - 保存当前 revision 与 ID 集合，供迟到请求判定。
3. `record_gateway_key_use` 必须同时接收认证所用 PublishedSnapshot revision。若观测 revision 不晚于 tracker 已
   reconcile 的 revision，且 ID 已不在当前集合中，则直接忽略，不更新实时覆盖、不排入 SQLite；仍存在的 ID
   可以接收旧快照请求的合法迟到观测。若请求使用更高 revision、发生在对应 reconcile 之前，先接收观测，随后
   由该 revision 的 ID 集合保留或删除，避免丢掉新建 Key 的首次使用。
4. 已经进入有界 telemetry 队列的删除 Key 更新不做队列扫描或撤回；Storage 的条件 UPDATE 对已删除行自然为
   no-op。运行态 map 淘汰不改变 SQLite 历史 RequestLog，也不恢复或持久化节流状态。
5. App 组合层用一个 `AppSnapshotReconciler` 顺序委托 RequestTelemetry 与 FileLoggingControl。文件日志继续只
   读取快照中的日志设置；没有引入配置事件总线、后台订阅任务或按 mutation 分类的中央 match。

## 备选方案

- 周期 TTL 清理：已删除 Key 的状态仍会按人为窗口滞留，并需要新增 timer/TaskTracker 生命周期；当前配置已经
  提供精确存活集合，不采用。
- 只在 Gateway 删除 Handler 清理：会漏掉其他发布入口并把 Runtime 生命周期规则泄漏到 Server，不采用。
- 给 `LoggingSettingsReconciler` 追加 Key 参数或第二个可选方法：短期改动较少，但接口名称与职责已经错误，
  后续还会继续堆参数，不采用。
- 每次查询管理列表时顺便清理：读路径产生共享写副作用，且没有查询就不会收敛，不采用。
- 不收缩 HashMap capacity：条目会消失但进程仍保留历史并发峰值容量；配置发布频率低，线性收缩成本可接受，
  因此不采用。

## 后果

- 两张 Gateway 使用 map 的稳定条目集合受当前 PublishedSnapshot 约束；历史创建/删除次数不再决定常驻内存。
- 配置发布后的 reconcile 从只读日志设置扩展为读取完整快照，但仍是单一、同步、无失败的派生状态更新点。
- Server 认证调用多传一个已捕获 revision；Gateway Key 仍不影响上游 Credential 选择或任何配额关系。

## 验证

- Tracker 单测覆盖删除淘汰、容量收缩、旧 revision 防重插、新 revision 在 reconcile 前的首次观测与过期
  reconcile 忽略。
- ConfigPublisher/RequestTelemetry 集成测试创建 Key、记录实时使用并删除，确认发布返回时两张状态均已淘汰，
  旧快照迟到观测不能恢复它。
- Release 手动基准保留 100,000 Key churn 场景：修复后插入阶段约 97.418 ms，reconcile 到空集合约
  3.124 ms，两张 map 的 len/capacity 都从 100,000/114,688 回到 0/0。
- Runtime 248 项中 244 项通过、4 项手动基准忽略，Server 64 项与 App 33 项通过；受影响 crate 的严格
  Clippy、workspace fmt、架构与 diff 门禁通过。
