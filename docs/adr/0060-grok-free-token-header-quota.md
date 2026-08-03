# ADR-0060：Grok Free Token 额度从响应头同步

- 状态：Accepted
- 日期：2026-07-28
- 决策人：项目维护者
- 部分修订：ADR-0070

## 背景

Grok Free 推理响应会返回 `x-ratelimit-limit-tokens` 与
`x-ratelimit-remaining-tokens`。这两个上游字段是 Token 总额与剩余值的唯一来源；本地默认值会在
xAI 调整额度时立即失真。

## 决策

1. billing 与 `/user?include=subscription` 仍负责 credits、套餐和账号诊断。只有同次
   user 响应确认为 Free 时，Grok Driver 才返回最小
   `POST /v1/chat/completions` Token 额度探测计划。
2. 探测使用当前 OAuth Token、subject、CLI 身份头、DIRECT/全局代理和严格
   SSRF 策略；Runtime 只执行 Driver 计划，不增加 Grok `match`。
3. 只有 `x-ratelimit-limit-tokens` 和 `x-ratelimit-remaining-tokens` 同时存在、均为
   `0..=Number.MAX_SAFE_INTEGER` 范围的整数、limit 大于零且 remaining 不超过
   limit 时，才返回 `source=upstream` 的 Token 余额。
4. 响应头缺失、不完整或无效时余额保持未知；不从 billing 金额、本地 usage、请求数或其他头猜测。
5. 探测若返回 `subscription:free-usage-exhausted` 且正文带有经验证的
   `tokens (actual/limit)`，该值作为同次上游 Token 余额。
6. 额度快照不持久化，不参与 RPM、持久化账号启停或 GatewayApiKey 准入。ADR-0070 与 ADR-0095 允许权威 `remaining=0` 临时阻止当前 OAuth 账号 `routing_generation` 进入路由候选。

## 后果

- xAI 修改 Free Token 上限后，下次额度刷新会自动同步，不需要发版。
- 刷新 Free 额度会执行一次最小推理请求；这是获取当前响应头快照的必要成本。
- API 未返回完整头时 Web 显示未知，不给出看似精确的假额度。

## 验证

- Provider 测试覆盖动态 limit/remaining、缺失/非法/越界值和
  `actual/limit` 耗尽响应。
- Runtime 与 HTTP 契约测试覆盖 Free 才发送探测、上游变更 limit 后直接反映，以及付费
  套餐不探测。
