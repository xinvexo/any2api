# ADR-0137：Codex 真实 Credits 与滚动额度美元等值观测

- 状态：Accepted
- 日期：2026-08-11
- 决策者：maintainer
- 修订：ADR-0034、ADR-0070、ADR-0111
- 后续修订：ADR-0138 已替代本文的累计窗口 estimator、reset boundary 与 USD 持久化模型；ADR-0145
  已把固定 Credits/USD 比例与 Provider 内模型费率改为 PublishedSnapshot 中的可配置版本化费率卡。
  Credits 上游字段、路由健康语义和“只展示美元等值”的边界仍有效。

## 背景

Codex `/backend-api/wham/usage` 除 5 小时/7 天使用率外，还会返回购买 Credits、工作区
spend control 和额度到达类型。现有 Driver 只读取 `rate_limit`，因此管理面看不到账号已有的
Credits；当 included rolling window 返回 `allowed=false` 或 `limit_reached=true` 时，Runtime
也会直接把账号从候选池排除，即使上游明确报告该账号仍有可用 Credits。

百分比本身没有货币单位。any2api 的部署约束进一步明确：进入本项目的 OAuthAccount 只由当前单节点
any2api 使用，不与其他客户端或实例共享。因此可以从同一账号当前上游额度窗口内的 RequestLog 汇总
实际模型、输入 Token、缓存命中 Token 与输出 Token，按官方标准 API 价计算窗口内本机已用美元等值，
再用当前累计使用率反推窗口美元等值总量。例如窗口内已记录 `$0.02` 且当前使用率为 `1%`，则窗口总量
约为 `$2.00`。这仍不是上游余额：百分比可能被取整，Fast/Priority、长上下文和未来价格变化也可能
改变实际消耗，RequestLog 被关闭、降级丢弃或提前清理时也可能没有完整样本。官方 Credits 费率卡与
同日标准 API 美元费率对当前支持模型保持统一的 `25 Credits = $1` 比例，因此真实 Credits 余额还可以
显示为标准费率卡美元等值，但该值不是可提现的法币余额。

OpenAI Codex 当前协议把购买余额定义为 `CreditsSnapshot { hasCredits, unlimited, balance }`，并把
`workspace_*_credits_depleted`、`workspace_*_usage_limit_reached` 与普通 rolling rate limit 分开。
官方客户端也只把原始 `balance` 显示为 Credits，而不是把它冒充美元。

## 决策

1. Codex Driver 从同一次只读 `/wham/usage` 响应解析 `credits.has_credits`、`credits.unlimited`、
   可选非负十进制 `credits.balance`、`spend_control.reached` 和声明的
   `rate_limit_reached_type`。真实 Credits 与 rate-limit reset credits 是两类不同资源，分别展示，
   禁止合并或互相换算。
2. 管理 API 原样返回经过校验的 Credits 状态与原始十进制余额。Web 标签只显示 `Credits`：无限、有限
   余额、余额隐藏但可用，或不可用。有限余额按版本化官方费率卡固定的 `25 Credits = $1` 显示标准费率卡
   美元等值，最多四位小数；悬浮文本保留原始 Credits 和换算率。该换算不标成现金余额，也不改变 API
   原始 Credits 契约。
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
5. OAuth 数据面 Attempt 进入 Transport 后继续由同一个 RAII 活动 Guard 结算一次，用于触发已有的
   活动驱动额度刷新；美元估算不再在 Guard 内维护第二份 Token 成本状态。额度刷新按窗口时间边界读取
   该 OAuthAccount 已提交的最终 RequestLog，按公开模型汇总已有的输入、缓存输入和输出 Token；输入成本按
   `(input_tokens - cached_tokens) × input_rate + cached_tokens × cached_rate`，再加
   `output_tokens × output_rate`。记录是否纳入只取决于是否具有公开模型与完整 input/output usage，不按
   成功、失败、取消、HTTP 状态或错误分类过滤；cache usage 缺失按零处理。缺少计价字段的记录从成本中
   排除并累计为 `unpriced_request_count`，不得阻断同窗其他完整记录。完整记录使用未知模型费率、没有任何
   可计价记录或计算结果非法时不生成估算。失败或取消仍触发原有额度活动刷新，但不伪造 Token 成本；
   异步日志尚未提交时，当前刷新只使用已提交记录，后续刷新自然补齐。
6. 每次成功权威额度刷新都独立按窗口 identity 计算。窗口起点优先使用
   `reset_at - limit_window_seconds`，没有 reset identity 时才以 `fetched_at - limit_window_seconds`
   形成近似边界，再与该账号最近一次成功消费 reset credit 的本地时间取较晚者；终点为当前
   `fetched_at`。成功 reset 必须在同一 SQLite 事务中持久化这个估算下界并删除旧 quota snapshot，避免
   reset 后或进程重启后把重置前日志重新计入。在使用率大于 `0%` 且样本成本大于零时计算：

   ```text
   estimated_used_usd = request_log_window_cost_usd
   estimated_capacity_usd = request_log_window_cost_usd × 100 / current_used_percent
   estimated_remaining_usd = estimated_capacity_usd - estimated_used_usd
   ```

   百分比不变时仍从同一完整窗口重新汇总，不依赖相邻快照的 `Δ%`，因此不会因单个取整边界反复替换样本。
   当购买 Credits 允许 included window 达到 `100%` 后继续调用时，后续日志成本已经无法区分 included 与
   Credits 消耗；此时只保留同一窗口 identity 且样本起点不早于本地 reset 下界的上一份估算，没有上一份
   则保持未知。没有可用 Credits 的 `100%` 窗口仍可按截至耗尽时的完整日志计算。`0%`、窗口边界无效或
   没有任何可计价证据时保持未知。
7. 美元等值结果必须携带窗口 ID、容量/已用/剩余、样本成本、当前累计百分比、样本起止时间、未计入记录
   数和费率卡 ID；
   管理 API 和持久化快照保留这些诊断字段。Web 在对应窗口的百分比同一行只显示 `$已用/$总量` 数值对，
   鼠标悬浮辅助文本再说明这是本机观测估算而非上游余额，并展开剩余、样本、时段、未计入记录数和费率卡；这些
   诊断不另占卡片行。估算不能用于扣费、配额、Gateway Key 权限、调度、健康、RPM、会话或重试。
8. 最后一次估算与安全 quota usage 一起写入 `oauth_quota_snapshots` 的 v4 有界 payload，允许跨页面、
   浏览器和重启展示其原始样本时间。新的进程取得当前权威快照后，可以直接从保留的 RequestLog 重算；
   持久化估算只在同一窗口已经达到 `100%` 且购买 Credits 仍可用时作为冻结结果复用，不恢复任何路由状态。
9. 前向 Migration 先把 v1 的裸 `OAuthQuotaUsage` payload 规范化为 v2；改用完整窗口 RequestLog 后，
   再把 snapshot 升为 v3，保留权威 `usage`、清空数学语义已经失效的 v2 相邻快照估算，并把诊断字段从
   `sample_used_percent_delta` 更正为 `sample_used_percent`。允许部分可计价窗口后再升为 v4，继续保留权威
   `usage`，并为 v3 中必然来自完整窗口的既有估算补入 `unpriced_request_count = 0`；生产 Rust 只读取
   v4，不保留旧版本双轨解析。

## 备选方案

- 用 `100 - used_percent` 直接标成美元：拒绝。百分比没有价格或总量单位。
- 用单次请求 Token 除以当前累计百分比：拒绝。必须汇总当前窗口内该账号的全部可用 RequestLog，不能把
  窗口此前消耗遗漏掉。
- 把真实 Credits 按推断的固定比例转成“真实美元”：拒绝。上游字段的权威单位是 Credits；美元换算若无
  同次权威货币契约，只能是另一种估算。
- 只要有 `has_credits=true` 就忽略所有限制：拒绝。工作区 spend control 和明确的 credits/usage hard stop
  仍是更强的上游证据。
- 在内存中用相邻快照 `Δ%` 校准：拒绝。百分比取整会让相邻样本过小且波动，进程重启也会丢失基线；在
  账号不外用的明确前提下，窗口内 RequestLog 是更直接且可恢复的本机已用证据。缺少计价字段的日志会
  使结果偏低，因此必须显示未计入数量，但不会成为路由依赖。

## 后果

- 有购买 Credits 的账号在 included rolling window 达到 100% 后仍能继续路由，除非上游同时报告工作区
  Credits 已耗尽或 spend/usage limit hard stop。
- 管理面以简洁的 `Credits` 标准费率卡美元等值和滚动窗口行内美元等值估算展示两类数据，来源语义保持分离。
- 美元估算只需要一个成功权威快照和该窗口内至少一条可完整计价、已提交的 RequestLog；进程重启后仍可
  恢复。日志被关闭/清理/丢弃或缺少 usage 时可能低估，悬浮诊断明确未计入数量；没有可计价日志或存在
  未知模型费率时保持未知，仍不宣称为上游真实余额。
- Snapshot payload 升级为 v4，并新增每账号一行的本地 reset 估算边界；不新增余额表、计费流水、用户/
  租户关系或恢复型运行状态。

## 验证

- Provider 测试覆盖 Credits 无限/有限/零/隐藏余额、spend control、五种 reached type、畸形余额和官方费率卡。
- Migration/Storage/Runtime 测试覆盖账号与窗口边界过滤、按模型汇总、错误/取消但有 usage 的记录照常计价、
  缺计价字段记录的排除与计数、reset 下界持久化与账号级联、2,000 Token/1% 的公式、缓存输入计价、
  百分比不变时使用完整窗口日志、窗口 reset、未知模型、Guard 单次结算，以及 Credits 下 `100%` 窗口冻结
  既有估算。
- 健康测试覆盖 Credits 可用覆盖 rolling exhaustion、workspace hard stop 优先、百分比/估算中性和后续
  明确可用清除。
- Migration/Storage 测试使用代表性 v1 usage 验证 v2 转换、账号级联、大小/版本约束和 v2 往返。
- HTTP/Web 测试覆盖 Credits 原始响应、`25 Credits = $1` 的最多四位小数展示、窗口美元估算与百分比同行、
  未计入记录悬浮诊断、精简字段和无年份更新时间。

## 证据

- OpenAI Codex `RateLimitSnapshot`、`CreditsSnapshot` 与 reached type（revision
  `41ece455b7fa7166f4fc38522952afdaa2604e18`）：
  <https://github.com/openai/codex/blob/41ece455b7fa7166f4fc38522952afdaa2604e18/codex-rs/protocol/src/protocol.rs>
- OpenAI Codex `/wham/usage` 映射：
  <https://github.com/openai/codex/blob/41ece455b7fa7166f4fc38522952afdaa2604e18/codex-rs/backend-client/src/client.rs>
- OpenAI Codex 标准定价与 Credits 费率：<https://learn.chatgpt.com/docs/pricing>
