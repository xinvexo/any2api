# ADR-0027: 内嵌 React 资源与单二进制发布

- 状态：Accepted
- 日期：2026-07-22
- 修订：2026-08-18，由 ADR-0166 重构资源准备与构建生命周期
- 决策者：maintainer

## 背景

架构要求正式部署为单 Rust 二进制。若服务默认读取运行目录中的前端产物，工作目录、源码树和部署脚本
都会成为运行时依赖；若 Cargo 随意读取某个先前生成的资源目录，又无法证明内嵌 UI 来自当前源码。

在 Rust `build.rs` 中启动前端工具会让普通 Cargo 隐式依赖 Node，并允许构建阶段联网或修改工作树。
提交每次 Vite 生成的哈希文件也会制造大规模生成物 churn，却仍不能证明一次构建实际选择了哪个快照。

## 决策

- 正式应用仍将 React 资源内嵌进 Rust 可执行文件；App 默认装配内嵌资源。只有显式非空
  `ANY2API_WEB_DIR` 才选择外部目录，不以任何源码树输出目录作为隐式运行时默认值。
- `web/src`、前端 public 目录与 Vite 配置是 Web 源码真相。完整应用构建由 ADR-0166 定义的根 Node
  生命周期拥有：Vite 直接写入 `target` 下的临时 staging，Node 校验普通文件、要求 `index.html`、
  计算逐文件 SHA-256 和 bundle digest，再原子发布内容寻址的资源根与 manifest。
- Node 通过显式环境变量把本轮 manifest 交给 Cargo。Rust `build.rs` 只读取并严格验证 manifest 和资源，
  拒绝非法路径、符号链接、特殊文件、缺失或额外文件、大小或摘要不匹配，并在 `OUT_DIR` 生成包含
  `include_bytes!` 与最终字节强 `ETag` 的排序清单。它不启动 Node、pnpm 或 Vite，不联网、不修改工作树。
- 不带 manifest 的 Cargo-only 构建只嵌入仓库维护的最小 `rust-only` 占位页并给出警告，不读取先前完整
  应用构建留下的 bundle。生产 HTML、JS 和 CSS 不进入源码树或 Git。
- Server 定义 `WebAssets` 与 `EmbeddedWebAsset` 入口类型。外部目录继续由 `tower-http` 服务，但
  `/assets` 使用独立文件服务，缺失 asset 不进入 SPA fallback；内嵌实现对构建期排序清单二分查找，
  并提供 Content-Type、HEAD、缓存策略及 `If-None-Match` 条件请求。匹配时返回无 Body 的 `304 Not
  Modified`，同时保留 `ETag` 与原有 `Cache-Control`；deep link 使用 `index.html` 自身验证器。
- 两种资源来源共享 API 命名空间隔离、SPA deep link、缺失 `/assets/*` 的 404、非读取方法 405，以及
  管理 Web 专属安全响应头：最小权限 CSP 与 `Referrer-Policy: no-referrer`。合并后的全局响应边界统一
  添加 `X-Content-Type-Options: nosniff`；不设置 HSTS，以保留受支持的内网 HTTP 部署。
- Playwright E2E 复用完整应用 build primitive，并通过 Cargo JSON 消息取得本轮真实二进制路径；测试
  从独立临时工作目录启动，不设置 `ANY2API_WEB_DIR`，以验证本轮 manifest 对应的内嵌资源。

## 取舍

- 二进制包含一份压缩前的前端产物，体积较大，但部署不需要独立 Web 目录。首版不引入运行时压缩、
  虚拟文件系统或模板引擎。
- 完整应用构建需要 Node 与 pnpm；Rust-only 质量门禁无需 Node，但其占位 Web 明确不等于正式应用。
- 内容寻址 staging 与严格 manifest 增加少量构建工具复杂度，换取当前源码到嵌入字节的可证明关系，
  并消除提交生成 bundle 的工作树 churn。
- 外部目录是开发和诊断入口，不是第二套正式发布模型；文件系统响应可继续使用 `tower-http` 的元数据
  实现，但必须保持与内嵌来源相同的 API 隔离和 SPA 入口语义。

## 后果

- 正式二进制可以离开仓库独立运行，管理页面不依赖当前工作目录。
- 完整 BUILD、E2E 与 Release 使用同一 manifest 和 Cargo artifact 解析路径，不会先构建新前端再嵌入旧 UI。
- 普通 Cargo 构建的 Web 能力是显式可见的占位状态，不能被误认为当前完整应用。

## 验证

- Server 单元测试覆盖内嵌首页、精确 JS/CSS、HEAD、deep link、二分路径查找、强/弱/通配
  `If-None-Match`、`304`、缺失 asset 404 与非读取方法 405；外部目录契约覆盖精确 asset、API 根隔离
  和 deep link。
- manifest 测试覆盖排序稳定性、内容摘要、非法路径、符号链接、缺失、额外文件、篡改与缺失首页；任一
  校验失败必须终止 Cargo 构建。
- 完整应用二进制复制到不含源码或外部前端目录的临时位置后仍返回本轮首页和哈希资源。
- 不带 manifest 的 Cargo-only 二进制返回明确占位页，且不包含任何先前完整应用 bundle。
- Playwright 在未设置 `ANY2API_WEB_DIR` 时完成登录、刷新 deep link、桌面与移动页面契约。
