# ADR-0009: 同协议 SSE 分帧与 GuardedBody 生命周期

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

## 背景

Responses 与 Messages 的非流式 JSON 已能完成鉴权、路由、Credential 选择、代理解析和上游执行，但 `stream=true` 仍在规划阶段被拒绝。直接把 Transport 字节流交给 Axum 会遗漏四个关键约束：SSE 帧可能跨任意网络 chunk；模型别名需要在协议事件中恢复；并发 Permit 必须覆盖完整响应体生命周期；首帧失败时不能先向客户端写出响应头。

本切片只实现同协议 SSE。自动重试、QueueTicket、会话粘性、Response ID 硬绑定、完整 PrecommitBudget 和提交后的协议内错误事件仍属于后续可靠性切片。

## 决策

- `protocol` 提供增量 `SseDecoder`，支持 LF/CRLF、任意字节切分、多行 `data:` 和 EOF 无尾空行，并对单帧缓冲设置固定上限。
- `ProtocolAdapter` 继续使用 `SseFrame -> AdapterEvent -> SseFrame` 边界；编码下游事件时接收 `public_model`，只改写协议已知的顶层 `model`、`response.model` 与 `message.model`。
- Codex 与 Claude Driver 显式声明 `TransportMode::Sse`；Runtime 根据请求的 `stream` 值选择 JSON 或 SSE 能力，禁止用 JSON 能力替代 SSE 能力。
- Runtime 在收到成功上游响应头后预读并转换首个完整 SSE 事件。空流、首帧 Transport 错误或首帧协议错误在下游响应提交前转换为普通协议错误响应。
- `GuardedBody` 持有上游字节流、增量分帧器、ProtocolAdapter、公开模型名、请求 Permit、取消标记和 CommitState。
- `GuardedBody` 第一次向 Axum 产出字节时从 `Pending` 进入 `TransportCommitted`；EOF、错误与 Drop 都只释放一次 Permit并标记取消。
- 提交后的 Transport/协议错误以 Body error 终止连接，不切换 Credential，不拼接第二条上游流，也不伪造成功结束事件。
- 流式响应强制输出 `Content-Type: text/event-stream` 与 `Cache-Control: no-cache`，并继续过滤认证、Cookie、hop-by-hop 与正文相关的敏感上游响应头。

## 备选方案

- 不使用按行 `read_line`：网络 chunk 与 SSE 行没有一一对应关系，且必须覆盖 CRLF、多行 data 与 EOF 残帧。
- 不在 Axum Handler 局部持有 Permit：Handler 返回后局部变量会立即释放，无法约束真实流式并发。
- 不递归替换所有 JSON `model` 字段：工具参数、用户内容或未来扩展对象可能合法包含同名字段。
- 不在本切片引入提交前多 Attempt 重试：没有 QueueTicket、RetryBudget、健康状态和身份提交状态机时，提前加入重试会破坏后续边界。

## 后果

- Responses 与 Messages 可以在同协议 Provider 上进行真实 SSE 转发，模型别名在身份事件中保持客户端可见名称。
- 流式请求会在整个 Body 生命周期占用原 Credential 生成并发槽位；客户端断开后 Drop 路径立即释放。
- 首个完整事件之前的错误仍可返回协议兼容 JSON；首字节之后的错误只能终止当前流。
- 后续实现 PrecommitBudget、重试与硬粘性时，可以在现有 `Pending/TransportCommitted` 和 `GuardedBody` 边界上扩展，不需要改写 Server Handler。

## 验证

- Protocol 测试覆盖任意字节切分、CRLF、多行 data、无尾空行、`[DONE]` 与已知模型字段改写。
- Runtime 测试覆盖首帧预读、EOF/错误/Drop 的 Permit 单次释放和提交状态。
- HTTP 契约测试覆盖 Codex Responses 与 Claude Messages 的真实 chunked SSE、上游模型改写、公开模型恢复和流式响应头。
