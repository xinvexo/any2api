# any2api 实施状态

> 最后更新：2026-07-28
> 用途：简要记录已经完成的代码、当前边界和下一步顺序。架构真相仍以根目录 `ARCHITECTURE.md` 为准。

## 当前状态

- 当前阶段：API Key 数据面、OpenAI 协议桥与 Images API、三 Provider OAuth2 核心链路、OAuth JSON 原子导入、三 Provider 额度查询、全局公开模型允许列表、可信客户端 IP 请求日志、完整 HTTP 系统日志、管理 Web 和真实二进制浏览器验收均已完成；剩余发布验收主要依赖真实上游账号与真实反代环境。
- 最近完成：接入 OpenAI Images 生成与编辑 API，支持 JSON、multipart 和 SSE，并为图片上传、base64 响应及长耗时请求设置独立缓冲和执行预算；同时保留文本链路既有限制。
- 阶段 0 基线：`6b7d00f chore: scaffold any2api phase 0`。
- ProviderEndpoint 切片：`08e4913 feat: add provider endpoint configuration`。
- Secret Vault 切片：`e71b8b9 feat: add versioned secret vault`。
- ProviderCredential 切片：`f3ca1fc feat: add provider credential management`。
- Credential Runtime 切片：`bc71133 feat: add credential runtime capacity`。
- Credential Auth Material 切片：`fbfc6ef feat: load credential auth material`。
- Proxy Transport 切片：`33f9f2d feat: add proxy transport manager`。
- Model catalog 切片：`354a431 feat: expose published model catalog`。
- 同协议 JSON 切片：`c83d6b0 feat: add same-protocol json execution`。
- OAuth JSON 导入切片：`235a1bd feat: import OAuth JSON accounts`。
- OAuth 额度扩展切片：`18f8e49 feat: expand OAuth quota support and organize modules`。
- Feature-first 收敛切片：`5f3d9ff refactor: complete feature-first crate organization`。
- 本切片主题：系统总览扁平化、RequestLog Token 累计与按时间/公开模型的调用图表。

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

- 新增强类型 `ProviderEndpoint`、`ProviderBaseUrl` 与 `ProviderEndpointConfiguration`；该初始切片只接受 Codex/`openai_responses` 和 Claude/`anthropic_messages` 配对，Grok 已由后续 ADR-0040 切片接入。
- Base URL 保存为固定 HTTPS/HTTP origin 加可选路径前缀；拒绝 query、fragment、userinfo、非 HTTP(S) scheme、空 host、零端口和路径穿越片段。
- ADR-0029 将 Base URL 确认为管理员受信任目标；HTTP、HTTPS、公网、loopback、局域网和容器网络地址直接接受，不再保存 `allow_insecure_http` 或 `allow_private_network`。
- 新增 SQLite `provider_endpoints` migration、配置仓储 CRUD、全局 revision 冲突保护，并纳入 `PublishedSnapshot` 的原子发布。
- Migration 0012 删除旧网络授权列；Domain、SQLite、管理 API 与 Web 不保留兼容字段或隐藏默认值。
- 更新 Endpoint 时额外校验原始 `config_version`；全局 revision 刷新后，旧草稿不能静默覆盖已被其他操作修改的 Endpoint。
- 新增 loopback-only 管理 API：`GET/POST /api/admin/provider-endpoints`、`PATCH/DELETE /api/admin/provider-endpoints/{id}`。
- 新增 Provider Web 页面，支持 URL 驱动编辑、失效 revision 自愈、草稿冲突保护和窄屏布局；Endpoint 表单只填写 Base URL，不再显示协议或地址类别授权开关。
- 浏览器与 React 测试覆盖直接创建 Claude 私网 HTTP Endpoint、重启读取、deep link、历史返回、焦点和无水平溢出。
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

- 新增 `ProviderCredential`、`ProviderCredentialConfiguration`、可选 `RequestsPerMinute` 和版本化 Secret 指纹领域模型；首版只接受 `api_key`，RPM 为空表示无限制，非空范围为 `1..=100_000`。
- 新增 SQLite `provider_credentials` migration；API Key 通过 Secret Vault 加密保存，AAD 绑定 Credential、Provider、Kind、Schema 与 Secret 版本，启动加载会解密并校验指纹。
- Credential 支持创建、元数据编辑、Secret 轮换和删除；`config_version`、`secret_version` 与 `credential_generation` 按 ADR-0003 的矩阵独立递增，无变化更新不增加 revision。
- Credential 可绑定 DIRECT、HTTP 或 SOCKS5 代理；删除被引用的 Proxy/Endpoint 会返回稳定冲突。Endpoint Base URL 改变时增加所有子 Credential 的 generation，已有 Credential 时禁止修改 Provider/协议身份。
- `PublishedSnapshot` 已承载脱敏 Credential 配置；`ConfigPublisher` 拆为发布流程、命令分发、错误映射与 Secret 包装，所有配置仍通过同一串行事务和单次快照切换发布。
- 新增管理 API：
  - `GET/POST /api/admin/provider-endpoints/{endpoint_id}/credentials`
  - `PATCH/DELETE /api/admin/provider-credentials/{credential_id}`
  - `POST /api/admin/provider-credentials/{credential_id}/rotate-secret`
- Credential 响应只包含指纹与可选尾号并设置 `Cache-Control: no-store`；普通 PATCH 不接受 Secret，创建和轮换也不回显 API Key。
- 新增 `/providers/:endpointId` Web 详情页，支持多 API Key、代理选择、可选 RPM、启停、独立轮换、删除、deep link 和一次性本地回执。
- 真实浏览器完成桌面与 390px 窄屏验证，覆盖 Endpoint → API Key deep link、DIRECT 继承全局代理显示、一次性回执、长内容布局和无水平溢出。
- Secret 创建/轮换不经过 React Query Mutation Cache；测试确认关闭回执后 API Key 不存在于 URL、DOM、Query Cache 或 Mutation Cache。
- Storage、Runtime、HTTP 契约和 Web 测试覆盖持久化重启、版本冲突、重复标签、引用保护、密文篡改、响应脱敏和 metadata PATCH 不携带 Secret。
- 本配置切片本身不包含真实上游连通性、RPM 预留、轮询选择或网络转发；这些能力已在后续 Runtime、Credential Auth Material 和 Proxy Transport 切片中接入。

### Credential Runtime RPM 切片

- 新增稳定的 `CredentialRuntimeHandle`；同一 Credential ID 的滚动 RPM 窗口与 `in_flight` 观测跨配置 revision 复用，不会在有限值热更新或 Secret 轮换时重置。
- 有限 RPM 使用精确滚动 60 秒时间戳窗口；选择 Credential 与预留一次上游 Attempt 名额在同一短锁内完成。改为无限制时清空窗口，再次启用时从空窗口开始。
- 新增 RAII `RoutingPermit`；正常完成、失败、取消或 Drop 都只结束一次 `in_flight` 生命周期，已预留 RPM 不归还并在 60 秒后自然到期。
- 新增 `CredentialGenerationRuntime` 与 Snapshot 固定绑定；Secret 轮换、重新启用或 Endpoint 身份变化后，新请求使用新 generation，旧请求仍持有旧 generation，迟到结果不会天然混入新代际。
- `PublishedSnapshot` 现在由持久化配置和稳定 `RuntimeRegistry` 一起编译，每个已发布 Credential 都有对应运行时绑定；删除 Credential 后，新 Snapshot 立即移除候选，旧绑定标记 retired 并由现存请求自然释放。
- 新增 `select_and_try_reserve`：按 Route tier 的稳定轮询顺序尝试候选，只返回已经预留 RPM 的 Credential；`in_flight` 不参与准入或排序。
- 模块测试覆盖多线程预留不超 RPM、精确 60 秒到期、动态升降/关闭限制、固定等待优先、generation 固定和 retired 生命周期。
- 契约测试覆盖真实 SQLite 发布链路中的 RPM 窗口复用、Secret rotation generation 隔离和删除时旧 Permit 生命周期。

### Credential Auth Material 装配切片

- Storage 配置读取现在返回“脱敏配置 + 已通过 Vault/AAD/指纹校验的 Secret 材料”两部分；Secret 材料不可 Clone，Debug 只显示 `[REDACTED]`。
- 所有有写入的配置事务在 Commit 前重新从同一事务视图加载完整配置，确保新建/轮换/Endpoint generation 变化返回的运行时材料与数据库版本一致；读取失败会回滚，不把半成品发布到 Runtime。
- Runtime 将 API Key 转换为 `ProviderSecret`，按 `credential_generation + secret_version + schema_version` 装配 generation；旧 Snapshot/Permit 继续持有旧 API Key，新 generation 不会覆盖旧请求。
- `RoutingPermit::credential_headers` 是认证注入的唯一公开入口，必须先完成 Credential 选择与 RPM 预留，才能调用 Provider Driver 生成上游认证头；Gateway API Key 不进入该路径。
- Provider API Key 不进入管理 DTO、日志或 `PublishedSnapshot` 的原始字段；Runtime/Storage 的 Debug 输出均脱敏，Secret 只在内存代际对象中存在并由 `secrecy` 清理。
- 模块测试覆盖 generation 轮换、旧/新 Secret 隔离和 Debug 脱敏；Storage 测试覆盖写入、重启解密和材料脱敏；契约测试覆盖真实发布、重启后重新装配、Permit 认证头和 scheduler epoch 从零开始。

### Proxy Transport 切片

- 新增基于 `reqwest` + Rustls 系统证书根的 `ReqwestTransportManager`，支持 DIRECT、HTTP 和 SOCKS5h；Client 禁用系统代理、Cookie Store、自动重定向和 `reqwest` 内建协议重试。
- Transport 只执行 Runtime 已解析的实际 `ProxyProfile`。显式 HTTP/SOCKS5 失败直接返回类型化错误，不存在回退全局代理或本机 DIRECT 的代码路径。
- `PublishedSnapshot::resolved_proxy_for_credential` 固定实现 `Credential DIRECT -> 全局代理 -> 本机 DIRECT`，后续数据面不重复解释代理继承规则。
- Client 按代理 ID/版本/类型与连接超时、TLS/HTTP 策略、池参数和池策略版本组成的完整 key 复用连接池；缓存使用有界 LRU，代理或网络策略热更新产生新 Client 代际，旧请求继续持有旧 Client。
- 请求 Body 使用 `Bytes`，响应 Body 是异步字节流；`TransportRequest` Debug 不显示 Header 内容或 Body 内容，错误消息不包含代理地址、目标 URL 或认证字段。
- 连接前错误标记 `DefinitelyNotSent`，等待响应头和读取 Body 的不确定错误标记 `Ambiguous`；失败阶段与健康归因已经拆分为 `Endpoint/Proxy/Unattributed`，无法可靠区分的 CONNECT/SOCKS/目标 TLS 错误不会误开共享 Endpoint 或 Proxy 熔断器。
- DIRECT 请求会解析 DNS A/AAAA 结果并把连接固定到本次地址集合，同时保留原 Host/SNI；管理员配置的公网、私网或回环地址均可访问。HTTP/SOCKS5 远端 DNS 仍属于显式受信代理边界。
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

- 新增并接受 ADR-0019，明确 DIRECT、HTTP forward、HTTPS CONNECT 与 SOCKS5 的本地/远端 DNS 信任边界、A/AAAA 解析、目标固定、Host/SNI、DNS rebinding 与多地址失败语义；地址类别授权由 ADR-0029 移除。
- SettingRegistry 新增 `upstream.strict_ssrf`，默认 `false`、支持热更新；Web 设置页显示默认值、覆盖值和生效值，代理编辑器说明默认远端 DNS 与严格模式入口。该切片完成时 Registry 共 44 项；加入后续设置、由 ADR-0037 删除两项辅助容量并新增 `models.allowed` 后，当前总数为 50 项。
- DIRECT 始终执行本地解析与 reqwest `resolve_to_addrs` 固定；严格模式关闭时 HTTP/SOCKS5 保持既有受信远端 DNS 行为。
- 严格模式下，HTTP forward 的 absolute-form authority 使用已验证 IP 并保留原始 Host；HTTPS CONNECT 向代理发送已验证 IP，隧道后继续使用原始 TLS SNI、证书名、HTTP Host 与 HTTP/2 authority。
- 严格 SOCKS5 使用 IP 地址类型，不发送目标域名；普通 HTTP 与 TLS 仍保留原始应用层 authority。代理自身地址仍是用户显式配置的信任边界。
- 固定目标 Hyper Client 与既有 reqwest Client 统一进入有界连接池缓存，key 包含代理配置代际、协议和完整解析地址集合；DNS 结果变化会进入新 Client 代际，旧请求继续持有旧连接。
- 严格 CONNECT/SOCKS 的代理握手错误归 Proxy，隧道建立后的 TLS 错误归 Endpoint；CONNECT 407 归 `ProxyHandshake + Proxy + RejectedBeforeExecution`。专属代理仍 Fail-Closed，不回退全局代理或 DIRECT。
- Transport 真实网络测试覆盖 DIRECT 私网访问、HTTP IP authority + Host、SOCKS5 IP target、CONNECT IP + SNI/Host、严格代理认证、407 和 TLS 归因；Settings HTTP 契约覆盖默认值、启用与恢复默认。

### 公开入口协议错误适配切片

- 新增并接受 ADR-0020；Gateway 鉴权失败、认证头冲突、公开 404 与 405 统一构造类型化 `PublicError`，再调用 Composition Root 注册的 ProtocolAdapter，Server 不维护第二套协议 JSON。
- `/v1/responses`、`/v1/responses/compact` 使用 OpenAI Responses envelope；`/v1/messages`、`/v1/messages/count_tokens` 与 `messages/` 子路径使用 Anthropic envelope；`/v1/models` 和无法可靠判断协议的未知公开路径默认使用 OpenAI 格式。
- `PublicErrorCode` 增加 `PublicApiNotFound` 与 `MethodNotAllowed`，由 Adapter 分别保持 404/405；所有入口错误继续返回 `x-request-id` 与 `Cache-Control: no-store`。
- `PublicRequestService` 改为 `AppState` 构造必填项，删除可选执行服务、`not_implemented` 分支和未装配时的简化 fallback；生产与测试 Router 共享唯一错误编码链。
- Protocol、Server 与真实 HTTP 契约覆盖 OpenAI/Anthropic 401、冲突认证头、404、405、路径方言选择、Request ID 和 no-store。

### Credential 模型发现与选择切片

- ADR-0028 取代了普通用户手工编辑 `ModelRoute`/`RouteTarget` 的流程；`provider_credential_models` 是配置真相来源，公开模型名首版固定等于上游模型名。
- 保存或更换 Provider API Key 后，Web 使用该 Credential 的实际 Endpoint、认证材料和代理请求 `GET /models`，支持搜索、全选、清除、重新拉取和勾选保存。
- 新增 `PUT /api/admin/provider-credentials/{id}/models`；模型集合、Credential `config_version`、内部 Route/Target 表、全局 revision 和 PublishedSnapshot 在同一个串行发布中更新。
- `ModelRoute`/`RouteTarget` 保留为数据面内部物化结构，Route ID 由协议和模型、Target ID 由 Route 和 Endpoint 确定性派生；全部 Target 首版使用 tier 0。
- 已删除 `/api/admin/model-routes` CRUD、Runtime 手工 Route 发布方法、React `/routes` 页面及整套模型路由编辑 feature，不保留别名和手工 tier 的兼容入口。
- 同一 Endpoint 下不同 Credential 可以保存不同模型集合；候选构建再次校验 `Credential + upstream_model`，不会把一把 Key 的模型权限套到另一把 Key。
- SQLite migration 会把旧 Target 按 Endpoint 展开为 Credential 模型集合；后续模型保存会按新规则完整重建内部路由。
- Provider、Storage、Runtime、HTTP 契约和 Web 测试覆盖目录解析、畸形 JSON、重复 ID、超大正文、读取失败、重启持久化、多 Key 模型隔离及“保存 Key -> 拉模型 -> 勾选 -> 保存”的完整流程。

### 全局公开模型允许列表切片

- SettingRegistry 新增 `models.allowed` 字符串列表，默认空列表表示允许当前 PublishedSnapshot 的全部公开模型；非空列表按精确、区分大小写的公开模型名放行，不支持通配符、Provider 推断或 Gateway Key 级覆盖。
- 管理设置响应动态返回当前已物化公开模型作为候选；Web“基础”设置提供可搜索多选、当前搜索结果全选/清除、默认值/覆盖值/生效值展示和恢复默认。
- `/v1/models` 使用同一快照过滤目录；Responses、Compact、Chat Completions、Messages 与 Count Tokens 在会话创建、候选选择、RPM 预留和上游 I/O 前统一拒绝未放行模型，并使用对应协议的模型不存在错误。
- ProviderCredential 或 OAuthAccount 的创建、编辑、模型变更和删除仍走唯一串行发布链；当最后一条可提供某模型的 Route 消失时，同一事务会从允许列表及 SQLite 覆盖值自动删除该名称，再 Commit、reconcile 并切换一次快照。
- Domain、Storage、Runtime、HTTP 与 Web 测试覆盖规范化、热更新快照隔离、目录过滤、零上游 I/O 拒绝、API Key/OAuth 最后 Route 删除时的自动裁剪、动态候选和恢复默认。完整决策见 `docs/adr/0049-global-public-model-allowlist.md`。

### GatewayApiKey 管理与公开鉴权切片

- 新增 `GatewayApiKey`、独立 HMAC verifier 和 `gateway_api_keys` SQLite migration；当前模型同时保存可查看的明文 Token 与用于常量时间认证的 Vault 派生 HMAC 摘要。
- Token 由 Runtime 使用 256 位 CSPRNG 生成，格式固定为 `a2k_v1_...`；管理列表、创建、更新和轮换响应始终返回当前完整 Token，日志和 Debug 仍禁止输出。
- 管理 API 已接入：
  - `GET/POST /api/admin/gateway-api-keys`
  - `PATCH /api/admin/gateway-api-keys/{id}`
  - `POST /api/admin/gateway-api-keys/{id}/rotate`
  - `POST /api/admin/gateway-api-keys/{id}/revoke`
- 管理写入继续使用全局 revision、资源 config version 和轮换 token version CAS；`revoke` 路由执行物理删除，成功后立即从 PublishedSnapshot 移除。
- `PublishedSnapshot` 现在携带 Gateway Key 配置和 HMAC verifier；鉴权、路由和 revision 使用同一快照，旧请求持有旧快照时不会被热更新中途改变。
- `/v1/models` 已返回 PublishedSnapshot 中的公开模型目录；Responses、Responses Compact、Chat Completions、Messages 和 Count Tokens 已进入执行链，Responses 可显式桥接到 Chat Completions；未知 `/v1/*` 不再回落到 SPA。
- `/v1/*` 支持 `Authorization: Bearer` 与 `x-api-key`，冲突 Token 拒绝；认证成功后剥离 `Authorization`、`x-api-key`、`Proxy-Authorization` 和 Cookie。
- 公开鉴权成功后立即更新进程内 `last_used_at`，并按每把 Key 最多每 60 秒一次进入现有有界遥测队列；SQLite 写入不推进配置 revision、不阻塞数据面，队列满时按遥测语义丢弃并计数。
- 管理列表合并 PublishedSnapshot 中的持久值与当前进程内最新值，因此成功请求后无需发布新配置即可立即显示最后使用时间；重启后继续读取 SQLite 已落库值。
- Gateway Key 管理列表从保留 RequestLog 按 `gateway_api_key_id` 聚合总请求、2xx 成功和非 2xx 失败；趋势与 Provider/OAuth 统一为最近 1 小时、30 个固定 2 分钟桶，Web 支持鼠标悬浮和键盘聚焦查看桶时间与成功/失败数，统计不参与权限、配额、计费或路由。
- React `/keys` 已替换占位页，支持创建、编辑、停用、轮换、物理删除、deep link 和响应式布局；已认证管理列表始终可查看明文 Token，Token 不进入 URL、浏览器持久存储、日志或 Debug。
- Storage、Runtime、HTTP 契约和 Web 测试覆盖 Token 生命周期、快照隔离、header 剥离、SPA fallback 防护、冲突版本、缓存脱敏、节流、单调落库和即时管理可见性。

### ProviderCredential 连通性测试切片

- Provider Driver 新增 `credential_test_plan`；当前注册的 Codex、Claude 与 Grok 都从各自 Base URL 结构化构造 `GET /models`，并复用同一 generation 的 Provider API Key 认证头。
- 新增受保护管理 API `POST /api/admin/provider-credentials/{id}/test`；请求固定使用当前 PublishedSnapshot、Credential、Endpoint 与解析后的实际代理，预留该 Credential 的普通 RPM 名额并持有 `in_flight` 观测 Guard，但允许绕过已有 `auth_error`；专属代理失败仍 Fail-Closed。
- 探测不经过模型路由、不切换 Credential、不回退代理，也不更新 Endpoint/Proxy 熔断或配置 revision；响应只返回捕获的配置/Secret/代理版本、HTTP 状态、延迟或 Transport 阶段/归因，不返回 URL、地址、正文或 Secret。
- 只有 2xx 会清除本次捕获 `CredentialGenerationRuntime` 的 `auth_error` 并推进统一 scheduler epoch；测试期间发生 Secret/Endpoint 身份轮换时，旧探测只持有旧 generation，不能修改新 generation。
- React Provider 详情页把该探测能力用于模型选择抽屉；新增或更换 Key 后自动拉取，也可从每把 Key 的“模型”操作重新拉取。
- Runtime 与真实 HTTP 契约覆盖当前 Secret/代理注入、上游 401 建立 `auth_error`、管理 `/models` 2xx 清除、目录解析、后续生成请求恢复以及响应脱敏；Provider Registry 契约枚举所有已注册 Driver 的探测路径。

### 同协议 JSON 请求执行切片

- 新增强类型 `ProtocolOperation`，静态注册 Codex/Claude/Grok Provider Driver 与 Responses/Messages ProtocolAdapter。
- `/v1/responses`、`/v1/responses/compact`、`/v1/messages` 和 `/v1/messages/count_tokens` 已接入 Runtime：同一 PublishedSnapshot 内解析 Route、过滤启用 Endpoint/Credential/Proxy、原子取得对应 Permit，并调用现有 Transport。
- Codex Driver 追加 `responses`/`responses/compact`，注入 `Authorization: Bearer`；Claude Driver 追加 `messages`，注入 `x-api-key` 与 `Anthropic-Version: 2023-06-01`；Web 官方默认 Base URL 均包含 `/v1` 固定前缀。
- Adapter 保留未知 JSON 字段，只替换上游 `model`，并按白名单保留 Claude `anthropic-beta`；成功响应恢复公开模型名；上游非 2xx 返回协议兼容的脱敏错误 envelope。
- Runtime 执行链按请求规划、单次 Attempt 和响应处理拆分；生产文件均保持单一职责，没有把网络、调度和响应过滤重新塞进中央文件。
- 同负载轮询游标按 `ModelRoute + fallback tier` 隔离，并由 RuntimeRegistry 跨连续配置代际复用；删除后旧快照仍持有旧游标，新生命周期从零开始，避免跨 Route 偏斜和无效请求扰动。
- 未知 `/v1/*`、已知路径的方法错误和普通公开路由现在经过同一 GatewayApiKey 鉴权层；上游认证头、Cookie、固定及动态 hop-by-hop 响应头不会返回客户端。
- 该切片完成时只支持非流式 JSON；后续 SSE、QueueTicket、会话粘性与可靠性切片已补齐 Responses/Messages 流式执行、生成请求饱和等待、固定会话路由、自动重试、冷却和健康状态。
- 模块测试与本地 HTTP 契约测试覆盖路径、认证头、客户端头剥离、出站 POST、模型替换、Compact 端点、敏感响应头过滤、fallback 鉴权、JSON 405 和 Route/tier 游标生命周期；Registry 契约从真实 App Composition Root 枚举全部 Adapter/Driver，避免生产漏注册仍通过测试。

### Count Tokens 统一 RPM 切片

- `/v1/messages/count_tokens` 使用 Claude 同协议路由、Provider API Key/OAuthAccount、代理和已选上游模型，并与生成请求共用所选账号唯一的可选 RPM。
- 每次 Count Tokens 上游 Attempt 都预留一个 RPM 名额并使用统一 QueueTicket；不再存在 `AuxiliaryScheduler`、辅助 Permit 或第二套容量设置。
- 当前 tier RPM 用尽时遵循同一 `on_rate_limited`、`fallback_on_rate_limit`、队列上限、超时和取消语义。
- Claude Count Tokens 上游明确返回 404 时分类为 operation unavailable，并转换为脱敏 Anthropic 404 `not_found_error`，供 Claude Code 回退本地 Token 估算；其他上游错误仍遵循当前 502 边界。
- 单元与契约测试覆盖统一 RPM 消耗、自动重试单独计数、字段保留、模型改写、Provider 认证头、成功响应和 404 脱敏。

### 同协议 SSE 与 GuardedBody 切片

- Codex Responses 与 Claude Messages 的 `stream=true` 已通过同协议 Route、Provider API Key/OAuthAccount、代理和 Credential RPM 预留执行；Compact 与 Count Tokens 仍只允许 JSON。
- Provider Driver 显式声明 `TransportMode::Sse`，Runtime 根据请求的流式标记选择 JSON/SSE 能力，不用 JSON 能力替代流式能力。
- Protocol `SseDecoder` 覆盖任意字节切分、LF/CRLF、多行 `data:` 和 EOF 无尾空行，并限制单帧缓冲；Adapter 只改写顶层、`response.model` 与 `message.model`。
- Runtime 在返回下游响应头前预读首个完整事件；空流、首帧错误和首帧 Transport 错误仍返回协议 JSON 错误。返回后由 `GuardedBody` 持有上游流、Permit、取消标记和 CommitState。
- EOF、上游错误和客户端 Drop 都只释放一次 Permit；提交后的错误终止当前流，不自动切换 Credential、不拼接第二条流。
- 模块、Driver/Adapter 契约和真实 chunked HTTP 测试覆盖 OpenAI/Anthropic 协议的首帧增量、模型别名、流式响应头、Permit 生命周期与错误边界。

### 生成请求有界 QueueTicket 切片

- 普通 Responses、Responses Compact、Messages 与 Count Tokens 在当前执行 tier 的 RPM 全部用尽时，使用快照级 `QueuePolicy` 选择等待或立即拒绝；默认等待 30 秒、最多 128 个等待请求、默认不进入 fallback tier。
- RuntimeRegistry 持有稳定 `QueueCoordinator` 和统一 `scheduler_epoch`；等待计数跨连续 PublishedSnapshot 复用但不持久化，进程重启后从零开始。
- QueueTicket 使用 RAII 计数，成功、超时、取消和错误路径都会归还等待名额；队列已满或超时返回 `local_rate_limit`。
- 等待者先订阅 epoch，再重新执行完整 select-and-reserve；RPM 到期定时器、健康变化和配置发布共同唤醒，超时边界额外执行最后一次完整选择，避免丢失唤醒和边界误拒绝。
- Route 显式 `fallback_on_rate_limit` 覆盖全局默认；开启时主 tier RPM 用尽可检查下一 tier，关闭时在主 tier 等待或拒绝。
- QueuePolicy 按值捕获在 PublishedSnapshot 中；scheduler SettingRegistry 将已提交候选策略编译进新快照，旧请求不会在等待中途混用新 revision 的策略。
- 单元测试覆盖 Reject、queue-full、fallback、NoCandidates、Permit 释放重选、epoch 竞态、超时最终重选和取消计数；快照测试覆盖协调器复用与策略 revision 隔离。

### Scheduler SettingRegistry 切片

- `SettingDefinition` 集中定义四项 `scheduler.*` 的类型、默认值、范围、枚举值、应用模式、Web 分组和描述；Duration 的持久化/HTTP 单位固定为整数秒。
- SQLite `setting_overrides` 只保存用户覆盖值；写入、恢复默认、no-op 和 revision 冲突沿用串行 `ConfigPublisher`。未知 key、损坏 JSON、类型错误和越界覆盖 Fail-Closed，显式覆盖等于默认值仍保留。
- `PublishedSnapshot` 从已提交 `SettingsConfiguration` 捕获 QueuePolicy；Credential RPM 通过无失败 Runtime reconcile 更新稳定窗口，发布只推进一次 scheduler epoch。
- 管理 API：`GET /api/admin/settings`、`PATCH /api/admin/settings/{key}`、`DELETE /api/admin/settings/{key}?expected_revision=N`。
- React `/settings` 使用“基础、路由策略、运行保护、日志”四个页签；高频项直接展示，低频项默认折叠到同页高级设置。默认、覆盖、生效值与恢复默认、revision 冲突草稿和响应式窄屏能力保持不变。
- Domain、Storage、Runtime、HTTP 和 Web 测试覆盖注册表元数据、持久化、缓存代际、冲突重试及真实 ConfigPublisher RPM 热更新。

### 会话粘性路由切片

- Protocol 解码新增 `IngressAffinity`，只按确认的显式来源提取会话：Codex `previous_response_id`、`X-Any2API-Session`、`X-Session-ID`、`Session-Id`/`Session_id`、Claude Code `metadata.user_id.session_id` 和 `conversation_id`；Count Tokens 不启用粘性，也不使用 Prompt Hash 猜测会话。
- 稳定 `RuntimeRegistry` 持有进程内 `AffinityRegistry`；启动时生成随机 HMAC-SHA256 会话键，并为软/硬用途做域分离。日志、Debug 与管理 DTO 不返回原始 Session ID 或 Response ID，进程重启后键和全部绑定直接清空。
- 软会话使用版本化 `Creating` 租约防止并发首请求选择不同 Credential；提交与 Drop 都会唤醒等待者。TTL 使用访问刷新，容量达到上限时只清理过期项，不引入后台恢复或外部缓存。
- `prefer` 先等待原 Credential，达到 `affinity.soft.prefer_wait_timeout` 后才撤销旧绑定并重新负载均衡；`strict` 与硬绑定只允许原 Credential、Route Target、上游模型和协议方言，缺失时返回 `session_binding_lost`。
- 每个 Credential Runtime 增加固定等待者计数；普通调度不会抢占该 Credential 新释放的槽位，固定会话仍使用全局有界 QueueTicket。优先级只影响同一个 Credential，不阻塞其他 Credential。
- Codex JSON 成功响应顶层 `id` 与 SSE `response.created.response.id` 在客户端可见前写入硬绑定；`Responses Compact` 只支持显式软会话，流式 Body 继续持有 Permit 到 EOF、错误、断连或 Drop。
- 新增六项统一设置：`affinity.soft.enabled`、`affinity.soft.mode`、`affinity.soft.ttl`、`affinity.hard.ttl`、`affinity.soft.prefer_wait_timeout`、`affinity.fixed_wait_timeout`。默认值、覆盖值和生效值均通过现有 SettingRegistry 热更新。
- 管理 API 保留 `GET/DELETE /api/admin/affinity` 与按 Credential 清理能力；普通 Web 使用 `limit=0` 的固定规模聚合，只读取软绑定、硬绑定和 Creating 数，不读取账号分布或 HMAC 样本。
- 独立 `/affinity` 页面与一级导航已删除，旧 deep link 跳转到“设置 → 路由策略”；会话聚合进入总览，`affinity.*` 与 `scheduler.*` 在同一设置分类中管理。
- Runtime、Protocol、HTTP 契约与 Web 测试覆盖并发 Creating、租约唤醒、TTL、身份冲突、重启空状态、固定等待优先、Codex JSON/SSE 硬续接、Claude 软粘性、prefer 重绑、strict 不切换、未知旧 Response ID、管理清理和设置保存/恢复。
- 真实浏览器完成 1440 桌面与 390×844 窄屏验证；桌面导航、移动菜单、自然滚动、设置表单和无水平溢出均通过，页面控制台无错误。

### 可靠性与预提交重试切片

- `ProviderDriver::classify_error` 现在返回强类型 `UpstreamErrorClassification`，Codex/Grok 共享 OpenAI 错误分类，Claude 解析 Anthropic 错误 envelope；认证、权限、额度、限流、模型不可用、操作不可用和临时故障不再折叠为一个上游错误。
- 标准 `Retry-After` 支持 delta-seconds 与 HTTP-date；无效值被忽略，公开响应只返回规范化秒数，不回显原始 Header 或上游正文。
- Credential generation 保存认证健康与模型冷却；429/模型错误只影响当前 Credential+模型，401 使当前 generation 进入永久 `auth_error`，权限/额度使用 Credential 级冷却。
- Endpoint 与 Proxy 按配置代际持有独立熔断器，支持真正的滑动失败窗口、Closed/Open/HalfOpen 和受限探测；DIRECT 网络失败归入 Endpoint，Provider 429/5xx 不会惩罚代理。
- HalfOpen 健康 Permit 在预检查后发生竞争时，会结束当前 `in_flight` Guard、保守保留已预留 RPM、移除竞态候选并继续尝试同 tier 其他候选。
- 冷却和熔断到期通过统一 scheduler epoch 唤醒现有 QueueTicket；所有健康状态只在内存中存在，热更新与进程重启不会恢复旧运行态。
- 普通 JSON 请求改为显式有界多 Attempt：每次失败先发布健康状态、释放 Permit，再按总 Attempt、Credential 切换、同 Credential、绝对耗时和 RetrySafety 预算决定退避与重选；请求内排除已失败 Endpoint/Proxy/Credential，硬粘性与 `strict` 永不跨 Credential。
- `DefinitelyNotSent`、`RejectedBeforeExecution` 和 `Idempotent` 才允许自动重试；`Ambiguous`（包括 5xx、响应体读取失败和 SSE 首帧后的不确定错误）默认不重试，避免重复生成。
- 外部 `Retry-After` 最长按 30 天归一化，异常 `u64` 秒数和时间转换不会因 deadline 溢出而让冷却立即失效。
- Buffered 上游成功后的硬 ID 提取、egress 编码、公开模型恢复和粘性提交错误会先结算健康成功、关闭 HalfOpen 探测，再释放 Credential Permit。
- SSE 首帧在下游提交前仍由 `GuardedBody` 验证；首帧读取失败按不确定结果结束当前请求，不拼接第二条流。首个下游字节提交后永久禁止切换上游。
- 新增运行时虚拟时间、熔断滑动窗口、健康代际隔离测试，以及真实发布快照上的连接前切换、Ambiguous 不重试、429 冷却/Retry-After、硬粘性不切换、Attempt/切换预算和 SSE 首帧边界契约测试。
- 为遵守文件职责门禁，`credential_runtime`、健康 Runtime、请求选择和上游 Attempt 已拆为 generation/rate_window、credential/endpoint/proxy/attempt、fixed/generation、prepared/buffered/streaming/failure 等模块；这些生产模块不依赖临时 allowlist。Codex OAuth 额度的有状态集成测试夹具按 ADR-0034 登记为带到期日的测试例外。

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
- 当前切片直接支持远程 HTTP 与外部 TLS 终止；内建 Rustls listener 和标准 `Forwarded` 头解析仍未实现。管理员密码在线轮换已由 ADR-0024 完成，Provider 专用 OAuth JSON 导入已由 ADR-0044 完成。

### RequestLog 与 Attempt 有界遥测切片

- `/v1` 外层统一生成本地 Request ID，并覆盖所有公开响应的 `x-request-id`；通过 GatewayApiKey 鉴权并进入模型执行链后创建请求记录。
- 管理面与公开面复用 Server 级可信代理来源解析；直连记录 TCP 对端，可信代理从 XFF 右向左剥离连续可信跳。RequestLog 只保存规范化 `client_ip`，不保存原始 XFF/CF 头；Migration 26 前的历史记录保持 `NULL`。
- 新增 `RequestLog`、`RequestAttempt` 与结果枚举，以及 SQLite `request_logs`/`request_attempts` 父子表；配置实体删除后历史外键自动置空，日志不参与启动恢复。
- 每个请求在内存中聚合全部 Attempt，结束时只进行一次同步 `try_send`；队列满、Writer 关闭或 SQLite 写入失败只计数并丢弃，不等待或阻塞数据面。
- 后台 Writer 使用小批量事务写入父子记录，并在空闲期也按保留期限或最大行数任一上限定时分批清理；日志设置随配置发布即时刷新，停机提供有限刷新窗口，不保存排队状态。
- Attempt 在健康结算后、Permit 释放前完成记录；最终 RequestLog 优先保留最后一次 Attempt 的精确错误分类，不把限流、认证、网络或代理错误折叠为通用 `upstream`。
- SSE 在首帧验证和软绑定提交成功后把记录责任交给 `GuardedBody`；EOF、提交后错误与客户端 Drop 只完成一次，首帧 Transport 错误保留 Network/Proxy 归因与 RetrySafety。
- 新增管理 API：`GET /api/admin/request-logs`、`GET /api/admin/request-logs/{request_id}`；React `/logs` 与 `/logs/:requestId` 展示最近请求、解析后的客户端 IP、队列/丢弃指标和 Attempt 时间线。
- SettingRegistry 新增 `logs.request.enabled`、`logs.request.retention`、`logs.request.max_rows`、`logs.telemetry_queue_capacity`，Web 可查看默认/覆盖/生效值并恢复默认。
- OpenAI/Anthropic 协议级精确 usage 与内容首 Token 钩子已由后续切片接入；上游未返回、终止事件前断开或非流式无法精确计时时，对应字段仍保持 `NULL`而不猜测。
- 当前测试覆盖父子事务、保留清理、队列丢弃、Request ID、日志详情契约、Credential 切换后的多 Attempt 顺序、Attempt 预算耗尽、SSE EOF/提交后错误/客户端 Drop 的单次持久化、列表与详情成功/空态/错误态、DTO 解析、deep link 和敏感文本不展示。
- 窄屏布局除超长模型名结构回归外，已由统一 Playwright 套件在真实服务和 390×844 Chromium 中覆盖移动导航、请求日志页面与全局水平溢出。

### 完整 HTTP 系统日志切片

- Migration 27 新增独立 `http_access_logs`，记录全局 Request ID、开始时间、配置 revision、规范客户端 IP、Method、客户端实际 URI path、HTTP version、可用状态码、Body 生命周期耗时、实际响应字节和 completed/body_error/cancelled 结果；Migration 28 在保持 27 首次 checksum 不变的前提下前向放宽 method 长度约束，并原样保留既有日志。
- 最外层 Axum 中间件覆盖公开/管理鉴权失败、健康检查、Web 资源与 deep link、404/405 和正常 API；全部响应统一覆盖 `x-request-id`，模型 RequestLog 复用同一 ID，但两类日志模型、表和管理用途保持独立。
- path 直接读取入口 `request.uri().path()`，不使用路由模板、不改为 `/api/*` 等通配形式，也不做内部路由归一化；query、Header、Cookie、User-Agent、Referer、请求体与响应体不落库。
- 系统日志复用现有有界非阻塞遥测队列和 `logs.request.*` 策略；RequestLog 与 HttpAccessLog 分别执行相同的 retention/max_rows 上限，任一类型都不会挤掉另一类历史。
- 新增受保护 `GET/DELETE /api/admin/system-logs`。DELETE 通过同一 FIFO writer 的控制消息执行：先刷完命令前记录，再清表并回执；清理请求本身和清理边界后完成的请求可以作为新记录保留。
- React 新增一级“系统日志”和 `/system-logs`，以桌面虚拟滚动表格/手机自然滚动日志卡片展示实际 path；桌面固定表头与虚拟行滚动区分层，并提供手动刷新、固定 5 秒周期的自动刷新 Switch 与二次确认清理。自动刷新选择使用版本化 `localStorage` 按浏览器持久化，缺失、无效或不可用时默认开启；仅已认证 Handler 成功响应的定时轮询通过内部响应标记排除，首次加载、手动刷新与异常访问仍记录。
- Storage/Runtime/HTTP/React 契约覆盖原始编码 path、query 排除、状态/字节、公开鉴权失败、队列顺序与清理后不回填；真实 Chromium 覆盖桌面虚拟滚动、固定表头、390×844、自动刷新开关、清理确认、控制台错误和整页横向溢出。完整决策见 `docs/adr/0051-complete-http-access-logging.md`。

### 真实服务浏览器 E2E 基础设施

- 新增独立 Playwright Chromium 套件；`pnpm test:e2e` 自行构建 any2api 与 Web、分配空闲 loopback 端口并启动真实 HTTP 服务。
- 每次执行使用独立系统临时数据目录、全新 SQLite/Vault 和固定测试管理员密码；不复用开发数据、主密钥、Cookie 或运行态，结束后清理隔离目录。
- 启动器只接受 Chromium 允许导航的随机端口；操作系统分配到 `4045` 等 unsafe port 时自动重试，避免页面加载前的偶发门禁失败。
- 浏览器契约覆盖 `/settings` 登录前 deep link 在登录后保持、服务端 SPA 刷新、总览/代理/Provider 分组/网关密钥/请求日志核心页面直达和真实 API 就绪状态。
- 390×844 契约覆盖折叠导航打开、跳转后关闭、请求日志空态与 `documentElement.scrollWidth <= innerWidth`；全部用例同时收集未处理 page error 和 error 级 console 输出。
- E2E 只验证跨层共享行为，业务 CRUD、字段边界、Secret 和故障矩阵继续由更快的 Rust/HTTP/React 测试覆盖。CI 使用独立 `e2e` job 安装 Chromium，普通 Vitest 门禁不承担浏览器启动成本。完整决策见 `docs/adr/0022-browser-e2e-contract.md`。

### 负载均衡运行态检查切片

- 稳定 `CredentialRuntimeHandle` 增加 Relaxed 原子计数，记录成功选中、RPM 用尽、Credential+模型、Endpoint、Proxy 健康过滤事件；只有 RPM、Credential Guard 与全部健康 Guard 都取得后才记录选中。
- `RuntimeRegistry::balancing_snapshot` 从当前 `PublishedSnapshot` 单次遍历路由凭据，只聚合 scheduler epoch、QueueTicket、全局/Provider 账号数、RPM 窗口、`in_flight`、固定等待者和成功选中次数；不再构造 Credential×Model 健康快照。
- 管理员只读 API `GET /api/admin/balancing` 复用既有管理员认证与 `Cache-Control: no-store`，只返回固定规模的全局和 Provider 汇总，不返回账号 ID、标签、Endpoint、Proxy、模型或单账号过滤计数。
- 独立 `/balancing` 页面与一级导航已删除；固定规模的全局/Codex/Claude/Grok 汇总进入总览，`scheduler.*` 进入“设置 → 路由策略”。账号配置、模型与历史请求统计继续分别位于 Provider/OAuth 页面。
- Credential 级选择/过滤计数仍只属于当前进程的调度实现与测试，热更新复用句柄时保留，删除 Credential 或进程重启后清空，但普通管理页面不逐账号展示。
- Runtime 模块测试覆盖多 Provider 聚合；HTTP 契约确认响应不含 `credentials`；Web 覆盖 1000 账号汇总、零值和刷新失败保留旧数据。完整决策见 `docs/adr/0038-aggregate-only-balancing-dashboard.md`。

### 总览与简化设置导航

- 一级导航不再暴露负载均衡和会话粘性；总览用紧凑面板呈现服务、调度、Provider 和会话绑定聚合，不展示账号或绑定样本。
- 系统设置固定为四类：基础、路由策略、运行保护、日志；旧 password/scheduler/affinity/reliability/upstream deep link 均重定向到新分类。
- 路由策略默认只展示 RPM 用尽行为、软粘性开关和模式；队列、fallback、TTL 与固定等待等低频项进入高级折叠区。其他分类同样只直接展示高频项。
- 全局代理只在出口代理页管理，不再在系统设置重复出现。完整决策见 `docs/adr/0039-overview-and-simplified-settings.md`。

### 本地文件日志轮转切片

- SettingRegistry 新增 `logs.file.level`、`logs.file.retention` 与 `logs.file.max_total_size`，默认分别为 `info`、`7d` 与 `256 MiB`；该切片完成时 Registry 共 47 项；加入后续设置、由 ADR-0037 删除两项辅助容量并新增 `models.allowed` 后，当前总数为 50 项，Web 显示默认值、覆盖值和生效值并支持恢复默认。
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

- SettingRegistry 新增 `upstream.read_timeout` 与 `stream.postcommit.idle_timeout`，默认分别为 `15_000ms` 与 `60_000ms`，均允许 `1..=86_400_000ms`、支持热更新且不能用 `0` 禁用；该切片完成时 Registry 共 43 项，后续设置加入、ADR-0037 删除两项辅助容量并新增 `models.allowed` 后，当前总数为 50 项。
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
- Registry 契约枚举实际注册的 Codex/Claude/Grok Driver 与全部 ProtocolAdapter；Runtime/SQLite 契约覆盖 Responses、Compact、Messages JSON/SSE 与 Count Tokens 排除，真实管理 HTTP 列表/详情与 Web 测试覆盖非空值、缺失值和真实零值。完整决策见 `docs/adr/0025-protocol-token-telemetry.md`。

### 有界优雅停机切片

- 新增进程级 `ProcessLifecycle`，状态固定为 `Running / Draining / Forced`；请求与后台任务分别追踪，健康定时任务在 Draining 退出，配置发布和密码轮换在 Forced 取消异步 future。
- Server 最外层 Guard 覆盖完整 Handler 与响应 Body 生命周期；普通响应、SSE EOF/error、客户端 Drop 和 Forced 静默 Body 都通过 RAII 释放请求计数、QueueTicket、Permit 与上游连接。
- SettingRegistry 新增 `shutdown.request_grace_period` 与 `shutdown.finalize_timeout`，默认 `30s` 与 `5s`；加入 OAuth 刷新后一度为 51 项，ADR-0037 删除两项辅助容量、再新增 `models.allowed` 后当前为 50 项。Web 设置页支持默认/覆盖/生效值，总览显示 shutdown phase、活动请求与后台任务数。
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

### OAuthAccount 与统一路由切片（核心完成）

- `ProviderCredential` 继续只接受 `api_key`；旧 OAuth Credential migration 保持不可变，Provider 页面不增加 OAuth 类型或入口。
- OAuth 登录由 Provider Driver 显式声明流程：Codex/Claude 使用固定 authorize/token Endpoint、localhost Redirect URI、PKCE 与内存单次 session；Grok 使用固定 device/token Endpoint、内存 device-code session 和显式 poll API。两种流程都不产生附件下载，并通过同一串行发布链创建 SQLite `OAuthAccount`。
- 已新增独立 `OAuthAccount` SQLite 聚合，明文保存 Provider JSON、账号安全元数据、模型、启用状态、可选 RPM、token/configuration/generation 版本；原始 JSON 不进 Vault、日志、DTO、浏览器状态或导出接口。
- Storage 已实现全局 revision 串行写、账号 config-version 冲突、token-version CAS、刷新时模型保留、DIRECT 固定绑定、重启恢复和损坏 JSON fail-closed；ProviderCredential 表与 API-key-only 约束未改变。
- 激活使用发布快照解析后的 DIRECT/全局代理并继承严格 SSRF 设置，失败不回退本机直连；多个同时完成的登录在发布锁内逐个读取最新 revision，均可完成 Commit/reconcile/快照切换。
- 激活回执只包含 Provider、账号 ID、标签、启用状态、可选 RPM、过期时间、安全邮箱、模型数、配置版本和新 revision；Web 仅保存该回执的页面内状态，不再创建 Blob、下载 Token JSON 或写浏览器存储。
- `/api/admin/oauth/accounts` 提供独立于 ProviderCredential 的安全列表、元数据 PATCH、模型 PUT 和带版本删除；管理 DTO 不接受或返回 OAuth JSON/Token/Endpoint/代理字段，所有写操作经过同一串行发布链。
- Provider Driver 已提供固定 OAuth 数据面与模型目录：Codex 使用 `https://chatgpt.com/backend-api/codex` 并按账号套餐选择目录，Claude 使用 `https://api.anthropic.com/v1`，Grok 使用 `https://cli-chat-proxy.grok.com/v1`；账号只能保存该 Provider 目录内的模型，新激活账号默认选择可用目录。
- Runtime 新增带来源标签的 `RoutingCredentialId` 与统一 `RoutingCredential` 投影；Provider API Key 和 OAuthAccount 共用稳定 Handle、原子选择+RPM 预留、排队、粘性、健康、重试与流式生命周期，OAuth 不再伪装成 ProviderCredential ID。
- OAuthAccount 固定绑定 DIRECT 并继承已发布全局代理；Codex 注入 Bearer、`Chatgpt-Account-Id` 与 `Originator`，Claude 注入 Bearer、固定 `anthropic-version` 并合并所需 OAuth beta，Grok 注入 Bearer 与固定 xAI CLI 身份头；Gateway 认证头仍在进入 Driver 前剥离。
- `/v1/models` 已合并 OAuth-only 模型；请求规划可在没有 ProviderEndpoint/ProviderCredential 时使用 OAuth 固定路由，同模型 API Key 与 OAuth 账号进入同一候选池。账号到期状态按请求时间动态判断，过期账号不进入目录或调度。
- RequestLog/Attempt 使用独立 `oauth_account_id` 标识 OAuth 来源，`credential_id` 与内部固定 `provider_endpoint_id` 保持为空；Balancing、Affinity 与对应 Web 契约均使用来源标签，OAuth 清理令牌固定为 `oauth_account:<uuid>`。
- Provider API Key、OAuthAccount 与 Gateway API Key 管理响应按各自来源聚合最终 RequestLog：累计总请求、成功、失败覆盖日志保留窗口，趋势统一为最近 1 小时、30 个固定 2 分钟桶；中间重试只保留在 Attempt 时间线，不重复计数，统计不参与路由、额度或计费。
- SettingRegistry 新增 `oauth.refresh.scan_interval=30s` 与 `oauth.refresh.lead_time=300s`，要求提前窗口不短于扫描间隔；Worker 启动即扫描并由快照 revision 唤醒，配置热更新后重新读取账号和生效值。定时扫描覆盖启用和停用账号，`enabled=false` 只移除路由资格，不停止 Token 保活；删除账号才终止刷新。
- 单进程 Worker 与请求侧共用 per-account singleflight gate；并发等待者共享成功或失败，拿锁后复核 token version。成功刷新保留启用状态、模型/管理元数据和 Provider 未返回的稳定 Token 字段，通过 SQLite token-version CAS、Runtime reconcile 与单次快照切换发布新认证 generation；停用账号刷新后仍不进入路由候选。完整决策见 `docs/adr/0048-disabled-oauth-token-keepalive.md`。
- OAuth 账号返回 retry-safe 401 时，仅在下游仍为 Pending 且重试预算允许时触发一次刷新；刷新成功或并发请求已更新账号后，基于新 PublishedSnapshot 完整重建候选。第二个 401、Ambiguous、刷新失败或提交后错误都不会再次刷新或发送第三条 Attempt。
- 刷新响应省略到期时间时继续使用 SQLite 账号的旧到期边界，禁止把有限 Token 误变成永不过期；Token Endpoint 和数据面始终使用 DIRECT/全局代理，失败无隐式直连回退。Worker 在 Draining 退出，已经进入串行发布的 CAS 按关键任务边界完成或在 Forced 取消。
- Codex OAuth 账号新增 `GET /api/admin/oauth/accounts/{id}/quota` 与 `POST /api/admin/oauth/accounts/{id}/quota/reset`；Provider 固定调用 ChatGPT `wham/usage`、reset-credit 查询和 consume Endpoint，Runtime 复用 OAuth 代理/严格 SSRF、401 单次刷新和有界正文读取。
- 重置前由服务端重新查询 `available_count`，无可用次数时返回结构化 409，不能仅依赖浏览器旧状态；每个账号的 reset 操作串行化，成功消费后只清除该账号当前 generation 的额度/限流临时冷却并唤醒 scheduler，不清除认证错误或其他账号状态。
- 通用额度响应使用带稳定 ID 的窗口列表，只返回已验证的使用率、窗口维度、重置时间、可选全局状态、Token 余额来源和 Codex reset credit，不写入 OAuth JSON、SQLite、RequestLog、日志或浏览器持久存储。Codex 支持查询与重置；Claude 显示 5 小时、7 天及可选模型窗口；Grok 显示官方套餐、credits/billing 信息，以及官方 Free 余额缺失时明确标注的本地 1M 滚动 24 小时 Token 计量。
- OAuth 账号集合已移除客户端分页，改用共享响应式 `VirtualGrid` 按动态网格行渲染完整 Provider 集合；Codex、Claude 与 Grok 页面都提供“刷新全部额度”，覆盖禁用和离屏账号，最多 6 并发并汇总部分失败。页面同时提供“删除失效账号”：只把刷新 Token 后仍明确返回认证失败的账号列入确认集合，其他错误保留，检测后 Token 版本变化则跳过；删除串行复用现有 revisioned API。额度 Query cache、批量进度与 reset mutation pending 独立于虚拟行挂载，滚动卸载不会取消批量请求，reset 后读取失败也不会保留旧快照。完整决策见 `docs/adr/0036-virtualized-oauth-quota-management.md`。
- React `/oauth` 已接入账号列表、标签/可选 RPM/启停编辑、可搜索模型多选与保存、单账号及失效批量删除确认、JSON 导入、三 Provider 额度、过期提示和 URL deep link；Token 与原始 JSON 不进入管理响应或浏览器持久状态。真实 Chromium 使用假 Token 经真实导入、SQLite 和发布链覆盖桌面/390px 编辑、模型保存后刷新、删除确认、deep link、额度控件和无横向溢出。完整决策见 `docs/adr/0033-server-side-oauth-file-output.md`。

### Grok Provider 与 OAuthAccount 切片

- 新增 `ProviderKind::Grok` 与独立 `GrokDriver`，使用 xAI Bearer API Key；支持 OpenAI Responses、Responses Compact、Chat Completions、JSON/SSE 和标准 `GET /models`，Web 默认 Base URL 为 `https://api.x.ai/v1`。
- Composition Root 和 Registry 契约枚举 Grok；配置能力由 Driver/Protocol Registry 推导，Runtime 调度、RPM、粘性、健康、重试、代理、流式生命周期和遥测没有增加 Provider 分支。
- Codex 与 Grok 共享具名 OpenAI 错误分类和 Bearer Header 构造，Claude 保持独立 Anthropic 行为；Provider Secret Vault 为 Grok 固定分配稳定 AAD code `3`。
- Migration 0024 前向重建受 `provider_endpoints` 外键影响的配置与日志表，完整保留 Credential、模型、Route、RequestLog、Attempt、索引和外键；既有 Migration 未修改，migration 16 升级回归与 `foreign_key_check` 已覆盖。
- Migration 0025 前向重建 OAuthAccount 与请求日志相关外键图，在保留账号、模型、RequestLog、Attempt、索引和级联语义的同时允许 Grok；既有 Migration 不修改，`foreign_key_check` 与升级回归已覆盖。
- Grok OAuth 使用 Device Authorization Grant，固定 xAI CLI device/token Endpoint、Client ID、scope、数据面与身份头；Provider 分类 pending/slow-down/拒绝/过期，Web 展示 user code 并按服务端间隔自动轮询。device code 只在服务端内存，Token 原始 JSON 作为独立 `OAuthAccount` 明文保存在 SQLite，不进入 Vault、管理 DTO、日志、浏览器状态或下载文件。
- Grok OAuth 首版只参与 Responses；API Key 与 OAuthAccount 仍通过同一 Registry 和通用 `RoutingCredential` 投影复用 RPM、排队、粘性、健康、重试、代理和流式生命周期，不复制第二套调度实现。
- Provider 源码按 `codex/`、`claude/`、`grok/` feature 目录归档；Provider 根目录只保留跨 Provider 的稳定 API、Registry、错误、Secret 与 OAuth/Routing 通用模块。
- Provider Web 增加 Grok 分类、xAI 默认地址与三列窄屏切换；总览聚合与请求日志空态识别 Grok，不展示逐账号运行态详情。完整决策见 `docs/adr/0040-grok-api-key-provider.md`。

### OAuth JSON 导入与三 Provider 额度切片

- `POST /api/admin/oauth/import` 接受最多 32 个 JSON 文件和单文件多账号 envelope，兼容已审计的 CLIProxyAPI/Sub2API Codex、Claude 与 xAI/Grok OAuth 结构；全部账号先规范化，再在一个 SQLite 事务、一次 revision 和一次快照切换中原子发布。
- 上传文件只存在于导入抽屉局部状态，提交开始、失败、关闭或卸载都会清空；服务端不保留文件副本，不提供 OAuth JSON 读取、下载或导出端点。
- Grok 固定读取 xAI billing 与实时 subscription；缺少权威 Free 余额时使用当前进程观察到的真实响应 usage 做本地 1M 滚动 24 小时计量，禁止发送生成请求探测或把推理限流 Header 冒充余额。Claude 固定读取 Anthropic OAuth usage 并保留全部有效窗口；两者均只读且不提供 reset。
- Provider、Runtime、Storage、HTTP、Web 与真实浏览器测试覆盖外部 JSON 解析、整批回滚、脱敏 DTO、额度窗口解析、代理/401 刷新边界和账号管理工作流。完整决策见 ADR-0044、ADR-0045 与 ADR-0046。

### Feature-first 目录收敛切片

- Domain、Protocol、Provider、Runtime、Storage、Server、Transport、App 与 xtask 已按 feature/工作流归档；crate 根目录只保留稳定入口、一级领域地图和少量跨 feature 基础类型。
- Runtime OAuth 分为 login/import/quota/refresh，Server OAuth 分为 account/login/import/quota；请求日志仓储、Responses → Chat 请求转换和 Reqwest Transport 已按职责拆分。
- 已无含义不明的生产 `service.rs`、`manager.rs`、`utils.rs` 或 `common.rs`，也无旧式 `#[path = "...tests.rs"]`；源文件体积 allowlist 为空。完整决策见 ADR-0047。

### OpenAI Images API 与媒体缓冲切片

- 新增独立 `openai_images` 方言与 `images_generations`、`images_edits` 操作，公开注册 `POST /v1/images/generations` 和 `POST /v1/images/edits`。
- 生成支持 JSON；编辑支持 JSON 图片引用和 multipart 文件上传。multipart 由协议层结构化解析并重新编码，保留未知字段、字段顺序、重复 `image[]`、文件字节与安全 Part Header，同时只替换已发布的上游模型。
- Images 支持 JSON/SSE usage 遥测、OpenAI 错误 envelope、Gateway Key 剥离和既有 Route/RPM/代理/健康/重试/粘性/流式生命周期；不新增图片专用调度器或跨协议 Bridge。
- Codex/OpenAI API Key Driver 声明 `images/generations` 与 `images/edits` 固定路径和 Images 能力；Codex OAuth、Claude 与 Grok 当前明确不声明该能力，避免把不兼容的上游图片契约误报为 OpenAI Images。
- 编辑请求上限为 `512 MiB`；Images buffered 响应上限为 `512 MiB`；单个 SSE 帧和首个预提交事件上限为 `128 MiB`。Images 等待、读取、流式和重试预算至少为 `180s`，普通文本路径仍保持原有 `32 MiB`/`16 MiB` 限制与超时。
- Rust/HTTP 契约覆盖 JSON、multipart、SSE、二进制图片响应、大响应边界、模型替换、敏感 Header 过滤和提交前重试；完整决策见 `docs/adr/0054-openai-images-api.md`。

### 系统总览调用分析切片

- 总览移除卡片套卡片，使用扁平 section、指标带和分隔线；请求数、总 Token 与平均 RPM 全部跟随 URL 中的 `1h / 24h / 7d / 30d` 范围。
- 新增只读 `GET /api/admin/overview/usage`，从最终 RequestLog 返回保留窗口与所选范围累计、连续时间桶及公开模型聚合；Token 通过十进制字符串传输，不新增计费或持久化计数器。
- 宽屏左侧使用 Chart.js monotone 平滑调用/失败曲线，右侧使用紧凑模型饼图；窄屏上下排列，横轴压缩为至多七个可读标签，饼图最多八个守恒扇区。
- Storage、Server DTO、真实 HTTP 契约、React 解析与组件测试覆盖统计口径、范围联动、空桶、模型聚合、超安全整数 Token 和无嵌套 Surface；桌面、390px、深色主题与无横向溢出已通过真实浏览器验收。完整决策见 `docs/adr/0055-flat-overview-request-analytics.md`。

## 当前边界

- DIRECT/HTTP/SOCKS5h 网络执行与连接池已接入公开 JSON/SSE 请求；代理认证和管理面代理测试已接入，健康熔断继续只由公开请求数据面驱动。
- Credential 模型配置、内部 ModelRoute 物化、公开 `/v1/models`、同协议 JSON/SSE、Chat Completions 入口与 Responses → Chat Completions 桥、普通生成请求有界排队、会话粘性和提交前多 Attempt 已实现。
- 当前代理支持 host/port 与 Vault 认证；HTTP/SOCKS5 默认使用远端 DNS，`upstream.strict_ssrf=true` 时统一改为本地解析和固定目标连接。Provider Base URL 可直接指向 HTTP(S) 公网或内网目标。
- 当前实现 admin、models、affinity、scheduler、retry、cooldown、breaker、upstream、stream、OAuth refresh、request logging、file logging 与 shutdown 共 50 项 SettingRegistry。
- 远程反代必须先配置 `ANY2API_TRUSTED_PROXY_CIDRS`，并确认 `admin.remote_enabled=true`；未配置认证服务的测试/嵌入 Router 仍不能远程管理。
- 数据目录由进程级文件锁独占；管理员密码可在线轮换，成功后仅保留当前请求获得的新会话，其他旧会话立即失效。
- 运行态 RPM 窗口、`in_flight`、请求等待、会话绑定、健康、冷却和熔断都只保存在内存；进程重启后这些状态全部从零开始。
- ProviderCredential 与 OAuthAccount 分别承载 API Key generation 和独立 token/account generation，并通过带来源标签的 `RoutingCredentialId` 编译到同一候选池；两类持久化模型和管理 API 保持分离。
- 当前 JSON/Compact/Count Tokens 与非成功 SSE 错误正文已使用统一上游 read timeout；成功 SSE 分别使用可配置 PrecommitBudget 与提交后 idle timeout。RequestLog/Attempt 与完整 HttpAccessLog 已写入 SQLite，规范客户端 IP、精确 Token Usage、客户端可见流式 TTFT 和 HTTP Body 生命周期已按各自协议契约采集；Migration 26 前的 RequestLog IP 和其他无法精确获取的值保持 `NULL`。
- RequestLog/Attempt 对 API Key 与 OAuth 使用互斥来源列；OAuth 不暴露内部固定 Endpoint。Provider/OAuth 管理页显示各自日志窗口统计，请求日志列表显式标识最终上游来源；Gateway Key 入口统计保持不变。负载均衡运行态 API 只返回两类来源合并后的全局/Provider 汇总；RPM 窗口、`in_flight`、队列和内部选择/过滤计数只读自当前进程内存，不持久化、不参与启动恢复。
- Gateway 鉴权失败、认证头冲突、公开 404/405 与已认证执行错误都由对应 Responses/Messages Adapter 编码；公开 Router 不再存在第二套简化 JSON。
- 正式运行默认从二进制内嵌 React 资源提供管理面；改变当前工作目录或删除源码树不会影响页面，外部 Web 目录必须通过 `ANY2API_WEB_DIR` 显式选择。

## 下一步

1. 使用实际 Codex、Claude 与 Grok 账号分别完成人工登录、JSON/SSE 数据面、自动刷新和 retry-safe 401 单次恢复，并 smoke 三 Provider 额度查询；只有 Codex 账号确有 reset credit 时才人工执行一次重置。该步骤需要外部账号授权，Token 不得写入测试产物或日志。
2. 在真实 Nginx，以及可选 Cloudflare -> Nginx 链路中 smoke `ANY2API_TRUSTED_PROXY_CIDRS`、HTTPS 判断和 RequestLog 客户端 IP，确认源站访问限制与 Nginx real-IP CIDR 配置符合部署环境。
3. 在 Unix CI 中增加真实子进程 SIGTERM 回归，补齐目前由单元测试和 Windows Ctrl-C 子进程测试覆盖的停机信号矩阵。
4. `/backend-api/codex/responses`、Codex WebSocket、内建 Rustls listener 及 Codex/OpenAI ↔ Claude 双向转换仍是明确的后续范围；通用 Secret 导入导出继续永久禁止。

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

当前 OpenAI Images API、Grok API Key Provider、OAuthAccount 统一路由、自动刷新、401 恢复、Codex/Claude/Grok 额度管理、全局公开模型允许列表、可信客户端 IP 请求日志、完整 HTTP 系统日志、统一凭据请求时间窗口与管理 Web 已通过 Rust fmt、workspace 严格 clippy、workspace 全特性测试（含 doc tests）、release build 和架构检查。Web 已通过 typecheck、lint、55 个文件共 170 项 Vitest、production build 与内嵌产物一致性检查；既有 6 项真实 Chromium E2E 保持为上一轮发布验收结果。回归覆盖 Images JSON/multipart/SSE、专用缓冲与超时、Driver/Registry、Device Authorization 请求与轮询分类、Bearer 数据面认证、协议能力、Endpoint 管理、OAuth 拒绝及失效清理、Vault AAD、模型允许列表裁剪、直连/可信代理/欺骗与无效转发头、migration 16→29、完整 Provider/OAuth/RequestLog 引用图保留、凭据统计固定时间桶与浮层交互、原始 HTTP path、响应 Body 结算、系统日志自动轮询排除、虚拟滚动和有序清理。
