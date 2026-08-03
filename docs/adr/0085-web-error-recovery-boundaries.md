# ADR-0085：Web 根级与路由级错误恢复边界

- 状态：Accepted
- 日期：2026-08-03
- 决策者：maintainer

## 背景

React Router 可以捕获路由元素内部的渲染和懒加载错误，但默认错误页是面向开发者的英文界面；`AppProviders`、应用更新 Provider、管理员认证门和全局通知宿主位于 Router 外，任一渲染错误都会卸载整棵 React 树并留下纯白页。自更新替换带 hash 的静态资源后，旧标签页首次进入未加载路由还可能遇到 chunk 404，空 `Suspense` fallback 无法向用户区分“正在加载”和“已经失效”。

## 决策

- `App` 的最外层使用 React class Error Boundary，并放在 `AppProviders` 之外，使 Provider 初始化、更新流程、认证门、RouterProvider 和 NotificationHost 的渲染异常都能落到同一最后恢复页。
- Router 根 Route 配置中文 `errorElement`，接管 AppShell、子页面、loader/action 和 lazy chunk 的未处理错误，不使用 React Router 默认开发者错误页。
- 两层恢复页只说明界面或页面加载失败，不渲染原始 `Error.message`、stack、Response body 或查询内容；异常可能含服务端诊断或敏感输入，普通管理 UI 不应扩大暴露面。
- 恢复入口使用当前 URL 的普通重新加载，保留 deep link 并让浏览器重新取得最新入口 HTML 与 hash 资源。禁止自动 reload 或无限重试，避免持续故障形成刷新循环。
- 每个页面 lazy boundary 使用非空、带 `role=status` 的中文加载状态；加载、路由错误和根级错误必须具有可区分的可访问语义。

## 后果

- Router 外 Provider 崩溃不再产生不可操作白屏；路由渲染和 chunk 失效也不会退回英文开发者页。
- Error Boundary 只恢复渲染失败，不替代 Query 的正常请求错误、表单就地错误或应用更新状态机；这些仍由各 feature 负责。
- 显式重新加载会丢弃未保存的纯前端草稿，这是最后恢复路径的预期代价，不额外持久化草稿或异常状态。

## 验证

- React 单元测试覆盖 Router 外子树抛错、生产 Router `errorElement` 接管路由抛错、中文重新加载入口和非空 Suspense fallback。
- TypeScript、ESLint、生产构建和 embedded assets 一致性门禁保持通过。
