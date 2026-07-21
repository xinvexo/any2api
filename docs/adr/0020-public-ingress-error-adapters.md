# ADR-0020: 公开入口错误复用协议适配器

- 状态：Accepted
- 日期：2026-07-21
- 决策者：maintainer

## 背景

公开模型请求进入 Runtime 后，类型化 `PublicError` 已由 OpenAI Responses 或 Anthropic Messages Adapter 编码；但 Gateway 鉴权、Axum 404 和 405 发生在请求体解码之前，最初使用 Server 自有的简化 JSON，导致同一入口出现两套错误 envelope。

## 决策

- Server 只根据稳定入口路径确定 `ProtocolDialect`：`messages` 前缀使用 Anthropic，其余公开目录默认使用 OpenAI；已知 Handler 直接使用 `ProtocolOperation::dialect()`。
- Gateway 认证失败、认证头冲突、公开 404 与 405 先构造 `PublicError`，再通过 `PublicRequestService::error_response` 调用 Composition Root 注册的同一个 `ProtocolAdapter`。
- `PublicErrorCode` 增加 `PublicApiNotFound` 和 `MethodNotAllowed`，由 Adapter 分别保留 HTTP 404/405。OpenAI envelope 暴露稳定 code；Anthropic envelope 使用 `not_found_error` 或 `invalid_request_error`。
- 所有这些错误继续由外层 Request ID 中间件写入 `x-request-id`，并设置 `Cache-Control: no-store`。公开 fallback 仍位于 Gateway 鉴权层内。
- Server 不直接依赖 protocol crate；窄的编码调用由已持有 `ProtocolRegistry` 的 Runtime service 暴露，保持既有依赖方向。
- `PublicRequestService` 是 `AppState` 的构造必填项；生产与测试 Router 都不能省略协议注册表，也不保留第二套 JSON fallback 或旧构造器。

## 后果

- 客户端在请求体解码前后看到同一种协议错误结构。
- 新协议只需注册 Adapter 并扩展入口到 Dialect 的稳定映射，不需要复制错误 JSON。
- 未知且无法可靠判断协议的 `/v1` 路径采用 OpenAI 默认格式；不会根据认证 Header 猜测协议。

## 验证

- Protocol 单测覆盖两个新增入口错误代码的状态与 envelope。
- Server 单测覆盖路径到 Dialect 的稳定映射。
- HTTP 契约覆盖 OpenAI/Anthropic Gateway 401、认证头冲突、404、405、Request ID 与 no-store。
