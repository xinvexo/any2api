# ADR-0146:Codex 本机额度累计统计

- 状态:Accepted
- 日期:2026-08-13
- 决策者:maintainer
- 影响范围:Codex quota estimator、RequestLog 区间、quota snapshot、reset、管理 DTO 与 Web 展示
- 修订:ADR-0144 的 estimator、区间边界、reset、持久化与展示结构;ADR-0145 的容量展示

## 问题

any2api 是该 OAuth 账号的唯一消费者。SQLite 中已经落库的 RequestLog Credits 与 Provider 返回的官方
使用率变化就是本机统计事实，不需要再推测记录是否可信。显式 reset credit 只切换上游额度窗口，不改变
账号容量，也不应删除重置前的累计记录。

## 决策

1. 每个有限正 `delta_used_percent` 且区间 SQL 返回正本地 Credits 的区间，立即累加
   `total_delta_used_percent`、`total_local_cost_credits` 与 `completed_interval_count`。容量唯一按下式计算:

   ```text
   capacity_credits = total_local_cost_credits * 100 / total_delta_used_percent
   ```

2. 当前模型只有一个观测锚点、两个累计总量和一个完成区间数。不保留区间列表、当前/历史窗口双层统计、
   最小增量阈值、淘汰、证据状态、置信度、离散度、异常值判断、样本质量或最近区间诊断。
3. RequestTelemetry 只为实际入队的 OAuth RequestLog 分配当前进程内单调 sequence。官方额度观测在同一个
   Writer 排入 flush barrier 并等待完成；SQL 只汇总同一进程 `(anchor.sequence, observation.sequence]`
   已落库日志的冻结 Credits。
4. SQL 返回零或查询失败时保持锚点，下一次观测继续统计同一区间。进程变化时只重建锚点，既有累计不变。
5. reset 的额度复核观测先按普通刷新持久化，确保 consume 前的全部本地记录进入累计；consume 本身不清空
   或切换 estimator。只有 consume 后的下一次官方观测显示 `reset_at` 变化或使用率下降时，才重建锚点并
   开始新窗口；重置前的全部累计原样保留。自然窗口切换使用同一规则。
6. 只有凭据身份、窗口 ID/类型/时长或已知 subscription tier 改变才清空累计，因为此时统计对象已经改变。
7. Snapshot payload 使用 v9 当前结构。Migration 0029 在 SQL 中把 v8 旧结构一次性汇总为累计字段；Runtime
   严格反序列化 v9，不保留旧字段、双读或代码内迁移。
8. 管理 DTO 和 Web 只展示确定容量与累计区间数。reset 后保留当前 Query cache，随后刷新失败不清除最后
   一次成功快照。

## 结果

- 页面刷新、普通额度刷新、reset 命令和官方窗口切换都不会删除既有累计；
- 只有 reset 后的下一次官方观测开始新窗口，reset 调用本身不提前制造边界；
- 本地统计直接参与唯一累计公式，不存在记录可信度或质量判断；
- 生产 Rust/TypeScript 只面向 v9 当前契约。
