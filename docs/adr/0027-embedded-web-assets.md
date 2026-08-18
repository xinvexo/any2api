# ADR-0027: 内嵌 React 资源与单二进制发布

- 状态：Accepted
- 日期：2026-07-22
- 修订：2026-08-18
- 决策者：maintainer

## 背景

架构要求正式部署为单 Rust 二进制，但当前服务默认从运行目录读取 `web/dist`。这会让工作目录、源码树和前端构建产物成为运行时依赖，也使浏览器 E2E 绕过正式发布路径。

直接在 Rust `build.rs` 中调用 pnpm 会让普通 `cargo build` 隐式依赖 Node，并在构建阶段修改工作树；只在发布脚本临时复制 `dist` 又无法保证仓库中的 Rust 提交可以独立重现当前二进制资源。

## 决策

- `web/src` 与前端配置是源码真相；`app/any2api/web-assets` 是机器生成的构建工作区，禁止手工编辑。仓库可保留最近一次生成快照供 Rust-only 构建使用，但快照不是 Web 源码变更的同步门禁。
- 固定 Node 脚本负责同步 `web/dist` 与 `web-assets`；`--check` 只提供同步后的显式诊断，不再作为 CI 或 Release 的阻断步骤。
- `cargo xtask package` 是从当前源码生成完整单二进制的唯一正式入口：每次依次执行前端生产构建、同步内嵌目录、同步结果复核和锁定依赖的 Rust release 构建。可选 `--target <triple>` 只改变 Rust 目标。GitHub CI 与 Release 使用同一自动同步入口，不能复制一套手写步骤。
- 普通 `pnpm build` 只生成 `web/dist`；`pnpm build:embedded` 是自动构建加同步原语。Web CI、E2E 和 Release 在编译前自动调用该流程，不要求前端变更额外提交哈希文件名产物。
- Rust `build.rs` 递归扫描打包阶段生成的资源，要求存在 `index.html`，拒绝符号链接及其他特殊文件，按规范化相对路径排序，并在 `OUT_DIR` 生成包含 `include_bytes!` 与最终字节强 `ETag` 的清单。Rust 构建不启动 Node、pnpm 或 Vite。
- Server 定义 `WebAssets` 与 `EmbeddedWebAsset` 入口类型。外部目录继续由 `tower-http` 服务，但 `/assets` 使用独立文件服务，缺失 asset 不进入 SPA fallback；内嵌实现对构建期排序清单二分查找，并提供 Content-Type、HEAD、缓存策略及 `If-None-Match` 条件请求。匹配时返回无 Body 的 `304 Not Modified`，同时保留 `ETag` 与该资源原有的 `Cache-Control`；deep link 使用 `index.html` 自身的验证器。两种来源共享 API 命名空间隔离、SPA deep link、缺失 `/assets/*` 的 404 和非读取方法 405 语义。
- 两种资源来源共享管理 Web 专属安全响应头：最小权限 CSP 与 `Referrer-Policy: no-referrer`。CSP 禁止 frame 嵌入且不依赖 nonce 或运行时 HTML 改写；合并后的 Server 全局响应边界为 Web 与全部 API 统一添加 `X-Content-Type-Options: nosniff`。不设置 HSTS，以保留受支持的内网 HTTP 部署。
- App 默认装配内嵌资源。只有显式非空 `ANY2API_WEB_DIR` 才选择外部目录；不再以 `web/dist` 作为隐式默认值。
- Playwright E2E 先构建并校验前端产物，再通过 Cargo JSON 构建消息取得本轮真实二进制路径；启动服务时按大小写不敏感规则移除宿主继承的全部 `ANY2API_*` 配置，只注入测试数据目录、监听地址和管理员密码，从独立临时工作目录验证正式内嵌路径。
- `app/any2api/web-assets/**` 在 Git 中按原始字节追踪，不执行文本换行转换；同步脚本的源和目标都只接受普通目录与普通文件。

## 取舍

- 仓库和二进制会增加一份压缩前的前端产物体积，但部署不再携带独立目录，Rust-only 构建也不依赖 Node。首版不引入运行时压缩、虚拟文件系统或模板引擎。
- 前端变更由 `cargo xtask package`、CI 和 Release 自动同步生成产物；内容哈希变化直接替换文件，不保留资源名兼容别名。显式 `check:embedded` 仍可用于诊断同步结果，但不会要求 GitHub checkout 与上一次快照相同。
- 外部目录是开发和诊断入口，不是另一套正式发布模型；两种来源共享 API 隔离和 SPA 入口语义，但文件系统响应可继续使用 `tower-http` 的元数据实现。

## 后果

- 正式二进制可以离开仓库独立运行，管理页面不受当前工作目录影响。
- 干净 Rust CI、`cargo test` 和普通 `cargo build --release` 使用 checkout 中最近一次生成快照，无需安装 Node；它们是 Rust-only 构建入口，不宣称重新生成当前 Web 源码。需要得到包含当前前后端源码的可发布二进制时使用 `cargo xtask package`，该入口要求 Node 与 pnpm。
- E2E、同步检查和 Server 单元测试共同覆盖资源来源选择、deep link、Content-Type、缓存和 API fallback 隔离。

## 验证

- Server 单元测试覆盖内嵌首页、精确 JS/CSS、HEAD、deep link、二分路径查找、强/弱/通配 `If-None-Match`、`304`、缺失 asset 404 与非读取方法 405；外部目录契约覆盖精确 asset、缺失 asset、API 根路径隔离和 deep link。
- 前端同步脚本在同步阶段遇到缺失、非法文件或复制失败时失败；自动打包入口在同步后再次复核生成结果。显式 `check:embedded` 只报告当前工作区是否已经同步。
- `xtask package` 的参数解析测试覆盖默认 host 构建、显式 target、帮助与非法参数；命令任一阶段失败必须立即停止，不能继续产出看似成功的旧资源二进制。
- Playwright 在未设置 `ANY2API_WEB_DIR` 时完成登录、刷新 deep link、桌面与移动页面契约。
- Release 二进制复制到不含 `web/dist` 的临时目录后仍能返回首页和哈希资源。
