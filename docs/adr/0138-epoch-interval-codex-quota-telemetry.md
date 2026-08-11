# ADR-0138：基于 Epoch 与快照区间的 Codex Quota Telemetry

- 状态：Accepted
- 日期：2026-08-11
- 决策者：maintainer
- 替代：ADR-0137 中滚动窗口累计成本除以当前累计百分比的 estimator、账号级本地 reset 下界和
  `OAuthQuotaUsdEstimate` 持久化模型

## 背景

ADR-0137 的第一版 estimator 在每次权威额度刷新时汇总当前官方窗口起点至今的本地 RequestLog 成本，
再计算：

```text
capacity = local_cost_since_window_start / (current_used_percent / 100)
```

这个公式只有在以下条件同时成立时才正确：any2api 从官方窗口起点持续在线；RequestLog 没有被关闭、
丢弃、写入失败或提前清理；账号没有被其他客户端或实例使用；窗口内没有漏抓的官方 reset；费率语义
在历史日志重算时没有变化。真实部署无法保证这些前提。服务中途启动时，分子只包含后半段成本而分母
包含整个窗口使用率，会稳定低估容量；进程重启、日志缺口和外部消费也会产生同类低估。结果仍以精确
小数展示，因此比保持未知更具误导性。

官方额度快照本身只提供累计 `used_percent`，没有权威容量。能够由本机直接解释的是两个连续快照之间的
本地消费和官方百分比增量，而不是从窗口起点开始的累计历史。新的 estimator 必须把 quota observation、
telemetry coverage、epoch 和容量样本分开建模。

OpenAI 官方 Codex 费率卡直接以 Credits/百万输入、缓存输入和输出 Token 定义消费；Fast mode 对 GPT-5.6
和 GPT-5.5 使用 2.5 倍 Credits、对 GPT-5.4 使用 2 倍 Credits。API 美元价格与 Credits 的当前对应关系
只用于 UI 美元等值，不是 estimator 的数学单位，也不能成为持久化架构前提。官方文档没有为 Codex quota
声明独立 cache-write 费率，因此 estimator 只使用输入、缓存输入和输出，不引入 cache-write 猜测。

## 决策

### 1. 成本单位与请求时计价

1. Estimator 的规范单位是 `codex_credits`，不是 API USD。Provider Driver 根据最终 Provider 已规范化的
   上游请求体确定精确模型与速度档位，并返回版本化 Credits 费率。
2. Codex OAuth 请求在 RequestLog 完成时，使用该次响应的完整 input/output usage、可选 cached input
   usage 和请求准备阶段冻结的费率，计算整数 nano-Credits；cache usage 缺失按零处理。计算结果连同
   `cost_unit`、`rate_card` 和 `service_tier` 一次写入 RequestLog。这样未来费率卡变化不会用新费率重算
   旧 Token。
3. `service_tier` 至少区分 `standard` 与 `fast`。当前 Codex OAuth wire 的 `priority` 档位投影为 Fast；
   未知速度档位、未知模型、缺少 input/output usage 或算术溢出都保持未计价，禁止猜价。
4. 请求是否成功、失败、取消以及 HTTP 状态不参与纳入判断；只要最终 RequestLog 带有完整冻结成本，就
   进入区间成本。没有冻结成本的记录累计为未计价记录，使整个 observation interval 不参与容量学习。

### 2. Telemetry coverage checkpoint

RequestTelemetry 保持数据面非阻塞：有界 channel、短时间合批、单事务批量写入和 shutdown flush。额度
控制面可以发送零记录 barrier；Worker 必须先提交 barrier 之前已经入队的 RequestLog，再返回：

```text
TelemetryCheckpoint {
    process_id,
    enabled,
    coverage_generation,
    queue_dropped_request_logs,
    storage_failed_request_logs,
    pruned_request_logs,
}
```

以下事件推进 `coverage_generation`：RequestLog 队列拒绝/发送失败、RequestLog SQLite 批次失败、
RequestLog 清理实际删除记录、日志 enabled 状态变化。进程启动生成新的非敏感 `process_id`。两个 checkpoint
只有在进程相同、前后均启用且 generation 相同时才证明区间没有已知本地遥测缺口。额度刷新先取得快照 B
对应的 checkpoint，执行区间 SQL 后再取得结束 checkpoint；前者保证此前已经入队的日志先落库，后者检测
查询期间的清理、写失败或 coverage 变化。B checkpoint 不能覆盖结束 checkpoint 时丢弃 SQL 结果。
barrier 使用有界入队和超时；Writer 不可用、队列无法接受控制消息或超时都返回不完整 checkpoint，不得
无限阻塞额度刷新。全局缺口保守地使所有账号跨过该点的区间失效；宁可少学习一个样本，也不能用已知
残缺分子生成容量。

### 3. Observation interval

每个官方窗口的首次成功快照只建立 baseline，不生成容量。之后相邻快照 A/B 在能够证明属于同一 epoch
时形成区间：

```text
delta_used_percent = B.used_percent - A.used_percent
completed_at_ms = RequestLog.started_at_ms + RequestLog.latency_ms
local_cost_credits = sum(RequestLog.frozen_cost where completed_at_ms in [A, B))
capacity_sample_credits = local_cost_credits * 100 / delta_used_percent
```

区间使用 RequestLog 的完成时刻而不是开始时刻。快照 B 返回时仍在执行、尚未形成最终 usage/冻结成本的
请求因此不会被错误归到 A/B，也不会因开始时间早于 B 而永久掉出后续样本；它在完成后自然属于下一段。

只有同时满足以下条件才产生样本：

- `delta_used_percent >= 0.5` 个百分点；
- A/B coverage checkpoint 完整且属于同一进程；
- 区间没有未计价 RequestLog，只有一个规范 cost unit，所有金额可安全求和；
- 本地成本大于零，结果为有限正数；
- 区间未跨 epoch/reset/credential identity 边界；
- 样本未被已有稳定分布判定为外部消费或异常值。

`delta` 为零或小于最小采样增量时更新 baseline 但不产生样本；这不是 telemetry degradation。小幅负向
变化在 reset jitter 容差内也只更新 baseline。区间失效后仍把 B 作为下一条 baseline，避免一个缺口永久
污染未来区间。

### 4. Quota epoch

每个 `(OAuthAccount, window_id, window_kind, window_duration)` 保存一个当前 epoch。以下情况结束旧 epoch、
清空样本并以当前快照建立新 baseline：

1. 官方 `reset_at` 在两个连续快照之间变化超过 60 秒，或从有值变为无值/从无值变为有值；两个有值
   `reset_at` 相差不超过 60 秒视为同一窗口的秒级漂移，只更新 baseline，不结束 epoch；
2. 没有可靠 `reset_at` 且相邻快照间隔达到或超过完整窗口时长；
3. 当前 `used_percent` 比上一快照低超过 `0.5` 个百分点；`50.0 → 49.8` 属于容差内 jitter，
   `70 → 3`、`70 → 0` 必须进入新 epoch；
4. OAuth 稳定身份指纹或 `account_generation` 变化；稳定账号 ID/邮箱缺失时，Token 版本变化也视为无法
   证明同一身份并开启新 epoch；
5. 显式 reset credit 成功，直接删除旧 quota telemetry snapshot；下一次刷新冷启动新 epoch；
6. 窗口 ID、类型或时长发生无法证明等价的变化。

Epoch 之间不共享样本、prior estimate 或 baseline。进程重启本身不会删除已有样本：若稳定身份和官方
`reset_at` 能证明仍是同一 generation，保留当前 epoch，但跨进程 observation interval 标为遥测不完整；
下一对同进程快照可以继续学习。没有可靠 reset identity 时，重启后不能证明同一 generation，保守开启
新 epoch。

`reset_at` 的秒级漂移容差及其生产故障依据由
`docs/adr/0140-codex-quota-reset-at-jitter-tolerance.md` 补充定义。

### 5. 鲁棒容量估计与外部消费

每个 epoch 最多保留最近 9 个有效容量样本。当前容量使用 median，不使用算术平均。一个异常样本不能
把中位数拉崩。达到 3 个样本后计算 median absolute deviation（MAD）与中位数的比值：

- 0 个样本：`unknown`；
- 1–2 个样本：`learning`；
- 至少 3 个样本且相对 MAD 不超过 20%：`stable`；
- 已有样本但最近区间出现 coverage gap、外部消费或非法数据：`degraded`。

当已有至少 3 个一致样本时，新候选低于当前 median 的 50%，更合理地解释为官方百分比包含本机无法
解释的外部消费，标记 `external_usage_suspected` 且不更新样本。其他落在稳定中心两倍范围之外，或偏离
超过 `max(3 × MAD, 25% × median)` 的候选标记 `outlier_rejected`。学习期无法可靠区分外部消费；样本
可以进入集合，但 UI 必须明确显示 `learning`，不能宣称高置信度。

容量投影只使用官方当前百分比：

```text
estimated_used_credits = median_capacity * current_used_percent / 100
estimated_remaining_credits = median_capacity - estimated_used_credits
```

Estimator 返回 Credits、样本数、相对离散度、epoch、最近区间状态与费率卡集合。管理 Web 再使用独立、
版本化的 Credits→USD 展示换算渲染紧凑 `$已用/$总量`；该换算不回流到 estimator。

### 6. 持久化与 Migration

`oauth_quota_snapshots` v5 payload 只保存最后一次权威 `usage` 和当前 `estimator_state`：稳定 credential
fingerprint、各窗口当前 epoch、上一 baseline/checkpoint、最近 9 个容量样本和最近区间诊断。公开 estimate
是读取时派生值，不另存一份。状态受原有 payload 大小、窗口数、文本长度和数值有限性校验。

Migration 0025：

- 给 `request_logs` 增加冻结的 quota cost unit、nano-unit amount、rate card 与 service tier；历史行保持
  NULL，不在新模型中猜价，并增加 OAuth 账号与完成时刻表达式索引支持区间聚合；
- 把 quota snapshot 升为 v5，保留权威 `usage`，把 estimator state 初始化为空。v4 的累计窗口 USD
  estimate 数学语义与新模型不兼容，必须丢弃；
- 删除只服务旧累计窗口公式的 `oauth_quota_estimation_boundaries` 表。

生产 Rust 只接受当前 v5 payload 和当前 RequestLog schema，不保留 v4 estimator 兼容解析。

## 状态流

```text
official response
  -> telemetry pre-query barrier/checkpoint
  -> validated QuotaObservation
  -> credential/window identity comparison
  -> QuotaEpoch transition
  -> interval RequestLog completed-at frozen Credits query
  -> telemetry post-query barrier/checkpoint
  -> health classification
  -> capacity candidate / rejected diagnostic
  -> bounded robust sample set
  -> confidence + current Credits projection
  -> SQLite v5 snapshot
  -> admin DTO
  -> Web-only USD equivalent
```

最近区间状态固定为 `awaiting_baseline`、`no_change`、`valid_sample`、`reset_boundary`、
`telemetry_incomplete`、`unpriced_usage`、`external_usage_suspected`、`outlier_rejected` 或 `invalid`。

## 后果

- 服务从窗口中途启动时，两次可靠快照后即可学习，不再要求窗口起点以来的完整历史。
- 服务重启保留已学样本；跨重启区间不学习，且无可靠官方 generation identity 时开启新 epoch。
- 已知 dropped/write-failed/pruned/disabled/unpriced 区间不会产生看似精确的低容量。
- 官方 reset 即使没有抓到 0，只要出现超过容差的百分比下降也会隔离旧样本。
- 外部消费在稳定分布形成后可以被拒绝；冷启动前两个样本仍无法证明外部消费，这是明确的不可观测边界。
- RequestLog 每行增加少量固定字段；请求路径只做内存中的定点成本运算和原有 try-send，不等待 SQLite。
- Quota refresh 使用查询前后两个 Worker barrier；它们位于控制面，不增加公开请求延迟，失败或超时只让
  当前区间停止学习。

## 验证

测试必须覆盖稳定 `0→10→20→30` 收敛、60% cold start、窗口中途启动、进程重启、正常 rollover、
`70→0`、`70→3`、`50→49.8` jitter、reset 后样本隔离、queue drop、SQLite write failure、日志清理、
未知模型/缺 usage、外部消费、正常样本中的单个异常值、多个样本收敛、shutdown flush 和 Migration 后状态
恢复。Telemetry benchmark 单独测量有界队列到 SQLite batch commit 的吞吐和持久化完整性，不给请求路径
增加同步 I/O。

## 仍不可观测

- 官方快照百分比的量化粒度、同步延迟和内部动态权重没有公开契约；小 `Δ%` 样本只能跳过。
- 首批学习样本形成稳定分布前，无法可靠区分外部消费与真实较小容量。
- Provider 可能在流式请求完成前就把部分消费反映到官方百分比，而本地只有最终 usage 才能冻结成本；
  完成时刻归档和 barrier 能避免永久漏算，但无法从公开接口消除这种上游记账时序偏差。
- 请求完成时刻以入口墙钟加单调 latency 近似；极端系统时钟跳变以及完成记录与 barrier 并发入队的
  线性化先后仍不可从现有 RequestLog 契约完全观测。
- `priority` 到 ChatGPT Fast mode 的 wire 映射依赖当前已审计 Codex OAuth 请求形状；未知 tier 保持
  未计价。
- 官方文档没有声明 Codex quota 的 cache-write 独立倍率，因此不估算 cache write。

## 证据

- OpenAI Codex Credits token rate card：<https://learn.chatgpt.com/docs/pricing>
- OpenAI Codex Fast mode 支持模型与倍率：<https://learn.chatgpt.com/docs/agent-configuration/speed>
