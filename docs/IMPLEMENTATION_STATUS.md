# any2api 实施状态

> 最后更新：2026-07-18
> 用途：简要记录已经完成的代码、当前边界和下一步顺序。架构真相仍以根目录 `ARCHITECTURE.md` 为准。

## 当前状态

- 当前阶段：阶段 1「配置与代理」。
- 最近完成：代理配置最小闭环，可通过 Web 创建、编辑、删除和设置全局代理。
- 阶段 0 基线：`6b7d00f chore: scaffold any2api phase 0`。
- 本切片提交主题：`feat: complete proxy configuration slice`。

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

## 当前边界

- 尚未实现真实 HTTP/SOCKS5 网络转发、连接池、代理连通性测试和代理健康状态。
- 当前代理只保存 host/port；用户名与密码要等 Secret Vault 完成后接入。
- 不要在单管理员认证完成前用 Nginx/Caddy 把管理 API 反代给远程客户端。
- 运行态并发、队列、健康、冷却和会话仍不持久化，重启后从空状态开始。

## 下一步

1. ProviderEndpoint 配置与 URL/SSRF 校验。
2. Secret Vault 主密钥与版本化 AEAD 信封。
3. Codex、Claude 的 API Key `ProviderCredential` 创建、轮换和测试。
4. Credential 绑定代理，并把 `DIRECT → 全局代理` 接入实际运行时解析。
5. 多 `GatewayApiKey` 管理。
6. HTTP/SOCKS5 `TransportManager`、连接池和代理测试。
7. SettingRegistry、单管理员认证与可选 HTTP/HTTPS 远程管理。
8. 阶段 2 Codex Responses 原生链路。

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
