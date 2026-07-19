# ADR-0013: 提交前重试、错误分类与代际健康状态

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

## 背景

数据面已经具备同协议 JSON/SSE、原子 Credential Permit、有界 QueueTicket、代理执行和会话粘性，但一次请求仍只有一个上游 Attempt。Provider Driver 只返回扁平 `ErrorClass`，Runtime 无法可靠地区分 Credential、模型、Endpoint 与 Proxy 故障，也无法使用 `Retry-After` 或在下游提交前安全切换。

可靠性实现不能把所有失败统一冷却 Credential，也不能为了成功率重复执行语义不明确的生成请求。健康状态还必须遵守配置代际隔离和进程重启清空的项目边界。

## 决策

- 在 domain 增加 `UpstreamErrorKind`、`UpstreamErrorClassification` 与 `RetryAfterHint`。Provider Driver 只分类上游 HTTP 响应；Transport 继续独立产生带阶段的 `TransportError + RetrySafety`。
- Codex 与 Claude 各自在独立错误模块解析自身受限错误 envelope；共享模块只处理标准 HTTP 状态和 `Retry-After`，Runtime 不按 `ProviderKind` 分支。
- `CredentialGenerationRuntime` 持有 generation-scoped 认证与模型健康状态。401 标记当前 generation 的认证错误；429、模型不支持和权限/额度冷却均不跨 Secret/Endpoint 身份代际复用。
- `RuntimeRegistry` 按 `(EndpointId, config_version)` 与 `(ProxyId, config_version)` 复用健康句柄。PublishedSnapshot 固定引用与自身配置版本一致的句柄；旧 Attempt 的迟到结果只能更新旧句柄。
- Endpoint 与 Proxy 使用相互独立的滑动窗口熔断器。进入 Open 后，到期只允许受 `breaker.half_open_max_probes` 限制的探测；探测成功关闭，失败重新打开。
- Transport 阶段与健康归因显式分离。Runtime 只根据 `TransportFailureScope` 更新 Endpoint/Proxy；reqwest 无法可靠区分的 CONNECT、SOCKS 或目标 TLS 故障使用 `Unattributed`，不污染共享健康状态，请求内只排除当前 Credential。
- Candidate 选择同时检查 Credential、模型、Endpoint 与 Proxy 的动态可用性。健康 Permit 与 Credential Permit 都由 `SelectedCandidate` 持有；健康状态在 Permit 释放前发布，随后统一推进 scheduler epoch。
- HalfOpen 探测名额在健康预检查后被并发请求抢占时，选择器释放已经取得的 Credential Permit、移除该候选并继续检查同 tier 其他候选；生成与辅助请求使用相同规则。
- 冷却和 Open 到期通过进程内定时任务推进统一 epoch。等待者继续使用现有 QueueTicket、超时、取消和最大等待数量，不为健康状态增加第二套队列。
- Public request 执行改为显式多 Attempt 循环。每次失败产生类型化 `AttemptFailure`，先更新健康状态，再释放当前 Permit，最后由 `RetryBudget` 判断是否退避和重新选择。
- 自动重试必须同时满足：CommitState 仍为 Pending、`RetrySafety::allows_automatic_retry()`、总尝试/切换/同 Credential/总耗时预算未耗尽、请求未取消。
- 硬粘性与 strict 软粘性只能重新取得原 Credential，绝不跨 Credential。未绑定与 prefer 可以切换；prefer 的重绑通过 AffinityRegistry 的版本化 Creating 租约完成。
- 当前请求会临时排除已经确认失败的 Endpoint 或 Proxy，避免在全局熔断达到阈值前立即重复同一路径；该排除只存在于请求内存中。
- HTTP 5xx、响应体读取失败和 SSE 成功状态后的无效/中断默认属于 `Ambiguous`。首版不提供 at-least-once 开关，也不因尚未向客户端输出就盲目重试。
- `Retry-After` 解析和运行时 deadline 使用可失败加法，并把外部延迟限制为 30 天，避免异常值溢出后立即解除冷却。
- 上游已经成功返回后发生的硬 ID 提取、egress 编码、公开模型恢复或粘性提交失败，仍先按健康成功结算并关闭 HalfOpen 探测，再释放 Credential Permit 并返回本地错误。
- SQLite Attempt/RequestLog 持久化不与本切片耦合；本切片只建立可靠性状态机与测试，历史遥测随后接入同一 Attempt 结果。

## 设置

统一 SettingRegistry 增加十八项 `retry.*`、`cooldown.*` 与 `breaker.*` 设置。Duration 继续使用整数毫秒；抖动使用 `0..=100` 的整数百分比。所有设置按值编译进 PublishedSnapshot，旧请求不在执行中途读取新 revision。

## 后果

- 新 Provider 只实现自身错误分类，不修改中央重试器、调度器或健康状态机。
- 运行态健康与配置代际严格分离，进程重启后全部清空，不引入恢复、后台数据库状态或外部缓存。
- 对安全性不明确的上游执行结果宁可返回错误，也不默认重复生成内容。
- 后续 RequestLog/Attempt 持久化可以消费现有类型化结果，不需要重新推断错误或重试原因。

## 验证

- Provider 测试覆盖 Codex/Claude 错误 envelope、429/额度/模型错误、Count Tokens 404 和两种 Retry-After。
- Runtime 虚拟时间测试覆盖模型冷却、认证代际隔离、Endpoint/Proxy 熔断、HalfOpen 探测竞态、超大 Retry-After、成功后处理结算、到期 epoch 唤醒和热更新代际隔离。
- Public request 契约覆盖提交前切换、硬粘性不切换、Ambiguous 不重试、Retry-After、总 Attempt 预算与 SSE 首帧提交边界。
- Web 测试覆盖新增设置的契约解析、中文展示、保存覆盖与恢复默认。
