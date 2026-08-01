# ADR-0080: 全面审查后的边界纠正与迁移顺序

- 状态：Accepted
- 日期：2026-07-31
- 决策者：maintainer

## 背景

本次审查以实际依赖图、事务路径、任务生命周期和浏览器契约为准，而不是假定现有文档已经被代码落实。
确认了三类实质偏差：Runtime 仍从 Adapter crate 根导入内部出口；应用更新使用未注册到进程生命周期的
`tokio::spawn`；配置 Repository 在候选快照完整校验和预编译前已经提交 SQLite。前端还保留了对旧存储
字段的显式识别，但项目已经决定正常运行只使用明文 SQLite。数据库历史仍按 ADR-0063 追加迁移，不能以
尚未正式发布为由改写既有 Migration。

远程管理默认开启是明确产品要求。它只放行已经能连接监听地址的非 loopback 客户端，不改变默认 loopback
监听、管理员认证、CSRF、会话、可信反向代理和明文 HTTP 风险提示。此次迁移不得把它改回默认关闭。

## 决策

1. 先删除日常运行文档和浏览器契约中的旧存储兼容描述。Provider API Key、代理密码、Gateway API Key 与
   OAuth Provider JSON 原样明文存入 SQLite；代码中的 Secret wrapper 只承担内存脱敏。前向 Migration 只形成
   当前 Schema，不提供旧 Secret 数据转换；正常 Repository 不保留双轨读写。
2. Runtime 生产代码只从 `protocol::api`、`provider::api`、`transport::api` 与 `storage::api` 导入跨 crate 类型。
   Adapter crate 根只保留 Composition Root 注册具体实现和 crate 自身测试所需的构造出口；增加机器门禁防止回退。
3. 应用更新任务由 Composition Root 注入唯一 `ProcessLifecycle`，使用其后台 TaskTracker 启动。更新请求取消
   不能取消任务；Draining 后拒绝新安装；Forced 能有界取消并等待任务，禁止遗留脱管 JoinHandle。
4. 配置发布迁移到“事务内形成候选 `StoredConfiguration` → 完整能力校验和快照预编译 → Commit → 无失败
   Runtime reconcile → 单次 ArcSwap”。Repository 不再拥有自行 Commit 后返回配置的发布协议；Storage 暴露受控
   的候选事务边界，Runtime/ConfigPublisher 负责候选验收，App 仍是唯一装配根。所有 OAuth 批量创建和刷新
   必须走同一路径。
   `xtask architecture-check` 同时禁止其他生产模块直接调用候选事务 API；Storage trait/实现是定义边界，
   Runtime ConfigPublisher 是唯一生产调用者，测试调用必须经过显式夹具。
5. 候选预编译返回显式错误，不以 `expect`/`assert` 充当外部配置或 OAuth 文档的校验。Commit 后只允许
   已经在候选阶段证明不会失败的内存 reconcile；若 Commit 本身失败，不切换快照。
6. `admin.remote_enabled` 的代码默认值保持 `true`，并由架构门禁和 Domain 测试固定。不得因安全审查把受
   支持的 HTTP、反向代理或公网部署路径错误地改成默认不可访问。

## 迁移顺序

1. 清理密文兼容字段和文档表述，固定远程管理默认值测试。
2. 收敛 Runtime 稳定 API 导入并增加架构检查。
3. 将更新任务迁入 `ProcessLifecycle`，补齐停机竞态测试。
4. 重构配置 Repository/Publisher 的候选事务协议；一次性迁移普通配置写、OAuth 批量激活和批量刷新。
5. 运行 Workspace fmt、Clippy、测试、架构检查以及前端 typecheck/lint/test/build/embedded 校验。

## 后果

发布链重构会修改 Storage 与 Runtime 的内部稳定 API，不保留错误的旧 Repository 兼容层或双轨发布路径；
这条源码重构规则不授权改写数据库 Migration 历史。迁移完成后，数据库真相、PublishedSnapshot revision 和运行时注册表不会出现已提交但未发布的
中间状态；更新任务也不会越过实例锁和 Tokio runtime 生命周期。

## 验证

- 架构检查拒绝 Runtime 生产代码从 Adapter crate 根导入。
- 更新器测试证明 HTTP 请求返回后任务继续、Draining 拒绝、Forced 收敛和成功后仅请求一次重启。
- 配置发布故障注入测试证明候选验证/预编译失败时 SQLite revision 不变，Commit 失败时快照不变，成功时
  数据库、快照和 Gateway 鉴权/路由 revision 一次切换。
- Domain 与 Web 契约测试证明远程管理默认开启，读取 DTO 不接受本地密文兼容形态。
