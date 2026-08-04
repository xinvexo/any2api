# ADR-0079：OAuth 额度拒绝与 Provider 出口诊断分离

- 状态：Accepted
- 日期：2026-07-31
- 决策者：maintainer
- 修订：ADR-0036、ADR-0070、ADR-0106

## 背景

OAuth 额度请求过去把所有 `403 Forbidden` 直接映射为账号访问受限。这个状态码也可能由 Provider 的区域策略、
IP 信誉或边缘访问策略产生；当 OAuthAccount 固定继承全局代理时，实际故障可能只是当前网络出口不被 OpenAI
接受。把二者合并会向管理员错误宣称账号被封禁，并可能诱导其停用或删除仍然有效的账号。

现有代理测试只访问 `https://example.com/`，只能证明通用 DNS、代理握手、TLS 和响应头可达，不能证明同一
出口符合某个 Provider 的访问策略。账号认证、账号限制和出口限制需要各自独立、可验证的证据。

## 决策

1. 除继续进入既有 Token 刷新链的 `401` 外，OAuth 额度非成功响应由 Provider Driver 读取有界响应，并从
   声明的结构化字段分类为“明确账号限制”“明确 Provider 出口拒绝”或“未分类”。禁止仅凭 `403` 或
   自然语言消息声明账号受限。
2. Codex 首批只读取顶层 `code` 及 `error.code/type`。`unsupported_country_region_territory` 表示出口拒绝；
   `account_deactivated`、`account_suspended` 和 `account_disabled` 表示账号限制。Grok 只在声明字段中识别
   `unauthorized:blocked-user` 为账号限制。未知码、字段位置不符、相互冲突的声明码和畸形正文保持未分类。
3. Codex 还接受同一次原始 403 正文中同时出现官方客户端使用的 `Cloudflare` 与 `blocked` 标记，作为边缘
   出口拒绝的明确证据。该匹配只读取有界正文；普通 HTML、单独出现任一词、未知 JSON 码和畸形正文保持
   未分类。
4. 禁止去掉 Authorization 或 `chatgpt-account-id` 后重放 `/backend-api/wham/usage`。该端点受保护，未认证
   请求自身可能返回 403；二次请求不能把账号/端点策略与出口策略可靠分离，反而增加延迟和边缘风控面。
5. Provider 出口诊断不再需要 Runtime 单飞缓存、revision 键或额外网络请求；分类只依赖当前响应，不更新
   Proxy、Endpoint 或 Credential 健康。通用 `example.com` 代理测试继续只表示公网连通性。
6. 管理 API 分别返回 `oauth_account_restricted`、`oauth_provider_egress_restricted` 和
   `oauth_quota_upstream_failed`。Web 文案必须分别描述账号限制、当前网络/全局代理出口拒绝和未知上游失败，
   不得使用“限制或封禁”覆盖多个原因。
7. “删除失效账号”仍只接受 `oauth_account_authentication_failed`。账号限制、出口拒绝、未分类 `403`、
   刷新无法确认和其他额度错误都不是删除候选。

## 后果

- 全局代理出口被 OpenAI 拒绝时，管理员会得到可操作的代理诊断，不再看到账号封禁误报。
- 明确账号限制仍保留独立诊断，但必须有 Provider 声明的结构化证据。
- 未分类 403 不再触发额外请求，因此不会制造探测延迟、并发洪峰或无认证边缘请求。
- Provider 新增或调整拒绝码时只修改自身 Driver 与契约测试，Runtime 不增加 Provider 分支。

## 验证

- Provider 测试覆盖账号码、区域码、未知 `403`、畸形正文和禁止递归扫描。
- Runtime 测试覆盖声明码与 Cloudflare 阻断正文的直接分类、未知 403 保持中性，并断言每次失败只有原始带认证请求。
- HTTP/Web 测试覆盖三个稳定错误码和文案，并确认账号限制与出口拒绝都不会进入删除集合。
