# ADR-0098：HTTP 409、413、422 上游错误矩阵

- 状态：Accepted
- 日期：2026-08-03
- 决策者：maintainer
- 修订：ADR-0061、ADR-0086、ADR-0093

## 背景

共享 Provider 状态分类原先没有覆盖 409、413 和 422，三者因而落入 `Unknown`。这会把明确的 4xx
请求拒绝记录成笼统上游错误，虽然最终响应仍透明，但内部遥测语义不准确。

RFC 9110 分别把 409 定义为目标资源当前状态冲突、413 定义为请求内容超过服务端愿意或能够处理的
大小、422 定义为语法正确但请求指令无法处理。OpenAI 与 Anthropic 官方 SDK 都提供独立的 409、
413/422 异常类型；两者的通用 SDK 还会自动重试 409。不过当前 SDK 的通用重试路径没有为这些请求
实际发送可由服务端去重的幂等键，因此其可用性取舍不能推翻本项目对非幂等生成请求的 at-most-once
边界。

Claude 审查报告同时建议删除 `billing_error`、改认 `request_too_large`。当前 Anthropic 官方 SDK 的
共享 `ErrorType` 恰好相反：仍明确包含 `billing_error`，而请求过大由独立的 HTTP 413
`RequestTooLargeError` 表达。错误正文中即使出现兼容的 `request_too_large` 字符串，状态基线也足以
正确分类，不需要删除已有额度兼容类型或依赖该字符串。

## 决策

1. 共享 HTTP 状态基线把 409、413、422 统一分类为 `UpstreamErrorKind::InvalidRequest`，重试安全性为
   `Ambiguous`。
2. 三者不进入自动重试候选，不建立 Credential 冷却，也不计入 Endpoint/Proxy 故障。409 由调用方
   根据透明错误正文解决冲突后重新提交；413/422 由调用方修改请求内容。
3. 不照搬官方 SDK 对 409 的盲重试。若将来 Provider 提供并由 any2api 端到端复用可靠幂等键，必须
   另行修订 RetrySafety，不能只凭状态码放宽。
4. Claude `billing_error` 保留为精确的额度分类类型。413 fixture 可以携带
   `error.type=request_too_large`，但其 `InvalidRequest` 结论来自 HTTP 状态；未知或兼容正文不得推翻
   409/413/422 的固定基线。
5. 内部分类不改写客户端响应。最终 Attempt 继续原样返回上游状态、允许 Header 与 64 KiB 内完整
   正文；被分类器识别的官方 message 仍只进入有界管理日志。

## 依据

- [RFC 9110 §15.5.10](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5.10) 定义 409
  为资源当前状态冲突，并要求响应提供调用方可识别和解决冲突的信息。
- [RFC 9110 §15.5.14](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5.14) 定义 413
  为服务端因请求内容过大而拒绝处理。
- [RFC 9110 §15.5.21](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5.21) 定义 422
  为内容语法正确但其中指令无法处理。
- [OpenAI 官方错误类型说明](https://developers.openai.com/api/docs/guides/error-codes#python-library-error-types)
  分别列出 ConflictError 与 UnprocessableEntityError，并把前者描述为并发资源更新冲突。
- [Anthropic 官方 Python SDK 状态异常](https://github.com/anthropics/anthropic-sdk-python/blob/main/src/anthropic/_exceptions.py)
  分别声明 409、413、422 的强类型异常；[当前共享 ErrorType](https://github.com/anthropics/anthropic-sdk-python/blob/main/src/anthropic/types/shared/error_type.py)
  保留 `billing_error`。

## 后果

- 三类明确 4xx 不再污染为笼统 `Unknown`，也不会误伤 Provider 健康状态。
- 409 不会因跟随宽松 SDK 行为而造成重复生成或重复工具调用。
- 413 `request_too_large` 与 422 错误会稳定呈现为请求错误，同时客户端仍能看到真实 Provider
  envelope。
- 已有 Claude 额度兼容不会因过时审查结论被删除。

## 验证

- Provider 状态单测枚举 409、413、422 的 kind、RetrySafety 和 Retry-After 透明解析边界。
- Claude 模块测试使用 413 + `request_too_large` 官方形状，确认分类为 `InvalidRequest`，并继续覆盖
  `billing_error` 为 `QuotaExhausted`。
- Registry 契约枚举实际 Codex、Claude、Grok Driver，确认三类状态在所有 Driver 上得到相同基线，
  Provider 正文不能推翻该基线。
