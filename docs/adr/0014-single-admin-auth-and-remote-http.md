# ADR-0014: 单管理员认证与远程 HTTP 管理边界

- 状态：Accepted
- 日期：2026-07-20
- 修订：2026-08-03
- 决策者：maintainer

> `admin.remote_enabled` 的默认值及可信代理配置来源已由 ADR-0072 部分取代；转发头缺失与重复策略由 ADR-0096 修订。

## 背景

个人实例的管理面需要独立管理员身份、会话撤销和 CSRF 防护，并允许在明确知悉风险的前提下通过明文 HTTP 远程管理。反向代理来源也必须在信任边界内解析，不能仅凭 TCP 对端地址判断客户端身份。

## 决策

- 采用 singleton 单管理员模型，不增加用户表、注册、角色、租户或 GatewayApiKey 登录能力。
- SQLite 新增 singleton 管理员凭据表，只保存 Argon2id PHC 摘要。首次初始化通过 loopback Setup API 或启动环境变量 `ANY2API_ADMIN_PASSWORD`；Setup API 额外要求启动终端显示的 256 位一次性 Token，Token 不进入 SQLite 或管理 API 响应，成功后立即失效。已有凭据时环境变量不执行在线轮换。
- `server` 持有稳定的 `AdminAuthService`，通过 app 层实现的窄 `AdminCredentialStore` 端口访问 SQLite。它是 `AppState` 与管理 Router 的必需构造依赖，生产 Composition Root 和测试夹具都必须显式注入；加载失败时不得启动 Router，受保护路由也不得降级为 loopback 免认证。Server 不依赖 storage，Storage 不依赖 Axum。
- 登录成功后签发 256 位随机会话 Token 和独立 CSRF Token。两者及登录失败窗口只保存在内存；进程重启、服务终止或管理员凭据重新初始化后会话全部失效。
- Setup 使用单独的 Argon2id Permit，登录使用固定上限的 Argon2id Permit；Permit 被移动进 blocking 任务，HTTP 请求取消不会提前释放。登录尝试在校验前进入失败窗口，只有成功后才清除，取消请求不能绕过失败记账。
- 会话 Cookie 固定使用 `any2api_admin`、`Path=/api/admin`、`HttpOnly`、`SameSite=Strict`；确认 HTTPS 时增加 `Secure`。所有非安全方法必须提供匹配会话的 `X-CSRF-Token`。
- 全部 `/api/admin` 成功、认证/提取失败与 fallback 响应都由管理 Router 最外层的单一响应中间件添加 `Cache-Control: no-store` 和 `Vary: Cookie`；具体 Handler、JSON 包装器和 `AdminApiError` 不重复设置这两个 Header。
- 管理 JSON Body 使用一个窄的强类型提取器，把 Axum JSON rejection 统一映射为稳定 `invalid_request` envelope；配置删除等 query 使用分别面向 `expected_revision` 与 `expected_revision + expected_config_version` 的强类型提取器。它们只集中 HTTP 输入错误，不接管 feature DTO、领域转换、Publisher 调用或响应组装，不能演化为通用管理 Handler。
- Web 登录表单只在组件内存中持有管理员密码，使用标准 `autocomplete=current-password` 交给浏览器密码管理器；禁止把密码写入 `localStorage`、`sessionStorage`、React Query 或其他浏览器持久化状态，也不识别任何历史密码存储格式。
- Server 在合并 Web、`/api/**` 与 `/v1/**` 后通过单一全局响应中间件为全部成功、错误和 fallback 设置 `X-Content-Type-Options: nosniff`，具体 Handler 与命名空间不重复注入。管理 HTML 与静态资源额外统一返回最小权限 CSP 和 `Referrer-Policy: no-referrer`；CSP 使用 `frame-ancestors 'none'` 禁止嵌入。服务继续支持明文 HTTP，不设置 HSTS、不强制跳转 HTTPS。
- 管理认证 API 固定为：

```text
GET  /api/admin/auth/session
POST /api/admin/auth/setup
POST /api/admin/auth/login
POST /api/admin/auth/logout
```

- `admin.remote_enabled`、会话 idle/absolute timeout、登录失败窗口和最大失败次数进入 SettingRegistry 并热更新。监听地址仍由 `ANY2API_BIND` 决定；仅开启远程设置不会隐式修改 socket bind。
- 服务支持远程 HTTP 和外部 TLS 终止。可信反代 CIDR 只来自当前 PublishedSnapshot 的热更新设置 `network.trusted_proxy_cidrs`；仅可信 TCP 对端可以提供 `X-Forwarded-For` 和 `X-Forwarded-Proto`。缺少 XFF 时回退规范化 TCP 对端，缺少 XFP 时按不安全 HTTP 处理；多行 XFF 按完整逻辑列表合并，非法 XFF 与重复/非法 XFP 仍拒绝。来源链从 TCP 对端开始按 XFF 右到左剥离连续可信代理。该逻辑地址只用于客户端身份、限流和审计；Setup 与 `admin.remote_enabled=false` 使用 ADR-0088 的直接 loopback TCP 边界，不能被客户端预置的 XFF loopback 绕过。完整缺失/非法边界见 ADR-0096。
- 非直接 loopback 且未确认 HTTPS 的已登录管理界面持续展示明文传输风险，但不拒绝请求。服务不提供内建 TLS listener。

## 备选方案

- 使用 GatewayApiKey 登录管理面：拒绝。两类凭据职责必须隔离，Gateway Key 不能获得配置权限。
- JWT 或客户端自包含会话：拒绝。单节点服务端会话更容易立即失效、执行 idle timeout，并且无需密钥轮换体系。
- 强制 HTTPS 才允许远程管理：拒绝。项目明确支持本地部署和受信网络中的 HTTP；风险通过显式开关与持续警告表达。
- 内建 TLS listener：不采用。外部 TLS 终止已经满足部署需求，且保持服务监听边界简单。

## 后果

- loopback 与远程管理使用同一管理员身份，不会形成两套权限模型。
- 缺失管理认证服务属于无法构造应用的组合错误，而不是能够在请求路径中放宽权限或临时降级的运行状态。
- 进程重启后管理员需要重新登录，符合“不恢复运行态”的项目边界。
- 外部 TLS 反代必须正确配置可信代理 CIDR；未配置时转发头被忽略，Cookie 不会被错误标记为 Secure。
- 管理员密码在线轮换与数据目录单实例锁遵循 ADR-0024。

## 验证

- Storage 测试覆盖 singleton 初始化、重复初始化和重启读取 Argon2id 摘要。
- Server 单元测试覆盖密码校验、会话 idle/absolute 过期、失败窗口、CSRF、取消后 Argon2 permit 生命周期和可信代理来源解析。
- HTTP 契约测试显式注入真实 `AdminAuthService`，并覆盖 loopback Setup Token、登录、Cookie/缓存属性、远程开关、明文风险、受保护 CRUD、CSRF、登出和重启会话失效；管理成功、错误与 fallback 均验证统一缓存 Header，畸形 JSON 和缺失 revision query 验证稳定管理错误；受保护 CRUD 不依赖任何 loopback 免认证测试回退。
- Web 测试覆盖 Setup/Login 门、密码不进入浏览器持久化状态、浏览器密码管理属性、CSRF 注入、401 响应头立即关闭会话、管理缓存清理和登录前/登录后的持续明文 HTTP 警告；Server/HTTP 契约测试覆盖内嵌与外部管理资源的 Web 专属安全头，以及健康、管理、公开 API 的成功、错误和 fallback `nosniff`。
