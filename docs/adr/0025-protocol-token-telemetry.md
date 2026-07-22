# ADR-0025: 协议级精确首 Token 与 Token Usage 遥测

- 状态：Accepted
- 日期：2026-07-22
- 决策者：maintainer

## 背景

RequestLog 已有 `first_token_ms`、输入/输出 Token 与缓存读写 Token 字段，但 ADR-0015 为了避免猜测而把它们固定为 `NULL`。Codex Responses 与 Claude Messages 使用不同的 JSON/SSE usage 结构，而 SSE 首个事件往往只是身份或生命周期控制帧。如果 Runtime 自行递归搜索 JSON，或把预读到的首个帧当作首 Token，日志会产生看似精确但语义错误的数据。

## 决策

- `domain` 提供无协议知识的 `TokenUsage`，四个字段分别为可缺失的输入、输出、缓存读与缓存写 Token。ProtocolAdapter 在解码已知响应或事件时生成旁路遥测元数据，原始载荷仍按同协议透传。
- Codex JSON 映射顶层 `usage.input_tokens`、`usage.output_tokens`、`usage.input_tokens_details.cached_tokens` 与 `usage.input_tokens_details.cache_write_tokens`。Codex SSE 只在 `response.completed` 或 `response.incomplete` 事件中从 `response.usage` 映射同样字段。
- Claude JSON 映射顶层 `usage.input_tokens`、`usage.output_tokens`、`usage.cache_read_input_tokens` 与 `usage.cache_creation_input_tokens`。Claude SSE 从 `message_start.message.usage` 和 `message_delta.usage` 取累计快照；新快照只覆盖其明确包含的字段，不把 start/delta 相加。
- 只读取上述固定路径；不支持递归搜索、同名兼容别名或缓存 breakdown 再次求和。值必须是 `0..=9_007_199_254_740_991`（JavaScript `Number.MAX_SAFE_INTEGER`）的 JSON 整数，以保证 SQLite INTEGER 和 Web JSON 消费者都可无损表达；不合法遥测值按缺失处理，不得使数据面请求失败。
- Codex 的内容首 Token 只识别官方有类型语义的非空模型输出 delta：文本、refusal、reasoning/reasoning summary、function/MCP/custom tool 参数、code interpreter code 与 audio transcript。Claude 只识别 `content_block_delta` 中非空的 `text_delta.text`、`thinking_delta.thinking` 或 `input_json_delta.partial_json`。
- `response.created`、`response.in_progress`、`message_start`、ping、item/part start/stop、done 与签名增量都不是内容首 Token。Runtime 必须把内容标记与编码后帧一起排队，只在 GuardedBody 真正向下游 yield 该帧时记录从整个请求开始计算的 `first_token_ms`，并且 first-write-wins。
- 非流式 JSON 只有完整响应时间，无法提供精确内容首 Token，因此 `first_token_ms` 保持 `NULL`。`/v1/messages/count_tokens` 不是生成操作，Runtime 按 `ProtocolOperation` 强制忽略根层 `input_tokens` 以及兼容上游可能夹带的任何 `usage`；`/v1/responses/compact` 只在上游正式返回顶层 `usage` 时采集。
- usage 在解码成功且当前帧通过本地预提交校验后合并；Recorder 完成后忽略迟到更新。客户端 Drop 不继续 drain 上游，未到达的终止 usage 允许保持 `NULL`。
- RequestLog 仍保存平铺字段，SQLite Schema、Attempt 模型和管理 API 响应形状不需要变更。Web 只在详情页展示 TTFT 和 Token Usage，`NULL` 显示为“未记录”而不是 `0`。

## 后果

- Protocol 是唯一知道供应商线协议 usage/event 路径的层，Runtime 与 Storage 不会随 Provider 数量增长出现中央分支。
- TTFT 表示客户端可见的第一个模型内容增量，包含本地规划、排队、重试和预提交处理时间；它不是上游首字节或首个控制帧时间。
- 上游未返回 usage、响应在终止事件前断开或遥测字段损坏时，可空字段保留“未知”语义，不猜测、不归零、不阻断响应。

## 验证

- Protocol 测试覆盖 Codex/Claude JSON usage、SSE 终止 usage、Claude 累计快照、非空内容 delta、控制帧与损坏/超界遥测字段。
- Runtime 测试覆盖 usage 按字段覆盖、TTFT first-write-wins、控制帧不计时、实际 yield 时计时、EOF/Drop 单次完成与 Count Tokens 不采集。
- Storage/HTTP 契约覆盖非空遥测值往返，Web 测试覆盖有值展示和 `NULL` 占位。
