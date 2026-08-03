# ADR-0016: SSE 预提交字节与时长预算

- 状态：Accepted
- 日期：2026-07-20
- 决策者：maintainer
- 修订：ADR-0084 将 `stream.precommit.max_duration` 的当前默认值调整为 `300s`
- 修订：ADR-0093 明确成功响应头后的首事件失败不能证明上游未执行，继续按 `Ambiguous` 禁止自动重放

## 背景

Runtime 在返回下游响应头前等待并验证一个完整 SSE 事件。上游如果持续发送不完整首帧，可能长期占用解码缓冲、健康探测与运行态 Guard；如果预算在热更新时被逐字段读取，同一个请求还可能混用不同配置 revision。

## 决策

- 注册 `stream.precommit.max_bytes` 与 `stream.precommit.max_duration`，当前默认分别为 `256 KiB` 和 `300s`，全部支持热更新。
- 每个流式请求从同一个 PublishedSnapshot 构造独立 `PrecommitBudget`。预算一旦进入 `GuardedBody` 就不再读取全局设置，确保已开始请求保持其捕获 revision。
- `max_duration` 是取得首个可接受下游事件的提交 deadline，覆盖等待上游字节、分帧、协议解码、模型恢复以及必要的会话绑定提交边界，并且仍受请求级 `retry.precommit_total_budget` 外层预算约束。同步临界区不能被强制抢占，但临界区返回后必须重新检查 deadline；如果已经超时，不得写入绑定或接受首事件。
- `max_bytes` 同时作为 SSE 单帧上限和首个可接受事件提交前的编码后字节预算。解码器每次最多复制当前帧剩余容量再加一个判超字节，未消费的 transport `Bytes` 以零拷贝切片保留。
- Runtime 每次只从解码器取得、编码并排队一个完整事件。首事件完成后立即返回下游响应，不在提交前批量处理同一 upstream chunk 的后续事件。
- 在尚无可接受事件时超时、超字节、空流、Body Transport 错误或协议失败，按提交前上游失败结束当前 Attempt。此时上游已经返回成功响应头，这些流式首事件失败视为不确定结果；即使下游仍为 `Pending` 也不自动启动第二条上游流。
- 一旦事件完成协议解码、公开模型恢复、预算校验和必要的粘性绑定，当前 Attempt 即锁定；同一上游 chunk 中之后的事件不再消耗预提交预算，也不能重新开启切换上游的机会。后续帧在同一 chunk 中损坏时，已锁定事件必须先交付，再以 Body 错误终止。
- 编码后的公开事件超过字节预算时仍使用公开上游错误契约，但这是本地预算失败，上游健康按成功结算，不能推进 Endpoint 或 Proxy 熔断。
- Runtime 自行产生的预提交超时无法可靠区分 Endpoint 与代理责任，统一按 `Unattributed` 结算，不推进任一熔断器。
- 提交后的协议或 Transport 错误继续直接终止流。本 ADR 不引入协议内错误事件，也不实现提交后 idle timeout。

## 后果

- 首事件等待、单帧大小和解码器复制缓冲有明确上限，设置修改通过既有管理 API 与 Web 设置页生效。
- 预算与配置 revision 一致，不需要全局可变计数器或运行态恢复。
- 当前 Adapter 在首个协议有效事件即可提交，因此不暴露无法产生行为差异的事件数量设置。
- 上游 read timeout 与提交后 SSE idle timeout 遵循 ADR-0017；提交后的失败直接终止 Body，不生成臆造的协议错误事件。

## 验证

- Domain 测试验证两项默认值、范围和 SettingRegistry 元数据。
- Runtime 测试验证原始/编码后字节耗尽、deadline、单事件预缓冲、同 chunk 先交付后报错、会话绑定超时、提交后停止计费、健康归因、Guard 结算与错误边界。
- HTTP 契约通过管理设置分别发布小字节预算和短等待时长，验证两类首事件失败都在下游提交前返回协议错误，并验证进行中的请求不会混入其他 revision 的预算。
