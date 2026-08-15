# ADR-0121: Anthropic 首个语义输出前的账号限流流式重试

- 状态：Accepted
- 日期：2026-08-06
- 决策者：maintainer
- 修订：ADR-0118、ADR-0120
- 相关决策：ADR-0136

## 背景

Anthropic Messages 的流式响应可能先返回一个或多个 `ping`，再在 HTTP 200 已建立后以完整的
SSE `error` envelope 返回：

```json
{"type":"error","error":{"type":"rate_limit_error","message":"Concurrency limit exceeded for account, please retry later"}}
```

这表示当前上游账号/模型准入被拒绝，而不是代理连接中断。现有 ADR-0118 只识别
`overloaded_error`；未知的 `rate_limit_error` 会被当作已经提交的流内失败直接交给客户端。这样同一个
错误在非流式 HTTP 429 路径可以切换到其他 Key，在流式路径却表现为“HTTP 200 后断流”，并且不会进入
候选重选或 Attempt 重试记录。

“收到 HTTP 200”或“尚未向客户端写出字节”本身仍不能证明生成请求没有执行。本 ADR 只扩展一个由
Anthropic 线协议精确声明、且在任何语义事件前出现的窄化拒绝，不改变 ADR-0093 的 at-most-once 边界。

## 决策

1. ProtocolAdapter 在 Anthropic Messages SSE 中只接受以下结构化形状作为该重试信号：若存在 SSE
   `event:` 名称则必须精确为 `error`，顶层 `type` 必须为 `error`，且顶层 `error.type` 精确为
   `rate_limit_error`。缺字段、未知类型、自然语言消息、递归字段或非 JSON 帧均不匹配。原始错误帧不
   转发给客户端，也不把它转换成通用错误字符串。
2. 该信号只在 Runtime 仍处于 `Pending`、没有语义内容/推理/工具调用/输出项、没有向下游写出响应头
   或任何字节时有效。`ping`、注释和其他已声明的生命周期控制帧继续受现有预提交字节/时间预算约束，
   但在信号命中前不提交给客户端或会话绑定。
3. 命中后把当前 Attempt 作为 `RejectedBeforeExecution` 交回既有重试循环。失败作用域为当前
   `RoutingCredential + upstream_model`；未绑定请求排除该 Credential-Model 后按现有候选环重新选择，
   继续复用总尝试数、Credential 切换数、绝对 deadline、RPM 预留和取消语义。它不是固定并发限制、
   全局信号量或机器规格相关准入。
4. 已建立会话绑定的请求不得切换 Credential。该请求只能按绑定规则重试原 Credential（若预算和
   RetrySafety 允许），或在预算耗尽时返回最后一次真实拒绝的入口协议状态与帧；不会因为流式限流降低粘性强度。
5. 该拒绝证明当前代理和 Endpoint 已成功返回协议响应，因此不惩罚共享 Endpoint、Proxy 或 EgressPath
   健康状态；只在当前请求排除失败 Credential-Model。其他请求仍可使用该 Endpoint 和代理。
6. 如果所有候选都耗尽或预算结束，仍在 Pending 时返回最后一次真实拒绝的状态、Header 和帧；不得向客户端
   伪造 HTTP 200 成功流。若任何语义事件、响应头或字节已经提交，`rate_limit_error` 与其他流内错误一样只
   终止当前 Body，永久禁止切换。
7. 非流式 HTTP 429 的既有 Provider 错误分类和 `credential_model` 重选路径保持不变；本 ADR 只补齐
   Anthropic SSE 的等价协议表达，禁止在 Runtime 中增加按 Provider 增长的中央分支。

## 后果

- Anthropic 账号并发暂满时，未绑定流式请求可以像非流式请求一样尝试其他 Key；客户端不再先收到失败
  Attempt 的 200/ping 帧。
- 如果多把 Key 实际共享同一个上游账号池，切换仍可能继续收到同样拒绝，但每次拒绝都会按准确的
  Credential-Model 作用域记录并探索其他候选，而不是把一个流内错误误报为网络断流。
- 预提交阶段会短暂保留控制帧，仍受既有预算限制；不引入固定并发限制、额外线程或平台专用代码。
- 已经输出内容后的重复生成风险不增加；Bounded Session 继续保持原 Credential 粘性。

## 验证

- Protocol 单元/契约测试：精确 `rate_limit_error` 命中；`overloaded_error` 继续命中；缺失 envelope、
  未知类型、自然语言消息和语义内容事件不命中。
- Runtime 流式测试：`ping` → `rate_limit_error` → 第二候选成功时，只向客户端交付第二条完整流，第一条
  Attempt 记录为 `RejectedBeforeExecution`/`credential_model`/`reselect`；在内容事件后出现同样错误时
  不重试。
- 绑定会话测试：同一错误不切换 Credential；预算耗尽时返回最后一次真实拒绝并清理预提交控制帧。
- 任意 SSE 字节切分、CRLF、多行 `data`、无尾空行和取消/Drop 的单次 Guard 结算继续由现有测试矩阵
  覆盖。
