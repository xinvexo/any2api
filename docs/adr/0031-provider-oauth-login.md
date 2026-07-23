# ADR-0031: Provider OAuth2 交互登录与主动刷新

- 状态：Accepted
- 日期：2026-07-23
- 决策者：maintainer

## 背景

ProviderCredential 已支持多凭据、独立代理、并发限制、Vault 加密和模型发现，但上游认证此前只有 API Key。Codex 与 Claude 的官方 OAuth Client 使用固定的 localhost Redirect URI，而 any2api 可能运行在 Windows、macOS、容器或远程主机上，服务端不能假定自己可以接管管理员浏览器所在机器的回调端口。

同时，管理员已经明确要求 Provider Base URL 按填写值原样作为数据面目标；OAuth 不能借机把模型或推理请求改写到另一个 Base URL。

## 决策

- Codex 与 Claude 都通过专用管理入口创建普通 `ProviderCredential`，其 `credential_kind=oauth2`；不建立第二套账号表或调度路径。
- 服务端生成 256 位随机 session、state 和 PKCE verifier。OAuth session 只保存在内存，最多同时 64 个，10 分钟过期，callback exchange 无论成功失败都只能消费一次。
- Provider Driver 固定各自的 authorize、token、client ID 和 localhost Redirect URI：
  - Codex：`http://localhost:1455/auth/callback`
  - Claude：`http://localhost:54545/callback`
- Web 打开授权页面。浏览器完成授权后即使 localhost 页面无法连接，管理员也只需把地址栏中的完整 callback URL 粘贴回当前抽屉；服务端严格校验 Redirect URI、state、Provider、Endpoint 版本、实际代理版本和 PKCE。
- OAuth authorize/token Endpoint 是 Provider 固定认证目标；模型发现和推理请求仍严格使用管理员填写的 Provider Base URL，不自动覆盖、不重写 authority。
- Token exchange 和 refresh 使用 Credential 选择的代理；绑定 `DIRECT` 时继承全局代理，专属代理失败不回退。
- access token、refresh token、ID token、过期时间和账号元数据立即转换为 Provider 专用版本化 Secret，通过现有 Vault AEAD 加密保存。普通管理响应只返回 Credential ID 和可选的脱敏账号信息，不返回 Token。
- API Key 创建端点明确拒绝 `credential_kind=oauth2`；API Key 轮换也不能替换 OAuth Secret。OAuth 元数据可以通过普通 Credential PATCH 编辑，但 CredentialKind 不可变。
- 后台刷新任务每 60 秒扫描已启用的 OAuth Credential，在过期前 5 分钟刷新。刷新请求由 Provider Driver 构造、Transport 执行；成功后通过串行 ConfigPublisher 和 `secret_version` CAS 持久化，新 generation 保留已选择模型。
- 单节点进程只有一个刷新 Worker，因此当前不叠加每 Credential 的第二层刷新锁。`secret_version` CAS、防重复 Worker 和串行发布共同阻止旧刷新结果覆盖管理员的新凭据。
- Provider 已返回新 Token 但持久化失败时，当前 generation 立即进入 fail-closed 的 `auth_error`，停止接收新请求并持续记录脱敏告警；管理员需要恢复存储后重新登录。刷新状态不跨重启恢复。
- OAuth session、callback URL、authorization code 和 Token 不进入日志、URL、React Query、Mutation Cache、localStorage 或一次性回执。Provider OAuth JSON 导入仍是未来独立能力，本次不实现。

## 备选方案

- 在 any2api 内监听固定 localhost 回调端口：无法覆盖服务端与管理员浏览器不在同一机器的情况，也容易与现有 Codex/Claude CLI 冲突。
- 直接导入第三方 OAuth JSON：来源 Schema 不稳定，扩大原始 Secret 上传面，且不符合当前“登录后直接拉模型”的交互目标。
- 把 OAuth 数据面 Base URL 固定为官方地址：会违反管理员配置什么就访问什么的 Endpoint 不变量，也会破坏兼容服务和本地网络场景。

## 后果

- 管理员可以在 Provider 行直接选择 OAuth 登录，授权成功后沿用现有模型发现和勾选流程。
- Codex OAuth Credential 要访问 ChatGPT Codex 数据面时，管理员通常需要把 Base URL 配为 `https://chatgpt.com/backend-api/codex`；系统只提示，不自动修改。
- 固定 localhost Redirect URI 会让浏览器出现一次预期的“无法访问”页面；粘贴完整 URL 是跨机器部署下的稳定折中。
- OAuth refresh 会增加少量后台网络请求和配置 revision 变化，但不持久化刷新锁、队列或 session。

## 验证

- Provider 单元测试覆盖 authorize URL、PKCE token request、Token 解析、OAuth 认证头、字段继承和 Debug 脱敏。
- Runtime 测试覆盖 state mismatch、session 过期、单次消费、到期刷新、Secret generation 轮换和模型保留。
- Storage 测试覆盖 OAuth Credential 的 migration、Vault 往返、无 Secret 尾号和刷新 CAS。
- Admin 契约测试覆盖完整 start/exchange、API Key 入口拒绝伪造 OAuth、响应不返回 Token以及 OAuth 禁止 API Key 轮换。
- React 契约与组件测试覆盖 OAuth 登录、callback 不进入 URL/Query/Mutation Cache、成功后自动进入模型选择。
