# ADR-0011: Scheduler SettingRegistry 与快照热更新

- 状态：Accepted
- 日期：2026-07-19
- 修订：2026-08-03
- 决策者：maintainer

## 背景

排队策略、重试、健康、日志和其他运行参数需要集中定义默认值、校验范围与管理界面元数据。SQLite 只保存用户覆盖值，而每次配置发布必须保证设置、网关鉴权和路由来自同一个 revision。

## 决策

- `SettingDefinition` 是设置元数据的唯一来源，包含 key、值类型、编译默认值、范围或枚举值、应用模式、Web 分组和描述。
- SQLite `setting_overrides` 只保存用户覆盖值；生效值等于覆盖值或编译默认值。显式覆盖等于默认值仍保留，恢复默认通过删除覆盖记录完成。
- Duration 的 SQLite 与管理 HTTP 表示统一为整数秒。未知 key、损坏 JSON、类型错误和越界持久化值均使配置加载 Fail-Closed。
- scheduler 组固定为四项：
  - `scheduler.on_rate_limited`
  - `scheduler.queue_timeout`
  - `scheduler.max_waiting_requests`
  - `scheduler.fallback_on_rate_limit`
- `QueuePolicy` 属于 Snapshot scope。每个 `PublishedSnapshot` 从同一事务候选配置编译一份策略值；已开始的请求继续使用其捕获 revision 的等待、超时和 fallback 策略。
- RPM 窗口、`in_flight`、QueueCoordinator 与 scheduler epoch 属于 Runtime scope。连续快照按稳定路由凭据身份复用这些句柄，配置发布只更新其可选 RPM 值，不重置现有窗口或等待计数。
- 有效配置发布使用全局串行锁。在事务中读取并校验完整候选配置、编译候选快照并提交 SQLite 后，Runtime 执行无 I/O、无 `Result` 的 reconcile，随后单次替换 Snapshot，最后推进统一 scheduler epoch。失败或 no-op 不推进 epoch。
- 管理 API 提供列表、写入覆盖和删除覆盖；响应同时包含默认值、覆盖值、生效值、范围、枚举值和应用模式。Web 按分组渲染合适控件并保留 revision 冲突后的草稿，但不提供删除覆盖或“恢复默认”入口。
- `ConfigurationRepository` 只负责加载已提交配置；`ConfigurationTransactionRepository` 在 Storage 内独占活 SQLite 事务，并只接受同步候选编译回调。全部设置和其他管理员配置写入统一由 `ConfigPublisher` 接收类型化 `ConfigurationMutation`，通过该回调执行串行预编译；Storage 根据回调结果提交或回滚，Runtime 只在已提交后执行 reconcile 与单次 Snapshot 切换。不保留按配置类别拆分的写 Repository，也不向 Runtime 暴露可跨 `await` 持有的事务句柄。

## 边界

- affinity、retry、cooldown、breaker、模型允许列表和日志设置都接入同一 Registry，不在使用模块中复制默认常量。
- Count Tokens 使用与生成请求相同的 Credential RPM 和 QueueTicket 语义，不存在辅助并发调度器。
- 设置和运行时计数不做导入导出；进程重启只读取覆盖配置，RPM 窗口、`in_flight`、waiting 和 epoch 从零开始。

## 备选方案

- 不采用巨型 YAML 或整份 JSON 设置文档：单项覆盖、版本默认值和恢复默认会失去稳定语义。
- 不在 QueueCoordinator 中保存可变 QueuePolicy：已捕获快照会观察到其他 revision 的参数，甚至一个请求可能混用两组策略。
- 不为每个设置组建立独立更新 API：统一 Registry、`ConfigPublisher` 和候选配置事务已经提供完整能力边界。

## 后果

- 默认值、范围和 Web 元数据集中定义，新增设置不需要复制 DTO 常量。
- 快照级策略与稳定 Runtime 句柄通过同一配置 revision 协调更新。
- 损坏覆盖会阻止启动或发布，维护者必须修正 SQLite 数据而不是依赖静默默认值。
- 热更新保留现有 RPM 窗口、`in_flight` 与等待计数；进程重启仍不恢复任何运行状态。

## 验证

- Domain 测试覆盖默认值、范围、枚举值和应用模式。
- Storage 测试覆盖写入、no-op、显式默认覆盖、恢复默认、revision 冲突、重启读取和损坏行 Fail-Closed。
- Runtime 测试覆盖 QueuePolicy revision 隔离、Runtime 句柄复用、RPM 窗口不被热更新重置和单次 epoch。
- HTTP 契约覆盖默认/覆盖/生效元数据、PATCH、DELETE、非法值、未知 key 和 revision 冲突。
- Web 测试覆盖响应解析、最新 revision 缓存、草稿校验、保存、无恢复默认入口和冲突刷新。
