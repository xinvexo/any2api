# ADR-0017: 上游读取与流式提交后空闲超时

- 状态：Accepted
- 日期：2026-07-20
- 决策者：maintainer
- 修订：2026-08-03；ADR-0084 将两个超时的当前默认值统一调整为 `300s`

## 背景

上游读取必须同时覆盖等待响应头、非流式响应体逐 chunk 读取和成功 SSE 提交后的静默连接。直接依赖 reqwest Client 级 read timeout 会把设置固化进 Client cache 代际，并且无法在同一成功 SSE 从预提交策略切换到提交后策略。

## 决策

- 注册 `upstream.read_timeout` 与 `stream.postcommit.idle_timeout`，当前默认均为 `300_000ms`，范围均为 `1..=86_400_000ms`，支持热更新且不允许 `0` 禁用。
- 每个请求从同一个 PublishedSnapshot 捕获 timeout。已开始的请求保持其捕获 revision，新请求使用当前设置，SQLite 继续只保存用户覆盖值。
- `upstream.read_timeout` 约束等待响应头，以及 JSON、Count Tokens、Compact 和非成功 SSE 错误正文的每次 body 读取。成功读取 chunk 后重新计时；它不是总请求 deadline。
- 成功 SSE 在首事件提交前继续使用 `stream.precommit.max_duration`。首个下游帧实际交付时启用 `stream.postcommit.idle_timeout`，每次成功读取任意上游 chunk 后重置；单个流只在首次启用时堆分配一个 pinned `Sleep`，后续 chunk 使用 `Sleep::reset` 更新绝对 deadline，不重建 timer；已缓冲事件优先交付。
- TransportRequest 携带请求级 read timeout。公开上游调用使用固定 JSON Body；Transport 从 `execute` 开始以绝对 `connect_timeout` 覆盖本地 DNS、TCP、代理握手与 TLS，并在请求体首次被连接层消费时停止连接 timer、启动等待响应头的 read timer。较短 read timeout 不得抢占连接阶段，较长 read timeout 也不得延长连接阶段；响应头超时生成 `AwaitHeaders + Ambiguous`。TransportResponse 暴露 body 读取失败归因，Runtime 在 buffered body 超时阶段生成 `ReadBody + Ambiguous`。
- DIRECT 的 read timeout 归因 Endpoint；代理路径无法证明责任时归入 `Unattributed`。二者沿既有健康结算执行，`Ambiguous` 默认不自动重试。
- post-commit idle timeout 发生时，下游已经提交，直接返回 Body error 并 AbortStream，不重试、不切换、不生成协议内错误事件。Attempt 记录为 `StreamError + Network + Ambiguous`，不再更新 Endpoint/Proxy 健康。

## 后果

- buffered 响应和成功 SSE 都不会因上游永久静默而无限持有运行态 Guard。
- 通用 read timeout 与 SSE precommit/postcommit timeout 各自只有一个明确阶段，不叠加两个竞争 timer。
- 不提供下游写超时、SSE keepalive、协议内错误事件或动态关闭 timeout。

## 验证

- Domain 测试验证两项默认值、范围与覆盖编译；管理契约和 Web 单测验证两项设置元数据与展示标签。
- Transport/Runtime 测试验证等响应头、buffered body、SSE post-commit idle、成功 chunk 重置且复用同一 `Sleep`、缓冲帧不重置、Ambiguous 分类、健康归因和 Guard 单次结算。
- HTTP 契约验证 JSON 超时不启动第二个 Ambiguous Attempt，SSE 首事件先交付、静默后 Body 失败且不切换上游。
