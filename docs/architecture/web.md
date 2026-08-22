# 管理 Web

本文是 React 管理面的代码所有权、服务端状态、实时更新和交互边界的当前规范。页面文案、视觉微调和组件结构
以实现为准，不在架构文档维护逐页布局清单。

## 目录与所有权

```text
web/src/
├── app/       router、应用 Provider、错误边界、导航和全局样式
├── pages/     路由级组合；不拥有可复用业务状态
├── features/  按管理能力组织 api、model、ui
├── shared/    API 基础设施、实时连接、通用 UI/工具和协议词汇
└── test/      测试环境与跨功能 helper
```

页面只组合所属 feature 的公开组件。一个 feature 拥有自己的 HTTP 调用、query/mutation 状态和业务 UI；只有被
多个 feature 真实共享、且没有更清晰业务所有者的机制才进入 `shared` 或应用壳。薄 barrel、只改名的 hook 和
重复 query-key wrapper 可以删除，调用方可直接依赖拥有该职责的模块。

`shared` 不成为业务垃圾桶：Provider 表格、OAuth 流程、日志游标等仍由相应 feature 拥有。跨层共享的
Provider/Protocol vocabulary 与 Rust 管理契约保持显式、集中和可测试。

## 服务端状态

TanStack Query 拥有可重新获取的服务端状态；组件局部 state 只保存搜索、展开、选中、表单草稿和短期交互。
Mutation 成功后以服务端返回 revision/对象更新或失效相关 query，不能维护一份长期平行配置模型。

配置编辑草稿绑定读取时的 `configRevision`。批量设置在一次管理 mutation 中提交；revision 冲突重新读取后由
用户决定是否重做。设置的类型、默认值、约束和 apply mode 来自管理 API；Web 只拥有标签、分组、单位和控件
呈现，不复制后端默认值。

管理 DTO 从 Rust 导出。手写 TypeScript vocabulary 只覆盖需要在收到响应前解析或展示的稳定公开枚举，并由
契约测试防止与后端漂移。

## 认证与持久化

管理员会话使用安全 Cookie，前端不把密码、Gateway token、Provider Secret、OAuth token document 或代理密码
写入浏览器持久化。Secret 表单只在局部 state 中存在，提交、关闭或卸载即释放；列表和详情使用脱敏投影。

允许持久化的内容限于非敏感显示偏好，例如主题或日志筛选，并使用版本化 key、严格解析和安全默认值。URL
只承载可分享的页面位置和非敏感筛选，不承载凭据。

401/403 使应用退出管理员态、关闭实时连接并停止自动重试。错误边界提供可恢复界面，不能把服务器错误正文
直接当 HTML 或诊断详情渲染。

## 实时状态和历史列表

认证后的应用壳只维护一个 `/api/admin/events` EventSource，由 `AdminRealtimeProvider` 向 feature 分发当前
overview snapshot 与日志失效 epoch。断线保留最近快照并标记 stale；重连恢复最新状态，不要求事件回放。

RequestLog 和 HTTP 系统日志仍通过 Keyset Cursor HTTP 查询。共享 cursor-feed hook 负责：

- 合并短时间内重复失效事件并单飞追赶；
- 从最新锚点读取到已知 ID 或缓存边界；
- 用户浏览历史时保持可见链稳定，回到最新时再应用待更新内容；
- scope、筛选或外部 generation 改变时取消旧结果；
- 限制缓存页数和累计条目数。

共享 hook 只拥有并发与游标机制；具体 item ID、合并方式、筛选 scope 和 UI 提示仍由日志 feature 提供。

## Provider 与协议界面

Provider 导航顺序和品牌展示属于 Web presentation；Provider 是否支持某种凭据、Operation、OAuth 或 Transport
来自后端 descriptor 投影。Endpoint 编辑器只允许 descriptor 声明的选择，不能通过 URL 或模型名自动改变
Provider kind。

路由检查页面展示已发布配置编译出的候选、可用性和理由，不创建另一套调度模拟器。管理操作成功后读取后端
事实，而不是在浏览器猜测 Runtime 变化。

## 响应式和可访问性

应用支持 deep link、浏览器自然滚动、文本选择和键盘操作。宽屏可以使用侧栏、表格和多列布局；窄屏使用折叠
导航、连续卡片或自然堆叠，不依赖固定桌面窗口。交互控件需要可见 focus、语义 label 和非颜色唯一状态；
loading、空态、错误与 stale 状态占用稳定布局，避免内容更新导致主要操作跳动。

大列表使用有界分页或虚拟化，大 JSON 详情限制语法高亮产生的节点数量；性能优化不能删除完整文本访问或键盘
路径。Chart 等大依赖按页面/组件延迟加载，刷新复用实例。

## 测试边界

- 纯 presentation、parser、hook 状态机和 query 合并使用 Vitest/Testing Library；
- feature API 契约验证请求/响应形状和错误映射；
- 页面测试验证组合、导航和关键可访问行为，不重复每个 feature 分支；
- Playwright 只覆盖登录/deep link、嵌入式 SPA fallback、核心页面可达、移动导航和无法由单元测试证明的
  前后端集成路径。

测试共享真实 Query/Router Provider，避免为每个组件复制不一致 mock 环境。生成绑定变化由完整应用构建后的
工作树差异检查验证。
