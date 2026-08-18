# any2api 项目规则

## 文档边界

- `ARCHITECTURE.md` 是当前架构事实的唯一来源。涉及跨模块设计、数据模型、协议、调度、代理、鉴权、存储或安全前，完整阅读它。
- `docs/adr/README.md` 是文档入口，`docs/adr/0170-current-decision-register.md` 是唯一当前登记册，只记录取舍理由和已舍弃方向，不复制当前事实。
- `README.md` 只写使用者需要的安装、运行和部署信息；`docs/baselines/` 只写外部证据；没有未完成事项时不要维护完成日志式 `TODO.md`。
- 用户改变架构要求时，先更新 `ARCHITECTURE.md`，必要时同步 ADR-0170，再实现代码。不要为同一事实新增第二份规范描述。
- `reference/` 仅用于只读参考 CLIProxyAPI、sub2api 和 new-api，禁止修改或复制其结构。

## 产品边界

- 项目是个人使用、自托管、单节点 AI API 聚合代理。永久不做用户注册、多租户、套餐、余额、计费、支付、API Key 销售和多节点调度。
- 不引入 Redis、PostgreSQL、消息队列或微服务解决单节点问题；不提供通用配置、数据库或 Secret 导入导出。
- OAuth JSON 只能作为独立 `OAuthAccount` 明文存入 SQLite，不进入普通日志、管理响应或浏览器存储，也不提供读取/导出端点。

## 依赖与代码结构

- 采用模块化单体：`domain` 不依赖 Web/SQLite/HTTP/Provider；`protocol` 负责线协议；`provider` 负责供应商契约但不发网络；`transport` 负责 DIRECT/HTTP/SOCKS5；`runtime` 负责编排；`storage` 负责 SQLite；`server` 负责 Axum/DTO；`app` 是唯一 Composition Root。
- Runtime 只依赖 Adapter 的稳定 `api` 出口。新增 Provider 必须通过局部模块、静态注册和契约测试完成，禁止修改中央调度器加入不断增长的 Provider `match`。
- 一个文件承担一个职责。生产源文件目标不超过 300 行；401–600 行必须进入 `architecture-allowlist.toml`，超过 600 行由架构检查拒绝。禁止垃圾桶式 `utils.rs`、`common.rs`、`manager.rs`、`service.rs`。
- React 按 feature 拆分，页面只组合，业务状态放 hooks/model，API 调用放 feature/api；feature 之间只通过公开出口依赖。Web 必须响应式、支持 deep link、自然滚动、文本选择和键盘访问。

## 生命周期与安全

- SQLite 是配置和必要凭据的真相来源。Schema 只追加连续、不可改写的前向 Migration；生产 Rust/TypeScript 只接受当前 Schema、当前 HTTP 契约和当前浏览器状态格式。
- 配置发布必须遵守“事务内构造并校验候选配置，Commit 后 reconcile，再一次性切换 PublishedSnapshot”；成功管理 API 只能在提交和快照切换完成后返回。
- `GatewayApiKey`、Provider API Key、OAuth Token、代理密码和原始 Session ID 不得进入普通 tracing/file log、RequestLog、错误正文、Debug 或浏览器持久化；ADR-0170 记录的 HttpAccessLog 详情例外必须保持已认证、受控和有界。
- 不实现运行态恢复、请求回放、队列恢复、会话恢复或复杂容灾。进程重启后 RPM、健康、冷却、队列、会话和请求进度从空状态开始。

## 工程规则

- 修改文件使用 `apply_patch`；保留用户已有改动，禁止 `git reset --hard`、`git checkout --` 或其他破坏性 Git 操作。
- 新功能和修复在能证明行为的最低充分层级提供测试；跨模块、公开协议或真实 I/O 才增加契约/集成测试，禁止机械重复同一分支。
- 调度关键路径必须覆盖 RPM 窗口、到期准入、无丢失唤醒、Guard 一次结算和热更新不重置窗口。SSE 必须覆盖任意字节切分、CRLF、多行 data、无尾空行、提交前重试和提交后禁止切换。
- Provider/Protocol 契约测试必须枚举实际 Registry 实现。提交前至少运行相关 `fmt`、`clippy`、`test`，以及前端 typecheck/lint/build；根 `pnpm build` 负责完整应用构建。
- Node/pnpm 是应用生命周期唯一编排者。根 `pnpm dev`、`pnpm build`、`pnpm package` 分别负责开发、production build 和分发；Cargo 命令必须保持 Rust-only，`build.rs` 不得调用 Node、联网或修改工作树。
