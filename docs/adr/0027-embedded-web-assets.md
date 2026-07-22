# ADR-0027: 内嵌 React 资源与单二进制发布

- 状态：Accepted
- 日期：2026-07-22
- 决策者：maintainer

## 背景

架构要求正式部署为单 Rust 二进制，但当前服务默认从运行目录读取 `web/dist`。这会让工作目录、源码树和前端构建产物成为运行时依赖，也使浏览器 E2E 绕过正式发布路径。

直接在 Rust `build.rs` 中调用 pnpm 会让普通 `cargo build` 隐式依赖 Node，并在构建阶段修改工作树；只在发布脚本临时复制 `dist` 又无法保证仓库中的 Rust 提交可以独立重现当前二进制资源。

## 决策

- `web/src` 与前端配置是源码真相；`app/any2api/web-assets` 是机器生成、需要提交的正式构建输入，禁止手工编辑。
- 固定 Node 脚本负责比较或同步 `web/dist` 与 `web-assets`。同步模式只在开发者明确执行时修改目标目录；校验模式只读比较，CI 不自动修复差异。
- Rust `build.rs` 递归扫描已提交资源，要求存在 `index.html`，拒绝符号链接及其他特殊文件，按规范化相对路径排序，并在 `OUT_DIR` 生成 `include_bytes!` 清单。Rust 构建不启动 Node、pnpm 或 Vite。
- Server 定义 `WebAssets` 与 `EmbeddedWebAsset` 入口类型。外部目录继续由 `tower-http` 服务，但 `/assets` 使用独立文件服务，缺失 asset 不进入 SPA fallback；内嵌实现只读取静态字节表，并提供 Content-Type、HEAD 与缓存策略。两种来源共享 API 命名空间隔离、SPA deep link、缺失 `/assets/*` 的 404 和非读取方法 405 语义。
- App 默认装配内嵌资源。只有显式非空 `ANY2API_WEB_DIR` 才选择外部目录；不再以 `web/dist` 作为隐式默认值。
- Playwright E2E 先构建并校验前端产物，再通过 Cargo JSON 构建消息取得本轮真实二进制路径；启动服务时按大小写不敏感规则移除宿主继承的全部 `ANY2API_*` 配置，只注入测试数据目录、监听地址和管理员密码，从独立临时工作目录验证正式内嵌路径。
- `app/any2api/web-assets/**` 在 Git 中按原始字节追踪，不执行文本换行转换；同步脚本的源和目标都只接受普通目录与普通文件。

## 取舍

- 仓库和二进制会增加一份压缩前的前端产物体积，但部署不再携带独立目录，Rust-only 构建也不依赖 Node。首版不引入运行时压缩、虚拟文件系统或模板引擎。
- 前端变更需要同步生成产物；CI 的只读一致性检查防止遗漏。内容哈希变化直接替换文件，不保留旧资源别名。
- 外部目录是开发和诊断入口，不是另一套正式发布模型；两种来源共享 API 隔离和 SPA 入口语义，但文件系统响应可继续使用 `tower-http` 的元数据实现。

## 后果

- 正式二进制可以离开仓库独立运行，管理页面不受当前工作目录影响。
- 干净 Rust CI、`cargo test` 和 `cargo build --release` 使用已提交资源，无需安装 Node。
- E2E、同步检查和 Server 单元测试共同覆盖资源来源选择、deep link、Content-Type、缓存和 API fallback 隔离。

## 验证

- Server 单元测试覆盖内嵌首页、精确 JS/CSS、HEAD、deep link、缺失 asset 404 与非读取方法 405；外部目录契约覆盖精确 asset、缺失 asset、API 根路径隔离和 deep link。
- 前端同步脚本在内容或文件清单不一致时失败，并提示明确的同步命令。
- Playwright 在未设置 `ANY2API_WEB_DIR` 时完成登录、刷新 deep link、桌面与移动页面契约。
- Release 二进制复制到不含 `web/dist` 的临时目录后仍能返回首页和哈希资源。
