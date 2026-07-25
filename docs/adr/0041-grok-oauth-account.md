# ADR-0041：Grok OAuth 作为独立 OAuthAccount 接入订阅数据面

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者
- 取代：ADR-0040 中“Grok 不进入 OAuthAccount”的阶段性边界
- 部分取代：登录方式已由 ADR-0043 改为 Device Authorization Grant；其余存储、数据面与路由决策继续有效

## 背景

Grok API Key Provider 已经通过 ADR-0040 接入。现在需要继续接入 Grok 订阅账号，但不能把 OAuth JSON 混入 `ProviderCredential`，也不能复制第二套调度、限速、粘性、健康和重试实现。

xAI 的 OIDC Discovery 文档公开 Authorization Code、Refresh Token 和 Device Code grant。CLIProxyAPI 使用 Device Code；Sub2API 使用同一个公共 xAI 客户端完成 Authorization Code + PKCE，并把订阅推理流量发送到 Grok CLI 数据面。any2api 已有受管理的 PKCE 会话、手工粘贴 loopback 回调 URL、服务端 Token 兑换和 SQLite 原子发布链路，因此复用 Authorization Code + PKCE 是当前最短且一致的实现。

## 决策

1. `ProviderKind::Grok` 支持独立 `OAuthAccount`。Grok OAuth JSON 明文保存在 `oauth_accounts.oauth_json`，不进入 ProviderCredential、Provider Endpoint、Vault、管理 DTO、日志、浏览器存储或导出接口。
2. 登录使用 `https://auth.x.ai/oauth2/authorize` 和 `https://auth.x.ai/oauth2/token`，公共客户端 ID 为 `b1a00492-073a-47ea-816f-4c329264a828`，redirect URI 为 `http://127.0.0.1:56121/callback`，scope 为 `openid profile email offline_access grok-cli:access api:access`。请求使用 state、nonce 和 S256 PKCE；刷新使用同一客户端与 Refresh Token grant。
3. 登录兑换、刷新和数据面固定使用 OAuthAccount 的 DIRECT/全局代理解析结果，禁用自动重定向，不增加 Grok 专用代理或隐式本机直连回退。
4. Grok OAuth 的固定数据面为 `https://cli-chat-proxy.grok.com/v1`。Driver 注入 `Authorization: Bearer`、`X-XAI-Token-Auth: xai-grok-cli`、稳定的 Grok CLI 版本头和对应 User-Agent。Grok API Key 仍使用管理员配置的 Endpoint，二者不共享存储模型。
5. Grok OAuth 首版只参与 OpenAI Responses 操作。订阅数据面没有原生 `/responses/compact`，因此 Grok OAuth 不进入 Responses Compact 或 Chat Completions 的候选；API Key Grok 的既有能力不变。
6. Grok OAuth 使用 Provider 内置的文本模型目录：`grok-4.5`、`grok-4.3`、`grok-build-0.1`、`grok-composer-2.5-fast`、`grok-4.20-0309-reasoning`、`grok-4.20-0309-non-reasoning`、`grok-4.20-multi-agent-0309`。首版没有图片或视频入口，因此不发布媒体模型。
7. Token JSON 使用 `type=grok`，保存 access token、可选 refresh/id token、subject、email、过期时间与刷新时间。刷新继续使用 Token Version CAS、串行配置发布、Runtime reconcile 和单次 PublishedSnapshot 切换。
8. SQLite 只增加 Migration 25，重建 `oauth_accounts` 的 Provider CHECK 以接受 `grok`，并保留既有账号、模型关系、请求日志引用、索引和外键完整性；不修改历史 Migration。
9. Grok API Key 和 OAuthAccount 只在通用 `RoutingCredential` 投影处合流，共用 RPM、轮询、粘性、健康、重试、代理、流式生命周期和遥测。中央调度器不增加 Grok 分支。

## 依据

- xAI OIDC Discovery：<https://auth.x.ai/.well-known/openid-configuration>
- CLIProxyAPI xAI OAuth 与数据面实现：<https://github.com/router-for-me/CLIProxyAPI>
- Sub2API Grok OAuth 与订阅数据面实现：<https://github.com/Wei-Shaw/sub2api>

## 结果

- 管理员在 OAuth 独立页面完成 Grok 授权后，服务端直接激活 SQLite OAuthAccount，不下载或保留凭据文件。
- Grok API Key 和订阅账号可以服务同一个公开模型，但 GatewayApiKey 不能选择、绑定或影响其中任何凭据。
- Grok Token 不会通过管理 API 回传；浏览器只持有一次性登录 session、授权 URL 和安全账号元数据。
- Provider 的操作能力显式区分 API Key 与 OAuth，避免把订阅代理不支持的端点错误加入候选。

## 未选择方案

- 把 Grok OAuth JSON 存进 ProviderCredential：会破坏两类凭据的永久隔离和 Vault 语义。
- 保存或下载 OAuth 文件：SQLite 已是唯一持久化真相来源，额外文件会产生双写和泄漏面。
- 为 Grok OAuth 新建调度器：会复制 RPM、粘性、健康、重试和流式生命周期。
- 在本阶段实现 Device Code：虽然 xAI 支持，但会引入第二套登录会话与轮询 API；现有 PKCE 回调流程已经完整满足管理 Web 的登录需求。
- 伪造 `/responses/compact` 支持：订阅数据面没有对应原生端点，首版不引入提示词压缩或跨数据面回退。
