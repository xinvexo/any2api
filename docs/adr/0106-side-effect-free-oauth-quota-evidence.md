# ADR-0106：OAuth 额度查询只读与拒绝证据收敛

- 状态：Accepted
- 日期：2026-08-04
- 决策者：maintainer
- 相关决策：ADR-0045、ADR-0079、ADR-0111

## 背景

Claude 全仓审查要求重新核对 Grok Free Token、`subscriptionTier` 空值和 Codex 403 出口探测。核对当前实现后确认两条主动探测都缺少可靠协议基础：

- Grok Free 额度刷新在两个只读 GET 之后又发送一次真实 `POST /v1/chat/completions`。即使 `max_tokens=1`，它仍是生成请求，会消耗 Provider 工作、增加延迟，并可能触发限流或风控。xAI 官方 Grok Build 的 billing UI 只读取 `/billing?format=credits`；官方仓库没有用生成请求抓取 Free Token Header 的对应实现。
- Codex 未分类 403 会去掉账号认证后重放受保护的 `/wham/usage`，再把无认证 403 解释为出口拒绝。端点可以因为缺少认证本身返回 403，因此该结果无法区分出口、认证与端点策略。OpenAI 官方 Codex 对 Cloudflare 403 直接读取原始响应正文中的 `Cloudflare` 与 `blocked` 标记，没有发第二次无认证请求。

Grok 官方 `UserInfo.subscription_tier` 是 `Option<String>`：字段缺失用于兼容旧响应，显式 null 表示没有活动订阅，空字符串仍是一个显式字符串但不构成有效订阅层级。把三者全部改写成 `Free` 会把未知协议状态伪装成确定套餐。

## 决策

1. 管理额度查询必须保持非生成、只读。Provider quota 查询计划只允许该 Provider 已审计的额度、账单、订阅或账号状态读取端点；禁止调用 Responses、Chat Completions、Messages、Images 或其他会执行模型工作的端点获取 Header。
2. 删除 ProviderDriver 的可选 Token balance 查询计划与解析钩子。首版只有 Grok 使用它，且唯一实现违反第 1 条；保留通用 `OAuthQuotaTokenBalance` 结果类型，用于真实数据面耗尽观测的安全投影。
3. Grok 管理额度刷新只执行 `GET /v1/billing?format=credits` 与 `GET /v1/user?include=subscription`。显式 `subscriptionTier: null` 投影为 `Free`；非空字符串去除首尾空白后原样使用；字段缺失或空白字符串保持未知。JWT tier 不参与覆盖。
4. Grok Free Token 数字只有在真实数据面返回 `subscription:free-usage-exhausted` 且正文携带通过安全整数校验的 `actual/limit` 时才展示。没有数字的明确耗尽只展示耗尽状态。只读额度查询没有明确可用证据时不得清除既有耗尽观测；成功数据面请求仍可清除。
5. Codex quota 403 只从当前有界原始响应分类：保留顶层 `code` 与 `error.code/type` 的固定结构化码表；正文同时包含 `Cloudflare` 与 `blocked` 时按官方客户端行为分类为 Provider 出口拒绝。其他 403 保持未分类。
6. 删除无认证 `/wham/usage` 探测、Provider egress probe trait、Runtime `EgressProbeCache`、revision 键和相应并发逻辑。`oauth_provider_egress_restricted` 错误码保留，用于第 5 条的直接证据；未知 403 继续返回中性 `oauth_quota_upstream_failed`。

## 证据

- xAI Grok Build billing 只读实现（核对 revision `e5478eff1e4050558e12e1328b85e6616632efb6`）：<https://github.com/xai-org/grok-build/blob/e5478eff1e4050558e12e1328b85e6616632efb6/crates/codegen/xai-grok-shell/src/extensions/billing.rs>
- xAI Grok Build `UserInfo.subscription_tier` 反序列化测试：<https://github.com/xai-org/grok-build/blob/e5478eff1e4050558e12e1328b85e6616632efb6/crates/codegen/xai-grok-shell/src/auth/model.rs>
- xAI Grok Build 活动订阅检查：<https://github.com/xai-org/grok-build/blob/e5478eff1e4050558e12e1328b85e6616632efb6/crates/codegen/xai-grok-shell/src/agent/subscription_check.rs>
- OpenAI Codex Cloudflare 403 原始正文分类（核对 revision `78306a32afe99ce88fbc3616f8ef325baae91cd0`）：<https://github.com/openai/codex/blob/78306a32afe99ce88fbc3616f8ef325baae91cd0/codex-rs/codex-api/src/api_bridge.rs>
- OpenAI Codex `/wham/usage` 单次认证读取：<https://github.com/openai/codex/blob/78306a32afe99ce88fbc3616f8ef325baae91cd0/codex-rs/backend-client/src/client/rate_limit_resets.rs>

## 后果

- Grok Free 额度刷新从三次上游请求降为两次只读 GET，不再消耗推理工作，也不会因探测生成失败让本可读取的账单快照整体失败。
- 未耗尽时 Free Token 总额与剩余值可能显示未知；这比从无官方只读契约的生成 Header 制造精确数字更诚实。
- Codex 未分类 403 少一次网络往返，并避免把无认证端点策略误报成出口限制；只有结构化码或明确 Cloudflare 阻断正文才返回出口诊断。
- ProviderDriver 与 Runtime quota 路径移除两组单实现扩展点、缓存和异步分支，职责更小。

## 验证

- Provider 测试覆盖 Grok null/缺失/空白/非空套餐，且实际 Registry 不再暴露生成式 quota 钩子。
- Provider 测试覆盖 Codex 声明区域码、Cloudflare 阻断正文、普通未知 403、冲突码和畸形正文。
- Runtime 与 HTTP 契约断言 Grok Free 只访问 billing/user 两个 GET；真实耗尽观测仍可投影数字且不会被无证据查询清除。
- Runtime 与 HTTP 契约断言 Codex 直接证据可返回出口错误、未知 403 保持中性，且每次额度失败只产生原始带认证请求。
