# ADR-0080: 全面审查后的边界纠正与迁移顺序

- 状态：Accepted
- 日期：2026-07-31
- 修订：2026-08-03
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
   Adapter 契约类型只有 `api` 一个公开路径，不在 crate 根重复重导出。Provider crate 根只公开 Composition Root
   静态注册所需的 `ClaudeDriver`、`CodexDriver` 与 `GrokDriver`；`ProviderRegistry`、`ProviderDriver`、错误、
   Secret、OAuth 类型和辅助函数统一从 `provider::api` 导入。架构门禁同时检查 Runtime 导入与这组三项根导出，
   测试辅助代码也迁移到同一公开路径，不保留双轨示例。
3. 应用更新任务由 Composition Root 注入唯一 `ProcessLifecycle`，使用其后台 TaskTracker 启动。更新请求取消
   不能取消任务；Draining 后拒绝新安装；Forced 能有界取消并等待任务，禁止遗留脱管 JoinHandle。
4. 配置发布迁移到“事务内形成候选 `StoredConfiguration` → 完整能力校验和快照预编译 → Commit → 无失败
   Runtime reconcile → 单次 ArcSwap”。Storage 暴露受控的候选事务边界，Runtime/ConfigPublisher 负责候选
   验收，App 仍是唯一装配根。进一步收窄后，活 SQLite `Transaction` 不再随 `PreparedConfiguration` 交给
   Runtime：ConfigPublisher 只传入同步候选编译回调，Storage 在内部按回调结果 Commit/Rollback，并只返回终态
   与无事务能力的已编译结果。所有 OAuth 批量创建和刷新必须走同一路径。
   `xtask architecture-check` 同时禁止其他生产模块直接调用候选事务 API；Storage trait/实现是定义边界，
   Runtime ConfigPublisher 是唯一生产调用者，测试调用必须经过显式夹具。
5. 候选预编译返回显式错误，不以 `expect`/`assert` 充当外部配置或 OAuth 文档的校验。Commit 后只允许
   已经在候选阶段证明不会失败的内存 reconcile；若 Commit 本身失败，不切换快照。
6. `admin.remote_enabled` 的代码默认值保持 `true`，并由架构门禁和 Domain 测试固定。不得因安全审查把受
   支持的 HTTP、反向代理或公网部署路径错误地改成默认不可访问。
7. Storage 保留配置 mutation 后从同一事务视图回读并核对候选的防线，但 revision 和各配置聚合的失配
   必须返回带组件枚举的 `ConfigurationWriteMismatch`，使候选事务自然回滚；禁止用 `assert_eq!` 把可恢复的
   持久化一致性故障升级为进程 panic。诊断只标识失配组件，不格式化含明文 Secret 的完整配置值。
8. 多行 RequestLog 列表只隔离明确的单行 `CorruptTelemetry`，每次查询汇总一次不含行内容的计数告警；
   SQL/事务错误、单条详情与 Attempt 解码、配置和 Secret 加载继续失败。该例外不能扩张成 Storage 全局
   “忽略损坏”策略，也不能把损坏行从精确的持久化 `total` 中按当前页局部扣除。
9. RequestLog 行数裁剪在写入事务内按集中预算执行；若历史积压或设置下调超过单笔预算，Storage 返回
   `has_more`，Runtime 通过持久化 Writer 内部 `Notify` 继续有界事务直到收敛。配置发布立即唤醒清理，
   不把公开请求当作清理触发器，也不把请求日志上限改成永久计数器或第二套队列。
10. SQLite 为 `request_attempts(request_id, attempt_no)` 复合主键生成的自动索引就是详情查询所需索引；
    与它同列同序的显式 `request_attempts_request_idx` 只增加每次 Attempt 写入成本。冻结 `0001` 不改写，
    由追加的 `0008` 删除冗余索引并用带数据升级测试固定最终 Schema。
11. 配置 revision 条件递增返回 0 行时，Storage 必须从同一事务视图读取实际 revision：不匹配返回
    `RevisionConflict { expected, actual }`，actual/expected 同为 SQLite INTEGER 上限才返回 `RevisionOverflow`；
    actual 仍等于可递增 expected 的异常未写入返回 revision `ConfigurationWriteMismatch`，不得误报溢出。
12. 系统日志 IP 先使用 Domain 的共享规范函数把 IPv4-mapped IPv6 转为 IPv4，再判断 loopback；Server 解析与
    Storage 写入共同执行，`0009` 规范化旧 `::ffff:127.*` 行。COUNT 与分页只引用一个 SQL 保留谓词常量，
    不再各自复制字符串近似规则；系统日志降噪不改变管理权限对直接 loopback TCP 的更强要求。

## 迁移顺序

1. 清理密文兼容字段和文档表述，固定远程管理默认值测试。
2. 收敛 Runtime 稳定 API 导入并增加架构检查。
3. 将更新任务迁入 `ProcessLifecycle`，补齐停机竞态测试。
4. 重构配置 Repository/Publisher 的候选事务协议；一次性迁移普通配置写、OAuth 批量激活和批量刷新。
5. 运行 Workspace fmt、Clippy、测试、架构检查以及前端 typecheck/lint/test/build/embedded 校验。

## 后果

发布链重构会修改 Storage 与 Runtime 的内部稳定 API，不保留错误的旧 Repository 兼容层、活事务句柄或双轨发布路径；
这条源码重构规则不授权改写数据库 Migration 历史。迁移完成后，正常发布不会把已提交但未发布的中间状态暴露给
继续运行的服务；SQLite phase-two I/O 或确认丢失导致提交结果不确定时进程 fail-fast，下次启动从数据库真相重建，
不会带旧 PublishedSnapshot 继续服务。更新任务也不会越过实例锁和 Tokio runtime 生命周期。

## 验证

- 架构检查拒绝 Runtime 生产代码从 Adapter crate 根导入，并拒绝 Provider crate 根公开三种具体 Driver 之外的
  任何平行 Adapter 出口；契约测试和测试夹具使用与生产代码相同的 `provider::api` 类型路径。
- 更新器测试证明 HTTP 请求返回后任务继续、Draining 拒绝、Forced 收敛和成功后仅请求一次重启。
- 配置发布故障注入测试证明候选验证/预编译失败和明确的 Commit 约束拒绝使 SQLite revision/快照不变；成功时
  数据库、快照和 Gateway 鉴权/路由 revision 一次切换；phase-two I/O 的不确定提交则进入进程 fail-fast。
- Domain 与 Web 契约测试证明远程管理默认开启，读取 DTO 不接受本地密文兼容形态。
- Storage 单元测试枚举 revision、Gateway Key、Proxy、OAuthAccount、Provider Endpoint/Credential、Model Route
  与 Setting 组件，证明一致时继续、失配时返回精确类型化错误；生产写路径不再包含配置回读断言。
- Storage 与管理 HTTP 契约测试证明损坏 RequestLog 只从列表 items 隔离、其余行与持久化 total 仍可读取，
  同一损坏记录的详情继续返回存储不可用；既有配置与 Secret 损坏测试保持 fail-closed。
- Storage/Runtime 测试证明单笔删除预算、超旧清理速率的突发写入、设置下调后的多轮收敛和内部唤醒，
  并确认变更 epoch 只在实际提交或删除后推进。
- Migration 升级测试证明 `0008` 保留 RequestLog/Attempt 代表数据和复合主键唯一性，最终只剩主键自动索引
  承担 `(request_id, attempt_no)` 查找，并与空库完整迁移链得到同一 Schema。
- Storage revision 测试覆盖成功递增、过期/超范围 expected 冲突、SQLite 上限溢出和异常忽略更新四条路径，
  并固定失败时数据库 revision 不变。
- Domain/Server/Storage 与管理 HTTP 契约覆盖 IPv4-mapped loopback 的规范化、写入降噪及列表过滤；`0008→0009`
  升级测试证明旧 mapped-loopback 行转换且外部地址不变，COUNT 与分页查询计划使用同一覆盖索引谓词。
