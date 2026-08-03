# ADR-0086：HTTP 400 的结构化额度错误细化

- 状态：Accepted
- 日期：2026-08-03
- 决策人：项目维护者
- 修订：ADR-0013、ADR-0070

## 背景

Provider 错误分类先按 HTTP 状态建立基线，再用供应商 envelope 做相容细化。原先 HTTP 400 只允许细化为普通请求错误或模型不可用，导致已经被 Provider 精确识别为 `QuotaExhausted` 的 `billing_hard_limit_reached` 等结构化值重新退化为 `InvalidRequest`。Runtime 因而既不冷却该 Credential，也不在提交前切换到其他可用 Credential。

另一方面，不能因为部分供应商把余额不足写进普通错误 message，就对自然语言做模糊匹配。文案可能变化、被兼容网关改写，也可能只是参数校验说明；误判会错误排除健康 Credential。

OpenAI 当前官方错误文档要求通过 `error.code` 区分 billing 类 429，并列出 `credit_balance_exhausted`、`organization_spend_limit_exceeded`、`project_spend_limit_exceeded` 与 `organization_usage_limit_exceeded`。项目同时需要兼容已存在的 `insufficient_quota`、`quota_exceeded` 和 `billing_hard_limit_reached` 结构化值。Anthropic 的普通 `invalid_request_error` 没有独立额度字段时，message 中的余额措辞不足以作为分类依据。

## 决策

1. HTTP 400 默认仍分类为 `InvalidRequest`。只有 Provider 分类器从其已声明错误 envelope 的精确 code/type 得到 `QuotaExhausted` 时，共享精炼规则才允许 400 → `QuotaExhausted`。
2. OpenAI 兼容分类器识别当前官方四个 billing/spend/usage code，并保留现有的 `insufficient_quota`、`quota_exceeded`、`billing_hard_limit_reached` 兼容值。匹配大小写规范化后的完整值，不做子串判断。
3. Claude 只有精确 `billing_error` 类型时沿用已有兼容分类；`invalid_request_error` 即使 message 提到 credit、balance、billing 或 limit，也保持 `InvalidRequest`。在出现供应商提供的独立结构化 code 前，不新增 message 推断。
4. 401、5xx、408 和 425 的固定基线不能被额度 code 推翻。未知字段、任意嵌套 code 和畸形 envelope 不参与健康分类。
5. `QuotaExhausted` 继续使用 `RejectedBeforeExecution`：未绑定且仍为 Pending 的请求可在预算内切换 Credential，当前 generation 建立有界额度冷却；绑定请求不跨 Credential。
6. 内部细化不得改变最终客户端响应。若没有后续可用候选，原上游 HTTP 状态、允许 Header 与有界原始正文照常返回。

## 后果

- 已被结构化证据确认欠费的 Credential 不会在每个请求中被反复选中，其他可用 Credential 可以接管未提交请求。
- 普通参数错误和只含余额文案的 Anthropic 400 不会污染额度健康。
- 兼容旧 OpenAI/网关错误值与跟进当前官方 code 可以并存，不需要按供应商在 Runtime 增加分支。

## 验证

- Provider 单元测试枚举当前官方及保留兼容额度 code，覆盖 400/429、重试安全性、普通 message 不误判与固定状态不可被推翻。
- Registry 契约枚举实际 Codex/Claude/Grok Driver，确认结构化 400 的分类边界。
- Public request 契约使用两个 Credential：第一个返回结构化 400 额度错误，断言同一请求切换并成功；后续请求继续避开处于额度冷却的 Credential，同时 Attempt 记录保留 `QuotaExhausted`。
