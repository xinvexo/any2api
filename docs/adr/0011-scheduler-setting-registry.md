# ADR-0011: Scheduler SettingRegistry 与双作用域热更新

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

## 背景

排队策略和 Count Tokens 辅助并发已有运行时实现，但默认值曾分别定义在 Runtime 模块中，SQLite、管理 API 和 Web 无法覆盖。继续为每组设置单独增加字段和更新入口会形成多套配置模型，也无法保证配置 revision、网关鉴权和路由快照一致。

## 决策

- `SettingDefinition` 是设置元数据的唯一来源，包含 key、值类型、编译默认值、范围或枚举值、应用模式、Web 分组和描述。
- SQLite `setting_overrides` 只保存用户覆盖值；生效值等于覆盖值或编译默认值。显式覆盖等于默认值仍保留，恢复默认通过删除覆盖记录完成。
- Duration 的 SQLite 与管理 HTTP 表示统一为整数毫秒。未知 key、损坏 JSON、类型错误和越界持久化值均使配置加载 Fail-Closed。
- 首批注册六项：
  - `scheduler.on_saturated`
  - `scheduler.queue_timeout`
  - `scheduler.max_waiting_requests`
  - `scheduler.fallback_on_saturation`
  - `scheduler.auxiliary_global_concurrency`
  - `scheduler.auxiliary_per_credential_concurrency`
- `QueuePolicy` 属于 Snapshot scope。每个 `PublishedSnapshot` 从同一已提交 `SettingsConfiguration` 编译一份策略值；旧请求继续使用旧 revision 的等待、超时和 fallback 默认策略。
- `AuxiliaryScheduler` 属于 Runtime scope。连续快照共享同一稳定句柄，发布新设置时只更新其容量参数；降限不取消已有 Permit，升限立即允许新请求竞争。
- 有效配置发布仍使用全局串行锁。Repository 返回同一事务已提交的完整配置，Runtime 执行无 I/O、无 `Result` 的 reconcile，随后单次替换 Snapshot，最后推进一次统一 scheduler epoch。收到该 epoch 的观察者必须能够读取到新 Snapshot；失败或 no-op 不推进 epoch。当前不引入没有真实资源准备需求的通用 `PreparedPublish` 抽象。
- 管理 API 提供列表、覆盖和恢复默认；响应同时包含默认值、覆盖值、生效值、范围、枚举值和应用模式。Web 按分组渲染合适控件，并保留 revision 冲突后的草稿。
- `ConfigurationRepository` 只组合窄能力；设置写入由独立 `SettingRepository` 定义，禁止继续把每类配置 mutation 塞进一个膨胀接口。

## 边界

- 本 ADR 不实现 affinity、retry、cooldown、breaker 或日志设置，只固定它们后续接入同一 Registry 的方式。
- Count Tokens 满载仍立即拒绝，不进入生成请求 QueueTicket。
- 设置和运行时计数不做导入导出；进程重启只读取覆盖配置，in-flight、waiting、Permit 和 epoch 从零开始。

## 备选方案

- 不采用巨型 YAML 或整份 JSON 设置文档：单项覆盖、版本默认值和恢复默认会失去稳定语义。
- 不在 QueueCoordinator 中保存可变 QueuePolicy：旧请求会在等待中途混用新 revision。
- 不重建 AuxiliaryScheduler：会丢失已有辅助请求计数并破坏 Permit 生命周期。
- 不为每个设置组建立独立更新 API 或兼容层：新项目直接使用统一 Registry 和 Repository 能力边界。

## 后果

- 默认值、范围和 Web 元数据集中定义，新增设置不需要修改中央调度器或复制 DTO 常量。
- QueuePolicy 与辅助并发拥有不同作用域，但都由同一配置 revision 发布。
- 损坏覆盖会阻止启动或发布，维护者必须修正 SQLite 数据而不是依赖静默默认值。
- 运行时热更新保留现有 Permit 和等待计数；进程重启仍不恢复任何运行状态。

## 验证

- Domain 测试覆盖默认值、范围、枚举值和应用模式。
- Storage 测试覆盖写入、no-op、显式默认覆盖、恢复默认、revision 冲突、重启读取和损坏行 Fail-Closed。
- Runtime 测试覆盖 QueuePolicy revision 隔离、QueueCoordinator 复用、AuxiliaryScheduler 身份稳定、降限不取消 Permit、新上限立即生效和单次 epoch。
- HTTP 契约覆盖默认/覆盖/生效元数据、PATCH、DELETE、非法值、未知 key 和 revision 冲突。
- Web 测试覆盖响应解析、最新 revision 缓存、草稿校验、保存、恢复默认和冲突刷新。
