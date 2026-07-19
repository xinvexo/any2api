# ADR-0014: 单管理员认证与远程 HTTP 管理边界

- 状态：Accepted
- 日期：2026-07-20
- 决策者：maintainer

## 背景

当前管理 API 只检查 TCP 对端是否为 loopback。该门禁不能提供管理员身份、会话撤销或 CSRF 防护，并且本机 Nginx/Caddy 反代会让远程请求看起来来自 loopback。项目需要支持个人实例的远程管理，同时明确允许用户在受信网络中选择明文 HTTP，不能把 TLS 设为强制前置条件。

## 决策

- 采用 singleton 单管理员模型，不增加用户表、注册、角色、租户或 GatewayApiKey 登录能力。
- SQLite 新增 singleton 管理员凭据表，只保存 Argon2id PHC 摘要。首次初始化通过 loopback Setup API 或启动环境变量 `ANY2API_ADMIN_PASSWORD`；Setup API 额外要求启动终端显示的 256 位一次性 Token，Token 不进入 SQLite 或管理 API 响应，成功后立即失效。已有凭据时环境变量不执行在线轮换。
- `server` 持有稳定的 `AdminAuthService`，通过 app 层实现的窄 `AdminCredentialStore` 端口访问 SQLite。Server 不依赖 storage，Storage 不依赖 Axum。
- 登录成功后签发 256 位随机会话 Token 和独立 CSRF Token。两者及登录失败窗口只保存在内存；进程重启、服务终止或管理员凭据重新初始化后会话全部失效。
- Setup 使用单独的 Argon2id Permit，登录使用固定上限的 Argon2id Permit；Permit 被移动进 blocking 任务，HTTP 请求取消不会提前释放。登录尝试在校验前进入失败窗口，只有成功后才清除，取消请求不能绕过失败记账。
- 会话 Cookie 固定使用 `any2api_admin`、`Path=/api/admin`、`HttpOnly`、`SameSite=Strict`；确认 HTTPS 时增加 `Secure`。所有非安全方法必须提供匹配会话的 `X-CSRF-Token`。
- 全部 `/api/admin` 成功与失败响应统一添加 `Cache-Control: no-store` 和 `Vary: Cookie`。
- 管理认证 API 固定为：

```text
GET  /api/admin/auth/session
POST /api/admin/auth/setup
POST /api/admin/auth/login
POST /api/admin/auth/logout
```

- `admin.remote_enabled`、会话 idle/absolute timeout、登录失败窗口和最大失败次数进入 SettingRegistry 并热更新。监听地址仍由 `ANY2API_BIND` 决定；仅开启远程设置不会隐式修改 socket bind。
- 首切片直接支持远程 HTTP，并支持外部 TLS 终止。可信反代 CIDR 由启动环境变量 `ANY2API_TRUSTED_PROXY_CIDRS` 显式配置；仅可信 TCP 对端可以提供 `X-Forwarded-For` 和 `X-Forwarded-Proto`。可信代理请求必须同时提供唯一且合法的两个头，来源链从 TCP 对端开始按 XFF 右到左剥离连续可信代理，防止客户端预置 loopback 值绕过远程开关或 Setup 限制。
- 非 loopback 且未确认 HTTPS 的已登录管理界面持续展示明文传输风险，但不拒绝请求。内建 Rustls listener 独立排期，不与认证状态机同时实现。
- 没有注入 `AdminAuthService` 的嵌入/契约测试 Router 保留旧的 loopback-only 门禁；正式 Composition Root 必须注入服务，且这种模式永远不能远程访问。

## 备选方案

- 使用 GatewayApiKey 登录管理面：拒绝。两类凭据职责必须隔离，Gateway Key 不能获得配置权限。
- JWT 或客户端自包含会话：拒绝。单节点服务端会话更容易立即失效、执行 idle timeout，并且无需密钥轮换体系。
- 强制 HTTPS 才允许远程管理：拒绝。项目明确支持本地部署和受信网络中的 HTTP；风险通过显式开关与持续警告表达。
- 同时实现内建 TLS listener：暂不采用。它会扩大监听器、证书加载和重启配置的改动面，不是验证管理员认证链路所必需。

## 后果

- loopback 与远程管理使用同一管理员身份，不会形成两套权限模型。
- 进程重启后管理员需要重新登录，符合“不恢复运行态”的项目边界。
- 外部 TLS 反代必须正确配置可信代理 CIDR；未配置时转发头被忽略，Cookie 不会被错误标记为 Secure。
- 本切片之后仍需实现内建 TLS、Web 中的可信代理/监听配置和管理员密码在线轮换。

## 验证

- Storage 测试覆盖 singleton 初始化、重复初始化和重启读取 Argon2id 摘要。
- Server 单元测试覆盖密码校验、会话 idle/absolute 过期、失败窗口、CSRF、取消后 Argon2 permit 生命周期和可信代理来源解析。
- HTTP 契约测试覆盖 loopback Setup Token、登录、Cookie/缓存属性、远程开关、明文风险、受保护 CRUD、CSRF、登出和重启会话失效。
- Web 测试覆盖 Setup/Login 门、CSRF 注入、401 响应头立即关闭会话、管理缓存清理和登录前/登录后的持续明文 HTTP 警告。
