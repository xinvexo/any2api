# ADR-0008: Count Tokens 辅助并发与 404 兼容语义

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

> 后续状态（2026-07-19）：ADR-0011 已接管六项 scheduler 设置。辅助默认值现在从 SQLite 覆盖编译并由 ConfigPublisher 热更新；本 ADR 中“未来 SettingRegistry”均指其余设置组。

## 背景

`POST /v1/messages/count_tokens` 已有协议操作、Claude 上游路径和认证注入，但不能复用生成请求的 `max_concurrency`。架构要求辅助请求同时受全局和单 Credential 上限约束，不影响生成负载率，不建立会话绑定，也不能在主 tier 仅因辅助容量满载时溢出到 fallback tier。

本 ADR 形成时统一 QueueTicket 和 SettingRegistry 尚未实现；当前实现已由 ADR-0011 接入 scheduler 设置覆盖，辅助请求仍不进入生成等待队列。

## 决策

- RuntimeRegistry 持有跨配置 revision 复用的 `AuxiliaryScheduler`，PublishedSnapshot 只持有该稳定句柄。
- 辅助并发默认值集中定义为全局 `32`、单 Credential `4`；使用强类型 `AuxiliaryConcurrencyLimits`，允许构造注入和运行时更新，由 ADR-0011 的 SettingRegistry 接管覆盖值。
- `AuxiliaryScheduler` 使用一个只保护计数和选择的短 `std::sync::Mutex` 作为线性化点。在锁内检查全局上限、比较候选辅助占用、选择 Credential，并同时增加全局与单 Credential 计数；锁绝不跨网络 I/O 或 `await`。
- 单 Credential 辅助计数存放在稳定 `CredentialRuntimeHandle`，与生成请求的 `in_flight/max_concurrency` 完全分离，并跨 Secret rotation 和连续配置 revision 保留。
- `AuxiliaryPermit` 不可 Clone，固定取得时的 `CredentialGenerationRuntime`。Drop 在同一个线性化点释放两层计数，并在锁外只推进一次统一 `scheduler_epoch`。
- 当前没有 QueueTicket，因此辅助容量满载立即返回本地 429；禁止使用固定 Semaphore 或私建等待链。
- 辅助选择只在最低存在永久候选的 tier 内进行。该 tier 辅助满载时立即返回 429，不读取 Route 的 `fallback_on_saturation`；只有当前 tier 完全没有候选时才检查下一 tier。
- ProviderDriver 的错误分类接收当前 `ProtocolOperation`。Claude 的 Count Tokens 上游明确返回 404 时分类为 `OperationUnavailable`，Runtime 转换为脱敏的 `UpstreamNotFound`，Anthropic Adapter 输出兼容 HTTP 404 `not_found_error`，不透传上游原始正文。

## 备选方案

- 不采用两个独立 CAS 再回滚：容易产生半取得 Permit、回滚遗漏和候选重选竞态。
- 不采用 `tokio::Semaphore`：固定 Permit 数难以动态缩容，并会绕过统一 QueueTicket、epoch、超时和取消语义。
- 不复用生成 `ConcurrencyPermit`：Count Tokens 不生成内容，不能占用或影响生成请求负载率。
- 不在 Runtime 按状态码硬编码 Count Tokens 404：上游错误差异属于 ProviderDriver 职责。

## 后果

- Count Tokens 与生成请求可以独立并发；任一侧满载不会消耗另一侧容量。
- Secret 轮换不会重置正在执行的辅助计数，旧 Permit 继续使用旧 generation，新请求使用新 generation。
- 辅助上限现在由 SettingRegistry 编译值或用户覆盖值提供；满载行为仍固定为立即拒绝。
- 辅助调度增加一个极短同步临界区；其范围仅为内存计数和候选比较，不包含异步操作。

## 验证

- 并发测试验证全局和单 Credential 上限、双层原子取得、Drop 单次释放与 epoch 唤醒。
- 测试验证辅助 Permit 不改变生成 `in_flight`，生成满载也不阻止辅助请求。
- 测试验证动态降限不取消已有请求，计数回落前拒绝新请求。
- HTTP 契约验证 Count Tokens 路径、Provider Key、模型改写、未知字段保留、成功响应和上游 404 兼容输出。
