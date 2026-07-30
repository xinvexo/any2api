# ADR-0068: 本地模型拒绝使用终局参数错误

- 状态：Accepted
- 日期：2026-07-30
- 决策者：maintainer

## 背景

客户端可能请求 any2api 未发布或未放行的模型。这是本地入口对 `model` 参数的
终局校验结果，不是可能通过重试恢复的上游资源 404。旧实现返回笼统的 404
`model route was not found`，部分客户端会把任意 404 视为可重试流失败，并且
RequestLog 只在路由规划成功后才记录模型，使管理页误显示为“未解析模型”。

## 决策

- 有效但未发布、未放行或无可用 Route 的模型请求使用
  `PublicErrorCode::ModelNotFound`，HTTP 状态固定为 400。
- OpenAI 方言返回 `invalid_request_error`、`code=model_not_found`、`param=model`；Anthropic
  方言返回 `invalid_request_error`。消息明确说明该请求模型在当前网关不可用。
- 未知与未放行共享同一错误，不向客户端暴露配置、Credential 或允许列表差异。
- Runtime 将请求解码与路由规划拆成两个具名阶段。经验证的模型、流式标记和
  思考级别在解码后立即进入 RequestRecorder；路由规划失败不得清空这些请求元数据。
- 拒绝路径不创建会话绑定，不预留 RPM，不选择 Credential，不产生 Attempt，不执行
  上游 I/O，也不做模型别名、替换或回落。

## 后果

客户端能把本地未知模型视为不可重试的请求参数错误，不再对同一请求进行指数退避重试。
管理日志显示客户端真实填写的模型，同时上游相关字段保持为空。实际上游返回的 404 仍按
ADR-0061 原样透传，不受本决策影响。

## 验证

- Protocol 单测覆盖 OpenAI/Anthropic 本地模型错误的状态、type、code 和 param。
- HTTP 契约覆盖单次未知模型请求返回 400，且未进入上游。
- RequestLog 契约覆盖精确 `public_model`、400 状态、零 Attempt 与空路由字段。
