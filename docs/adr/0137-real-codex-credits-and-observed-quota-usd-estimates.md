# ADR-0137：Codex 真实 Credits 与滚动额度美元等值观测

- 状态：Accepted
- 日期：2026-08-11
- 决策者：maintainer
- 修订：ADR-0034、ADR-0070、ADR-0111

## 背景

Codex `/backend-api/wham/usage` 除 5 小时/7 天使用率外，还会返回购买 Credits、工作区
spend control 和额度到达类型。现有 Driver 只读取 `rate_limit`，因此管理面看不到账号已有的
Credits；当 included rolling window 返回 `allowed=false` 或 `limit_reached=true` 时，Runtime
也会直接把账号从候选池排除，即使上游明确报告该账号仍有可用 Credits。

百分比本身没有货币单位，但相邻两次权威额度快照之间，如果 any2api 完整观察到了实际模型、输入
Token、缓存命中 Token 与输出 Token，就可以按同一模型的官方标准 API 价计算这段样本的美元成本，
再用使用率增量反推该窗口的美元等值容量。例如样本成本为 `$0.02` 且使用率增加 `1%`，则窗口总量
约为 `$2.00`。这仍不是上游余额：百分比可能被取整，账号也可能在其他客户端使用，Fast/Priority、
长上下文和未来价格变化也可能改变实际消耗。

OpenAI Codex 当前协议把购买余额定义为 `CreditsSnapshot { hasCredits, unlimited, balance }`，并把
`workspace_*_credits_depleted`、`workspace_*_usage_limit_reached` 与普通 rolling rate limit 分开。
官方客户端也只把原始 `balance` 显示为 Credits，而不是把它冒充美元。

## 决策

1. Codex Driver 从同一次只读 `/wham/usage` 响应解析 `credits.has_credits`、`credits.unlimited`、
   可选非负十进制 `credits.balance`、`spend_control.reached` 和声明的
   `rate_limit_reached_type`。真实 Credits 与 rate-limit reset credits 是两类不同资源，分别展示，
   禁止合并或互相换算。
2. 管理 API 原样返回经过校验的 Credits 状态。Web 标签只显示 `Credits`：无限、实际余额、余额隐藏但
   可用，或不可用。有限 `balance` 在卡片中只显示不大于原始值的整数部分且不重复 `Credits` 单位；
   API 仍保留经过校验的原始十进制字符串。没有上游权威货币字段时，不用本地固定比例把它标成真实美元。
3. OAuth 额度健康按官方语义组合证据：
   - `spend_control.reached=true`，以及 workspace owner/member credits depleted 或 usage limit reached，
     是工作区硬停止，始终可以建立临时额度耗尽健康；
   - 普通 `allowed=false`、`limit_reached=true` 或 `rate_limit_reached` 只表示 included rolling window
     不可用；当同一权威快照报告 `credits.unlimited=true` 或 `credits.has_credits=true` 时，不得据此
     排除账号，并应清除此前仅由 rolling window 建立的耗尽状态；
   - 百分比和任何本地美元估算仍不参与路由。冲突快照中工作区硬停止优先于 Credits 可用标记。
4. Provider Driver 可选提供模型的版本化标准美元费率。首批 Codex 费率卡固定为
   `openai_api_standard_2026_08_11`，只覆盖当前官方公开且有明确标准输入、缓存输入和输出价的
   `gpt-5.4`、`gpt-5.4-mini`、`gpt-5.5`、`gpt-5.6-luna`、`gpt-5.6-terra` 与
   `gpt-5.6-sol`。未知模型不得按名称相似度猜价。
5. OAuth 数据面 Attempt 进入 Transport 后继续由同一个 RAII 活动 Guard 结算一次。Guard 在已有
   ProtocolAdapter Token 遥测旁路上累积最终累计 usage；输入成本按
   `(input_tokens - cached_tokens) × input_rate + cached_tokens × cached_rate`，再加
   `output_tokens × output_rate`。缺少输入/输出 usage、费率未知或计算结果非法时，把本段样本标记为
   不完整，不用部分成本生成估算。失败或取消仍触发原有额度活动刷新，但不伪造 Token 成本。
6. 每个账号只在当前进程内按窗口 identity 保留估算基线和累计成本。首次权威快照建立基线；后续成功
   快照命中相同窗口 ID、类型、时长和 reset identity、使用率严格增加，且从基线开始的本机样本完整、
   累计成本大于零时，才计算：

   ```text
   estimated_capacity_usd = cumulative_observed_cost_usd × 100 / cumulative_used_percent_delta
   estimated_used_usd = estimated_capacity_usd × current_used_percent / 100
   estimated_remaining_usd = estimated_capacity_usd × (100 - current_used_percent) / 100
   ```

   百分比不变时不得前移基线或清空已累计成本，而是等待后续可测增量；每次新的正增量继续从同一基线
   合并全部完整证据，禁止用最新单个百分比间隔覆盖已有估算。窗口 reset、使用率下降、缺失费率或不完整
   usage 才丢弃当前校准段并在当前快照建立新基线。同一进程中已生成的容量估算可在窗口 identity 不变时
   按新百分比更新已用/剩余显示。
7. 美元等值结果必须携带窗口 ID、容量/已用/剩余、样本成本、百分比增量、样本起止时间和费率卡 ID；
   管理 API 和持久化快照保留这些诊断字段。Web 在对应窗口的百分比同一行只显示 `$已用/$总量` 数值对，
   鼠标悬浮辅助文本再说明这是本机观测估算而非上游余额，并展开剩余、样本、增量、时段和费率卡；这些
   诊断不另占卡片行。估算不能用于扣费、配额、Gateway Key 权限、调度、健康、RPM、会话、重试或启动恢复。
8. 最后一次估算与安全 quota usage 一起写入 `oauth_quota_snapshots` 的 v2 有界 payload，允许跨页面、
   浏览器和重启展示其原始样本时间。启动后不得把持久化估算作为下一次推断的数学基线；新的进程必须
   先取得当前权威快照，避免把停机期间或其他客户端的未知使用归入本机成本。
9. 前向 Migration 把 v1 的裸 `OAuthQuotaUsage` payload 规范化为
   `{ "usage": <current usage>, "usd_estimates": [] }`，补齐新增的 Credits/访问状态空字段并把
   schema version 升为 2。生产 Rust 只读取 v2，不保留 v1 双轨解析。

## 备选方案

- 用 `100 - used_percent` 直接标成美元：拒绝。百分比没有价格或总量单位。
- 用单次请求 Token 除以当前累计百分比：拒绝。必须使用相邻快照的增量，否则会把窗口此前消耗错误归给
  当前样本。
- 把真实 Credits 按推断的固定比例转成“真实美元”：拒绝。上游字段的权威单位是 Credits；美元换算若无
  同次权威货币契约，只能是另一种估算。
- 只要有 `has_credits=true` 就忽略所有限制：拒绝。工作区 spend control 和明确的 credits/usage hard stop
  仍是更强的上游证据。
- 从 RequestLog 事后扫描成本：拒绝。日志可关闭、可降级丢弃且异步落盘；与 Attempt Guard 同生命周期的
  有界观测能更准确地定义样本完整性，也不把历史日志变成路由依赖。

## 后果

- 有购买 Credits 的账号在 included rolling window 达到 100% 后仍能继续路由，除非上游同时报告工作区
  Credits 已耗尽或 spend/usage limit hard stop。
- 管理面以简洁的 `Credits` 整数余额和滚动窗口行内美元等值估算展示两类数据，来源语义保持分离。
- 美元估算需要至少两个当前进程内成功快照；首次刷新、未知模型或不完整 usage 时保持未知。百分比暂时
  未变化时继续累计完整成本，后续随累计百分比增量扩大逐渐收敛，但仍不宣称为上游真实余额。
- Snapshot payload 升级为 v2，但不新增余额表、计费流水、用户/租户关系或恢复型运行状态。

## 验证

- Provider 测试覆盖 Credits 无限/有限/零/隐藏余额、spend control、五种 reached type、畸形余额和官方费率卡。
- Runtime 测试覆盖 2,000 Token/1% 的公式、缓存输入计价、零增量不丢成本、连续 1% 样本累计收敛、窗口
  reset、百分比下降、未知模型、usage 缺失、Guard 单次结算，以及持久化估算不作为重启基线。
- 健康测试覆盖 Credits 可用覆盖 rolling exhaustion、workspace hard stop 优先、百分比/估算中性和后续
  明确可用清除。
- Migration/Storage 测试使用代表性 v1 usage 验证 v2 转换、账号级联、大小/版本约束和 v2 往返。
- HTTP/Web 测试覆盖 Credits 原始响应、整数展示、美元估算与百分比同行、精简字段和无年份更新时间。

## 证据

- OpenAI Codex `RateLimitSnapshot`、`CreditsSnapshot` 与 reached type（revision
  `41ece455b7fa7166f4fc38522952afdaa2604e18`）：
  <https://github.com/openai/codex/blob/41ece455b7fa7166f4fc38522952afdaa2604e18/codex-rs/protocol/src/protocol.rs>
- OpenAI Codex `/wham/usage` 映射：
  <https://github.com/openai/codex/blob/41ece455b7fa7166f4fc38522952afdaa2604e18/codex-rs/backend-client/src/client.rs>
- OpenAI Codex 标准定价与 Credits 费率：<https://learn.chatgpt.com/docs/pricing>
