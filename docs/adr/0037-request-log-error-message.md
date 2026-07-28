# ADR-0037: RequestLog 保存安全有界错误消息

- 状态：Superseded by ADR-0061
- 日期：2026-07-25
- 决策者：maintainer

> 取代说明（2026-07-28）：ADR-0061 允许把最终 Provider 已声明 envelope 中的原始 `message`
> 写入有界 RequestLog/RequestAttempt 并供管理面展示，不再按状态码或内部分类生成 Provider 摘要。
> 完整上游正文仍不进入 SQLite；any2api 本地错误继续保存自己的有界消息。

## 背景

RequestLog 与 RequestAttempt 原本只持久化 `error_class` 与状态码。管理面展开失败请求时无法看到 any2api 生成的安全错误摘要，或区分 DNS、TLS、代理握手等受控传输阶段。

上游错误正文是不可信输入。兼容服务可能回显认证头、Prompt、Session 标识或完整请求，因此“截断后保存正文”仍会泄露 Provider API Key、OAuth access token 或用户内容，不能作为诊断来源。

## 决策

- 为 `request_logs` 与 `request_attempts` 增加可选 `error_message` 文本列。
- 请求级消息只保存由 any2api 根据本地错误、状态码和类型化分类构造的安全摘要，或固定的取消/流式诊断；不再以“客户端可见”为持久化准入条件。
- Attempt 级消息只保存受控摘要：
  - 上游非 2xx：由 HTTP 状态码和类型化 `ErrorClass` 合成；
  - 传输错误：由类型化 DNS/TCP/代理握手/TLS/写入/等待响应头/读取响应体阶段合成；
  - 本地、协议和流式错误：保存代码内固定诊断文案。
- 禁止从上游响应的 `message`、`error.message`、纯文本正文或其他任意 JSON 字段提取持久化消息。Provider 响应体继续只用于有界错误分类，分类完成后不得进入 RequestLog、SQLite 或管理 DTO。
- 消息最多 1,024 个 Unicode 字符，省略号包含在上限内；换行和其他控制字符规范化为空格。Runtime 在记录边界统一处理，Storage 在写入和读取时再次拒绝超长、首尾空白或含控制字符的数据。
- 管理 API 与 Web 可以展示该安全摘要，但不得把它解释为完整上游错误正文。

## 备选方案

- 截断并保存原始上游正文：拒绝。Secret 或 Prompt 位于前 1,024 个字符时仍会直接泄露。
- 对正文应用 Token 正则脱敏：拒绝。Prompt、Session 与未知 Provider Secret 没有可靠的通用格式，黑名单无法形成安全边界。
- 只保存 `error_class`：可行但诊断能力不足，无法区分受控传输阶段，也无法直接对照客户端公开错误。

## 后果

管理面不会显示 Provider 返回的详细自然语言错误，只显示 any2api 的安全类型摘要。诊断信息减少，但不扩大 RequestLog 的敏感数据边界；需要深挖 Provider 行为时仍使用脱敏的错误分类、状态码、阶段和 Request ID。

## 验证

- Domain 测试覆盖 Unicode 字符上限、省略号和控制字符规范化。
- Runtime 测试覆盖上游错误只生成状态码/分类摘要，且 post-commit 消息经过统一边界。
- Storage 测试覆盖安全消息往返，以及超长或含控制字符消息拒绝写入。
- 管理 DTO/Web 契约只验证安全摘要字段，不使用原始 Provider 正文夹具。
