# ADR-0041：Grok OAuth 作为独立 OAuthAccount 接入订阅数据面

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

Grok 订阅账号必须作为独立 OAuthAccount 接入，不能把 OAuth JSON 混入 `ProviderCredential`，也不能复制第二套调度、限速、粘性、健康和重试实现。xAI Device Authorization Grant 提供无需本地 callback 的公共客户端登录流程，订阅推理流量使用 Grok CLI 数据面。

## 决策

1. `ProviderKind::Grok` 支持独立 `OAuthAccount`。Grok OAuth JSON 明文保存在 `oauth_accounts.oauth_json`，不进入 ProviderCredential、Provider Endpoint、Vault、管理 DTO、日志、浏览器存储或导出接口。
2. 登录使用 `POST https://auth.x.ai/oauth2/device/code` 和 `https://auth.x.ai/oauth2/token`，公共客户端 ID 为 `b1a00492-073a-47ea-816f-4c329264a828`，scope 为 `openid profile email offline_access grok-cli:access api:access`。Device Code 只保存在服务端内存，浏览器只获得 session ID、user code、验证地址、有效期和安全轮询间隔；刷新使用同一客户端与 Refresh Token grant。完整轮询契约见 ADR-0043。
3. 登录兑换、刷新和数据面固定使用 OAuthAccount 的 DIRECT/全局代理解析结果，禁用自动重定向，不增加 Grok 专用代理或隐式本机直连回退。
4. Grok OAuth 的固定数据面为 `https://cli-chat-proxy.grok.com/v1`。Driver 注入 `Authorization: Bearer`、`X-XAI-Token-Auth: xai-grok-cli`、稳定的 Grok CLI 版本头和对应 User-Agent。Grok API Key 仍使用管理员配置的 Endpoint，二者不共享存储模型。
5. Grok OAuth 只参与 OpenAI Responses 操作。订阅数据面没有原生 `/responses/compact`，因此 Grok OAuth 不进入 Responses Compact 或 Chat Completions 候选；Grok API Key 的能力不变。
6. Grok OAuth 使用 Provider 内置且可测试的文本模型目录，不发布媒体模型。
7. Token JSON 使用 `type=grok`，保存 access token、可选 refresh/id token、subject、email、过期时间与刷新时间。刷新继续使用 Token Version CAS、串行配置发布、Runtime reconcile 和单次 PublishedSnapshot 切换。
8. SQLite `oauth_accounts` Provider CHECK 接受 `grok`，并保持账号、模型关系、请求日志引用、索引和外键不变量。
9. Grok API Key 和 OAuthAccount 只在通用 `RoutingCredential` 投影处合流，共用 RPM、轮询、粘性、健康、重试、代理、流式生命周期和遥测。中央调度器不增加 Grok 分支。

## 依据

- xAI OIDC Discovery：<https://auth.x.ai/.well-known/openid-configuration>
- CLIProxyAPI xAI OAuth 与数据面实现：<https://github.com/router-for-me/CLIProxyAPI>
- Sub2API Grok OAuth 与订阅数据面实现：<https://github.com/Wei-Shaw/sub2api>

## 结果

- 管理员在 OAuth 独立页面完成 Grok 授权后，服务端直接激活 SQLite OAuthAccount，不下载或保留凭据文件。
- Grok API Key 和订阅账号可以服务同一个公开模型，但 GatewayApiKey 不能选择、绑定或影响其中任何凭据。
- Grok Token 不会通过管理 API 回传；浏览器只持有一次性登录 session、user code、验证地址和安全账号元数据。
- Provider 的操作能力显式区分 API Key 与 OAuth，避免把订阅代理不支持的端点错误加入候选。

## 未选择方案

- 把 Grok OAuth JSON 存进 ProviderCredential：会破坏两类凭据的永久隔离和 Vault 语义。
- 保存或下载 OAuth 文件：SQLite 已是唯一持久化真相来源，额外文件会产生双写和泄漏面。
- 为 Grok OAuth 新建调度器：会复制 RPM、粘性、健康、重试和流式生命周期。
- 增加 PKCE callback 入口：会形成第二套 Grok 登录交互和无必要的兼容分支。
- 伪造 `/responses/compact` 支持：订阅数据面没有对应原生端点，不引入提示词压缩或跨数据面回退。
