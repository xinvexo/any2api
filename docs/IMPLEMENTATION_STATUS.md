# any2api 实施状态

> 最后更新：2026-07-21
> 用途：简要记录已经完成的代码、当前边界和下一步顺序。架构真相仍以根目录 `ARCHITECTURE.md` 为准。

## 当前状态

- 当前阶段：阶段 4 可靠性、统一上游读取超时、SSE PrecommitBudget/提交后 idle timeout、RequestLog/Attempt 有界遥测、单管理员远程 HTTP 认证以及代理认证/管理探测切片已完成；严格 SSRF 本地 DNS 与 OAuth2 仍待完成。
- 最近完成：代理 HTTP Basic/HTTPS CONNECT/SOCKS5 RFC 1929 认证、代理认证代际、受限 ProviderEndpoint 管理探测，以及配置发布任务脱离 HTTP 请求取消的执行边界；此前完成可热更新的上游响应头/buffered body 读取超时、SSE 提交后空闲超时与首事件字节/时长预算，以及本地 Request ID、RequestLog/Attempt SQLite 历史、有界非阻塞遥测队列、软/硬会话粘性、QueueTicket、同协议 JSON/SSE、错误分类、冷却、熔断和提交前多 Attempt 重试。
- 阶段 0 基线：`6b7d00f chore: scaffold any2api phase 0`。
- ProviderEndpoint 切片：`08e4913 feat: add provider endpoint configuration`。
- Secret Vault 切片：`e71b8b9 feat: add versioned secret vault`。
- ProviderCredential 切片：`f3ca1fc feat: add provider credential management`。
- Credential Runtime 切片：`bc71133 feat: add credential runtime capacity`。
- Credential Auth Material 切片：`fbfc6ef feat: load credential auth material`。
- Proxy Transport 切片：`33f9f2d feat: add proxy transport manager`。
- Model catalog 切片：`354a431 feat: expose published model catalog`。
- 同协议 JSON 切片：`c83d6b0 feat: add same-protocol json execution`。
- 上一切片提交主题：`feat: add upstream read and stream idle timeouts`。
- 本切片主题：代理认证、管理面代理探测与 Transport 认证代际。

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
- TransportManager 已装配进公开模型请求入口；代理用户名/密码与管理面代理探测已实现。HTTP/SOCKS5 仍使用远端 DNS 受信边界，严格 SSRF 本地 DNS 模式仍未实现。

### 代理认证与管理探测切片

- `ProxyProfile` 只保存认证用户名、是否配置密码和 `authentication_version`；用户名拒绝控制字符与 HTTP Basic 分隔符 `:`，密码进入独立 `proxy_passwords` 表并使用现有 Secret Vault/AAD 加密。
- 认证状态实际发生设置、替换或清除时增加认证版本与代理 `config_version`；重复清除已关闭认证是 no-op。连续 PublishedSnapshot 使用新的 Transport Client 代际，旧请求继续持有旧认证材料。
- HTTP forward proxy、HTTPS CONNECT 和 SOCKS5 RFC 1929 用户名密码认证均通过 Transport sidecar 接入；密码不进入普通读取/响应 DTO、`TransportRequest.headers`、URL、日志、Debug 或缓存 key，Transport 只在受控边界写入代理认证材料。
- 管理 API 新增 `PUT/DELETE /api/admin/proxies/{id}/authentication`；普通代理响应只返回用户名、`password_configured` 和认证版本。
- 管理 API 新增 `POST /api/admin/proxies/{id}/test`，仅接受已有 Provider Endpoint，发送不带 ProviderCredential 的空 GET；普通上游 HTTP 状态视为链路可达，但 HTTP forward proxy 的 407 认证拒绝作为 `ProxyHandshake + Proxy` 失败。响应携带捕获的全局 revision、Proxy/Endpoint config version 与脱敏结果，不更新健康熔断。
- React 代理页增加认证局部表单、清除操作、Provider Endpoint 测试目标选择和行内延迟/状态结果；密码在提交成功或失败后都会清空，不进入查询缓存、URL、Storage 或持久化表单状态；探测结果按配置 revision 和目标 Endpoint 隔离，停用代理不提供测试操作。
- Domain、Storage、Transport、Runtime、HTTP 契约和 Web 测试覆盖用户名/密码边界、加密重启加载、重复清除 no-op、版本代际、HTTP/CONNECT/SOCKS5 认证、HTTP/CONNECT 407 代理拒绝、错误密码 Fail-Closed、DTO 脱敏、受限测试目标与管理探测。

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
- 当前切片直接支持远程 HTTP 与外部 TLS 终止；内建 Rustls listener、标准 `Forwarded` 头解析、管理员密码在线轮换和 OAuth2 JSON 上传仍未实现。

### RequestLog 与 Attempt 有界遥测切片

- `/v1` 外层统一生成本地 Request ID，并覆盖所有公开响应的 `x-request-id`；通过 GatewayApiKey 鉴权并进入模型执行链后创建请求记录。
- 新增 `RequestLog`、`RequestAttempt` 与结果枚举，以及 SQLite `request_logs`/`request_attempts` 父子表；配置实体删除后历史外键自动置空，日志不参与启动恢复。
- 每个请求在内存中聚合全部 Attempt，结束时只进行一次同步 `try_send`；队列满、Writer 关闭或 SQLite 写入失败只计数并丢弃，不等待或阻塞数据面。
- 后台 Writer 使用小批量事务写入父子记录，并在空闲期也按保留期限或最大行数任一上限定时分批清理；日志设置随配置发布即时刷新，停机提供有限刷新窗口，不保存排队状态。
- Attempt 在健康结算后、Permit 释放前完成记录；最终 RequestLog 优先保留最后一次 Attempt 的精确错误分类，不把限流、认证、网络或代理错误折叠为通用 `upstream`。
- SSE 在首帧验证和软绑定提交成功后把记录责任交给 `GuardedBody`；EOF、提交后错误与客户端 Drop 只完成一次，首帧 Transport 错误保留 Network/Proxy 归因与 RetrySafety。
- 新增管理 API：`GET /api/admin/request-logs`、`GET /api/admin/request-logs/{request_id}`；React `/logs` 与 `/logs/:requestId` 展示最近请求、队列/丢弃指标和 Attempt 时间线。
- SettingRegistry 新增 `logs.request.enabled`、`logs.request.retention`、`logs.request.max_rows`、`logs.telemetry_queue_capacity`，Web 可查看默认/覆盖/生效值并恢复默认。
- 首版没有协议级精确 usage 与内容首 Token 钩子，因此 `first_token_ms` 和各 Token Usage 字段保持 `NULL`；不猜测未知 JSON 字段或把 SSE 控制事件当作首 Token。
- 当前测试覆盖父子事务、保留清理、队列丢弃、Request ID、日志详情契约、Attempt 生命周期、列表渲染、DTO 解析和敏感文本不展示；多 Attempt/错误/SSE 持久化、详情空态与错误态、deep link 及响应式页面仍没有自动化覆盖。

### SSE PrecommitBudget 切片

- SettingRegistry 新增 `stream.precommit.max_bytes` 与 `stream.precommit.max_duration`，默认分别为 `256 KiB` 和 `5s`；Web 可查看默认/覆盖/生效值并恢复默认。
- 每个流式请求从当前 PublishedSnapshot 捕获不可变预算；旧请求不会在等待首事件期间混入新 revision 的配置。
- `GuardedBody` 在下游响应头提交前使用配置 deadline 约束等待、分帧、协议解码、模型恢复与必要的硬/软粘性提交，并用同一字节上限限制每个 SSE 帧；超限、超时、空流或无效首帧均在提交前失败并释放健康 Guard 与 Credential Permit。
- 解码器按当前帧容量增量消费 transport chunk，Runtime 每次只编码并排队一个事件；同一 chunk 的未消费 `Bytes` 零拷贝保留，不会在首事件交付前扩张成整批编码帧。后续帧超限时，已完成事件先交付，再以 Body 错误终止。
- 编码后的公开事件超过字节预算时仍返回公开上游错误，但按本地预算失败结算上游健康；Runtime 自行产生的超时按 `Unattributed` 结算，二者都不会误开 Endpoint 或 Proxy 熔断。
- `GuardedBody` 状态机、帧处理管线、完成/错误结算、预算与错误类型按职责拆分到独立模块；相关生产文件均保持在 300 行以内。
- Domain/Runtime 测试覆盖两项设置元数据、原始/编码后字节预算、deadline、同 chunk 顺序、单事件预缓冲、硬/软绑定超时、提交后停止计费和健康归因；真实 HTTP 契约覆盖字节超限、首事件等待超时与旧请求保持旧 PublishedSnapshot。

### 上游读取与 SSE 提交后空闲超时切片

- SettingRegistry 新增 `upstream.read_timeout` 与 `stream.postcommit.idle_timeout`，默认分别为 `15_000ms` 与 `60_000ms`，均允许 `1..=86_400_000ms`、支持热更新且不能用 `0` 禁用；当前 Registry 共 43 项。
- `TransportRequest` 按请求快照携带 read timeout，不把它放进连接池 Client key。固定请求体开始被连接层消费后才启动响应头 timer，因此较短的 read timeout 不会取代 DNS、连接、代理握手或 TLS 的既有阶段边界。
- 等待响应头超时记录为 `AwaitHeaders + Ambiguous`；JSON、Compact、Count Tokens 与非成功 SSE 错误正文逐 chunk 收集时使用相同空闲时长，超时记录为 `ReadBody + Ambiguous`。DIRECT 归因 Endpoint，无法证明责任的代理路径归入 `Unattributed`。
- 成功 SSE 提交前只使用 `stream.precommit.max_duration`，不叠加通用 read timeout；首个下游帧交付时启动 post-commit idle timer，每个成功上游 chunk（包括不完整帧）重置，缓冲完整事件始终优先交付。
- 提交后 idle timeout 只返回 Body error 并终止当前流，不重试、不切换 Credential、不生成协议内错误事件，也不再次惩罚已按成功结算的 Endpoint/Proxy 健康；Attempt 记录为 `StreamError + Network + Ambiguous`。
- Runtime/Transport/HTTP 契约覆盖响应头停滞、连接阶段隔离、buffered body 停滞、成功 SSE 不混用 read timeout、idle timer 启动/重置、缓冲帧优先、Permit 单次释放、健康不受罚和提交后不启动第二条流。

## 当前边界

- DIRECT/HTTP/SOCKS5h 网络执行与连接池已接入公开 JSON/SSE 请求；代理认证和管理面代理测试已接入，健康熔断继续只由公开请求数据面驱动。
- ModelRoute 配置、管理面、公开 `/v1/models`、同协议 JSON/SSE 请求、普通生成请求有界排队、会话粘性和提交前多 Attempt 已实现。
- 当前代理支持 host/port 与 Vault 认证；严格 SSRF 本地 DNS 尚未接入，HTTP/SOCKS5 仍明确依赖远端代理解析目标域名。
- 当前实现 admin、affinity、scheduler、retry、cooldown、breaker、upstream、stream 与 request logging 共 43 项 SettingRegistry；`logs.file.*` 本地文件日志轮转仍未接入。
- 远程反代必须先配置 `ANY2API_TRUSTED_PROXY_CIDRS`，并确认 `admin.remote_enabled=true`；未配置认证服务的测试/嵌入 Router 仍不能远程管理。
- 运行态并发、生成请求等待、会话绑定、健康、冷却和熔断都只保存在内存；进程重启后容量、队列、会话和健康状态全部从零开始。
- Credential generation 已承载首版 API Key 认证材料及认证/模型健康；OAuth 刷新锁和 OAuth2 执行链路仍未实现。
- 当前 JSON/Compact/Count Tokens 与非成功 SSE 错误正文已使用统一上游 read timeout；成功 SSE 分别使用可配置 PrecommitBudget 与提交后 idle timeout。RequestLog/Attempt 已写入 SQLite，但精确首 Token 与 Token Usage 尚未采集。
- Gateway 鉴权失败与方法错误当前使用管理面统一 JSON envelope；已认证的协议执行错误才由 Responses/Messages Adapter 编码，协议专用 401 envelope 留待公开错误适配切片。

## 下一步

1. 先起草并接受严格 SSRF 本地 DNS 独立 ADR，再实现受控解析、目标地址固定、HTTP CONNECT 保留 Host/SNI 与 SOCKS5 本地解析。
2. 让 Gateway 401、公开 404/405 等入口错误通过 Responses/Messages 协议适配器编码。
3. 后续再实现内建 Rustls、管理员密码在线轮换与 OAuth2 Provider 专用扩展。

## 验证结果

```text
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo test --locked --doc --workspace
cargo build --locked --release --workspace
cargo xtask architecture-check
cargo deny check

cd web
pnpm typecheck
pnpm lint
pnpm test
pnpm build
```

以上 Rust 与 Web 门禁在本切片完成时全部通过；`cargo deny` 仅报告基线已有的重复传递依赖 warning，Vite 仅报告当前单入口 bundle 超过 500 kB 的提示。Web 共 27 个测试文件、65 项测试通过；`/logs` 与 `/logs/:requestId` 已完成一次桌面和 390×844 人工验收记录，但该验收尚未纳入仓库自动化脚本。
