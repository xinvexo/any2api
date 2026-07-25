# ADR-0045：Grok OAuth 混合额度查询

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者
- 补充：ADR-0034、ADR-0041
- 修订：兼容 xAI unified billing 响应

## 背景

Grok OAuthAccount 已能登录、刷新 Token 并参与统一路由，但管理页需要独立的额度刷新。xAI 的 CLI subscription 数据面提供只读 billing 接口；旧式响应包含 `creditUsagePercent`，新版 unified billing 响应则只包含 `currentPeriod`、`onDemandCap`、`onDemandUsed`、`prepaidBalance` 和账单周期等字段。

`onDemandCap=0` 与 `onDemandUsed=0` 既不能证明订阅额度剩余 100%，也不能证明账号无限；billing 响应本身也没有 `x-ratelimit-*` 头。继续强制解析 `creditUsagePercent` 会把合法 unified billing 响应误报为无效，而把缺失值当作零使用率会伪造额度。

Sub2API 在 billing 没有权威使用率时发送一次最小 Responses 请求并读取限额响应头。该方法能够取得真实 requests/tokens 限额，但会被 xAI 计作一次真实上游请求，因此只能作为显式额度刷新中的有条件回退，不能在账号列表加载或后台扫描时隐式执行。Codex 的 reset credit 是 ChatGPT 专属能力，不能套用到 Grok。

## 决策

1. Grok Driver 先注册 `GET https://cli-chat-proxy.grok.com/v1/billing?format=credits`，注入当前 OAuth Bearer Token、`X-XAI-Token-Auth: xai-grok-cli`、Grok CLI 版本与 User-Agent。
2. 旧式 billing 响应存在权威 `creditUsagePercent` 时，Driver 校验 `currentPeriod`、百分比和周期边界，并直接投影为 credits 周窗口，不执行探测。
3. billing 周期结构有效但缺失 `creditUsagePercent` 时，Driver 返回类型化的 `ProbeRequired`，而不是无效响应或伪造百分比；`isUnifiedBillingUser` 和金额字段只用于识别上游账单形态，不作为请求/Token 额度。Runtime 随后执行 Driver 预编译的 `POST https://cli-chat-proxy.grok.com/v1/responses`，请求体固定为 `{"model":"grok-4.5","input":"hi","stream":true}`。
4. 探测只接受 2xx 或 429，并只把 `x-ratelimit-limit-requests`、`x-ratelimit-remaining-requests`、`x-ratelimit-reset-requests`、对应 tokens 三个字段和明确状态投影为通用额度窗口。limit/remaining 必须是有效非负整数且 remaining 不得大于非零 limit；reset 只接受 Unix 秒、Unix 毫秒或 RFC 3339。未知的总周期长度保持 `NULL`，禁止用 reset 剩余时间或 billing 周期代替。
5. Runtime 在 Transport 返回响应头后立即丢弃探测流 Body，不解析、转发或持久化生成内容。该取消不能改变“探测已经消耗一次真实 xAI 请求”的事实；Web 的单账号和批量刷新都必须提示这一副作用。
6. 查询复用现有 OAuth quota Runtime：固定 DIRECT/全局代理、严格 SSRF、禁用重定向、有界 billing 响应体和读取超时；401 最多刷新 Token 一次并完整重试一次。Provider 差异通过查询计划、解析结果和响应元数据表达，中央 Runtime 不增加 Provider `match`。
7. Grok quota 是只读临时快照，不写 SQLite、OAuth JSON、PublishedSnapshot、日志或浏览器持久化，也不得根据本地请求统计推算上游额度。
8. Grok 不实现 `quota/reset`。Web 为 Grok 显示单账号及批量刷新，但只为 Codex 显示 reset credit 与重置按钮；Claude 的只读额度入口由 ADR-0046 定义。

## 参考

- Sub2API xAI billing 与 quota probe：<https://github.com/Wei-Shaw/sub2api>
- CLIProxyAPI xAI OAuth 数据面：<https://github.com/router-for-me/CLIProxyAPI>

## 后果

- 旧式 xAI 账号继续用无副作用 billing 刷新；unified billing 账号可以显示真实 requests/tokens 限额，不再因字段升级误报无效响应。
- unified billing 的每次显式额度刷新最多额外产生一个最小推理请求；批量刷新会按账号产生对应探测，必须继续使用前端既有有界并发。
- xAI 未返回可验证 billing 百分比或限额头时查询失败并显示安全错误，不伪造“无限”“剩余 100%”或未知周期。
- 额度探测结果仍是管理面瞬时快照，不混入数据面 RPM、健康状态、请求遥测或持久化运行态。

## 验证

- Provider 测试覆盖 billing/probe URL、CLI 身份头、Token 与请求体 Debug 脱敏、旧式周窗口、新式 `ProbeRequired`、限额头解析和畸形值拒绝。
- Runtime/HTTP 契约测试覆盖 billing 短路、unified billing 两阶段查询、探测 Body 立即释放、429、DIRECT 代理、401 刷新和无 reset credits。
- Web 测试覆盖 Grok requests/tokens 展示、探测副作用提示、单账号刷新、批量刷新以及不显示重置按钮。
