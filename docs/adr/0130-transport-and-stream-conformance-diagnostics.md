# ADR-0130: Transport 与流式 conformance 诊断契约

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer

## 背景

上游可观测特征审计确认了三个不能靠静态代码阅读长期回答的问题：Reqwest/Hyper/Rustls 依赖升级可能改变实际 TLS、HTTP/2 或 HTTP/1.1 wire；流式链路只记录请求级首 Token，无法区分上游 frame 已到达、预提交仍在等待、已经 commit、下游 Body 尚未 poll 或客户端取消；Transport 的 resolver、proxy 和 timeout 结果只隐含在分支中，日志无法说明某次 Attempt 实际采用了哪套策略。

这些问题需要可验证的本地诊断，但不应演变为模拟官方客户端、记录 Secret、给请求增加随机延迟，或把 Client cache 命中误报成物理连接复用。

## 决策

1. `generic-rustls-hyper-v2` 增加 loopback conformance harness，并提交三份规范化文本 fixture：
   - TLS ClientHello：记录 cipher suite、extension type 集合、supported groups、signature algorithms、ALPN 与 supported versions；删除 random、session bytes、key share 公钥等每次变化的材料。Rustls 0.23 会按 `order_seed` 随机排列无顺序要求的 extension，因此 fixture 记录 `rustls_order_seed_randomized` 策略和规范排序后的集合，另由多次 raw capture 证明顺序不是固定常量，不把某次排列冻结成虚假契约。
   - HTTP/2：在自签 TLS 后直接记录解密的 client preface 与首个请求 HEADERS 前的 SETTINGS/WINDOW_UPDATE，不用高层 h2 配置对象反推 wire。
   - HTTP/1.1：由 raw `TcpListener` 记录 request head，保留 header casing/order、Host 与 Content-Length，只把动态 loopback authority 规范化。
2. fixture 从真实 `ReqwestTransportManager` 发起，使用假 Credential 与 loopback server，不访问真实 Provider。依赖或配置变更造成 fixture 差异时，提交者必须审核差异、更新 fixture 并提升 `TransportWireProfile.policy_version`；仅重写测试以接受任意输出不算审核。
3. fixture 是 any2api 自身线路契约，不是 Codex、Claude、Grok、Kimi 或 OpenAI SDK 基线。没有带 Provider、客户端版本、平台、操作和日期的独立采集时，不得声称 fixture 与官方客户端一致或不一致。
4. Transport API 提供只读 `TransportRequestDiagnostics`。生产 Manager 在网络 I/O 前从最终 Request、Proxy 和 Manager 配置生成：wire profile ID/version、timeout policy version、最终 upstream resolver mode、proxy kind、connect/read/pool idle timeout、traffic class，以及 isolation 的 routing/authentication generation。它不包含 RoutingCredentialId、API Key、OAuth Token、代理地址/密码、目标 URL、Header、Body、TLS ticket 或 connection ID。
5. `TransportManager` 的自定义测试实现可以不提供诊断；正式 `ReqwestTransportManager` 必须提供。Runtime 在 `TransportRequest` 已成功构造、发送前把快照交给当前 Attempt recorder，因此网络失败也保留策略事实，构造前失败保持空值。
6. 不记录没有可靠观测来源的事实。Client cache 命中只能说明 Client 对象复用，不能命名为 TCP/H2 connection reused；当前抽象无法可靠观察 TLS resumption 时保持未知，不能从第二次请求或相同 isolation 推断。
7. 流式 Attempt 增加四个相对 Attempt 起点的可空单调毫秒值：
   - `first_upstream_frame_ms`：SSE decoder 首次产出完整 frame；
   - `stream_commit_ms`：全部预提交 frame、usage 与 continuation 状态成功提交；
   - `first_downstream_byte_ms`：GuardedBody 首次向下游 Body yield 非空编码 frame；
   - `stream_cancel_ms`：已交接的流在完成前因 Drop/取消结算。
8. 四个时间点均 first-write-wins，只读取已有 `Instant`，不得驱动等待、flush、重试或 backpressure。提交前失败、没有完整 frame、客户端从未 poll Body、正常完成等场景允许相应字段为空；不得为补齐遥测继续 drain 上游。
9. RequestAttempt 通过连续前向 Migration 增加结构化 Transport 与 stream timing 字段。管理 API 只在已认证详情响应中返回嵌套诊断，Web 以毫秒时间线展示；旧行和非流 Attempt 的不可用字段为 `NULL`。这些字段仍走既有有界 RequestTelemetry 队列和保留策略，不建立新日志、恢复状态或路由输入。

## 后果

- Hyper/Rustls 默认值不再只存在于审计文字中；升级依赖会产生可审阅的 fixture diff。
- 操作员可以区分“上游未给完整 frame”“precommit 等待”“已经交给下游”和“客户端取消”，也能看到请求实际采用的 resolver/proxy/timeout profile。
- 诊断增加少量 Attempt 列和详情 JSON，但不复制请求内容、不改变流式延迟，也不声称消除了通用 Rust transport 的可观测性。
- 物理 connection reuse/resumption 仍只由专用 loopback probe 验证；在生产缺少可靠 hook 时明确保持未知。

## 验证

- Transport 测试从真实 Manager 生成 TLS/H2/H1 capture，并与提交 fixture 精确比较；TLS 另验证同一稳定集合存在多个实际 extension 顺序；同时验证 strict/direct/proxy resolver mode 和 timeout/profile 版本。
- Runtime stream 测试控制 upstream frame、prime、Body poll 与 Drop，验证时间点存在性、顺序和取消分支，且正常完成不写 cancel。
- Migration 使用代表性既有 RequestAttempt 验证新增列保持 `NULL`，新记录往返验证全部诊断字段。
- Server/Web contract 测试验证嵌套 Transport/stream timing 结构、空值和详情展示。
