# ADR-0022: 真实服务浏览器 E2E 契约

- 状态：Accepted
- 日期：2026-07-21
- 决策者：maintainer

## 背景

React 单元测试和 Axum 契约测试已经分别覆盖页面状态与 SPA fallback，但它们无法共同证明浏览器登录 Cookie、登录后的原始 deep link、真实静态资源、移动导航和最终 CSS 布局。此前这些能力依赖分散的人工桌面/390px 验收，容易随页面增长而遗漏。

## 决策

- 使用 Playwright Chromium 建立统一浏览器 E2E；它属于全站工程基础设施，不绑定单一 feature。
- 测试启动真实 any2api 二进制，使用独立临时数据目录、固定测试管理员密码和随机可用 loopback 端口。ADR-0027 完成后，服务从临时工作目录使用二进制内嵌 React 资源，不设置 `ANY2API_WEB_DIR`；已构建 `web/dist` 只用于和提交的内嵌产物执行一致性校验。测试结束后停止服务并删除临时状态。
- 每个测试使用新的 BrowserContext，不复用 Cookie；首次访问 deep link 时完成真实管理员登录，并断言登录后仍停留在原目标 URL。
- 桌面用例覆盖核心管理页面的直接访问与刷新；移动用例使用 390×844，覆盖折叠导航、页面切换和 `scrollWidth <= innerWidth`。
- 所有用例收集浏览器 `pageerror` 和 error 级 console 事件，测试结束时统一断言为空。
- CRUD、复杂表单、错误矩阵、Secret 生命周期和后端业务不变量不在浏览器层重复覆盖，继续由已有 Domain、HTTP 契约和 React 单元测试负责。
- 浏览器产物只在失败时保留 trace；截图、报告、临时数据和浏览器缓存不得提交到仓库。

## 备选方案

- 继续人工验收：无法形成可重复的回归门禁，放弃。
- 只使用 jsdom 断言 Tailwind 类名：不能证明真实布局、静态资源、Cookie 和导航历史，保留为组件级快速测试但不足以替代 E2E。
- 为每个页面建立完整浏览器 CRUD 套件：执行慢且与现有契约/组件测试大量重复，违背复杂度比例原则，放弃。

## 后果

Web 开发依赖增加 Playwright，CI 需要安装 Chromium。普通 `pnpm test` 仍只运行快速 Vitest；`pnpm test:e2e` 构建 Rust 与 Web 并运行浏览器契约。浏览器套件保持小而稳定，只扩展无法在更低测试层证明的跨层行为。

## 验证

- 登录后保留 `/settings` deep link，并在刷新后仍显示设置页。
- 桌面直接访问核心管理页面，等待真实 API 数据并断言无水平溢出。
- 390×844 下打开移动导航并跳转请求日志页，断言菜单关闭、标题正确且无水平溢出。
- 全部用例断言没有未处理 page error 或 error 级 console 日志。
