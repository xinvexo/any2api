# ADR-0045：Grok OAuth Billing 额度查询

- 状态：Accepted（Free Token 计量的第 6、9–12 条由 ADR-0060 取代）
- 日期：2026-07-25
- 决策人：项目维护者
- 补充：ADR-0034、ADR-0041
- 修订：对齐 xAI 官方 Grok Build billing、账号诊断、真实 Free 耗尽信号与本地 1M Token 计量

## 背景

Grok OAuthAccount 已能登录、刷新 Token 并参与统一路由，但管理页需要独立的额度刷新。xAI 的 CLI subscription 数据面提供只读 billing 接口；响应可能包含 `creditUsagePercent`、`currentPeriod`、旧式 `monthlyLimit`/`used`、`onDemandCap`、`onDemandUsed`、`prepaidBalance` 和账单周期等字段。

`onDemandCap=0` 与 `onDemandUsed=0` 既不能证明订阅额度剩余 100%，也不能证明账号无限。xAI 官方 Grok Build 客户端优先读取 `creditUsagePercent`，旧响应才以 `used / monthlyLimit` 计算 included allowance 使用率；其 UI 会把两者都缺失解释为 `0%` 已用，但已观察到 Free 账号产生使用量后仍持续缺失该字段，因此这个显示降级不能作为真实余额。预付余额与按量使用金额仍须分开显示，不能用于推算 included allowance。

xAI 官方 rate-limit 文档把推理响应中的请求和 Token 限额定义为 team/model 级 RPS/TPM，而不是订阅 credit 或账户余额。此前在 billing 缺少使用率时发送最小 Responses 请求并把 `x-ratelimit-*` 百分比显示为余额，会使空闲限流窗口长期呈现 100%，语义错误且额外消耗一次真实请求。官方 Management API 的 prepaid balance 端点需要独立 Management Key，不能用 Grok OAuth Token 或普通 API Key 调用，也不能为此把新凭据类型隐式并入 OAuthAccount。Codex 的 reset credit 是 ChatGPT 专属能力，不能套用到 Grok。

官方 Grok Build `/user` 响应使用 camelCase，并可携带 `userBlockedReason`、`teamBlockedReasons` 与实时 `subscriptionTier`。其中团队 blocked reason 还被官方客户端用于表达 ZDR/数据保留策略，不能一律解释成封禁。`grok2api` 对 Build 机器人标记的实现来自 access token JWT 的数值型 `bot_flag_source` claim：只有值为 `1` 才标记。Free 额度真正耗尽时，数据面返回稳定错误码 `subscription:free-usage-exhausted`，部分响应正文还包含 `tokens (actual/limit)`；这能证明耗尽及当次 actual/limit，却不能在未耗尽时反推出实时剩余值。

产品决定在官方 Free 剩余值缺失时采用本地 Token 计量，默认总额为 1M。该值只能表示当前 any2api 进程通过真实响应 usage 观察到的滚动用量，不能伪装成 xAI 官方账本，也不能扩张为 GatewayApiKey 套餐、收费账单或数据面准入限制。

## 决策

1. Grok Driver 固定注册 `GET https://cli-chat-proxy.grok.com/v1/billing?format=credits` 和 `GET https://cli-chat-proxy.grok.com/v1/user?include=subscription`，注入当前 OAuth Bearer Token、`X-XAI-Token-Auth: xai-grok-cli`、OAuth subject 对应的 `x-userid`、Grok CLI 版本、`x-grok-client-mode: interactive` 与 User-Agent。缺失 OAuth subject 时拒绝构造查询，不能发送不完整的 billing 请求。
2. Driver 优先使用有限且非负的 `creditUsagePercent`；缺失时在 `monthlyLimit > 0` 且 `used` 有效时计算 `used / monthlyLimit * 100`。只有得到上述实际上游数值时才投影 credits 窗口；`currentPeriod` 只用于区分周/月周期与重置时间，不能单独证明零使用率。新周期缺失时兼容旧式 `billingPeriodStart`/`billingPeriodEnd`。
3. `prepaidBalance` 表示已购买 credits 的剩余美元分；`onDemandUsed` 和 `onDemandCap` 表示当前按量使用和上限。三者作为独立、可选的安全整数金额返回，Web 按绝对值格式化美元；不得把金额换算成 included allowance 百分比，也不得把预付余额和按量余额合并。
4. `/user?include=subscription` 按官方 camelCase 字段解析；非空 `subscriptionTier` 表示当前套餐层级，是官方用于实时订阅检查的来源，空值表示 Free。JWT `tier` claim 可能滞后，只能作为官方客户端自身的降级提示，管理面不得用它覆盖实时接口结果。同次响应中只有非空的 `userBlockedReason` 与 `teamBlockedReasons` 才作为原始账号/团队策略证据展示；缺失时不显示“未报告”占位，团队原因不得自动改写成“机器人”或“账号失效”。
5. billing 没有可解析使用率但包含有效周期、金额或账单形态时仍返回有效快照，但官方使用率窗口保持缺失。`isUnifiedBillingUser` 和有效周期都不用于猜测官方使用率。
6. 禁止发送 `/v1/responses` 作为余额探测，禁止把推理响应的 `x-ratelimit-*` 请求/Token 窗口投影成 Grok 订阅余额。xAI Management API `GET /v1/billing/teams/{team_id}/prepaid/balance` 只适用于独立 Management Key，不属于 OAuthAccount 额度查询路径。
7. 当前 Grok access token 的 JWT 只做本地、只读、安全派生：`bot_flag_source` 为数值 `1` 时返回已标记状态，为明确数值 `0` 时返回未标记状态，缺失、非数值或其他值保持未知。管理 API 禁止返回其他 claim、JWT payload 或 Token 原文。Web 只在已标记时于账号卡片顶部状态标记之后显示机器人图标，不显示 Build 标记文字；未标记或未知不占展示位置。
8. 查询复用现有 OAuth quota Runtime：固定 DIRECT/全局代理、严格 SSRF、禁用重定向、有界响应体和读取超时；401 最多刷新 Token 一次并完整重试一次。两次均被拒绝时返回明确认证失效错误；403 返回账号访问受限错误。Provider 差异通过查询计划与解析结果表达，中央 Runtime 不增加 Provider `match`。
9. Free 套餐在官方余额缺失时启用稳定 OAuthAccount 句柄上的滚动 24 小时本地 Token 窗口，默认上限 `1_000_000`。只累计成功数据面响应由 ProtocolAdapter 解析出的 `input_tokens + output_tokens`；`cache_read_tokens` 与 `cache_write_tokens` 是输入明细，不重复计费。SSE 累计 usage 先合并后在 Body 生命周期结束时只结算一次。缺失 usage、在终止 usage 事件前客户端提前断连或畸形字段保持未计量，禁止用请求数、字符数、限流 Header 或本地 tokenizer 猜测。
10. 本地窗口跨 OAuth Token 刷新与普通配置 revision 复用，账号删除或进程重启后清空；不写 SQLite、RequestLog、OAuth JSON、PublishedSnapshot 或浏览器持久化。它独立于请求日志开关，只用于管理面展示，绝不参与路由准入、RPM、冷却、账号启停或自动请求探测。管理响应必须返回明确的 `source=local` 与 `window_seconds=86400`，Web 标注为“本地计量”并展示剩余值/上限，不在进度条下重复显示已用量与滚动周期。
11. Grok 数据面错误分类必须识别稳定错误码 `subscription:free-usage-exhausted`，并只在真实响应出现时记录当前运行代际的内存耗尽观测。若正文包含 `tokens (actual/limit)`，只接受非负安全整数并随观测时间展示；该 `source=upstream` 的真实数值覆盖本地 1M 结果。成功数据面请求清除此观测。不得主动发送探测生成，也不得把本地值宣称为上游精确余额。
12. Grok quota 与账号诊断仍是只读临时快照；唯一合入的是上述稳定运行时 Token 窗口，所有结果均不持久化。
13. Grok 不实现 `quota/reset`。Web 使用与 Codex/Claude 相同的账号卡片和额度面板；实时套餐层级映射到账号卡片既有的 `plan` badge。刷新成功本身不再显示“认证状态”，机器人状态按第 7 条只用卡片顶部图标表达，上游账号/团队限制只在真实返回时显示；通用额度详情继续显示 included allowance、本地或上游 Token 余额、真实耗尽观测、预付余额和按量使用，不增加 Grok 专属面板。只有 Codex 显示 reset credit 与重置按钮；Claude 的只读额度入口由 ADR-0046 定义。

## 参考

- xAI Grok Build billing 实现：<https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-shell/src/extensions/billing.rs>
- xAI Grok Build 余额计算与展示：<https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-pager/src/app/effects/helpers.rs>
- xAI Grok Build 实时套餐检查：<https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-shell/src/agent/subscription_check.rs>
- xAI Grok Build `/user` 账号信息模型：<https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-shell/src/auth/model.rs>
- grok2api Build 机器人标记解析：<https://github.com/chenyme/grok2api/blob/main/backend/internal/infra/provider/cli/adapter.go>
- xAI API rate limits：<https://docs.x.ai/developers/rate-limits>
- xAI Management API billing：<https://docs.x.ai/developers/rest-api-reference/management/billing>
- xAI Management API authentication：<https://docs.x.ai/developers/management-api-guide>

## 后果

- 旧式与 unified billing 都只执行一次无生成副作用的官方 billing 查询；不再因刷新额度消耗推理请求。
- Free 账号在 billing 缺少官方使用率时显示默认 1M 的本地滚动 24 小时余额，并明确标注“本地计量”；它可能因缺失 usage 或进程重启而低估已用量，不能解释为 xAI 官方精确余额。
- 管理面以刷新成功隐含本次认证通过，不重复显示成功认证文字；刷新后认证失效和账号访问受限仍显示明确错误，机器人只在确认为已标记时显示顶部图标，最近真实请求确认的 Free 耗尽仍单独展示。
- 额度查询结果仍是管理面瞬时快照，不混入本地 RPM 或持久化运行态；允许合并稳定 OAuthAccount 的本地 Token 窗口，以及由稳定上游错误码产生的当前代际耗尽观测。

## 验证

- Provider 测试覆盖 billing/user URL、camelCase 套餐与限制字段、完整 CLI 身份头、Token Debug 脱敏、`bot_flag_source` 三态、Free 与付费套餐、显式零使用率、缺失使用率、权威百分比、旧式比例、真实 Free 耗尽错误及 actual/limit、周/月周期、金额字段和畸形值拒绝。
- Runtime/HTTP 契约测试覆盖 billing 与 subscription 查询、OAuth subject 请求头、DIRECT 代理、401 刷新后认证失效、403 受限、滚动 24 小时本地 Token 计量、跨 Token generation 复用、真实 actual/limit 优先和无 reset credits。
- Web 测试覆盖 Grok Free 与付费套餐、机器人图标位置、真实限制的条件展示、成功认证与本地余额冗余文字不出现、真实耗尽观测及上游优先级、included allowance、预付余额、按量金额、单账号刷新、批量刷新以及不显示重置按钮。
