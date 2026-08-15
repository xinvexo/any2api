# ADR-0149：上游请求面与 prompt cache 连续性

- 状态：Accepted
- 日期：2026-08-13
- 决策者：maintainer
- 影响范围：请求 Header 投影、Transport 连接池、请求正文编码、负载均衡
- 相关决策：ADR-0123、ADR-0125、ADR-0154

## 问题

上游 prompt cache 依赖会话标识、请求前缀和物理线路的连续性。若同一逻辑请求在切换
Credential 时改变这些输入，缓存命中会显著下降；若为不同账号隐式增加路由亲和，又会
削弱按请求负载均衡。请求面必须在安全的账号绑定值与可重放的公共标识之间划出明确边界。

## 决策

1. Codex、Claude、Grok 的会话、设备、请求关联和分布式追踪 Header（installation、session、
   thread、window、conversation、agent、request ID、`traceparent`/`tracestate`、Claude
   `x-stainless-*` 等）属于可重放的请求面。`affinity.enabled=false` 时，换 Credential 的
   Attempt 继续投影这些值；`affinity.enabled=true` 时，会话已固定到单一 Credential，换号
   Attempt 删除会话域值以避免跨账号关联。上游为特定账号签发或校验的 attestation、usage
   limit 和 `x-codex-turn-state` 等值在任何模式下换号即删。
2. 数据面 Transport 隔离随 `affinity.enabled` 决定：均衡模式使用单一
   `TransportIsolationKey::shared_data_plane()`，相同代理与线路 profile 的推理请求共享连接池；
   粘性模式按已绑定 Credential、路由代际和认证代际隔离。OAuth Token、Quota 和诊断控制面
   始终按账号与认证代际隔离。认证材料逐请求注入，不进入均衡模式的 Client cache key。
3. 所有上游请求发送 identity JSON，并删除入口 `Content-Encoding`。any2api 只在入口边界
   解压受支持的请求表示，不按 Provider、URL 或 Credential 猜测上游请求压缩能力；响应侧
   仍由 Transport 统一解码并清理表示元数据。
4. 删除任何按 prompt cache 局部性暗中提示选路的状态。未绑定请求继续按稳定轮询选择候选；
   已有会话粘性仍按其固定 Credential、Route Target、上游模型和方言的既有语义执行。
5. 第 1 条仅改变请求面投影，不改变 Gateway Key 剥离、Provider-owned Header、RetrySafety、
   会话绑定或响应提交边界。最终响应始终归属于实际提交的最后一次 Attempt。

## 后果

- 均衡模式下跨 Credential 的同一逻辑请求保持可重放 Header、正文和共享线路，缓存连续性
  不依赖隐藏调度状态。
- 粘性模式下已绑定会话和数据面连接池都保持账号隔离；换号重试不会把会话域或账号绑定值带到另一账号。
- 连接池与 OAuth 控制面使用不同隔离用途，不能用数据面共享池读取或推断 Token/Quota 状态。

## 验证

- Provider 契约测试覆盖均衡/粘性两种模式的 Header 投影、Credential-owned 值和 turn-state
  删除规则。
- Transport/Runtime 测试覆盖均衡模式共享数据面连接池、粘性模式按凭据代际隔离、控制面代际隔离、
  identity JSON 和响应编码解码，以及换号重试的正文与可重放 Header 一致性。
- 调度测试确认未绑定选择保持稳定轮询，不存在额外 prompt-cache 亲和状态。
