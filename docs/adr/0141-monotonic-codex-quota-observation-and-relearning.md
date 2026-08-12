# ADR-0141：单调 Codex Quota Observation 与可恢复容量模型

- 状态：Accepted（§3 竞争簇与 §1 的全局 prune 计数已被 ADR-0144 取代；observation fence 与双锚点保留）
- 日期：2026-08-12
- 决策者：maintainer
- 影响范围：RequestTelemetry、RequestLog、OAuth quota snapshot、Codex quota estimator

## 背景

ADR-0138 的 interval estimator 已经避免用“窗口累计本地成本 / 当前百分比”猜容量，但生产复核发现四个
仍会系统性损坏样本的问题：

1. `delta < 0.5` 时仍推进唯一 baseline，连续小增量、零增量平台期和官方计量延迟会永久丢掉本地成本；
2. Codex primary usage 之后还会查询 reset Credits，旧 `fetched_at_ms` 在整条查询完成后记录，使 usage
   response 之后完成的本地请求错误进入当前 interval；
3. 墙钟先于 telemetry barrier 记录，边界后的 queue/storage gap 可能被吸收到 baseline checkpoint，
   下一 interval 因 generation 相等而错误声明完整；
4. 稳定簇会持续拒绝远端候选，持续一致的新容量在当前 epoch 内无法替换早期错误模型。

墙钟无法同时表达并发 RequestLog 完成顺序、Writer 持久化顺序和 primary usage observation。官方计量仍
可能异步或量化，本机不能消除该不可观测误差，但必须先消除自身可证明的边界错配。

## 决策

### 1. 单调 observation fence

RequestTelemetry 只为完成的 OAuth RequestLog 分配 `(process_id, sequence)`。sequence 在有界队列入队前、
与 quota observation fence 共用的短同步临界区内严格递增；队列拒绝以及请求启动时日志 policy 已关闭的
OAuth 完成记录都会推进 sequence 和显式 loss counter。primary
usage Body 完整接收且状态成功后，Runtime 立即在同一临界区捕获当前 sequence 与展示墙钟，并在继续解析
supplement/reset-credit 网络阶段前等待 QuotaCheckpoint barrier。barrier 提交此前已经接受的 RequestLog，
或返回 `enabled=false` 的不完整 fence。

checkpoint 不能在 Worker 最终处理 barrier 时直接读取一份全局快照，否则 barrier 后立即遗漏/丢弃的新请求可能
抢先改变计数并被旧边界吸收。入队临界区因此同时冻结 policy generation 与 queue-drop counter；Worker
只在提交边界前日志后补齐 storage-failure/prune counter。下一 observation 必然看到边界后的 loss。

RequestLog 持久化 process ID 与 sequence。Estimator 只查询同一进程中
`(sample_anchor.sequence, current.sequence]` 的账号冻结 Credits。sequence 决定成员关系；墙钟只用于展示、
诊断与缺少 reset identity 时的保守窗口时长判断。跨进程 observation interval fail-closed。

SQL 查询后的 checkpoint 只检测查询期间可能删除已持久化区间行的 prune。primary fence 已经结算此前日志
的 storage failure；fence 后请求的 queue/storage 结果属于下一 interval，不能反向拒绝当前样本。状态始终
保存 primary usage fence 自身的 checkpoint，使其后的 gap 在下一 interval 被发现。任意 prune 仍保守增加
全局删除计数；0026 不引入每账号 prune ledger，删除完全早于 anchor 的记录可能多作废一个区间，这是安全
的已知可用性限制。

### 2. 双 observation anchor

每个窗口保存：

- `last_observation`：每次成功官方 observation 都推进，只用于相邻 reset、`reset_at` jitter、身份和自然
  rollover 判断；
- `sample_anchor`：容量分子与分母的共同起点，可以跨多个干净的小 delta observation 保留。

`current.used_percent - sample_anchor.used_percent < 0.5` 且 sample anchor 到 current 的 coverage 完整时，
不查询或消费区间，只推进 last observation。达到阈值后查询整个 sequence 区间。有效样本、未计价、无本地
成本、查询失败、coverage gap、候选拒绝或 reset 都把 current 设为新 sample anchor；gap 不能继续累积。
reset 判断始终比较相邻 last observation，继续服从 ADR-0140 的 `reset_at <= 60 秒` 漂移容差。

### 3. 有界竞争簇

Accepted samples 继续最多保留 9 项并用 median/MAD 投影。稳定模型拒绝的高、低候选分别进入同侧竞争簇；
方向反转、正常候选、gap、reset 或非法区间清空竞争簇。竞争簇最多保留最近 4 项，只有连续 4 项自身相对
MAD 不超过 20% 时才整体替换旧 accepted samples。单个异常值或离散拒绝值不会触发重学习；持续一致的新簇
可以在同一 epoch 内恢复为 Stable。

低候选仍可诊断为 `external_usage_suspected`，但该名称只是方向性解释，不宣称存在 any2api 之外的客户端；
官方计量延迟、百分比量化和旧模型错误也可能形成低候选。

### 4. Epoch prior 与计价版本

本次不把 Stable capacity 跨正常 rollover 携带。当前 Codex usage 没有权威 plan/capacity signature，窗口
类型与时长相同不能证明容量不变。未来若引入 prior，必须作为独立降级状态并由新样本失效，不能直接冒充
当前 Stable。

Capacity 的规范单位仍是 Codex Credits。冻结 rate card 版本用于正确重放每条 RequestLog 的 Credits 成本，
普通 rate-card 版本变化本身不等于 quota capacity identity 变化，因此不作为强制 epoch/signature 边界。

### 5. 持久化

Migration 0026：

- 给 RequestLog 增加可选 telemetry process ID 与正数 sequence，并建立 OAuth account/process/sequence 索引；
- 用 v6 quota snapshot 保留最后成功 usage、清空不兼容的 v5 estimator state；
- 后续 v6 payload 持久化双锚点、accepted samples 和 competing samples。

历史 RequestLog 没有 sequence，不进入新 estimator 区间。生产代码只读取当前 v6 schema，不保留 v5 状态
兼容分支。

## 结果

- 连续 `<0.5%`、零增量平台期与 delayed accounting 不再丢失本地 Credits；
- supplement/reset-credit 延迟和相同毫秒不再改变 interval 成员；
- fence 后 telemetry gap 不会被吸收到下一 anchor；
- 错误稳定簇可以被持续一致的新簇替换；
- 官方异步入账、百分比量化以及本机无法观测的 Provider 内部规则仍然只能由鲁棒采样近似。
