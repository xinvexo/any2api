# ADR-0009: 同协议 SSE 分帧与 GuardedBody 生命周期

- 状态：Accepted
- 日期：2026-07-19
- 修订：2026-08-03
- 决策者：maintainer

## 背景

直接把 Transport 字节流交给 Axum 会遗漏四个关键约束：SSE 帧可能跨任意网络 chunk；公开模型名需要在协议事件中恢复；运行态 Guard 必须覆盖完整响应体生命周期；首帧失败时不能先向客户端写出响应头。

## 决策

- `protocol` 提供增量 `SseDecoder`，按 WHATWG 定义支持 LF、单个 CRLF pair 与裸 CR，覆盖任意字节切分、多行 `data:` 和 EOF 无尾空行，并对单帧缓冲设置固定上限。CRLF 不能被误判为两个空行；chunk 尾部 CR 在看到下一字节或 EOF 前保持待定，使分帧结果不依赖网络 chunk 边界。payload 解析与模型改写使用同一行尾归一化，不能只在分帧器识别裸 CR。
- `ProtocolAdapter` 继续使用 `SseFrame -> AdapterEvent -> SseFrame` 边界；编码下游事件时接收 `public_model`，只改写协议已知的顶层 `model`、`response.model` 与 `message.model`。
- Codex 与 Claude Driver 显式声明 `TransportMode::Sse`；Runtime 根据请求的 `stream` 值选择 JSON 或 SSE 能力，禁止用 JSON 能力替代 SSE 能力。
- Runtime 在收到成功上游响应头后预读并转换首个完整 SSE 事件。空流、首帧 Transport 错误或首帧协议错误在下游响应提交前转换为普通协议错误响应。
- `GuardedBody` 持有上游字节流、增量分帧器、ProtocolAdapter、公开模型名、运行态 Guard、取消标记和 CommitState。
- `GuardedBody` 第一次向 Axum 产出字节时从 `Pending` 进入 `TransportCommitted`；EOF、错误与 Drop 都只结算一次 Guard 并标记取消。
- 所有生成型 SSE 都以协议显式终止标记而不是 HTTP EOF 作为成功边界：Responses 使用 `response.completed` / `response.incomplete`，Messages 使用 `message_stop`，Chat Completions 使用精确 `[DONE]`，Images 使用 `image_generation.completed` / `image_edit.completed`。对应的顶层或命名 `error` 事件标记失败；终止标记前 EOF 是不完整上游流。
- `SseEventPayload` 必须把精确 `[DONE]` 与没有 `data` 的注释、心跳或空帧区分为不同类型。空心跳可以原样转发但不能提前结束 Chat Completions 流。
- 首个普通 SSE 事件只允许提交下游响应，不能提前把 Attempt、Request 或健康状态结算为成功。`AttemptHealth` 必须由 `GuardedBody` 持有到成功终止、失败、取消或 Drop；只有成功终止调用健康成功，失败事件和截断不得清除 Credential 已有的额度耗尽等失败状态。
- 有状态 Bridge 的成功终止必须先把 Pending continuation 转为 Ready；失败终止可以先交付协议错误事件，再由统一错误结算 Drop Lease 并 Abort Pending，不能为了满足 Ready 前置条件而吞掉真实失败事件。
- 提交后的 Transport/协议错误以 Body error 终止连接，不切换 Credential，不拼接第二条上游流，也不伪造成功结束事件。
- 流式响应强制输出 `Content-Type: text/event-stream` 与 `Cache-Control: no-cache`，并继续过滤认证、Cookie、hop-by-hop 与正文相关的敏感上游响应头。

## 备选方案

- 不使用按行 `read_line`：网络 chunk 与 SSE 行没有一一对应关系，且必须覆盖 CRLF、多行 data 与 EOF 残帧。
- 不在 Axum Handler 局部持有 Guard：Handler 返回后局部变量会立即释放，无法覆盖真实流式生命周期。
- 不递归替换所有 JSON `model` 字段：工具参数、用户内容或扩展对象可能合法包含同名字段。

## 后果

- Responses 与 Messages 可以在同协议 Provider 上进行真实 SSE 转发，模型别名在身份事件中保持客户端可见名称。
- 流式请求会在整个 Body 生命周期持有原 Credential 的 `in_flight` 观测状态和取消令牌；客户端断开后 Drop 路径立即结算。
- 首个完整事件之前的错误仍可返回协议兼容 JSON；首字节之后的错误只能终止当前流。
- PrecommitBudget、重试与固定会话绑定复用现有 `Pending/TransportCommitted` 和 `GuardedBody` 边界。

## 验证

- Protocol 测试覆盖任意字节切分、LF/CRLF/`\r\r`/混合行尾、多行 data、无尾空行、`[DONE]`/空心跳区分、各方言成功/失败终止事件与已知模型字段改写；属性测试继续保证切分不变与原始字节重组无损。
- Runtime 测试覆盖各方言终止前 EOF、流内失败、首帧预读、错误/Drop 的 Guard 单次结算和提交状态；失败终止不得因先收到普通事件而清除 Credential 已有失败状态。
- HTTP 契约测试覆盖 Responses、Chat Completions、Messages 与 Images 的真实 chunked SSE（含跨 chunk 裸 CR 行尾）、上游模型改写、公开模型恢复、终止事件和流式响应头。
