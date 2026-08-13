# ADR-0144:单消费者前提下的 Codex 绝对容量标定

- 状态:Accepted
- 日期:2026-08-12
- 决策者:maintainer
- 影响范围:Codex quota estimator、RequestTelemetry coverage、OAuth quota snapshot payload、管理 DTO 与 Web 展示
- 取代:ADR-0143 全部;ADR-0142 §2(打捞收割)与 §3 的 `inherited` 置信度;ADR-0141 §3(竞争簇)与 §1 的全局 prune 计数

## 业务前提(核心约束)

**放入 any2api 的 OAuth 账号只会经由 any2api 消费。** 不存在官方 CLI、手机端或其他服务并用同一
凭据的场景。因此:

```text
该 OAuth 的全部实际消费 ≡ 本地 RequestLog 能记录到的全部消费
```

这一前提写入本 ADR 作为 estimator 的设计边界。今后不得再为"external usage"设计防御逻辑;若产品
约束改变,重新设计而不是叠加防御。

## 问题

系统真正要回答的问题是绝对容量标定:

```text
OAuth X 的 100% Codex quota ≈ 多少美元($/credits)
```

ADR-0141/0143 把候选建模为"真值的下界",引入方向性学习(pending_high、low_streak、±25% 接受带、
只允许两个一致高样本向上收养、容量永不自动下调)。这些机制的唯一存在理由是外部消费会无界压低
候选。在单消费者前提下该误差源不存在,方向性模型反而使正确的低样本无法修正偏高的估计。

## 决策

### 1. 对称测量模型

对 fence 完整的观测区间,容量样本为:

```text
capacity_sample = local_cost × 100 / Δused%
```

每个有限正候选直接进入样本集(FIFO,上限 9,带 epoch 标记)。删除 pending_high、low_streak、
接受带、竞争簇与"容量只升不降"约束;估计随测量自然上下修正。样本记录 `Δused%`、`local_cost`、
铸造时刻、epoch 与费率卡集合。

### 2. Δ 加权中位数聚合

官方百分比量化与入账时序偏差是 Δ 上的**绝对**误差,样本相对误差随分母缩小。聚合采用以
`Δused%` 为权重的加权中位数(权重恰好过半时取相邻均值,等权时退化为普通中位数),兼顾鲁棒性与
大分母样本的主导权;`relative_mad`(围绕加权中位数的普通 MAD / 中心)仅作展示与置信度诊断。

### 3. 铸造阈值

- 探测阈值 0.5%:低于它不查询区间,仅推进 last observation;
- 引导阈值 3%:样本集为空时允许较早给出 learning 级估算(±0.5 端点量化下误差上界约 ±17%);
- 标准阈值 5%:此后样本分母至少 5 个百分点(同等假设下约 ±10%),配合加权聚合快速稀释引导样本。

### 4. 无本地成本的官方增量 = 继续累计

单消费者前提下,官方增量对应的成本必然会在同一进程的后续完成记录中落地(流式请求先被官方计量、
完成时才冻结成本)。因此区间查询成本为零时**保持 sample anchor 继续累计**(状态 `accumulating`),
让区间 telescope 成一个完整样本,而不是重锚把成本与其百分比拆开。旧 `external_usage_suspected`
状态删除。

### 5. 遥测完整性:保留 fence,删除打捞

sequence fence(ADR-0141 §1)原样保留:同进程 `(anchor.sequence, current.sequence]` 决定区间
成员,跨进程 fail-closed。coverage gap、区间查询失败、未计价请求仍然一票否决当前区间并重锚——
但不再打捞"干净前缀"(ADR-0142 §2 作废):segment progress 结构删除,宁可丢一个区间也不为
1.5% 分母的样本引入边界复杂度。

### 6. 账号级 coverage 与位置感知 prune

- checkpoint 的队列丢失/写入失败计数改为 **per OAuth account**(入队临界区与 Worker 均按账号
  归因;无法归因的损失进入 unattributed 计数,对所有账号 fail-closed;账号表有界,溢出退化为
  unattributed)。B 账号丢日志不再作废 A 账号的区间。
- prune 上报被删行的 per-process 最大 telemetry sequence,checkpoint 维护
  `pruned_through_sequence` 高水位。仅当 prune 越过区间 anchor 时才判定 coverage 破坏;清理
  早于 anchor 的历史(常态)不再作废活动区间,解除 ADR-0141 遗留的可用性限制。

### 7. Reset / Rollover / 容量签名

- reset 与自然 rollover(ADR-0140 抖动容差不变)丢弃开放区间、重锚 usage,**保留样本集**:容量
  是订阅属性,不随 5 小时窗口改变。`inherited` 置信度档位删除,继承状态由 `fresh_sample_count`
  表达。
- 容量签名 = credential 指纹 + 窗口 key(id/kind/时长) + **subscription tier**(新增,tier 从
  无到有不算变化)。签名变化清空样本重新引导;显式 reset credit 成功仍删除整个 snapshot。

### 8. 置信度为派生值

```text
无样本 → unknown;最近区间 telemetry_incomplete/unpriced_usage/invalid → degraded;
≥3 样本且 relative_mad ≤ 0.20 → stable;其余 → learning
```

状态机消失。`outlier_rejected` 状态删除。

### 9. 持久化与展示

Snapshot payload 升 v8(Migration 0028 按 0027 先例重建表,保留最后成功 usage,清空旧 estimator
state)。管理 DTO 与 Web:状态/置信度枚举收窄,区间诊断的 `pruned_request_logs` 计数改为
`interval_pruned` 布尔。Credits 仍为规范单位；美元等值换算后续由 ADR-0145 改为读取当前可配置费率卡。

## 数学依据

设真实容量为 C。区间 (A→B) 满足 coverage 完整且无 reset 时,官方
`Δused% = (真实区间消费 + ε) / C × 100`,其中 ε 为端点量化与入账时序噪声,有界且近似零均值;
单消费者前提保证 `local_cost = 真实区间消费`。故
`sample = local_cost × 100 / Δused% = C / (1 + ε/真实消费)`——样本围绕 C 双侧散布,误差随
Δ 增大收缩。加权中位数是该分布下 C 的一致稳健估计;不存在使其系统性偏离 C 的未观测消费项。

## 结果

- estimator 状态项从 9 个/窗口降到 6 个,分类分支全部删除;
- 正确的低样本可以下修偏高估计,collapsed 到"测量 + 稳健聚合"的可解释模型;
- 账号级 coverage 与位置感知 prune 显著提高样本吞吐(长驻 anchor 不再被例行清理作废);
- 代价:真正的凭据外泄共用(违反前提)会表现为持续偏低样本并拉低估计,系统不再防御该场景。
