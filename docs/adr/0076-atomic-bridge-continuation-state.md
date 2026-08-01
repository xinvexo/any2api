# ADR-0076: 协议桥 Continuation 状态与路由目标原子归属

- 状态：Accepted
- 日期：2026-07-31
- 决策者：maintainer

## 背景

Responses -> Chat Completions 桥需要在本机保存多轮消息，因为 Chat Completions 没有
`previous_response_id` 对应的服务端状态。旧实现由 Protocol 内部的 `ChatHistoryStore` 保存消息，
Runtime 的 `AffinityRegistry` 另行保存 Credential、Route Target、模型和方言。这形成两个 TTL、容量与
提交时点不同的真相来源：一边可以存在而另一边已经丢失，缺失历史也可能在 Credential 选择和 RPM 预留
之后才被发现。流式响应还会在历史完成前暴露本地 Response ID。

## 决策

- `AffinityRegistry` 是 Continuation 的唯一运行时真相来源。一条 Continuation 记录原子包含固定
  Credential、Route Target、上游模型、入口/上游方言，以及可选的协议桥状态；Protocol 不再维护独立
  ID 索引、TTL 或 LRU。
- Protocol 通过强类型的不透明 `ProtocolContinuationState` 能力对象交付桥状态。该对象可以验证自己的
  协议对与操作并恢复下一次 Bridge Session；Runtime 只保存、计量和原样交还，不使用 `Any`、JSON Map
  或按具体 Bridge 分支解释内容。其 `Debug` 只显示协议元数据与字节数，不显示消息正文。
- Continuation 生命周期只有以下三种：

  ```text
  Pending  -> Ready
      |          |
      +-> Abort <-+
  ```

  同协议上游不需要本地桥状态，创建时直接进入 `Ready(None)`。桥接 buffered 响应在向客户端编码并交付
  Response ID 前一次性提交 `Ready(Some(state))`。桥接 SSE 在 `response.created` 可见前提交 `Pending`，
  在成功终止事件可见前提交 `Ready(Some(state))`；EOF、错误、取消或 Body Drop 通过 Lease `Abort` 删除
  Pending 记录并归还预留。
- 命中 Pending 的后续请求先取得统一有界 `QueueTicket`，订阅同一个 scheduler epoch，并在
  `affinity.wait_timeout` 内取消安全地等待 Ready 或 Abort。该等待发生在固定候选选择和 RPM 预留之前；
  Abort、超时或记录丢失返回 `session_binding_lost`，不得猜测目标或启动上游 Attempt。
- 桥状态使用代码级硬边界：单条完整序列化状态最大 `16 MiB`，全部 Ready/Pending 桥状态合计最大
  `64 MiB`。`16 MiB` 与普通 buffered JSON 的既有硬上限一致，足以保存一次允许范围内的文本对话，
  又避免单会话吞掉全部进程预算；`64 MiB` 沿用原桥历史总预算，使单节点最坏占用保持不增长。
  这两项是状态字节预算，独立于 `AffinityRegistry` 的 300,000 条索引数量上限。
- Pending 在 ID 可见前预留完整 `16 MiB` 并计入 `64 MiB` 总预算，Ready 后按实际序列化字节缩减；因此
  完成提交不会因其他请求抢占总预算失败。活跃记录不按 LRU 提前驱逐，只有 TTL、显式清理、Credential
  清理、Abort 或进程退出释放容量。
- `64 MiB` 是 Registry 中 Ready/Pending 状态的存量预算，不能代替在途请求的工作集准入。任何
  `IngressAffinity::Continuation` 请求都必须在 Route、RPM 预留和上游 I/O 前，从现有进程级
  PublicRequest 内存 Permit 额外预留一条最大状态的 `16 MiB`。该额度覆盖不透明状态引用、恢复桥会话
  与对话工作副本，并持有到 buffered/SSE 响应 EOF、错误、断连或 Drop；即使 Registry 条目在途被 TTL
  或显式清理移除，也不能让其实际工作集脱离计量。这复用同一 Permit，不新增 semaphore 或 Credential 并发限制。
- Protocol 在构造 Ready 状态时执行精确序列化大小检查，并在流式增量累积过程中执行完整硬上限检查；
  一旦累积将超过 `16 MiB` 立即失败并 Abort，不允许先生成超限状态再依赖全局驱逐，也不允许单条绕过
  `64 MiB` 总预算。尚未提交时返回本地失败，已提交流则终止 Body 并 Abort Pending 记录。
- Protocol 仍不依赖 Runtime。Continuation 记录只存在于当前进程内存，不持久化、不恢复，也不改变普通
  显式 Session 的活动会话统计口径。

## 备选方案

- 让 Protocol History 与 Affinity 使用相同 TTL：仍然存在两次提交、两套容量和部分失败窗口，不能证明
  原子一致性。
- Runtime 保存 `Any` 后由 Bridge 向下转型：隐藏了协议契约错误，也让错误只能在 RPM 预留后暴露。
- 暴露 Response ID 后才保存完整历史：客户端可以在原流尚未完成时命中只有路由、没有历史的半条记录。
- 全局容量不足时驱逐仍在 TTL 内的历史：会主动制造已有绑定的 `session_binding_lost`，因此改为预留和
  Fail-Closed。

## 后果

- Continuation 查找一次即可得到固定目标和恢复 Bridge 所需的完整状态，缺失与 Pending 都在调度前处理。
- 同时进行的桥接流最多占用四个完整 Pending 预留；Ready 状态通常按实际大小迅速释放未使用预留。这是
  为单节点进程提供确定内存上界的有意取舍，不形成 Credential 并发配置。
- 新增有状态 Bridge 必须实现不透明 Continuation 能力和大小计量，但不需要修改 Runtime 中央调度器。

## 验证

- Protocol 测试覆盖状态恢复、协议对不匹配、单条状态超限，以及 streaming 只有终止后才产生 Ready 状态。
- Runtime 测试覆盖 Pending 等待、Ready 唤醒、Abort 唤醒、全局预留上限、TTL/清理释放容量，以及状态在
  Credential 选择和 RPM 预留前解析。
- 公开请求内存测试覆盖续接工作集在路由前预留、并发容量拒绝，以及 buffered/SSE Drop 释放同一 Permit。
- JSON/SSE 契约覆盖 buffered 原子提交、`response.created` 前 Pending、终止前 Ready、客户端 Drop 后
  `session_binding_lost`，并确认原 `ChatHistoryStore` 不再存在。
