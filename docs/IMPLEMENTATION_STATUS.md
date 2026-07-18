# any2api 实施状态

> 最后更新：2026-07-18
> 用途：简要记录已经完成的代码、当前边界和下一步顺序。架构真相仍以根目录 `ARCHITECTURE.md` 为准。

## 当前状态

- 当前阶段：阶段 1「配置与代理」。
- 最近完成：Secret Vault 主密钥生命周期、版本化 AEAD 信封和启动校验闭环。
- 阶段 0 基线：`6b7d00f chore: scaffold any2api phase 0`。
- ProviderEndpoint 切片：`08e4913 feat: add provider endpoint configuration`。
- 本切片提交主题：`feat: add versioned secret vault`。

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
- 新增 Provider Web 页面，支持 URL 驱动编辑、风险开关提示、失效 revision 自愈、草稿冲突保护和窄屏布局；本切片未接入 Credential/Secret。
- 真实浏览器完成桌面与 390px 窄屏验证，覆盖创建 Claude 私网 HTTP Endpoint、两项授权、重启读取、deep link、历史返回、焦点和无水平溢出。
- 重启验证确认 SQLite revision 与 Endpoint 配置会重新读取，而 `scheduler_epoch` 按约束从 `0` 开始，不恢复旧运行态。
- DNS 最终地址校验、重定向限制和 Transport 连接绑定仍属于后续网络执行切片；本切片不进行网络 I/O。

### Secret Vault 切片

- 新增数据库外的版本化 `master-key.json`；默认位于数据目录，可用 `ANY2API_MASTER_KEY_FILE` 指向受保护文件或容器 Secret 挂载。
- 仅在 SQLite 尚无 Vault 元数据时允许首次自动生成 256 位主密钥；文件使用 create-new 语义，已有文件不会被覆盖。
- 首版固定使用 XChaCha20-Poly1305，每个密文信封使用独立的 192 位随机 nonce，并保存 envelope、algorithm、key、AAD 版本。
- AAD 使用固定二进制编码绑定记录 ID、Secret 类型和 Provider/Credential 类型，防止密文跨记录或跨 Provider 替换。
- SQLite migration 保存加密校验哨兵；后续启动必须使用同一主密钥成功解密，缺失文件、错误 Key、未知版本、篡改或认证失败都会在监听端口前终止启动。
- Unix 主密钥文件创建为 `0600` 并拒绝 group/other 权限；Windows 依赖数据目录或容器挂载继承的用户 DACL。
- `SqliteStore` 持有已验证的 `SecretVault`，后续 Credential 和代理密码仓储只能通过这一稳定 API 加解密。
- 单元与 Storage/契约测试覆盖随机 nonce、重启解密、错误 AAD、篡改、缺失/错误主密钥、同路径保护和脱敏 Debug。
- 本切片尚未创建 Credential 表、接收 Provider API Key、加密代理密码或实现主密钥轮换/恢复。

## 当前边界

- 尚未实现真实 HTTP/SOCKS5 网络转发、连接池、代理连通性测试和代理健康状态。
- 当前代理仍只保存 host/port；用户名与密码尚未接入，后续必须通过 Secret Vault 保存。
- 不要在单管理员认证完成前用 Nginx/Caddy 把管理 API 反代给远程客户端。
- 运行态并发、队列、健康、冷却和会话仍不持久化，重启后从空状态开始。

## 下一步

1. Codex、Claude 的 API Key `ProviderCredential` 创建、轮换和测试。
2. Credential 绑定代理，并把 `DIRECT → 全局代理` 接入实际运行时解析。
3. 多 `GatewayApiKey` 管理。
4. HTTP/SOCKS5 `TransportManager`、连接池和代理测试。
5. SettingRegistry、单管理员认证与可选 HTTP/HTTPS 远程管理。
6. 阶段 2 Codex Responses 原生链路。

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

以上门禁在本切片完成时全部通过；`cargo deny` 仅报告基线已有的重复传递依赖 warning。
