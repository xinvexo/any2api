# ADR-0044：Provider 专用 OAuth JSON 批量导入

- 状态：Accepted
- 日期：2026-07-25
- 决策人：项目维护者
- 修订：ADR-0112 将 SQLite 持久化格式收敛为唯一 any2api OAuthAccount Schema；ADR-0133 拒绝可证明重复的导入身份；本 ADR 只定义仍受支持的外部导入协议。

## 背景

any2api 已能通过交互式登录创建 Codex、Claude 和 Grok `OAuthAccount`，但无法接收已有工具产生的账号文件。实际迁移来源有两类：CLIProxyAPI 的单账号平铺 JSON，以及 Sub2API 的 Codex session JSON、账号对象和带 `accounts` 数组的备份包。一个文件也可能包含多个 OAuth 账号，管理员还需要一次选择多个文件。

导入后的 Token 必须真正进入 SQLite `OAuthAccount` 并参与统一路由，不能只保存上传文件。与此同时，项目仍禁止通用配置、数据库或 Secret 导入，OAuth JSON 也不得通过管理响应、日志、浏览器存储或导出端点泄漏。

## 决策

1. 新增受管理员会话与 CSRF 保护的 `POST /api/admin/oauth/import`。请求使用 `multipart/form-data`，通过重复的 `files` part 接收多个 JSON 文件；不接受路径、服务器目录或浏览器下载回路。
2. 每个文件只接受一个账号对象、账号对象数组，或已审计的 Sub2API `accounts` envelope。CLIProxyAPI 的 `type=codex|claude|xai`、Sub2API 的 `platform=openai|anthropic|grok` + `type=oauth`、Codex `tokens`/camelCase session 字段会在对应 Provider 模块中解析。`xai` 规范化为 any2api 的 `grok`。
3. Provider Driver 负责识别和解析自己的外部账号结构。跨 Provider 基础设施只展开文件 envelope、枚举已注册 Driver 并拒绝无法识别或多重识别的条目；中央调度器和 Server Handler 不按 Provider 增长 `match`。
4. 外部对象只提取 Token、账号身份、安全邮箱、绝对过期时间和可见标签。Sub2API 的代理、并发、优先级、分组、费率、运营扩展和非 OAuth 账号不进入 any2api。解析结果立即序列化为 ADR-0112 定义的唯一 any2api OAuthAccount JSON；未知字段、字段别名和外部 wrapper 不进入 SQLite 或运行时读取路径。
5. 整次 HTTP 请求是 all-or-nothing：所有文件和账号先完成大小、数量、JSON、Provider、Token、路由目录与领域校验；随后在一个 `BEGIN IMMEDIATE` 事务中创建全部 `OAuthAccount` 和默认模型集合，只增加一次 config revision。Commit 后执行一次 Runtime reconcile 和一次 `ArcSwap<PublishedSnapshot>`。
6. 导入账号默认启用、RPM 不限、固定绑定 DIRECT 并选择 Provider OAuth 默认模型。标签优先使用来源名称或安全邮箱，并在发布锁内针对当前 Provider 生成唯一后缀；导入不覆盖、合并或更新既有账号。ADR-0133 进一步要求在同一发布锁内拒绝与当前账号或同批输入可证明重复的 Provider 身份，或同一 Provider 下任一完全相同的 access/refresh/ID Token，冲突整批回滚；这不是覆盖或合并。
7. 响应只返回导入数量、新 revision 和新账号的安全元数据。错误只包含文件/账号序号和稳定错误分类，不回显文件名、JSON、Token 或 Provider 原始错误正文。
8. 上传边界固定为最多 32 个文件、单文件 2 MiB、请求总文件内容 8 MiB、最多 2,000 个账号。任一边界超限时整批拒绝。前端文件只保存在导入抽屉的局部组件状态，提交完成、失败、关闭或卸载时清空，不进入 React Query、Mutation Cache、URL、localStorage 或 sessionStorage。
9. 本功能不增加 OAuth JSON 的读取、下载、导出、通用 Secret 导入或 ProviderCredential 导入，也不改变 OAuthAccount 与 API Key 只在 `RoutingCredential` 投影合流的边界。

## 备选方案

- 逐账号调用现有激活接口：会产生多个 revision，并允许中途失败留下半批账号。
- 原样保存上传文件：会把外部 wrapper 和无关运营字段变成持久化契约，也无法保证刷新与运行时读取使用 canonical schema。
- 自动覆盖同邮箱或账号 ID 的既有记录：不同订阅账号可能共享邮箱或组织 ID，隐式合并会造成 Token 覆盖；导入只创建新账号。能够按 ADR-0133 证明为重复时拒绝整批，而不是覆盖。
- 把导入放进 Provider API Key 页面：会混淆 `ProviderCredential` 与 `OAuthAccount` 的永久管理边界。

## 后果

- CLIProxyAPI/Sub2API 中已有的受支持 OAuth 账号可以直接迁入 SQLite 并立即参与统一 RPM、轮询、粘性、健康、重试和遥测路径。
- 大批量导入只触发一次完整配置编译和快照切换；任一坏条目不会留下半成品。
- CLIProxyAPI/Sub2API 格式是显式、可测试的当前外部输入协议，不是 any2api 历史 Schema 的运行时兼容层，也不演变为通用备份恢复或 Secret 导入框架。

## 验证

- Provider 单元测试覆盖 CLIProxyAPI Codex/Claude/xAI、Sub2API `credentials` 账号、Codex camelCase/`tokens`、数组与 `accounts` envelope。
- Storage 测试覆盖多账号单事务、单 revision、标签冲突和中途失败全量回滚。
- HTTP 契约测试覆盖多文件、单文件多账号、边界限制、CSRF、脱敏响应和失败时零写入。
- Web 单元测试覆盖多文件 FormData、成功/失败清空文件状态，且查询与 Mutation Cache 不持有 `File`。
