# any2api 实施状态

> 最后更新：2026-07-19
> 用途：简要记录已经完成的代码、当前边界和下一步顺序。架构真相仍以根目录 `ARCHITECTURE.md` 为准。

## 当前状态

- 当前阶段：阶段 2/3 交叉切片「同协议 JSON/SSE」；会话粘性、scheduler/affinity SettingRegistry 已完成，管理员认证和远程管理仍待完成。
- 最近完成：进程内软/硬会话粘性、固定 Credential 等待优先级、粘性管理 API 与 Web 页面、scheduler 设置注册表与管理页、生成请求有界 QueueTicket 排队、Codex/Claude 同协议 SSE、GuardedBody、Count Tokens 辅助并发、同协议 JSON 数据面、Transport 接入、多 GatewayApiKey 管理与 `/v1/models` 已发布模型目录。
- 阶段 0 基线：`6b7d00f chore: scaffold any2api phase 0`。
- ProviderEndpoint 切片：`08e4913 feat: add provider endpoint configuration`。
- Secret Vault 切片：`e71b8b9 feat: add versioned secret vault`。
- ProviderCredential 切片：`f3ca1fc feat: add provider credential management`。
- Credential Runtime 切片：`bc71133 feat: add credential runtime capacity`。
- Credential Auth Material 切片：`fbfc6ef feat: load credential auth material`。
- Proxy Transport 切片：`33f9f2d feat: add proxy transport manager`。
- Model catalog 切片：`354a431 feat: expose published model catalog`。
- 同协议 JSON 切片：`c83d6b0 feat: add same-protocol json execution`。
- 本切片提交主题：`feat: add session affinity routing`。

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
- 尚未加密代理密码或实现主密钥轮换/恢复；首版明确不实现在线轮换与内建恢复。

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
- 连接前错误标记 `DefinitelyNotSent`，等待响应头和读取 Body 的不确定错误标记 `Ambiguous`；更精确的 DNS/TLS/代理归因在熔断实现前继续完善。
- DIRECT 请求会解析并校验全部 DNS A/AAAA 结果，未授权 Endpoint 遇到私网/回环地址时在发送 Provider Key 前拒绝；通过校验后把连接固定到本次已验证地址，同时保留原 Host/SNI。HTTP/SOCKS5 远端 DNS 仍属于显式受信代理边界。
- 模块网络测试覆盖真实 DIRECT、HTTP absolute URI、HTTPS 经 HTTP CONNECT 完成 TLS 隧道、SOCKS5h 远端 DNS、禁重定向、缓存代际、授权头 Debug 脱敏和 fail-closed。
- 公共 API 契约测试确认目标本机可达时，指定不可用代理仍然失败且目标端口没有收到连接。
- TransportManager 已装配进公开模型请求入口；代理用户名/密码、HTTP/SOCKS5 严格 SSRF 本地 DNS 模式和代理健康熔断仍未实现。

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

## 当前边界

- DIRECT/HTTP/SOCKS5h 网络执行与连接池已接入公开 JSON/SSE 请求，但尚未覆盖健康熔断和管理面代理测试按钮。
- ModelRoute 配置、管理面、公开 `/v1/models`、同协议 JSON/SSE 请求、普通生成请求有界排队与会话粘性已实现；重试、冷却和健康仍未完成。
- 当前代理仍只保存 host/port；用户名与密码尚未接入，后续必须通过 Secret Vault 保存。
- 当前实现 scheduler 与 affinity 两组共十二项 SettingRegistry；retry、cooldown、breaker 和日志保留设置仍未接入统一注册表。
- 不要在单管理员认证完成前用 Nginx/Caddy 把管理 API 反代给远程客户端。
- 运行态并发、生成请求等待计数和会话绑定都只保存在内存；健康与冷却仍未实现，进程重启后容量、队列和会话状态全部从零开始。
- Credential generation 已承载首版 API Key 认证材料；认证健康、模型健康和刷新锁仍未实现。
- 当前 JSON/SSE vertical slice 尚未提供统一请求 deadline/read timeout；SSE 只对首帧提供 5 秒预提交超时，提交后的错误直接结束流。上游错误状态暂按脱敏 `502` envelope 返回，`Retry-After` 和精细错误状态留到可靠性切片。
- Gateway 鉴权失败与方法错误当前使用管理面统一 JSON envelope；已认证的协议执行错误才由 Responses/Messages Adapter 编码，协议专用 401 envelope 留待公开错误适配切片。

## 下一步

1. 增加冷却、熔断、错误分类默认值与多 Attempt 重试预算，并保持提交后禁止切换上游的不变量。
2. 实现单管理员认证与可选 HTTP/HTTPS 远程管理。
3. 补齐其余 SettingRegistry 分组、请求日志、Attempt、可观测性和 OAuth2 扩展边界。

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

以上 Rust 与 Web 门禁在本切片完成时全部通过；`cargo deny` 仅报告基线已有的重复传递依赖 warning。会话粘性浏览器预览使用隔离的本地数据目录与临时端口；公开 Codex/Claude JSON/SSE 上游路径、硬/软粘性、prefer/strict、Count Tokens、认证头、模型别名、404 兼容和响应头过滤由本地上游契约测试覆盖。
