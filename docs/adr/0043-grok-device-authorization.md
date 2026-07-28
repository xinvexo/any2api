# ADR-0043：Grok OAuth 使用 Device Authorization Grant

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者

## 背景

Grok OAuthAccount 使用 xAI OIDC Discovery 公布的 Device Authorization Grant。公共客户端先申请 device code，再轮询 Token Endpoint；管理 Web 不接收 localhost callback，也不持有 Device Code 或 Token。

## 决策

1. Grok 登录固定使用 `POST https://auth.x.ai/oauth2/device/code`，公共客户端 ID 为 `b1a00492-073a-47ea-816f-4c329264a828`，scope 为 `openid profile email offline_access grok-cli:access api:access`。
2. Provider Driver 构建设备授权与设备 Token 请求，解析 device response，并分类 `authorization_pending`、`slow_down`、`access_denied` 和 `expired_token`。Token 轮询使用 `urn:ietf:params:oauth:grant-type:device_code`。
3. `device_code` 使用 Secret 类型，只存在于服务端内存 session，不进入 SQLite、日志、Debug、管理 DTO 或浏览器。浏览器只获得 session ID、user code、验证地址、有效期和安全轮询间隔。
4. Runtime 使用显式管理 poll 端点。一次 poll 原子消费 session；pending 或 slow_down 时更新下一次允许时间并恢复 session，成功、拒绝、过期或不可恢复错误时终止 session。`slow_down` 每次增加 5 秒间隔。
5. Web 打开验证地址、突出显示 user code，并自动使用服务端给出的间隔轮询；Grok 不显示 callback URL 输入。Codex 与 Claude 继续使用现有 Authorization Code + PKCE 流程。
6. Device session 使用 xAI 返回的有效期，最长 30 分钟；所有设备授权和 Token 请求继续走 OAuthAccount 的 DIRECT/全局代理解析结果，禁用重定向且不回退本机直连。
7. Token 成功后复用现有 Provider 解析、SQLite 激活、Runtime reconcile、PublishedSnapshot 切换和通用 `RoutingCredential` 投影。Grok API Key 与 OAuthAccount 的存储和管理模型仍严格分离。
8. Grok refresh grant、SQLite OAuth JSON、固定 `https://cli-chat-proxy.grok.com/v1` 数据面、Bearer 和 xAI CLI 身份头遵循 ADR-0041。

## 依据

- xAI OIDC Discovery：<https://auth.x.ai/.well-known/openid-configuration>
- CLIProxyAPI xAI Device Code 实现：<https://github.com/router-for-me/CLIProxyAPI>
- Sub2API xAI OAuth 与 Grok CLI 数据面实现：<https://github.com/Wei-Shaw/sub2api>

## 结果

- Grok 管理登录与 CLI 客户端认证方式一致，不要求管理员截取 localhost callback。
- Device Code 不离开服务端，浏览器不会得到 Token 或 Provider JSON。
- 新登录方式只增加 Provider 局部协议实现和通用 OAuth session 分支，不复制账号存储、刷新、调度或数据面。

## 未选择方案

- 增加 Grok PKCE 作为第二种入口：会形成重复交互模型并扩大测试面。
- 后端单个请求阻塞到用户授权完成：会占用长连接，难以表达页面关闭、取消和轮询退避。
- 把 device code 存入 SQLite：登录 session 是短期运行态，持久化会扩大 Secret 泄漏面且不提供重启恢复价值。
