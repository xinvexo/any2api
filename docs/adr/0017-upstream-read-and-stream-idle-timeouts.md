# ADR-0017: 上游读取与流式提交后空闲超时

- 状态：Accepted
- 日期：2026-07-20
- 决策者：maintainer

## 背景

当前 Transport 只限制建连时间。非流式响应体逐 chunk 读取时没有空闲超时，只能被外层重试总 deadline 取消；成功 SSE 在首事件之后也没有 timer，静默连接可以长期持有 Credential Permit。直接依赖 reqwest Client 级 read timeout 会把设置固化进 Client cache 代际，并且无法在同一成功 SSE 从预提交策略切换到提交后策略。

## 决策

- 注册 `upstream.read_timeout` 与 `stream.postcommit.idle_timeout`，默认分别为 `15_000ms` 和 `60_000ms`，范围均为 `1..=86_400_000ms`，支持热更新且不允许 `0` 禁用。
- 每个请求从同一个 PublishedSnapshot 捕获 timeout。旧请求保持旧 revision，新请求使用新设置，SQLite 继续只保存用户覆盖值。
- `upstream.read_timeout` 约束等待响应头，以及 JSON、Count Tokens、Compact 和非成功 SSE 错误正文的每次 body 读取。成功读取 chunk 后重新计时；它不是总请求 deadline。
- 成功 SSE 在首事件提交前继续使用 `stream.precommit.max_duration`。首个下游帧实际交付时启用 `stream.postcommit.idle_timeout`，每次成功读取任意上游 chunk 后重置；已缓冲事件优先交付。
- TransportRequest 携带请求级 read timeout。首版公开上游调用使用固定 JSON Body，Transport 在请求体开始被连接层消费后启动等待响应头的 timer，避免较短 read timeout 抢占 DNS、连接、代理握手或 TLS；超时生成 `AwaitHeaders + Ambiguous`。TransportResponse 暴露 body 读取失败归因，Runtime 在 buffered body 超时阶段生成 `ReadBody + Ambiguous`。
- DIRECT 的 read timeout 归因 Endpoint；代理路径无法证明责任时归入 `Unattributed`。二者沿既有健康结算执行，`Ambiguous` 默认不自动重试。
- post-commit idle timeout 发生时，下游已经提交，直接返回 Body error 并 AbortStream，不重试、不切换、不生成协议内错误事件。Attempt 记录为 `StreamError + Network + Ambiguous`，不再更新 Endpoint/Proxy 健康。

## 后果

- buffered 响应和成功 SSE 都不再因上游永久静默而无限占用 Permit。
- 通用 read timeout 与 SSE precommit/postcommit timeout 各自只有一个明确阶段，不叠加两个竞争 timer。
- 本切片不实现下游写超时、SSE keepalive、协议内错误事件或动态关闭 timeout；这些能力必须另行建立契约。

## 验证

- Domain 测试验证两项默认值、范围与覆盖编译；Admin 契约验证 43 项 Registry 和两项设置元数据；Web 单测验证两项展示标签。
- Transport/Runtime 测试验证等响应头、buffered body、SSE post-commit idle、成功 chunk 重置、Ambiguous 分类、健康归因和 Permit 单次释放。
- HTTP 契约验证 JSON 超时不启动第二个 Ambiguous Attempt，SSE 首事件先交付、静默后 Body 失败且不切换上游。
