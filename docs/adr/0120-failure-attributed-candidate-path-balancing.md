# ADR-0120: 按故障归因与候选路径执行负载均衡

- 状态：Accepted
- 日期：2026-08-06
- 决策者：maintainer
- 修订：ADR-0033、ADR-0093、ADR-0095、ADR-0118

## 背景

同一个 Provider Endpoint 可以配置多个 API Key/OAuthAccount，每个运行时凭据又可能通过不同的实际
出口代理访问上游。一次 `401/403/404/429`、代理握手错误或 5xx 不能仅凭 HTTP 状态推断为整个
Endpoint、整个 Proxy 或整个 Credential 的故障：Key 可能填错、账号权限可能不同、代理本身可能不可达，
也可能只有该代理的出口 IP/区域被特定 Endpoint 拒绝。

现有 Runtime 只支持按 Credential、Endpoint 或 Proxy 排除候选；`Transient` 上游错误统一影响整个
Endpoint，无法表达 `Endpoint × EffectiveProxy`，也无法只排除一次请求中的精确 RouteCandidate。
同时，`Authentication` 与 `OperationUnavailable` 虽具有 `RejectedBeforeExecution` 安全性，却没有进入
普通候选切换路径，导致未绑定请求可能在尚有其他 Key/账号时直接返回失败。

## 决策

1. 负载均衡的最小可执行单位固定为 `AttemptPath`：RouteTarget、ProtocolOperation、
   RoutingCredential generation、Endpoint config generation 与解析后的 EffectiveProxy config generation。
   DIRECT 继承全局代理后使用实际解析结果，不能把配置中的 DIRECT 名称当成真实出口。
2. 初始选择继续对同 tier 的实际 RouteCandidate 等权 round-robin，不增加 weight、最少连接、
   `in_flight` 排序、并发 Semaphore 或新的准入限制。选择从稳定完整候选环的游标位置开始循环扫描，
   不先把可用候选压缩成会改变取模身份的临时数组；成功选择跨过被过滤槽位时，游标额外前进相同槽数，
   下一次从实际选中槽位之后开始，防止坏 Key 或 RPM 冷却槽位让其后方候选长期获得双倍流量。全健康
   快路径不增加额外原子操作，只有发生过滤时才执行一次无锁游标推进。
3. 故障语义与 RetrySafety 正交。Provider 的 `UpstreamErrorClassification` 除 kind、RetrySafety、
   Retry-After 外还必须给出强类型归因；只有声明过的结构化 code/type 可以扩大归因范围，状态码基线与
   自然语言消息不得猜测账号、模型、出口或 Endpoint 责任。
4. Runtime 支持以下健康/排除作用域：
   - authentication version：有结构化证据的错误 Key/Token；
   - Credential routing generation：明确账号权限或额度；
   - Credential + upstream model：明确模型不可用或模型限流；
   - exact candidate：证据不足但 RetrySafety 允许切换的当前 AttemptPath；
   - egress path：`EndpointGeneration × EffectiveProxyGeneration`；
   - Proxy generation：代理自身 DNS/TCP/认证/握手失败；
   - Endpoint generation：已证明与 Credential、出口无关的 Endpoint 整体故障。
5. 状态码本身得到的普通 `401/403/404/429` 默认归因为 Unattributed。未绑定请求在安全且预算允许时
   只排除精确候选，并继续尝试其他 Key/账号；Provider 声明 envelope 中的精确认证、额度、权限、模型或
   操作 code/type 才能提升到对应作用域。通用 5xx 仍为 `Ambiguous`，不重放当前非幂等请求；没有更强
   证据时只更新精确候选的短暂健康，不因单个响应立即毒化整个 Endpoint。
6. Transport 明确区分代理自身故障与出口路径故障。连接代理地址、代理认证和 407 归 Proxy；HTTP CONNECT、
   SOCKS 或代理后的目标 DNS/TLS/连接失败归 EgressPath；DIRECT 的目标 DNS/TCP/TLS 归 Endpoint；无法
   证明更大范围时保持 Unattributed，由 Runtime 只排除精确候选。
7. 未绑定请求的安全重试在同一 fallback tier 内优先探索尚未尝试的 Credential 和 EgressPath：先选择
   Credential 与出口都未尝试的候选，再选择新 Credential、再选择新出口，最后才选择其他未尝试精确
   候选。它只影响失败后的探索顺序，不改变初始 round-robin、公平计数、RPM 或 fallback tier 语义。
8. OAuth 数据面 `401` 最多对同一请求触发一次 refresh。刷新成功后用新 authentication version 重建
   计划；刷新不可用、失败或新 Token 再次被拒绝时，未绑定请求仍可在剩余预算内切换其他
   ProviderCredential/OAuthAccount。已绑定请求不能改变 Credential；除 OAuth 同身份 refresh 外，只有
   可能恢复的安全临时故障才可退避后重试原路径，确定性的认证、权限、额度、模型或操作拒绝直接结束。
9. Retry Runtime 使用一个穷举的 `RetryDecision` 同时决定 Terminal、OAuthRefresh、RetrySamePath 或
   Reselect 及其排除作用域，禁止由独立 kind allowlist 与独立排除 `match` 形成漂移。任何 Committed 状态、
   `Ambiguous`、取消或预算耗尽仍永久禁止切换。
10. EgressPath 与 ExactCandidate 健康只为配置中实际访问过的组合按需创建；Registry 只强持有当前有效
    target、credential、endpoint 与 proxy generation 的状态。配置换代后旧 Arc 随旧快照和 Attempt
    自然回收；迟到的旧快照可以自持旧 Arc 完成本次请求，但不能把已退役路径重新插回 Registry。
    所有到期继续复用唯一 SchedulerEpoch keyed-slot worker，不为路径或失败创建 Tokio task。
11. RequestAttempt 增加安全的 routing mode、failure scope 与 retry decision。它们不包含 Key、Token、
    代理密码、URL 或原始错误正文，只用于已认证详情和测试；历史记录不得恢复任何运行态健康或调度状态。

## 后果

- 同 Endpoint 下错误 Key 不再阻止其他 Key；代理不可达、出口 IP 被拒绝和 Endpoint 整体失败具有不同
  作用域。
- 有限三次 Attempt 可以优先覆盖不同账号与出口，而不是连续撞击同一未知坏路径。
- 新增两次常数级健康检查和按配置有界的运行态对象，但不增加请求准入、网络 I/O、后台任务或平台专用
  调度行为；实现仅依赖 Rust/Tokio，在 Linux、macOS 与 Windows 上一致。
- 更保守的 Unattributed 归因可能让真正的 Endpoint 整体故障先在多个候选上各观测一次；这是避免单个
  模糊响应误伤全部账号和出口的明确取舍。只有后续出现可审计证据时才扩大作用域。

## 验证

- Domain/Provider 测试枚举每种错误 kind、RetrySafety 与归因，并证明自然语言和状态码不能扩大范围。
- Transport 真实连接测试分别覆盖代理地址不可达、407、CONNECT/SOCKS 目标失败、DIRECT 目标失败与
  无法归因路径。
- Runtime 与公共 HTTP 契约测试覆盖同 Endpoint 坏 Key 后好 Key 成功、同 Endpoint 坏出口后其他出口仍
  可选、一个 Proxy 对不同 Endpoint 的健康隔离、通用 403/404/429 精确候选切换、OAuth refresh 失败后
  二次选路、绑定禁止切换、RPM 原子性、健康 Guard 单次结算与热更新代际隔离。
- 选择测试验证持续可用候选分布差不超过一，并验证动态过滤、RPM 恢复和健康竞争不会改变稳定环身份。
- RequestAttempt Migration 使用带代表性旧数据的升级测试；契约测试验证最终透明错误与新增安全诊断。
