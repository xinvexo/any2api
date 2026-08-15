# ADR-0146：Codex 本机额度累计统计

- 状态：Accepted
- 日期：2026-08-13
- 决策者：maintainer
- 影响范围：Codex quota snapshot、RequestLog 区间、reset、管理 DTO 与 Web
- 相关决策：ADR-0111、ADR-0137、ADR-0145

## 问题

any2api 是每个 OAuthAccount 的唯一消费者。已提交的 RequestLog Credits 与 Provider 返回的
官方使用率变化可以直接作为本机统计事实；不需要把区间拆成候选列表、置信度或异常模型。
显式 reset credit 只切换上游额度窗口，不改变账号容量，也不删除重置前的累计记录。

## 决策

1. 每个有限正 `delta_used_percent` 且区间 SQL 返回正本地 Credits 的区间，立即累加
   `total_delta_used_percent`、`total_local_cost_credits` 与 `completed_interval_count`。
   容量唯一按下式计算：

   ```text
   capacity_credits = total_local_cost_credits * 100 / total_delta_used_percent
   ```

2. 当前模型只有一个观测锚点、两个累计总量和一个完成区间数。不保留区间列表、窗口双层统计、
   最小增量阈值、淘汰、证据状态、置信度、离散度、异常值判断、样本质量或最近区间诊断。
3. RequestTelemetry 只为实际入队的 OAuth RequestLog 分配当前进程内单调 sequence。官方额度
   观测在同一个日志 Writer 排入 flush barrier 并等待完成；SQL 只汇总同一进程
   `(anchor.sequence, observation.sequence]` 已落库且带冻结 Credits 的日志。
4. SQL 返回零或查询失败时保持锚点，下一次观测继续覆盖同一区间。进程变化时只重建锚点，
   既有累计不变。
5. reset 的额度复核观测先按普通刷新持久化，确保 consume 前的本地记录进入累计。consume
   不清空或切换统计；只有 consume 后的下一次官方观测显示 `reset_at` 变化或使用率下降时，
   才重建锚点并开始新窗口。自然窗口切换使用相同规则。
6. 只有凭据身份、窗口 ID/类型/时长或已知 subscription tier 改变才清空累计。
7. Snapshot payload 使用 v9 当前结构。Migration 0029 在 SQL 中把 v8 旧结构一次性汇总为
   累计字段；运行时严格反序列化 v9，不保留旧字段、双读或代码内迁移。
8. 管理 DTO 和 Web 只展示确定容量与累计区间数。reset 后保留当前 Query cache，随后刷新
   失败不清除最后一次成功快照。本机统计只用于展示，不参与健康、路由、RPM、重试、计费或
   运行态恢复。

## 验证

- Runtime/Storage 测试覆盖 sequence fence、Writer barrier、正增量累计、零/失败查询保持
  锚点、进程重启重建锚点、reset 后下一次观测边界和身份变化清空。
- Migration 测试覆盖 v8→v9 的代表性数据转换，并证明运行时拒绝旧 payload。
- HTTP/Web 测试覆盖容量公式、累计区间数、reset 后保留展示和失败刷新保留快照。
