# ADR-0036: RequestLog 保存有界错误消息

- 状态：Accepted
- 日期：2026-07-25
- 决策者：maintainer

## 背景

RequestLog 与 RequestAttempt 只持久化 `error_class` 与状态码。管理面展开失败请求时只能看到分类标签，无法看到返回给客户端的消息，或上游/传输层的具体报错文本，排障需要对照外部日志。

## 决策

- 为 `request_logs` 与 `request_attempts` 增加可选 `error_message` 文本列。
- 消息长度有界（1 024 字符），禁止持久化完整请求体/响应体或密钥。
- 请求级消息优先保存返回给客户端的 `PublicError.message`；若仅有 Attempt 诊断，则回退到最后一次 Attempt 消息。
- Attempt 消息来源：
  - 上游错误：从有界错误响应体提取 `error.message` / 等价字段，失败时再截断 UTF-8 正文
  - 传输错误：保存 `TransportError.message`
  - 本地/流式/取消：保存对应诊断文案
- 管理 API 与 Web 详情/展开面板展示错误消息；列表卡片在失败时优先展示消息摘要。

## 后果

- 旧日志迁移后 `error_message` 为空，仅新请求具备消息。
- 上游返回体仍只截断诊断文本，不改变“不缓存 Prompt/完整响应体”的边界。
- 公开错误对客户端的消息与管理面请求级消息对齐，便于对照。
