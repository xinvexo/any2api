# any2api 实施状态

> 最后更新：2026-07-18
> 用途：简要记录已经完成的代码、当前边界和下一步顺序。架构真相仍以根目录 `ARCHITECTURE.md` 为准。

## 当前状态

- 当前阶段：阶段 1「配置与代理」。
- 最近完成：Credential 稳定运行时句柄、动态并发 Permit、代际绑定与最低负载率原子选择。
- 阶段 0 基线：`6b7d00f chore: scaffold any2api phase 0`。
- ProviderEndpoint 切片：`08e4913 feat: add provider endpoint configuration`。
- Secret Vault 切片：`e71b8b9 feat: add versioned secret vault`。
- ProviderCredential 切片：`f3ca1fc feat: add provider credential management`。
- 本切片提交主题：`feat: add credential runtime capacity`。

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
- DNS 最终地址校验、重定向限制和 Transport 连接绑定仍属于后续网络执行切片；本切片不进行网络 I/O。

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
- 本切片没有实现真实上游连通性测试、并发 Permit、负载均衡选择或网络转发；当前预览只能管理配置，尚不能代理真实 Codex/Claude 请求。

### Credential Runtime 容量切片

- 新增稳定的 `CredentialRuntimeHandle`；同一 Credential ID 的 `in_flight` 与动态并发上限跨配置 revision 复用，不会在热更新时重置为零。
- `max_concurrency` 与 `in_flight` 打包在同一个原子状态中，动态降限与并发获取共享同一 CAS 线性化点；降限不取消已有请求，但在占用回落前禁止新增 Permit。
- 新增 RAII `ConcurrencyPermit`；正常完成、失败、取消或 Drop 都只释放一次容量，并推进统一 `scheduler_epoch`。
- 新增 `CredentialGenerationRuntime` 与 Snapshot 固定绑定；Secret 轮换、重新启用或 Endpoint 身份变化后，新请求使用新 generation，旧请求仍持有旧 generation，迟到结果不会天然混入新代际。
- `PublishedSnapshot` 现在由持久化配置和稳定 `RuntimeRegistry` 一起编译，每个已发布 Credential 都有对应运行时绑定；删除 Credential 后，新 Snapshot 立即移除候选，旧绑定标记 retired 并由现存请求自然释放。
- 新增 `select_and_try_acquire`：只返回已经取得 Permit 的 Credential，最低负载率使用整数交叉相乘，CAS 竞争失败后重新完整选择，相同比例由调用方提供的轮询序号打破平局。
- 模块测试覆盖 64 路并发竞争不超限、动态升降限、Permit 释放、generation 固定、retired 生命周期和精确负载率选择。
- 契约测试覆盖真实 SQLite 发布链路中的容量复用、Secret rotation generation 隔离和删除时旧 Permit 生命周期。
- 本切片仍未把解密后的 Provider API Key 放入 generation-scoped auth material；Snapshot 中也没有 Secret。真实认证注入和网络调用继续留在后续数据面切片。

## 当前边界

- 尚未实现真实 HTTP/SOCKS5 网络转发、连接池、代理连通性测试和代理健康状态。
- 当前代理仍只保存 host/port；用户名与密码尚未接入，后续必须通过 Secret Vault 保存。
- 不要在单管理员认证完成前用 Nginx/Caddy 把管理 API 反代给远程客户端。
- 运行态并发已实现且只保存在内存；队列、健康、冷却和会话仍未实现，进程重启后容量状态从零开始。
- Credential generation 当前只固定版本身份，尚未承载解密认证材料、认证健康或模型健康。

## 下一步

1. 把解密后的 Provider API Key 安全装配到 generation-scoped auth material，保持 Secret 不进入 `PublishedSnapshot`、日志和管理 DTO。
2. 实现 `DIRECT → 全局代理` 的实际解析，以及 HTTP/SOCKS5 `TransportManager`、连接池和 fail-closed 代理错误。
3. 增加最小模型 Route/Route Target 配置，为公开模型和上游模型建立可发布映射。
4. 实现多 `GatewayApiKey` 管理和客户端认证头剥离，再接 Codex Responses 与 Claude Messages 原生请求链路。
5. 增加饱和排队 epoch、QueueTicket、会话粘性、冷却、熔断与重试预算。
6. 实现 SettingRegistry、单管理员认证与可选 HTTP/HTTPS 远程管理。

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

以上 Rust 与 Web 门禁在本切片完成时全部通过；`cargo deny` 仅报告基线已有的重复传递依赖 warning。浏览器预览使用 `http://127.0.0.1:3211`，验证后端管理 API、Endpoint 详情页和 390px 响应式布局。
