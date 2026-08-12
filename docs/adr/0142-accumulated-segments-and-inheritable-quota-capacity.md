# ADR-0142:累计区段采样与可继承 Codex 容量先验

- 状态:Accepted
- 日期:2026-08-12
- 决策者:maintainer
- 影响范围:Codex quota estimator、OAuth quota snapshot payload、管理 DTO 与 Web 展示
- 修订:ADR-0138 的 0.5% 即采样阈值与 epoch 间不共享样本;落实 ADR-0141 §4 预留的降级容量 prior

## 背景

ADR-0141 已消除本机可证明的边界错配,但生产使用中估算仍然大幅波动。复核确认三个结构性噪声源:

1. **样本分母过小**。`delta >= 0.5%` 即生成 `capacity = cost × 100 / delta` 并立刻重锚,每个样本
   分母只有约 0.5–2 个百分点。官方百分比量化粒度与上游异步入账(流式请求先在官方计量中反映、
   本地完成时才冻结成本)是绝对误差;单个错配请求即可造成样本级 50%–200% 偏差。样本按固定小
   分母铸造,使用量再多也无法提高单样本质量,median 收敛缓慢且带比值偏置。
2. **正常 rollover 全弃样本**。Codex 5 小时窗口每次 reset 后冷启动,绝大多数时间估算依赖 1–3 个
   高噪声样本,`used/remaining` 随之跳动;每个 epoch 重复支付学习成本。
3. **污染即弃整段**。未计价请求、遥测缺口或进程重启把 sample anchor 以来已验证的干净累计全部
   作废,可用信息被丢弃。

窗口容量是订阅计划的属性,不随 5 小时窗口滚动改变;但上游没有可验证的 plan/capacity signature,
因此旧容量跨 rollover 只能按 ADR-0141 §4 以"独立降级、可由新样本失效的 prior"引入,不能冒充
当前 Stable。

## 决策

### 1. 累计区段与两级铸造阈值

每个窗口在 sample anchor 之上维护一个可选的**累计区段进度**(segment progress):最近一次干净
探测的官方百分比、墙钟、区间冻结成本、计价请求数与费率卡集合。

- `delta_used_percent = current - sample_anchor < 0.5`:不查询、不消费,只推进 last observation
  (`no_change`,与 ADR-0141 相同)。
- `delta >= 0.5`:执行 anchor→current 的 sequence 区间查询与既有 fence 校验(**探测**)。区间干净
  (coverage 完整、无未计价、单一 cost unit、成本为正)但 `delta` 未达铸造阈值时,仅把查询结果
  写入区段进度,保留 sample anchor,状态为新增的 `accumulating`。
- `delta` 达到铸造阈值且区间干净时铸造容量样本并重锚,鲁棒分类不变。
- 铸造阈值两级:accepted samples 为空时 `1.5`(冷启动快速给出 learning 级估算),否则 `5.0`。
  分母放大约十倍,端点量化与入账时序误差在连续干净区段内telescoping 抵消,样本相对误差同比
  缩小;median/MAD 在高质量样本上快速收敛。

### 2. 打捞收割(salvage harvest)

区段被迫终止时,不再无条件丢弃干净前缀:

- 终止原因:coverage gap(含跨进程重启)、区间查询失败、未计价请求、reset/epoch 边界。
- 若已存进度的 `delta >= 1.5`,先按该进度铸造一个容量样本(经过与正常样本相同的鲁棒分类),
  再重锚/转移 epoch。进度中的成本在其自身探测时已通过 fence 验证,后续缺口不追溯污染它。
- 无进度或进度不足 `1.5` 时行为与 ADR-0141 相同:直接重锚。
- 诊断状态仍报告终止原因(`unpriced_usage`、`telemetry_incomplete` 等),被打捞的样本照常计入
  sample count。

### 3. 跨 rollover 的可失效容量先验

- 正常 epoch 边界(reset_at 变化、百分比大幅下降、自然滚动)**保留 accepted samples**,清空竞争
  簇与区段进度。样本新增铸造时的 `epoch` 标记。
- credential 身份指纹变化、窗口 ID/类型/时长变化仍清空全部样本;显式 reset credit 成功仍删除
  整个 snapshot 冷启动。
- 置信度新增独立档位 `inherited`:样本集存在但没有当前 epoch 铸造的样本时展示,明确表达"沿用
  上期估算",不得宣称 `stable`。当前 epoch 出现一致新样本后按原规则恢复 `stable`/`learning`。
- 先验失效:纯继承模型(无当前 epoch 样本)被 **2** 个自身一致的同侧竞争候选整体替换;有当前
  epoch 样本确认的模型仍需 ADR-0141 的连续 4 个。单个异常值仍不触发重学习。

### 4. 展示与持久化

- `OAuthQuotaIntervalStatus` 新增 `accumulating`;`OAuthQuotaEstimateConfidence` 新增 `inherited`;
  estimate 新增 `fresh_sample_count`(当前 epoch 铸造的样本数),Web 区分"样本总数/本期样本"。
  `inherited` 与 `degraded` 一样以近似前缀展示。
- Snapshot payload 升 v7:窗口状态增加可选区段进度,样本增加 epoch 标记。Migration 0027 按
  0026 先例重建表(`schema_version = 7`),保留最后成功 `usage`,清空不兼容的 v6 estimator
  state;生产代码只读 v7,不保留双轨。

## 结果

- 样本分母从 ~0.5–2% 提升到 ≥5%(冷启动 ≥1.5%),量化与入账时序噪声的相对影响缩小约一个数量级;
- 正常 rollover 不再冷启动,估算连续可用且明确标注 `inherited`,计划变化仍可被快速推翻;
- 未计价请求、重启与 reset 不再清零干净累计,学习吞吐显著提高;
- 外部消费与上游内部规则仍不可观测,长区段中混入的外部消费表现为温和低偏样本,由 median 与
  竞争簇继续兜底。

## 验证

估算测试覆盖:多次小增量累计至阈值铸造、进度打捞(未计价、跨进程、reset 三类边界)、冷启动
1.5% 引导、rollover 继承与 `inherited` 置信度、继承模型被 2 个一致候选替换、已确认模型仍需 4 个、
v7 状态往返与 Migration 0027 升级测试。
