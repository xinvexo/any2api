# 路由、重试与流式生命周期

本文是数据面请求阶段、候选选择和响应提交语义的当前规范。设置的具体默认值与范围以 Setting Registry 为准。

## 请求阶段

公开请求按以下所有权推进：

1. `server` 分配 request ID、解析规范客户端地址、验证 Gateway API Key，并收集有界请求 Body。
2. `runtime` 在同一个 `PublishedSnapshot` 上解析 Operation、公开模型和路由要求。
3. 候选构建过滤未启用、已到期、模型或 Operation 不匹配、代理不可用以及配置 generation 不一致的凭据。
4. 选择器在 fallback tier 内轮询，并原子完成本地 RPM 预留；必要时以 QueueTicket 等待下一次准入变化。
5. Provider、Protocol 和 Transport 形成并执行一次 Attempt。
6. 响应在提交前通过状态、编码和协议首帧验证；完成、失败或取消最终只结算一次资源 Guard 和遥测。

RequestLog 表示客户端的一次逻辑请求，RequestAttempt 表示每次实际上游尝试。总览和 Gateway 统计按逻辑请求
计数，Provider 凭据与 OAuth 账号统计按 Attempt 归属计数。

## 路由池和准入

API Key 凭据与 OAuth 账号持久化生命周期不同，但发布后都编译为 `RoutingCredential`，共享候选过滤、RPM、
健康、冷却、重试和粘性机制。公开模型名映射到一个或多个内部 target；同一 target 的候选按显式 fallback
tier 分组，tier 内使用稳定 cursor 轮询。

本地 RPM 是 Credential 唯一面向管理员的调度准入限制。`in_flight` 只记录尚未结算的资源生命周期，不形成
第二套并发上限或权重。选择与 RPM 预留必须是一个原子决定，不能先选中后再与其他请求竞争额度。

当所有候选只是等待 RPM、短暂健康恢复或配置变化时，QueueCoordinator 使用有界等待和统一 epoch 唤醒；
不存在可恢复候选、达到等待预算、请求取消或进程进入停机时立即结束。热更新会重建候选投影，但同一凭据仍在
当前进程内的 RPM 窗口不能因配置发布被清零。

## 粘性与 continuation

显式会话或协议 continuation 可以绑定到一个完整 route candidate，而不只是 Provider 名称。绑定包含凭据、
目标、模型、协议和相关 generation；配置删除、禁用、模型变化、代理切换或凭据换代使不再匹配的绑定失效。

首次并发创建通过版本化 lease 防止多个请求竞相建立不同绑定。必须续接的 continuation 找不到有效绑定时返回
明确错误，不静默落到另一账号；普通会话可以在尚未提交且旧绑定不可用时重新选择。绑定和 lease 都是进程内
有界状态，重启后不恢复。

## 健康、冷却和重试

Provider 把失败分类为稳定的错误种类、`RetrySafety` 和可选 Retry-After。Runtime 只依据类型化结果更新对应
Endpoint、代理出口或 Credential path 的健康状态；错误正文和自然语言不直接决定账号身份或永久状态。

重试同时受以下条件约束：

- 下游尚未提交；
- Operation 与失败阶段允许安全重试；
- 总尝试数、总时间、Retry-After 和请求取消预算仍允许；
- 存在满足模型、协议、代理和粘性要求的候选。

重试可以换候选，也可以在安全时对同一候选重试；每次都产生独立 Attempt。未知或可能已被上游执行的非幂等
失败不会仅为了提高成功率而重放。

## 提交边界与流式响应

下游提交是不可逆边界。提交前可以读取有界的状态、Header 和协议事件，以排除空流、损坏编码或明确上游失败；
提交任意响应字节后，该 Attempt 独占剩余 Body，系统不能切换上游、拼接第二条流、补造终止事件或把流内错误
改成 HTTP 成功。

SSE parser 必须正确处理任意网络字节切分、CRLF、连续注释、多行 `data`、空事件和没有尾空行的 EOF。Bridge
保存增量协议状态和未消费字节，只有完整事件才能进入目标转换。下游取消停止继续读取，不为了统计或成本估计
在后台 drain 上游。

请求、Attempt、RPM reservation、in-flight、stream body 和 telemetry completion 使用一次性 Guard。所有
正常结束、错误、取消和 Drop 路径都必须结算一次，不能泄漏准入额度，也不能重复记录结果。

## 配置和进程生命周期

公开请求持有开始时的快照与必要 Runtime binding；新 revision 一次性影响后续选择。健康、RPM、队列和绑定
保留在稳定 Runtime Registry 中，使发布新快照不等于重建整个运行态。

进入 Draining 后停止接受新的公开工作和新的后台任务，已有受跟踪请求在宽限期内完成；Forced 阶段取消仍可
取消的任务。进程退出后不恢复请求、队列、会话或健康状态。

## 必须保持的测试面

- RPM 窗口、原子预留、有限等待和无丢失唤醒；
- 热更新不重置有效窗口，失效 generation 不被继续选择；
- 粘性并发创建、必须续接、容量回收和过期；
- 每类 RetrySafety 的提交前行为，以及提交后禁止切换；
- SSE 任意切分、EOF、工具增量、协议错误和 Guard 单次结算；
- 请求取消、Body 错误、停机和 telemetry 背压。
