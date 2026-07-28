# ADR-0056: Provider 感知的双向 Header 透明性

- 状态：Accepted
- 日期：2026-07-28
- 决策者：maintainer
- 调整：ADR-0007、ADR-0015、ADR-0051 的 Header 与 Request ID 边界

## 背景

Codex CLI、Claude Code 与 Grok Build 不只依赖 JSON/SSE Body。它们会发送客户端身份、会话、实验和
追踪 Header，并从响应 Header 读取 Request ID、重试指示、动态限流、模型能力以及服务端粘性状态。
现有实现只保留 Claude `anthropic-beta`，并主动删除上游 `x-request-id`；终态非 2xx 在重试状态机中
又丢失原始状态和安全 Header，导致官方客户端功能退化，Claude 429/529 还会被折叠为 502 `api_error`。

全量反向代理式透传同样不可接受：它会把 Gateway Key、Cookie、连接级字段、错误认证语义或与旧 Body
绑定的校验元数据发往错误的 Provider，并可能在切换 Credential 后重放账号绑定状态。

## 决策

1. Provider Driver 为每个 Provider、协议方言和端点声明请求/响应 Header 投影；Runtime 只调用稳定
   接口，不在中央调度器增加 Provider `match`。只有入口与上游方言相同且 Provider 匹配时才投影真实
   客户端身份/会话 Header；跨协议桥默认丢弃源协议 Header。
2. 请求合并优先级为：官方缺省身份 < 客户端白名单 < ProtocolAdapter 重建的 Body/协议一致性 Header
   < 当前 Credential/OAuthAccount 的认证与账号 Header。固定客户端身份不再藏在 OAuth 认证函数中，
   因而 API Key 与 OAuth 共享同一身份策略，认证材料仍严格分离。
3. `Authorization`、`x-api-key`、Provider 认证/账号字段、Cookie、Host、转发/代理认证字段、hop-by-hop
   字段、`Connection` 动态点名字段、`Content-Length`、客户端 `Accept-Encoding`、`baggage` 以及重编码
   后失效的压缩/摘要/ETag 字段始终删除或重建。出站 Header 数量、单值和总字节有固定上限。
4. `x-grok-model-override` 按最终上游模型重建；Claude OAuth 保留全部有界的客户端 beta Header 行，并
   去重追加 `oauth-2025-04-20`。`x-oai-attestation` 只能原样用于同一请求的首个 Credential 路径，不能
   生成、缓存、记录或跨 Provider/Credential 重放。
5. `x-codex-turn-state` 视为上游签发的 Credential/Route Target 粘性令牌。只有请求已有匹配的硬或严格
   绑定时发送；无绑定、绑定丢失和换 Credential 均删除。新的响应状态只属于最终提交 Attempt。
6. 成功和错误响应都只投影最终 Attempt 的安全 Header。重试掉的 Attempt 不暴露任何 Header 或错误
   消息；SSE 在首帧验证与绑定成功前保持 Pending。上游正文继续有界解析，不直接透传；只有 Provider
   已声明错误 envelope 中的官方 `message` 可以进入当前客户端响应，且不得进入日志或持久化。完整
   边界见 ADR-0057。
7. 上游 `x-request-id`、`request-id` 与 `x-oai-request-id` 按 Provider 白名单保留；Codex 最终 Attempt
   只有 `x-oai-request-id` 时将同一上游值镜像为 `x-request-id`，避免官方客户端优先读到本地 ID。本地
   Request ID 始终使用 `x-any2api-request-id`；只有缺少可归一化的上游 `x-request-id` 时才用本地值补齐
   旧字段。本地错误因此同时返回两个本地关联字段。聚合模型目录使用 PublishedSnapshot 生成的本地
   ETag，不借用任一账号 ETag。
8. 最终 429/529 保持协议可识别的 `rate_limit_error`/`overloaded_error` 语义和安全重试 Header；上游
   Credential 的 401/403 仍返回脱敏上游错误，不能冒充 Gateway Key 认证失败。
9. JSON 型 Codex/OpenAI 入口支持有界 `Content-Encoding: zstd`：压缩体与解压结果分别限长，解析后
   对同方言 Codex 上游重新压缩最终 Body 并重建编码字段。未知/重复/损坏编码和 multipart 编码请求在
   上游 I/O 前按入口协议拒绝。

## 后果

真实官方客户端的身份、会话、重试、额度和模型能力功能可以跨 any2api 工作，同时 Gateway Key、上游
Secret 和连接级状态仍不会串线。Provider 新增 Header 契约时只改对应 Driver 与枚举式契约测试。
白名单需要随已验证官方客户端版本维护；未知新 Header 默认不发送，因此兼容性扩展必须是显式改动。

## 验证

- 注册表契约枚举全部 Provider Driver，覆盖 API Key/OAuth、同方言/跨协议、JSON/SSE 与默认身份。
- 请求测试覆盖认证/Cookie/hop-by-hop 不泄漏、客户端身份透传、模型 Header 重建、Claude beta 合并、
  attestation 与 turn-state 的 Credential 边界，以及 Header 大小上限。
- 响应测试覆盖成功、最终错误、被重试错误、上游/本地 Request ID、429/529、重试/限流/模型能力 Header。
- zstd 测试覆盖有效压缩、损坏帧、未知或重复编码、压缩前与解压后超限，并确认重编码 Body 可解压。
