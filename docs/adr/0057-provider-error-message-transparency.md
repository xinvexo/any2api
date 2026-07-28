# ADR-0057: 最终 Provider 官方错误消息透明返回

- 状态：Accepted
- 日期：2026-07-28
- 决策者：maintainer
- 调整：ADR-0007、ADR-0037、ADR-0056 的上游错误正文边界

## 背景

Provider Driver 已经读取有界的非 2xx 响应正文来分类认证、权限、额度、限流、模型和临时错误，
但分类结果只保留机器语义。Runtime 随后把所有客户端可见消息替换为少量固定英文摘要，导致 Codex
CLI、Claude Code、Grok Build 和普通 SDK 看不到 Provider 对当前账号、模型或请求给出的具体原因。

整段正文透传仍不可接受。自定义兼容服务可能返回 HTML、任意 JSON、认证材料或用户内容；重试链中
间 Attempt 的正文也不属于最终响应。客户端兼容需要的是 Provider 官方 envelope 中的可读消息，不是
无边界反向代理。

## 决策

- Provider Driver 返回 `UpstreamError`，其中机器分类与可选官方消息分离。健康、重试、冷却、OAuth
  刷新和遥测只读取 `UpstreamErrorClassification`，不依赖自然语言。
- HTTP 状态先建立不可矛盾的机器分类基线，正文 code/type 只能做相容细化；401 和 5xx 不能被正文
  改成其他健康作用域，429 可以细分为限流或已确认额度耗尽。Provider 特殊 code 只读取已声明字段，
  不递归搜索任意 JSON。
- Codex/OpenAI/Grok 只从其已声明错误结构的 `error.message` 提取消息；Grok 额外接受已观察到的顶层
  字符串 `error` 结构。Claude 只从 Anthropic 错误结构的 `error.message` 提取消息。未知字段、纯文本、
  HTML 和任意递归 JSON 值不作为公开消息。
- Driver 只能读取 Runtime 通过独立错误正文收集路径取得、并在读取阶段限制为 64 KiB 的完整正文。
  消息缺失、空白、过长、正文超限、读取超时、中途断开或结构解析失败时，Runtime 丢弃不完整正文，
  使用 HTTP 状态基线分类和对应固定摘要；已经收到的非 2xx 状态、安全 Header 与健康归因不得被改写
  为传输或本地错误。
- 只有实际结束请求的最终 Attempt 消息进入客户端协议 envelope。被重试、Credential 切换或 OAuth
  刷新淘汰的 Attempt 消息和 Header 一并丢弃。
- HTTP 状态、公开 code/type 和 Gateway 鉴权语义仍由入口 ProtocolAdapter 与 Runtime 决定。尤其上游
  401/403 不得伪装为 Gateway Key 认证失败；透明的是官方 `message`，不是整套上游 envelope。
- `PublicError` 在类型内区分本地安全消息与 Provider 客户端消息；Provider 消息只能通过客户端访问器
  交给协议 Adapter，其 `Debug` 与遥测视图固定脱敏。Runtime 的最终失败同时携带独立安全遥测摘要，
  禁止再从客户端消息反推或复制日志内容。
- 官方消息只存在于当前请求内存和客户端响应，不写入 RequestLog、RequestAttempt、HttpAccessLog、
  本地文件日志、tracing 字段或管理 DTO。请求与 Attempt 遥测继续使用状态码、分类和阶段生成安全摘要。

## 后果

官方客户端能够显示 Provider 返回的具体错误原因，同时重试、健康和鉴权仍依赖稳定类型，不会通过
字符串判断行为。错误解析继续由各 Provider 局部维护，Runtime 不增加按 Provider 扩张的 `match`。

该决策有意扩大已认证客户端的错误可见性，但不扩大日志和管理面的持久化边界。管理员配置的 Provider
Endpoint 仍是受信任目标；若未来接入不受信任的第三方 Endpoint，应在该 Provider Driver 中声明其错误
结构，而不是开启通用正文透传。

## 验证

- 注册表契约枚举 Codex、Claude、Grok Driver，确认各自保留声明结构中的官方消息并忽略无效结构。
- Runtime 契约覆盖 buffered 与请求为流式但上游直接非 2xx 的路径，确认最终客户端收到官方消息。
- 超限、超时与中途断开的错误正文契约确认仍保留既有非 2xx 状态、安全 Header 和状态基线分类，只
  放弃官方消息并使用固定摘要。
- 重试契约确认中间 Attempt 消息不会覆盖最终 Attempt；遥测契约确认 RequestLog 与 RequestAttempt 只
  保存安全摘要，不保存官方消息。
