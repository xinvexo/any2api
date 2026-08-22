# any2api 协作指南

## 导航

- [ARCHITECTURE.md](ARCHITECTURE.md) 是系统地图；具体当前事实按主题放在
  [docs/architecture/](docs/architecture/README.md)，设计取舍按状态索引在
  [docs/adr/](docs/adr/README.md)。只读取与当前改动相关的部分。
- Rust 是模块化单体：`domain` 放领域类型，`protocol` 放线协议和桥接，`provider` 放供应商契约，
  `transport` 发起网络请求，`runtime` 编排请求与配置，`storage` 访问 SQLite，`server` 适配 HTTP，
  `app/any2api` 是 Composition Root。
- React 页面位于 `web/src/pages`，功能代码位于 `web/src/features`，跨功能基础设施位于
  `web/src/shared`，应用壳位于 `web/src/app`。
- `reference/` 是只读外部参考，不修改，也不直接复制其结构。

## 安全编辑

- 保留工作树中已有和并行产生的改动；不要使用 `git reset --hard`、`git checkout --` 或其他破坏性命令。
- 手工修改文件使用 `apply_patch`。格式化器、代码生成器和依赖管理器可以更新其拥有的机械产物。
- 改动应围绕一个可说明的职责。文件大小只用于评审提示，不是拆分目标；按领域或执行阶段形成内聚模块，
  不为满足数字机械增加包装文件。
- 不把 API Key、OAuth Token、代理密码、管理员凭据、Cookie、原始 Session ID 或包含它们的请求内容写入
  tracing、文件日志、请求日志、错误正文、Debug、管理 DTO、测试快照或浏览器持久化。

## 数据与迁移

- SQLite Migration 只追加，编号连续；已经提交的 SQL 和 checksum 不改写。Schema 变化同时更新下一条
  Migration、`migrations/checksums.toml` 和能证明旧库升级行为的测试。
- 生产路径只接受当前 Schema、当前 HTTP 契约和当前浏览器状态格式。兼容转换应在 Migration 或明确的
  外部导入边界完成。
- 配置变更必须经过现有 `ConfigPublisher` 路径；不要绕过候选配置校验、持久化提交、快照发布和运行态
  reconcile。

## 验证

- 运行与改动风险相称的最低充分检查。Rust 通常包括相关 `cargo fmt --check`、`cargo clippy` 和 `cargo test`；
  Web 通常包括相关测试、typecheck、lint，涉及完整应用资源时再运行根 `pnpm build`。
- 跨模块公开协议或真实 I/O 使用契约/集成测试；纯转换和状态分支优先单元测试，不在多个层级重复相同断言。
- Provider/Protocol 的公共契约测试应从实际 Registry 枚举实现。SSE、重试提交边界、RPM 准入、配置发布和
  Migration 等高风险路径必须保留针对其语义的覆盖。
- `cargo xtask architecture-check` 只检查 Workspace 依赖边界、Migration 历史和官方客户端基线安全性。
- 根 `pnpm dev`、`pnpm build`、`pnpm package` 是完整应用的便利入口；Cargo 命令保持 Rust-only，
  `build.rs` 不调用 Node、不联网、不修改源码树。
