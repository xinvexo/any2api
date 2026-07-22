# any2api 实施状态

> 最后更新：2026-07-22
> 用途：简要记录已经完成的代码、当前边界和下一步顺序。架构真相仍以根目录 `ARCHITECTURE.md` 为准。

## 当前状态

- 当前阶段：阶段 4 可靠性、RequestLog 生命周期、精确 Token 遥测、运行态负载均衡观察页、统一浏览器 E2E、管理员凭据维护、有界优雅停机与单二进制 Web 资源切片已经完成；API Key 首版主链可运行，OAuth2 仍待后续阶段实现。
- 最近完成：React dist 已作为机器生成资源编译进 Rust 二进制；默认启动不再依赖工作目录或外部 `web/dist`，显式 `ANY2API_WEB_DIR` 继续供开发和测试覆盖。
- 阶段 0 基线：`6b7d00f chore: scaffold any2api phase 0`。
- ProviderEndpoint 切片：`08e4913 feat: add provider endpoint configuration`。
- Secret Vault 切片：`e71b8b9 feat: add versioned secret vault`。
- ProviderCredential 切片：`f3ca1fc feat: add provider credential management`。
- Credential Runtime 切片：`bc71133 feat: add credential runtime capacity`。
- Credential Auth Material 切片：`fbfc6ef feat: load credential auth material`。
- Proxy Transport 切片：`33f9f2d feat: add proxy transport manager`。
- Model catalog 切片：`354a431 feat: expose published model catalog`。
- 同协议 JSON 切片：`c83d6b0 feat: add same-protocol json execution`。
- 上一切片提交主题：`feat: add bounded graceful shutdown`。
- 本切片主题：内嵌 React 资源与单二进制发布。

## 已完成

### 阶段 0 工程基线

- Rust 模块化 Workspace、Axum/Tokio 入口、SQLite Migration、`ArcSwap` 快照和 Registry 骨架。
- Provider/Protocol Registry 契约、React/Vite/Tailwind Web、响应式顶部导航和 light/dark/system 主题。
- CI、`cargo-deny`、格式/Lint/测试/构建以及 `xtask architecture-check` 门禁。

### 代理配置切片

- `ProxyProfile`、`ProxyAddress`、`ProxyConfiguration` 领域模型与结构化校验。
- 固定 nil UUID 的内置 `DIRECT`；不可删除、修改或禁用。
- HTTP、SOCKS5 地址配置，以及 `Credential DIRECT → 全局代理` 的领域解析规则。
- SQLite `proxy_profiles` 与 `proxy_settings`，包含 DIRECT、全局引用、启用状态和 singleton 防篡改约束。
- RAII `BEGIN IMMEDIATE` Repository；CRUD、全局代理、revision 冲突、无变化不增版和取消安全。
- `PublishedSnapshot` 已承载代理配置；唯一 `ConfigPublisher` 路径串行执行事务、无失败 reconcile、单次快照替换和 epoch 通知。
- 管理 API：
  - `GET /api/admin/proxies`
  - `POST /api/admin/proxies`
  - `PATCH /api/admin/proxies/{id}`
  - `DELETE /api/admin/proxies/{id}?expected_revision=N`
  - `POST /api/admin/proxies/{id}/set-global`
- 管理写请求使用 `expected_revision`，错误返回稳定 JSON code，不向响应泄露 SQLite 细节。
- 单管理员认证尚未实现前，管理 API 强制要求实际 TCP 对端为 loopback；HTTP/HTTPS 远程管理仍在后续切片实现。
- React“代理”页面接入真实 API：全局代理、列表、URL 驱动编辑器、创建/编辑/删除、revision 自愈和响应式窄屏布局。
- 真实浏览器完成桌面与 390px 窄屏验证，覆盖新增代理、切换全局代理、deep link、焦点和无水平溢出。

### ProviderEndpoint 配置切片

- 新增强类型 `ProviderEndpoint`、`ProviderBaseUrl` 与 `ProviderEndpointConfiguration`；首版只接受 Codex/`openai_responses` 和 Claude/`anthropic_messages` 配对。
- Base URL 保存为固定 HTTPS/HTTP origin 加可选路径前缀；拒绝 query、fragment、userinfo、非 HTTP(S) scheme、空 host、零端口和路径穿越片段。
- `allow_insecure_http` 与 `allow_private_network` 按 Endpoint 独立授权，默认均为关闭；字面私网、loopback、link-local、metadata 和本地命名空间在未授权时拒绝。
- 新增 SQLite `provider_endpoints` migration、配置仓储 CRUD、全局 revision 冲突保护，并纳入 `PublishedSnapshot` 的原子发布。
- 更新 Endpoint 时额外校验原始 `config_version`；全局 revision 刷新后，旧草稿不能静默覆盖已被其他操作修改的 Endpoint。
- 新增 loopback-only 管理 API：`GET/POST /api/admin/provider-endpoints`、`PATCH/DELETE /api/admin/provider-endpoints/{id}`。
- 新增 Provider Web 页面，支持 URL 驱动编辑、风险开关提示、失效 revision 自愈、草稿冲突保护和窄屏布局。
- 真实浏览器完成桌面与 390px 窄屏验证，覆盖创建 Claude 私网 HTTP Endpoint、两项授权、重启读取、deep link、历史返回、焦点和无水平溢出。
- 重启验证确认 SQLite revision 与 Endpoint 配置会重新读取，而 `scheduler_epoch` 按约束从 `0` 开始，不恢复旧运行态。
- 本配置切片当时不进行网络 I/O；DNS 最终地址校验、重定向限制和 Transport 连接绑定已在后续 Proxy Transport 切片完成。

### Secret Vault 切片

- 新增数据库外的版本化 `master-key.json`；默认位于数据目录，可用 `ANY2API_MASTER_KEY_FILE` 指向受保护文件或容器 Secret 挂载。
- 仅在 SQLite 尚无 Vault 元数据时允许首次自动生成 256 位主密钥；文件使用 create-new 语义，已有文件不会被覆盖。
- 首版固定使用 XChaCha20-Poly1305，每个密文信封使用独立的 192 位随机 nonce，并保存 envelope、algorithm、key、AAD 版本。
- AAD 使用固定二进制编码绑定记录 ID、Secret 类型、Provider/Credential 类型和 Credential Secret 版本，防止密文跨记录、跨 Provider 或旧版本回放。
- SQLite migration 保存加密校验哨兵；后续启动必须使用同一主密钥成功解密，缺失文件、错误 Key、未知版本、篡改或认证失败都会在监听端口前终止启动。
- Unix 主密钥文件创建为 `0600` 并拒绝 group/other 权限；Windows 依赖数据目录或容器挂载继承的用户 DACL。
- `SqliteStore` 持有已验证的 `SecretVault`，后续 Credential 和代理密码仓储只能通过这一稳定 API 加解密。
- 单元与 Storage/契约测试覆盖随机 nonce、重启解密、错误 AAD、篡改、缺失/错误主密钥、同路径保护和脱敏 Debug。
- 代理密码现通过独立 `proxy_passwords` Vault 密文记录保存；主密钥轮换/恢复仍不实现，首版不提供内建备份容灾。

### ProviderCredential API Key 配置切片

- 新增 `ProviderCredential`、`ProviderCredentialConfiguration`、`MaxConcurrency` 和版本化 Secret 指纹领域模型；首版只接受 `api_key`，最大并发范围为 `1..=10_000`。
- 新增 SQLite `provider_credentials` migration；API Key 通过 Secret Vault 加密保存，AAD 绑定 Credential、Provider、Kind、Schema 与 Secret 版本，启动加载会解密并校验指纹。
- Credential 支持创建、元数据编辑、Secret 轮换和删除；`config_version`、`secret_version` 与 `credential_generation` 按 ADR-0003 的矩阵独立递增，无变化更新不增加 revision。
- Credential 可绑定 DIRECT、HTTP 或 SOCKS5 代理；删除被引用的 Proxy/Endpoint 会返回稳定冲突。Endpoint Base URL 改变时增加所有子 Credential 的 generation，已有 Credential 时禁止修改 Provider/协议身份。
- `PublishedSnapshot` 已承载脱敏 Credential 配置；`ConfigPublisher` 拆为发布流程、命令分发、错误映射与 Secret 包装，所有配置仍通过同一串行事务和单次快照切换发布。
- 新增管理 API：
  - `GET/POST /api/admin/provider-endpoints/{endpoint_id}/credentials`
  - `PATCH/DELETE /api/admin/provider-credentials/{credential_id}`
  - `POST /api/admin/provider-credentials/{credential_id}/rotate-secret`
- Credential 响应只包含指纹与可选尾号并设置 `Cache-Control: no-store`；普通 PATCH 不接受 Secret，创建和轮换也不回显 API Key。
- 新增 `/providers/:endpointId` Web 详情页，支持多 API Key、代理选择、最大并发、启停、独立轮换、删除、deep link 和一次性本地回执。
- 真实浏览器完成桌面与 390px 窄屏验证，覆盖 Endpoint → API Key deep link、DIRECT 继承全局代理显示、一次性回执、长内容布局和无水平溢出。
- Secret 创建/轮换不经过 React Query Mutation Cache；测试确认关闭回执后 API Key 不存在于 URL、DOM、Query Cache 或 Mutation Cache。
- Storage、Runtime、HTTP 契约和 Web 测试覆盖持久化重启、版本冲突、重复标签、引用保护、密文篡改、响应脱敏和 metadata PATCH 不携带 Secret。
- 本配置切片本身不包含真实上游连通性、并发 Permit、负载均衡选择或网络转发；这些能力已在后续 Runtime、Credential Auth Material 和 Proxy Transport 切片中接入。

### Credential Runtime 容量切片

- 新增稳定的 `CredentialRuntimeHandle`；同一 Credential ID 的 `in_flight` 与动态并发上限跨配置 revision 复用，不会在热更新时重置为零。
- `max_concurrency` 与 `in_flight` 打包在同一个原子状态中，动态降限与并发获取共享同一 CAS 线性化点；降限不取消已有请求，但在占用回落前禁止新增 Permit。
- 新增 RAII `ConcurrencyPermit`；正常完成、失败、取消或 Drop 都只释放一次容量，并推进统一 `scheduler_epoch`。
- 新增 `CredentialGenerationRuntime` 与 Snapshot 固定绑定；Secret 轮换、重新启用或 Endpoint 身份变化后，新请求使用新 generation，旧请求仍持有旧 generation，迟到结果不会天然混入新代际。
- `PublishedSnapshot` 现在由持久化配置和稳定 `RuntimeRegistry` 一起编译，每个已发布 Credential 都有对应运行时绑定；删除 Credential 后，新 Snapshot 立即移除候选，旧绑定标记 retired 并由现存请求自然释放。
- 新增 `select_and_try_acquire`：只返回已经取得 Permit 的 Credential，最低负载率使用整数交叉相乘，CAS 竞争失败后重新完整选择，相同比例由调用方提供的轮询序号打破平局。
- 模块测试覆盖 64 路并发竞争不超限、动态升降限、Permit 释放、generation 固定、retired 生命周期和精确负载率选择。
- 契约测试覆盖真实 SQLite 发布链路中的容量复用、Secret rotation generation 隔离和删除时旧 Permit 生命周期。

### Credential Auth Material 装配切片

- Storage 配置读取现在返回“脱敏配置 + 已通过 Vault/AAD/指纹校验的 Secret 材料”两部分；Secret 材料不可 Clone，Debug 只显示 `[REDACTED]`。
- 所有有写入的配置事务在 Commit 前重新从同一事务视图加载完整配置，确保新建/轮换/Endpoint generation 变化返回的运行时材料与数据库版本一致；读取失败会回滚，不把半成品发布到 Runtime。
- Runtime 将 API Key 转换为 `ProviderSecret`，按 `credential_generation + secret_version + schema_version` 装配 generation；旧 Snapshot/Permit 继续持有旧 API Key，新 generation 不会覆盖旧请求。
- `ConcurrencyPermit::provider_credential_headers` 是认证注入的唯一公开入口，必须先取得并发 Permit 才能调用 Provider Driver 生成上游认证头；Gateway API Key 尚未进入该路径。
- Provider API Key 不进入管理 DTO、日志或 `PublishedSnapshot` 的原始字段；Runtime/Storage 的 Debug 输出均脱敏，Secret 只在内存代际对象中存在并由 `secrecy` 清理。
- 模块测试覆盖 generation 轮换、旧/新 Secret 隔离和 Debug 脱敏；Storage 测试覆盖写入、重启解密和材料脱敏；契约测试覆盖真实发布、重启后重新装配、Permit 认证头和 scheduler epoch 从零开始。

### Proxy Transport 切片

- 新增基于 `reqwest` + Rustls 系统证书根的 `ReqwestTransportManager`，支持 DIRECT、HTTP 和 SOCKS5h；Client 禁用系统代理、Cookie Store、自动重定向和 `reqwest` 内建协议重试。
- Transport 只执行 Runtime 已解析的实际 `ProxyProfile`。显式 HTTP/SOCKS5 失败直接返回类型化错误，不存在回退全局代理或本机 DIRECT 的代码路径。
- `PublishedSnapshot::resolved_proxy_for_credential` 固定实现 `Credential DIRECT -> 全局代理 -> 本机 DIRECT`，后续数据面不重复解释代理继承规则。
- Client 按代理 ID/版本/类型与连接超时、TLS/HTTP 策略、池参数和池策略版本组成的完整 key 复用连接池；缓存使用有界 LRU，代理或网络策略热更新产生新 Client 代际，旧请求继续持有旧 Client。
- 请求 Body 使用 `Bytes`，响应 Body 是异步字节流；`TransportRequest` Debug 不显示 Header 内容或 Body 内容，错误消息不包含代理地址、目标 URL 或认证字段。
- 连接前错误标记 `DefinitelyNotSent`，等待响应头和读取 Body 的不确定错误标记 `Ambiguous`；失败阶段与健康归因已经拆分为 `Endpoint/Proxy/Unattributed`，无法可靠区分的 CONNECT/SOCKS/目标 TLS 错误不会误开共享 Endpoint 或 Proxy 熔断器。
- DIRECT 请求会解析并校验全部 DNS A/AAAA 结果，未授权 Endpoint 遇到私网/回环地址时在发送 Provider Key 前拒绝；通过校验后把连接固定到本次已验证地址，同时保留原 Host/SNI。HTTP/SOCKS5 远端 DNS 仍属于显式受信代理边界。
- 模块网络测试覆盖真实 DIRECT、HTTP absolute URI、HTTPS 经 HTTP CONNECT 完成 TLS 隧道、SOCKS5h 远端 DNS、禁重定向、缓存代际、授权头 Debug 脱敏和 fail-closed。
- 公共 API 契约测试确认目标本机可达时，指定不可用代理仍然失败且目标端口没有收到连接。
- TransportManager 已装配进公开模型请求入口；代理用户名/密码与管理面代理探测已实现。HTTP/SOCKS5 默认继续使用远端 DNS 受信边界，开启 `upstream.strict_ssrf` 后改用本地解析与固定目标连接。

### 代理认证与管理探测切片

- `ProxyProfile` 只保存认证用户名、是否配置密码和 `authentication_version`；用户名拒绝控制字符与 HTTP Basic 分隔符 `:`，密码进入独立 `proxy_passwords` 表并使用现有 Secret Vault/AAD 加密。
- 认证状态实际发生设置、替换或清除时增加认证版本与代理 `config_version`；重复清除已关闭认证是 no-op。连续 PublishedSnapshot 使用新的 Transport Client 代际，旧请求继续持有旧认证材料。
- HTTP forward proxy、HTTPS CONNECT 和 SOCKS5 RFC 1929 用户名密码认证均通过 Transport sidecar 接入；密码不进入普通读取/响应 DTO、`TransportRequest.headers`、URL、日志、Debug 或缓存 key，Transport 只在受控边界写入代理认证材料。
- 管理 API 新增 `PUT/DELETE /api/admin/proxies/{id}/authentication`；普通代理响应只返回用户名、`password_configured` 和认证版本。
- 管理 API 新增 `POST /api/admin/proxies/{id}/test`，仅接受已有 Provider Endpoint，发送不带 ProviderCredential 的空 GET；普通上游 HTTP 状态视为链路可达，但 HTTP forward proxy 的 407 认证拒绝作为 `ProxyHandshake + Proxy` 失败。响应携带捕获的全局 revision、Proxy/Endpoint config version 与脱敏结果，不更新健康熔断。
- React 代理页增加认证局部表单、清除操作、Provider Endpoint 测试目标选择和行内延迟/状态结果；密码在提交成功或失败后都会清空，不进入查询缓存、URL、Storage 或持久化表单状态；探测结果按配置 revision 和目标 Endpoint 隔离，停用代理不提供测试操作。
- Domain、Storage、Transport、Runtime、HTTP 契约和 Web 测试覆盖用户名/密码边界、加密重启加载、重复清除 no-op、版本代际、HTTP/CONNECT/SOCKS5 认证、HTTP/CONNECT 407 代理拒绝、错误密码 Fail-Closed、DTO 脱敏、受限测试目标与管理探测。

### 严格 SSRF 本地 DNS 切片

- 新增并接受 ADR-0019，明确 DIRECT、HTTP forward、HTTPS CONNECT 与 SOCKS5 的本地/远端 DNS 信任边界、全部 A/AAAA 校验、目标固定、Host/SNI、DNS rebinding 与多地址失败语义。
- SettingRegistry 新增 `upstream.strict_ssrf`，默认 `false`、支持热更新；Web 设置页显示默认值、覆盖值和生效值，代理编辑器说明默认远端 DNS 与严格模式入口。该切片完成时 Registry 共 44 项，后续文件日志与停机设置使当前总数达到 49 项。
- DIRECT 始终执行本地解析、全部地址校验与 reqwest `resolve_to_addrs` 固定；严格模式关闭时 HTTP/SOCKS5 保持既有受信远端 DNS 行为。
- 严格模式下，HTTP forward 的 absolute-form authority 使用已验证 IP 并保留原始 Host；HTTPS CONNECT 向代理发送已验证 IP，隧道后继续使用原始 TLS SNI、证书名、HTTP Host 与 HTTP/2 authority。
- 严格 SOCKS5 使用 IP 地址类型，不发送目标域名；普通 HTTP 与 TLS 仍保留原始应用层 authority。代理自身地址仍是用户显式配置的信任边界。
- 固定目标 Hyper Client 与既有 reqwest Client 统一进入有界连接池缓存，key 包含代理配置代际、协议和完整已验证地址集合；DNS 结果变化会进入新 Client 代际，旧请求继续持有旧连接。
- 严格 CONNECT/SOCKS 的代理握手错误归 Proxy，隧道建立后的 TLS 错误归 Endpoint；CONNECT 407 归 `ProxyHandshake + Proxy + RejectedBeforeExecution`。专属代理仍 Fail-Closed，不回退全局代理或 DIRECT。
- Transport 真实网络测试覆盖 HTTP IP authority + Host、SOCKS5 IP target、CONNECT IP + SNI/Host、严格代理认证、407、TLS 归因和未授权私网在连接代理前拒绝；Settings HTTP 契约覆盖默认值、启用与恢复默认。

### 公开入口协议错误适配切片

- 新增并接受 ADR-0020；Gateway 鉴权失败、认证头冲突、公开 404 与 405 统一构造类型化 `PublicError`，再调用 Composition Root 注册的 ProtocolAdapter，Server 不维护第二套协议 JSON。
- `/v1/responses`、`/v1/responses/compact` 使用 OpenAI Responses envelope；`/v1/messages`、`/v1/messages/count_tokens` 与 `messages/` 子路径使用 Anthropic envelope；`/v1/models` 和无法可靠判断协议的未知公开路径默认使用 OpenAI 格式。
- `PublicErrorCode` 增加 `PublicApiNotFound` 与 `MethodNotAllowed`，由 Adapter 分别保持 404/405；所有入口错误继续返回 `x-request-id` 与 `Cache-Control: no-store`。
- `PublicRequestService` 改为 `AppState` 构造必填项，删除可选执行服务、`not_implemented` 分支和未装配时的简化 fallback；生产与测试 Router 共享唯一错误编码链。
- Protocol、Server 与真实 HTTP 契约覆盖 OpenAI/Anthropic 401、冲突认证头、404、405、路径方言选择、Request ID 和 no-store。

### ModelRoute 配置切片

- 新增 `ModelRoute`、`RouteTarget`、模型名校验和 `ModelRouteConfiguration` 领域模型；Route 是管理与持久化聚合根，Target ID 由服务端生成并在策略修改时保持稳定。
- 同一入口协议下公开模型名精确、区分大小写唯一；首版拒绝 wildcard、别名链、跨协议 Target 和不完整聚合，启用 Route 必须至少包含一个启用 Target。
- 新增 SQLite `model_routes` 与 `route_targets` migration；Route 元数据和完整 Target 集合在同一 `BEGIN IMMEDIATE` 事务中保存，Route `config_version` 覆盖所有聚合变化。
- 更新已有 Target 时要求携带当前 ID；只允许改变 `fallback_tier` 与 `enabled`。更换 Endpoint 或上游模型必须省略旧 ID，由服务端生成新 Target，避免硬粘性身份被静默改写。
- `ProviderEndpoint` 删除、Provider/协议身份修改均检查 ModelRoute 引用；ModelRoute 写入、删除和 Endpoint 引用保护均有 Storage、Runtime 与 HTTP 契约测试。
- 新增管理 API：
  - `GET /api/admin/model-routes`
  - `POST /api/admin/model-routes`
  - `PATCH /api/admin/model-routes/{id}`
  - `DELETE /api/admin/model-routes/{id}?expected_revision=N&expected_config_version=N`
- React `/routes` 已替换占位页：支持 URL deep link、完整 Route/Target 聚合编辑、同协议 Endpoint 过滤、三态主 tier 满载策略、响应式窄屏编辑、行内删除确认和 revision/config version 冲突保护。
- Web 测试覆盖响应解析、缓存代际、新建请求不携带客户端 Target ID、策略修改保留 ID、身份变化生成新 ID、延迟 Endpoint、深链刷新、移动端无效链接和旧保存回调。
- `/v1/models` 已接入同一认证快照，只返回启用 Route 的公开模型；跨协议同名稳定去重，不根据瞬时容量或健康状态删模型。
- ModelRoute 已接入 Codex Responses、Responses Compact、Claude Messages 的同协议 JSON/SSE 请求调度；会话粘性已由后续切片接入。

### GatewayApiKey 管理与公开鉴权切片

- 新增 `GatewayApiKey`、独立 HMAC verifier 和 `gateway_api_keys` SQLite migration；数据库只保存前缀、摘要、版本和状态，不保存明文 Token。
- Token 由 Runtime 使用 256 位 CSPRNG 生成，格式固定为 `a2k_v1_...`；创建和轮换成功后通过独立响应只返回一次明文。
- 管理 API 已接入：
  - `GET/POST /api/admin/gateway-api-keys`
  - `PATCH /api/admin/gateway-api-keys/{id}`
  - `POST /api/admin/gateway-api-keys/{id}/rotate`
  - `POST /api/admin/gateway-api-keys/{id}/revoke`
- 管理写入继续使用全局 revision、资源 config version 和轮换 token version CAS；撤销是终态，重复无变化不会推进 revision。
- `PublishedSnapshot` 现在携带 Gateway Key 配置和 HMAC verifier；鉴权、路由和 revision 使用同一快照，旧请求持有旧快照时不会被热更新中途改变。
- `/v1/models` 已返回 PublishedSnapshot 中的公开模型目录；Responses、Responses Compact、Messages 和 Count Tokens 已进入同协议 JSON 执行链；未知 `/v1/*` 不再回落到 SPA。
- `/v1/*` 支持 `Authorization: Bearer` 与 `x-api-key`，冲突 Token 拒绝；认证成功后剥离 `Authorization`、`x-api-key`、`Proxy-Authorization` 和 Cookie。
- React `/keys` 已替换占位页，支持创建、编辑、停用、轮换、撤销、deep link、响应式布局和一次性回执；明文 Token 不进入 Query/Mutation Cache、URL、Storage 或普通 DTO。
- Storage、Runtime、HTTP 契约和 Web 测试覆盖 Token 生命周期、快照隔离、header 剥离、SPA fallback 防护、冲突版本和缓存脱敏。

### 同协议 JSON 请求执行切片

- 新增强类型 `ProtocolOperation`，静态注册 Codex/Claude Provider Driver 与 Responses/Messages ProtocolAdapter。
- `/v1/responses`、`/v1/responses/compact`、`/v1/messages` 和 `/v1/messages/count_tokens` 已接入 Runtime：同一 PublishedSnapshot 内解析 Route、过滤启用 Endpoint/Credential/Proxy、原子取得对应 Permit，并调用现有 Transport。
- Codex Driver 追加 `responses`/`responses/compact`，注入 `Authorization: Bearer`；Claude Driver 追加 `messages`，注入 `x-api-key` 与 `Anthropic-Version: 2023-06-01`；Web 官方默认 Base URL 均包含 `/v1` 固定前缀。
- Adapter 保留未知 JSON 字段，只替换上游 `model`，并按白名单保留 Claude `anthropic-beta`；成功响应恢复公开模型名；上游非 2xx 返回协议兼容的脱敏错误 envelope。
- Runtime 执行链按请求规划、单次 Attempt 和响应处理拆分；生产文件均保持单一职责，没有把网络、调度和响应过滤重新塞进中央文件。
- 同负载轮询游标按 `ModelRoute + fallback tier` 隔离，并由 RuntimeRegistry 跨连续配置代际复用；删除后旧快照仍持有旧游标，新生命周期从零开始，避免跨 Route 偏斜和无效请求扰动。
- 未知 `/v1/*`、已知路径的方法错误和普通公开路由现在经过同一 GatewayApiKey 鉴权层；上游认证头、Cookie、固定及动态 hop-by-hop 响应头不会返回客户端。
- 该切片完成时只支持非流式 JSON；后续 SSE、QueueTicket 与会话粘性切片已补齐 Responses/Messages 流式执行、生成请求饱和等待和固定会话路由。自动重试、冷却和健康仍未实现。
- 模块测试与本地 HTTP 契约测试覆盖路径、认证头、客户端头剥离、出站 POST、模型替换、Compact 端点、敏感响应头过滤、fallback 鉴权、JSON 405 和 Route/tier 游标生命周期；Registry 契约从真实 App Composition Root 枚举全部 Adapter/Driver，避免生产漏注册仍通过测试。

### Count Tokens 辅助并发切片

- `/v1/messages/count_tokens` 使用 Claude 同协议路由、Provider API Key、代理和模型别名，但不占用生成请求的 `in_flight/max_concurrency`。
- 新增 RuntimeRegistry 级稳定 `AuxiliaryScheduler`；默认全局辅助并发为 `32`，单 Credential 为 `4`，强类型限制支持构造注入和运行时升降限，现由 scheduler SettingRegistry 接管覆盖值。
- 辅助选择与双层 Permit 取得在一个短 Mutex 临界区内完成；网络 I/O 不持锁。`AuxiliaryPermit` 固定取得时的 Credential generation，Drop 同时释放全局和单 Credential 计数并推进一次统一 epoch。
- 辅助容量满载时当前立即返回 Anthropic 429，不取得生成请求 QueueTicket，也不因 Route 的 `fallback_on_saturation` 溢出到下一 tier。
- Claude Count Tokens 上游明确返回 404 时分类为 operation unavailable，并转换为脱敏 Anthropic 404 `not_found_error`，供 Claude Code 回退本地 Token 估算；其他上游错误仍遵循当前 502 边界。
- 单元与契约测试覆盖全局/单 Credential 上限、与生成容量相互独立、动态降限、并发竞争、字段保留、模型改写、Provider 认证头、成功响应和 404 脱敏。

### 同协议 SSE 与 GuardedBody 切片

- Codex Responses 与 Claude Messages 的 `stream=true` 已通过同协议 Route、Provider API Key、代理和 Credential 生成并发槽位执行；Compact 与 Count Tokens 仍只允许 JSON。
- Provider Driver 显式声明 `TransportMode::Sse`，Runtime 根据请求的流式标记选择 JSON/SSE 能力，不用 JSON 能力替代流式能力。
- Protocol `SseDecoder` 覆盖任意字节切分、LF/CRLF、多行 `data:` 和 EOF 无尾空行，并限制单帧缓冲；Adapter 只改写顶层、`response.model` 与 `message.model`。
- Runtime 在返回下游响应头前预读首个完整事件；空流、首帧错误和首帧 Transport 错误仍返回协议 JSON 错误。返回后由 `GuardedBody` 持有上游流、Permit、取消标记和 CommitState。
- EOF、上游错误和客户端 Drop 都只释放一次 Permit；提交后的错误终止当前流，不自动切换 Credential、不拼接第二条流。
- 模块、Driver/Adapter 契约和真实 chunked HTTP 测试覆盖 Codex/Claude 首帧增量、模型别名、流式响应头、Permit 生命周期与错误边界。

### 生成请求有界 QueueTicket 切片

- 普通 Responses、Responses Compact 与 Messages 请求在当前执行 tier 全部满载时，使用快照级 `QueuePolicy` 选择等待或立即拒绝；默认等待 30 秒、最多 128 个等待请求、默认不进入 fallback tier。
- RuntimeRegistry 持有稳定 `QueueCoordinator` 和统一 `scheduler_epoch`；等待计数跨连续 PublishedSnapshot 复用但不持久化，进程重启后从零开始。
- QueueTicket 使用 RAII 计数，成功、超时、取消和错误路径都会归还等待名额；队列已满返回 `local_concurrency_limit`。
- 等待者先订阅 epoch，再重新执行完整 select-and-acquire；Permit 释放和配置发布推进 epoch，超时边界额外执行最后一次完整选择，避免丢失唤醒和边界误拒绝。
- Route 显式 `fallback_on_saturation` 覆盖全局默认；开启时主 tier 满载可检查下一 tier，关闭时在主 tier 等待或拒绝。Count Tokens 仍使用独立辅助并发并立即拒绝。
- QueuePolicy 按值捕获在 PublishedSnapshot 中；scheduler SettingRegistry 将已提交候选策略编译进新快照，旧请求不会在等待中途混用新 revision 的策略。
- 单元测试覆盖 Reject、queue-full、fallback、NoCandidates、Permit 释放重选、epoch 竞态、超时最终重选和取消计数；快照测试覆盖协调器复用与策略 revision 隔离。

### Scheduler SettingRegistry 切片

- `SettingDefinition` 集中定义六项 `scheduler.*` 的类型、默认值、范围、枚举值、应用模式、Web 分组和描述；Duration 的持久化/HTTP 单位固定为整数毫秒。
- SQLite `setting_overrides` 只保存用户覆盖值；写入、恢复默认、no-op 和 revision 冲突沿用串行 `ConfigPublisher`。未知 key、损坏 JSON、类型错误和越界覆盖 Fail-Closed，显式覆盖等于默认值仍保留。
- `PublishedSnapshot` 从已提交 `SettingsConfiguration` 捕获 QueuePolicy；稳定 AuxiliaryScheduler 热更新全局/单 Credential 上限，已有 Permit 不取消，新上限立即生效，发布只推进一次 scheduler epoch。
- 管理 API：`GET /api/admin/settings`、`PATCH /api/admin/settings/{key}`、`DELETE /api/admin/settings/{key}?expected_revision=N`。
- React `/settings` 按功能分组拆为管理页、行级表单、控件、草稿校验和展示映射；同时显示默认、覆盖、生效值，支持恢复默认、revision 冲突后保留草稿和响应式窄屏。
- Domain、Storage、Runtime、HTTP 和 Web 测试覆盖注册表元数据、持久化、缓存代际、冲突重试及真实 ConfigPublisher 辅助并发热更新。

### 会话粘性路由切片

- Protocol 解码新增 `IngressAffinity`，只按确认的显式来源提取会话：Codex `previous_response_id`、`X-Any2API-Session`、`X-Session-ID`、`Session-Id`/`Session_id`、Claude Code `metadata.user_id.session_id` 和 `conversation_id`；Count Tokens 不启用粘性，也不使用 Prompt Hash 猜测会话。
- 稳定 `RuntimeRegistry` 持有进程内 `AffinityRegistry`；启动时生成随机 HMAC-SHA256 会话键，并为软/硬用途做域分离。日志、Debug 与管理 DTO 不返回原始 Session ID 或 Response ID，进程重启后键和全部绑定直接清空。
- 软会话使用版本化 `Creating` 租约防止并发首请求选择不同 Credential；提交与 Drop 都会唤醒等待者。TTL 使用访问刷新，容量达到上限时只清理过期项，不引入后台恢复或外部缓存。
- `prefer` 先等待原 Credential，达到 `affinity.soft.prefer_wait_timeout` 后才撤销旧绑定并重新负载均衡；`strict` 与硬绑定只允许原 Credential、Route Target、上游模型和协议方言，缺失时返回 `session_binding_lost`。
- 每个 Credential Runtime 增加固定等待者计数；普通调度不会抢占该 Credential 新释放的槽位，固定会话仍使用全局有界 QueueTicket。优先级只影响同一个 Credential，不阻塞其他 Credential。
- Codex JSON 成功响应顶层 `id` 与 SSE `response.created.response.id` 在客户端可见前写入硬绑定；`Responses Compact` 只支持显式软会话，流式 Body 继续持有 Permit 到 EOF、错误、断连或 Drop。
- 新增六项统一设置：`affinity.soft.enabled`、`affinity.soft.mode`、`affinity.soft.ttl`、`affinity.hard.ttl`、`affinity.soft.prefer_wait_timeout`、`affinity.fixed_wait_timeout`。默认值、覆盖值和生效值均通过现有 SettingRegistry 热更新。
- 新增管理 API：`GET/DELETE /api/admin/affinity` 与 `DELETE /api/admin/affinity/credentials/{id}`；返回总量、Creating 数、按 Credential 统计和截断 HMAC 样本，并支持全部或按 Credential 清理。
- React `/affinity` 已替换占位页，展示运行时指标、Credential 分布、脱敏绑定样本和清理操作，并复用统一 SettingsManagement 只显示 `affinity.*` 设置。
- Runtime、Protocol、HTTP 契约与 Web 测试覆盖并发 Creating、租约唤醒、TTL、身份冲突、重启空状态、固定等待优先、Codex JSON/SSE 硬续接、Claude 软粘性、prefer 重绑、strict 不切换、未知旧 Response ID、管理清理和设置保存/恢复。
- 真实浏览器完成 1440 桌面与 390×844 窄屏验证；桌面导航、移动菜单、自然滚动、设置表单和无水平溢出均通过，页面控制台无错误。

### 可靠性与预提交重试切片

- `ProviderDriver::classify_error` 现在返回强类型 `UpstreamErrorClassification`，Codex/Claude 分别解析受限错误 envelope；认证、权限、额度、限流、模型不可用、操作不可用和临时故障不再折叠为一个上游错误。
- 标准 `Retry-After` 支持 delta-seconds 与 HTTP-date；无效值被忽略，公开响应只返回规范化秒数，不回显原始 Header 或上游正文。
- Credential generation 保存认证健康与模型冷却；429/模型错误只影响当前 Credential+模型，401 使当前 generation 进入永久 `auth_error`，权限/额度使用 Credential 级冷却。
- Endpoint 与 Proxy 按配置代际持有独立熔断器，支持真正的滑动失败窗口、Closed/Open/HalfOpen 和受限探测；DIRECT 网络失败归入 Endpoint，Provider 429/5xx 不会惩罚代理。
- HalfOpen 健康 Permit 在预检查后发生并发竞争时，生成与辅助选择都会释放当前容量 Permit、移除竞态候选并继续尝试同 tier 其他候选。
- 冷却和熔断到期通过统一 scheduler epoch 唤醒现有 QueueTicket；所有健康状态只在内存中存在，热更新与进程重启不会恢复旧运行态。
- 普通 JSON 请求改为显式有界多 Attempt：每次失败先发布健康状态、释放 Permit，再按总 Attempt、Credential 切换、同 Credential、绝对耗时和 RetrySafety 预算决定退避与重选；请求内排除已失败 Endpoint/Proxy/Credential，硬粘性与 `strict` 永不跨 Credential。
- `DefinitelyNotSent`、`RejectedBeforeExecution` 和 `Idempotent` 才允许自动重试；`Ambiguous`（包括 5xx、响应体读取失败和 SSE 首帧后的不确定错误）默认不重试，避免重复生成。
- 外部 `Retry-After` 最长按 30 天归一化，异常 `u64` 秒数和时间转换不会因 deadline 溢出而让冷却立即失效。
- Buffered 上游成功后的硬 ID 提取、egress 编码、公开模型恢复和粘性提交错误会先结算健康成功、关闭 HalfOpen 探测，再释放 Credential Permit。
- SSE 首帧在下游提交前仍由 `GuardedBody` 验证；首帧读取失败按不确定结果结束当前请求，不拼接第二条流。首个下游字节提交后永久禁止切换上游。
- 新增运行时虚拟时间、熔断滑动窗口、健康代际隔离测试，以及真实发布快照上的连接前切换、Ambiguous 不重试、429 冷却/Retry-After、硬粘性不切换、Attempt/切换预算和 SSE 首帧边界契约测试。
- 为遵守文件职责门禁，`credential_runtime`、健康 Runtime、请求选择和上游 Attempt 已拆为 generation/capacity、credential/endpoint/proxy/attempt、fixed/generation/auxiliary、prepared/buffered/streaming/failure 等模块；架构检查不依赖临时 allowlist。

### 单管理员认证与远程 HTTP 管理切片

- 新增 SQLite `admin_credentials` singleton 表，只保存 Argon2id PHC 摘要；重复初始化使用数据库唯一约束保护，进程重启后可重新加载摘要。
- 首次管理员初始化仅允许 loopback `POST /api/admin/auth/setup`，并要求输入启动终端显示的 256 位一次性 Setup Token；Token 不持久化、不由 API 返回，成功后立即失效。也可在首次启动通过 `ANY2API_ADMIN_PASSWORD` 完成；已有摘要时环境变量不会在线轮换密码。
- 新增认证 API：`GET /api/admin/auth/session`、`POST /api/admin/auth/setup`、`POST /api/admin/auth/login`、`POST /api/admin/auth/logout`。
- 登录签发 256 位随机服务端会话和独立 CSRF Token；会话、登录失败窗口与 CSRF 状态只保存在内存。Cookie 固定为 `HttpOnly`、`SameSite=Strict`、`Path=/api/admin`，可信 HTTPS 连接额外设置 `Secure`。
- Setup/登录 Argon2id 使用随 blocking 任务存活的有界 Permit；请求取消不会放大并发哈希或跳过登录失败记账。
- 受保护管理写请求统一检查会话 Cookie 与 `X-CSRF-Token`；`GatewayApiKey` 仍不能登录管理面。未注入认证服务的嵌入测试 Router 只保留 loopback-only 门禁，正式 Composition Root 始终注入认证服务。
- 全部 `/api/admin` 响应统一设置 `Cache-Control: no-store` 与 `Vary: Cookie`，登出后不会从浏览器或共享反代复用旧配置响应。
- 新增 `admin.remote_enabled`、会话 idle/absolute timeout、登录失败窗口与最大失败次数五项 SettingRegistry 设置；Web 显示默认/覆盖/生效值并支持热更新。
- `ANY2API_TRUSTED_PROXY_CIDRS` 显式配置可信反代网段；只有命中网段的 TCP 对端才解析唯一且合法的 `X-Forwarded-For` 与 `X-Forwarded-Proto`。来源链从右向左剥离可信代理，缺头、重复头和客户端预置 loopback 欺骗均 Fail-Closed。
- React 新增首启 Setup、登录、会话恢复、登出、CSRF 自动注入和明文远程 HTTP 持续警告；远程登录前即提示密码传输风险，受保护请求收到 401 响应头时立即关闭管理面并清空 Query/Mutation Cache。
- 当前切片直接支持远程 HTTP 与外部 TLS 终止；内建 Rustls listener、标准 `Forwarded` 头解析和 OAuth2 JSON 上传仍未实现，管理员密码在线轮换已由 ADR-0024 完成。

### RequestLog 与 Attempt 有界遥测切片

- `/v1` 外层统一生成本地 Request ID，并覆盖所有公开响应的 `x-request-id`；通过 GatewayApiKey 鉴权并进入模型执行链后创建请求记录。
- 新增 `RequestLog`、`RequestAttempt` 与结果枚举，以及 SQLite `request_logs`/`request_attempts` 父子表；配置实体删除后历史外键自动置空，日志不参与启动恢复。
- 每个请求在内存中聚合全部 Attempt，结束时只进行一次同步 `try_send`；队列满、Writer 关闭或 SQLite 写入失败只计数并丢弃，不等待或阻塞数据面。
- 后台 Writer 使用小批量事务写入父子记录，并在空闲期也按保留期限或最大行数任一上限定时分批清理；日志设置随配置发布即时刷新，停机提供有限刷新窗口，不保存排队状态。
- Attempt 在健康结算后、Permit 释放前完成记录；最终 RequestLog 优先保留最后一次 Attempt 的精确错误分类，不把限流、认证、网络或代理错误折叠为通用 `upstream`。
- SSE 在首帧验证和软绑定提交成功后把记录责任交给 `GuardedBody`；EOF、提交后错误与客户端 Drop 只完成一次，首帧 Transport 错误保留 Network/Proxy 归因与 RetrySafety。
- 新增管理 API：`GET /api/admin/request-logs`、`GET /api/admin/request-logs/{request_id}`；React `/logs` 与 `/logs/:requestId` 展示最近请求、队列/丢弃指标和 Attempt 时间线。
- SettingRegistry 新增 `logs.request.enabled`、`logs.request.retention`、`logs.request.max_rows`、`logs.telemetry_queue_capacity`，Web 可查看默认/覆盖/生效值并恢复默认。
- Codex/Claude 协议级精确 usage 与内容首 Token 钩子已由后续切片接入；上游未返回、终止事件前断开或非流式无法精确计时时，对应字段仍保持 `NULL`而不猜测。
- 当前测试覆盖父子事务、保留清理、队列丢弃、Request ID、日志详情契约、Credential 切换后的多 Attempt 顺序、Attempt 预算耗尽、SSE EOF/提交后错误/客户端 Drop 的单次持久化、列表与详情成功/空态/错误态、DTO 解析、deep link 和敏感文本不展示。
- 窄屏布局除超长模型名结构回归外，已由统一 Playwright 套件在真实服务和 390×844 Chromium 中覆盖移动导航、请求日志页面与全局水平溢出。

### 真实服务浏览器 E2E 基础设施

- 新增独立 Playwright Chromium 套件；`pnpm test:e2e` 自行构建 any2api 与 Web、分配空闲 loopback 端口并启动真实 HTTP 服务。
- 每次执行使用独立系统临时数据目录、全新 SQLite/Vault 和固定测试管理员密码；不复用开发数据、主密钥、Cookie 或运行态，结束后清理隔离目录。
- 浏览器契约覆盖 `/settings` 登录前 deep link 在登录后保持、服务端 SPA 刷新、总览/代理/Provider/模型路由/网关密钥/请求日志核心页面直达和真实 API 就绪状态。
- 390×844 契约覆盖折叠导航打开、跳转后关闭、请求日志空态与 `documentElement.scrollWidth <= innerWidth`；全部用例同时收集未处理 page error 和 error 级 console 输出。
- E2E 只验证跨层共享行为，业务 CRUD、字段边界、Secret 和故障矩阵继续由更快的 Rust/HTTP/React 测试覆盖。CI 使用独立 `e2e` job 安装 Chromium，普通 Vitest 门禁不承担浏览器启动成本。完整决策见 `docs/adr/0022-browser-e2e-contract.md`。

### 负载均衡运行态检查切片

- 稳定 `CredentialRuntimeHandle` 增加 Relaxed 原子计数，分别记录普通生成/固定会话选中、辅助请求选中，以及满载、Credential+模型、Endpoint、Proxy 健康过滤事件；只有容量 Permit 与全部健康 Permit 都取得后才记录选中。
- `RuntimeRegistry::balancing_snapshot` 从当前 `PublishedSnapshot` 只读聚合 scheduler epoch、QueueTicket 等待数与策略、辅助并发容量、Credential 实时容量/固定等待者/辅助占用/计数和按 upstream model 分层的三类健康状态。
- 新增管理员只读 API `GET /api/admin/balancing`，复用既有管理员认证与 `Cache-Control: no-store`。响应只返回 ID、脱敏标签、启停状态和运行指标，不返回 URL、地址或 Secret；总可调度容量同时要求 Credential、Endpoint 与解析后的实际 Proxy 均启用。
- React `/balancing` 已替换占位页，展示总览与 Provider 汇总、Credential 负载进度、生成选择比例、分开的过滤原因和三层健康状态，并组合现有 `scheduler.*` 设置表单。刷新失败时保留最近一次有效数据，空配置有独立引导态。
- 计数只表示当前进程内的调度检查事件；等待重选、健康竞态或 CAS 竞争可使过滤次数高于客户端请求数，不能用于计费、额度或 RequestLog 替代。热更新复用 Credential 句柄时保留，删除 Credential 或进程重启后清空。
- Runtime 热路径测试覆盖普通/辅助成功选中、满载和 HalfOpen 健康竞态计数；HTTP 契约覆盖真实配置发布、实时 Permit 容量、停用实际代理对可调度容量的影响、标签与 no-store；Web 覆盖严格 DTO、空态、分层健康和刷新失败保留旧数据。完整决策见 `docs/adr/0023-balancing-runtime-inspection.md`。

### 本地文件日志轮转切片

- SettingRegistry 新增 `logs.file.level`、`logs.file.retention` 与 `logs.file.max_total_size`，默认分别为 `info`、`7d` 与 `256 MiB`；该切片完成时 Registry 共 47 项，后续停机设置使当前总数达到 49 项，Web 显示默认值、覆盖值和生效值并支持恢复默认。
- Composition Root 在读取 SQLite 当前配置后一次性安装控制台与文件 tracing layer；控制台继续使用 `RUST_LOG`，文件层只服从 `logs.file.level`。
- 本地日志写入 `<data-dir>/logs` 下的 JSONL 分段文件，使用 `tracing-appender` 的有界丢弃式非阻塞队列和独立写线程；请求线程不等待文件系统，进程结束时 Guard 尽力刷新已经入队的日志。
- 分段同时按 UTC 日期与大小轮转；单段目标上限为总容量八分之一与 `32 MiB` 的较小值。关闭分段先按保留期限清理，再从最旧文件开始按总容量清理；活跃文件和非 any2api 命名文件不会被删除。
- `ConfigPublisher` 的日志发布特例已收敛为窄 `LoggingSettingsReconciler`；提交后的无失败阶段只同步更新 RequestTelemetry 与文件日志内存策略，不执行文件 I/O 或建立第二套配置系统。
- App 单元测试覆盖动态级别、合法 JSONL、大小/日期轮转、期限/容量清理、活跃文件与非托管文件保护；管理契约覆盖三项设置的默认、覆盖、持久化和恢复默认。完整决策见 `docs/adr/0021-bounded-local-file-logging.md`。

### SSE PrecommitBudget 切片

- SettingRegistry 新增 `stream.precommit.max_bytes` 与 `stream.precommit.max_duration`，默认分别为 `256 KiB` 和 `5s`；Web 可查看默认/覆盖/生效值并恢复默认。
- 每个流式请求从当前 PublishedSnapshot 捕获不可变预算；旧请求不会在等待首事件期间混入新 revision 的配置。
- `GuardedBody` 在下游响应头提交前使用配置 deadline 约束等待、分帧、协议解码、模型恢复与必要的硬/软粘性提交，并用同一字节上限限制每个 SSE 帧；超限、超时、空流或无效首帧均在提交前失败并释放健康 Guard 与 Credential Permit。
- 解码器按当前帧容量增量消费 transport chunk，Runtime 每次只编码并排队一个事件；同一 chunk 的未消费 `Bytes` 零拷贝保留，不会在首事件交付前扩张成整批编码帧。后续帧超限时，已完成事件先交付，再以 Body 错误终止。
- 编码后的公开事件超过字节预算时仍返回公开上游错误，但按本地预算失败结算上游健康；Runtime 自行产生的超时按 `Unattributed` 结算，二者都不会误开 Endpoint 或 Proxy 熔断。
- `GuardedBody` 状态机、帧处理管线、完成/错误结算、预算与错误类型按职责拆分到独立模块；相关生产文件均保持在 300 行以内。
- Domain/Runtime 测试覆盖两项设置元数据、原始/编码后字节预算、deadline、同 chunk 顺序、单事件预缓冲、硬/软绑定超时、提交后停止计费和健康归因；真实 HTTP 契约覆盖字节超限、首事件等待超时与旧请求保持旧 PublishedSnapshot。

### 上游读取与 SSE 提交后空闲超时切片

- SettingRegistry 新增 `upstream.read_timeout` 与 `stream.postcommit.idle_timeout`，默认分别为 `15_000ms` 与 `60_000ms`，均允许 `1..=86_400_000ms`、支持热更新且不能用 `0` 禁用；该切片完成时 Registry 共 43 项，后续严格 SSRF、文件日志与停机设置使当前总数达到 49 项。
- `TransportRequest` 按请求快照携带 read timeout，不把它放进连接池 Client key。固定请求体开始被连接层消费后才启动响应头 timer，因此较短的 read timeout 不会取代 DNS、连接、代理握手或 TLS 的既有阶段边界。
- 等待响应头超时记录为 `AwaitHeaders + Ambiguous`；JSON、Compact、Count Tokens 与非成功 SSE 错误正文逐 chunk 收集时使用相同空闲时长，超时记录为 `ReadBody + Ambiguous`。DIRECT 归因 Endpoint，无法证明责任的代理路径归入 `Unattributed`。
- 成功 SSE 提交前只使用 `stream.precommit.max_duration`，不叠加通用 read timeout；首个下游帧交付时启动 post-commit idle timer，每个成功上游 chunk（包括不完整帧）重置，缓冲完整事件始终优先交付。
- 提交后 idle timeout 只返回 Body error 并终止当前流，不重试、不切换 Credential、不生成协议内错误事件，也不再次惩罚已按成功结算的 Endpoint/Proxy 健康；Attempt 记录为 `StreamError + Network + Ambiguous`。
- Runtime/Transport/HTTP 契约覆盖响应头停滞、连接阶段隔离、buffered body 停滞、成功 SSE 不混用 read timeout、idle timer 启动/重置、缓冲帧优先、Permit 单次释放、健康不受罚和提交后不启动第二条流。

### 管理员凭据维护切片

- 新增受保护的 `POST /api/admin/auth/password/rotate`，使用当前密码、新密码、现有管理员 Session 和 CSRF；当前密码错误返回独立 403，不触发 Web 的全局 401 会话过期处理。
- 管理员摘要通过 SQLite 单条 CAS 更新，成功后同步替换内存摘要、清空登录失败窗口和全部旧会话，并只为当前浏览器重签发 Cookie/CSRF；`ANY2API_ADMIN_PASSWORD` 仍只负责首次初始化。
- 登录从摘要验证到会话签发持有读锁，轮换持有写锁，因此旧密码登录不能跨轮换提交点留下有效会话；轮换任务脱离 HTTP 请求取消继续完成。
- React 设置页新增独立密码表单；密码只保存在组件局部状态，成功或失败后清空，不进入 URL、浏览器存储或 React Query Mutation Cache。
- 启动在打开 SQLite 前取得 `<data-dir>/any2api.instance.lock` 独占锁，第二个进程直接启动失败；锁随进程退出释放，不引入跨进程状态同步。
- Storage、Server、HTTP、Web 与 App 测试覆盖摘要 CAS、错误当前密码、旧会话撤销、当前会话重签发、新密码重启生效、CSRF 和实例锁释放。完整决策见 `docs/adr/0024-admin-password-rotation-and-instance-lock.md`。

### 协议级精确 Token 遥测切片

- Domain 新增强类型 `TokenUsage`，累计快照按字段覆盖而不相加；计数上限固定为 `Number.MAX_SAFE_INTEGER`，保证 SQLite 与 Web JSON 契约均可无损表达。
- Codex JSON/Compact 从顶层 `usage` 读取输入、输出、缓存读与缓存写；SSE 只从 `response.completed`/`response.incomplete` 的 `response.usage` 读取。
- Claude JSON 从顶层 `usage` 读取同类计数；SSE 按字段合并 `message_start.message.usage` 与 `message_delta.usage`，不把累计快照相加。
- 遥测只读取明确字段路径；负数、字符串、浮点或超界值按该字段未知处理，不会改变原始响应透传或使请求失败。
- `GuardedBody` 使用带内容标记的 PendingFrame；`first_token_ms` 从整个请求开始计时，只在第一个非空文本、reasoning 或工具参数增量真正 yield 时 first-write-wins。控制帧预读不计时，JSON 不伪造 TTFT。
- `/v1/messages/count_tokens` 的根层 `input_tokens` 仍是辅助结果，不写入生成 usage；客户端 Drop 不为补齐日志继续 drain 上游。
- React 请求日志详情页新增 TTFT 与四项 Token 计数；`NULL` 显示为“未记录”，与真实 `0` 严格区分。
- Registry 契约枚举实际注册的 Codex/Claude Adapter；Runtime/SQLite 契约覆盖 Responses、Compact、Messages JSON/SSE 与 Count Tokens 排除，真实管理 HTTP 列表/详情与 Web 测试覆盖非空值、缺失值和真实零值。完整决策见 `docs/adr/0025-protocol-token-telemetry.md`。

### 有界优雅停机切片

- 新增进程级 `ProcessLifecycle`，状态固定为 `Running / Draining / Forced`；请求与后台任务分别追踪，健康定时任务在 Draining 退出，配置发布和密码轮换在 Forced 取消异步 future。
- Server 最外层 Guard 覆盖完整 Handler 与响应 Body 生命周期；普通响应、SSE EOF/error、客户端 Drop 和 Forced 静默 Body 都通过 RAII 释放请求计数、QueueTicket、Permit 与上游连接。
- SettingRegistry 新增 `shutdown.request_grace_period` 与 `shutdown.finalize_timeout`，默认 `30s` 与 `5s`、当前共 49 项；Web 设置页支持默认/覆盖/生效值，总览显示 shutdown phase、活动请求与后台任务数。
- 停机信号到达时从当前 `PublishedSnapshot` 一次性捕获两项设置；配置热更新立即影响下一次停机，已经开始的停机不会混用后续 revision。
- RequestTelemetry Writer 纳入同一 Tracker；关闭 sender 后先排空，超时则 abort 并 await，禁止遗留脱管 SQLite Writer。SQLite Pool 随后显式关闭。
- Argon2 通过 Tracker 的 blocking 入口运行；请求或外层密码轮换 future 被取消后，blocking closure 仍保持计数直到真正返回，避免 Tokio Runtime Drop 无界等待被误判为已收尾。
- 二进制入口改为显式 Tokio Runtime，并在 Runtime 外持有实例锁。完整收尾后确认文件日志根 `Arc` 是最后所有者、释放有界 `WorkerGuard`、关闭 Runtime，最后释放实例锁；任一步失败都在持锁状态进入致命进程退出。
- App/Runtime/Server 测试覆盖自然 drain、信号时读取最新设置、Draining 拒绝新请求、Forced 静默 Body、Telemetry abort + join、blocking JoinHandle Drop 后继续追踪和最终收尾失败。完整决策见 `docs/adr/0026-bounded-graceful-shutdown.md`。

### 单二进制 Web 资源切片

- `app/any2api/web-assets` 保存由当前 Vite 版本生成的 HTML/JS/CSS 根资源；前端源码仍以 `web/src` 为真相，产物禁止手工编辑。
- App `build.rs` 只扫描已提交资源、要求存在 `index.html`，并在 `OUT_DIR` 生成排序后的 `include_bytes!` 清单；普通 Rust 构建不需要 Node，也不会修改工作树。
- Server 新增 `WebAssets`/`EmbeddedWebAsset` 边界。外部目录与内嵌实现共享 `/api`/`/v1`（含尾斜杠）隔离、deep link、缺失 `/assets/*` 404 和非 GET/HEAD 405；内嵌实现额外提供精确 Content-Type、HEAD、哈希资源一年 immutable 缓存与根资源 no-cache。
- `ANY2API_WEB_DIR` 未设置或为空时固定使用内嵌资源；只有显式路径才切换外部 `ServeDir`，用于 Vite 开发、定向契约和部署诊断。
- 生成目录按原始字节存储并拒绝符号链接/特殊文件；`build.rs` 只读取已提交资源，不调用 Node 或修改工作树。
- `pnpm build:embedded` 负责构建并同步产物；`pnpm check:embedded` 只读比较文件清单与字节，Web CI 和 E2E 均在不一致时失败。
- Playwright 从 Cargo JSON 构建消息取得本轮真实二进制，启动时清除宿主全部 `ANY2API_*` 配置并使用独立临时数据目录；登录、刷新 deep link、桌面核心页面和 390px 移动导航均已通过默认内嵌路径。
- Release 二进制复制到不含源码和 `web/dist` 的临时目录后，首页、`/settings`、哈希 JS、缓存头、缺失 asset 404、API 根隔离和未知 API 不回落 SPA 均验证通过。完整决策见 `docs/adr/0027-embedded-web-assets.md`。

## 当前边界

- DIRECT/HTTP/SOCKS5h 网络执行与连接池已接入公开 JSON/SSE 请求；代理认证和管理面代理测试已接入，健康熔断继续只由公开请求数据面驱动。
- ModelRoute 配置、管理面、公开 `/v1/models`、同协议 JSON/SSE 请求、普通生成请求有界排队、会话粘性和提交前多 Attempt 已实现。
- 当前代理支持 host/port 与 Vault 认证；HTTP/SOCKS5 默认使用远端 DNS，`upstream.strict_ssrf=true` 时统一改为本地解析、全部地址校验和固定目标连接。
- 当前实现 admin、affinity、scheduler、retry、cooldown、breaker、upstream、stream、request logging、file logging 与 shutdown 共 49 项 SettingRegistry。
- 远程反代必须先配置 `ANY2API_TRUSTED_PROXY_CIDRS`，并确认 `admin.remote_enabled=true`；未配置认证服务的测试/嵌入 Router 仍不能远程管理。
- 数据目录由进程级文件锁独占；管理员密码可在线轮换，成功后仅保留当前请求获得的新会话，其他旧会话立即失效。
- 运行态并发、生成请求等待、会话绑定、健康、冷却和熔断都只保存在内存；进程重启后容量、队列、会话和健康状态全部从零开始。
- Credential generation 已承载首版 API Key 认证材料及认证/模型健康；OAuth 刷新锁和 OAuth2 执行链路仍未实现。
- 当前 JSON/Compact/Count Tokens 与非成功 SSE 错误正文已使用统一上游 read timeout；成功 SSE 分别使用可配置 PrecommitBudget 与提交后 idle timeout。RequestLog/Attempt 已写入 SQLite，精确 Token Usage 与客户端可见流式 TTFT 已按协议契约采集；无法精确获取时保持 `NULL`。
- 负载均衡运行态 API 与 `/balancing` 页面已完成；容量、队列、选择/过滤计数及分层健康只读自当前进程内存，不持久化、不参与启动恢复。
- Gateway 鉴权失败、认证头冲突、公开 404/405 与已认证执行错误都由对应 Responses/Messages Adapter 编码；公开 Router 不再存在第二套简化 JSON。
- 正式运行默认从二进制内嵌 React 资源提供管理面；改变当前工作目录或删除源码树不会影响页面，外部 Web 目录必须通过 `ANY2API_WEB_DIR` 显式选择。

## 下一步

1. 补齐 GatewayApiKey `last_used_at` 节流遥测写入与 ProviderCredential 连通性测试/成功后清除 `auth_error` 操作；两者仍不得形成 Gateway Key 到 Provider Credential 的绑定。
2. 完成首版小契约收尾：Compact 本地必填 `input` 校验、关键调度 `tracing` 事件、并发模型/固定 fuzz corpus 门禁，以及远程管理的标准 `Forwarded` 与 Web 可见的监听/可信反代配置。
3. 补充 Unix 真实子进程 SIGTERM、成功停机后同数据目录立即重启等进程级 CI 契约；它们用于加固现有语义，不引入运行态恢复。
4. 上述 API Key 首版收尾后，再按独立切片实现可选内建 Rustls 与 Provider 专用 OAuth2 扩展；这些能力不阻塞当前主链。

## 验证结果

```text
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo test --locked --doc --workspace
cargo build --locked --release --workspace
cargo xtask architecture-check
cargo deny --offline check

cd web
pnpm typecheck
pnpm lint
pnpm test
pnpm build
pnpm check:embedded
pnpm test:e2e
```

本切片已通过完整门禁：Rust fmt、clippy、workspace test/doc test、release build 与 architecture-check 全部通过；`cargo deny --offline check` 使用本地 RustSec 缓存通过，仅报告基线已有的重复传递依赖 warning。Web typecheck、lint、build、内嵌产物一致性检查与 31 个 Vitest 文件的 81 项测试全部通过；`pnpm test:e2e` 在污染宿主 `ANY2API_*` 配置的条件下仍通过 3 项默认内嵌资源 Chromium 契约。复制到空目录的 Windows release 单文件验证了首页、deep link、哈希资源、缓存、缺失 asset 404 和 API/SPA 隔离。Vite 仍只有单入口 bundle 超过 500 kB 的既有提示。
