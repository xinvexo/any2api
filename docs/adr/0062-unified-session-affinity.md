# ADR-0062: 统一固定会话绑定

- 状态：Accepted
- 日期：2026-07-29
- 决策者：maintainer

## 背景

会话粘性只有一种固定绑定语义：同一会话一旦确定上游执行目标，后续请求必须回到完全相同的目标，
不能因负载、限流、冷却或代理故障换账号。普通显式 Session 与 `previous_response_id` 的区别只在于
未命中时能否首次创建，这属于输入意图，不是不同绑定类型或强度。

## 决策

### 适用操作

会话绑定只适用于 Responses、Responses Compact、Chat Completions 和 Messages。Images Generations、
Images Edits 与 Messages Count Tokens 没有会话或续接语义，ProtocolAdapter 必须为这些操作返回
`IngressAffinity::None`。通用 Session Header、`conversation_id` 或其他未知字段可以继续按各自协议的
透传规则处理，但不得让这些无会话操作创建、命中或等待绑定。

### 单一绑定对象

`AffinityRegistry` 只保存一种 `SessionBinding`：

```text
session_key
  -> Credential
  + Route Target
  + upstream model
  + ingress/upstream protocol dialects
  + optional opaque continuation state
  + last-seen timestamp
```

绑定一旦进入 `Bound`，后续请求只能使用该完整目标。RPM 用尽时等待原 Credential；目标处于冷却、
代理不可用、被禁用或不再支持该模型时保留绑定并返回明确的 any2api 本地错误。等待超时也返回本地
错误，不得切换 Credential、Route Target、模型或方言，不得删除现有绑定并自动建立新绑定。上游已经
返回的非 2xx 状态、正文和协议错误仍按透明上游错误契约返回，不因会话绑定生成替代消息或类型。

Credential 删除不反向清理 Registry，也不使已经捕获旧 PublishedSnapshot 的在途 `Creating` Lease 或
Continuation 失效；旧请求仍可按其 snapshot 提交固定目标。新 revision 无法在当前候选集中解析该目标时，
普通显式 Session 与 Continuation 都返回 `session_binding_lost`，不得删除绑定后自动重建或改选其他目标。
此类失败访问不得刷新 `last_seen_at`，重复请求不能阻止旧记录按统一 TTL 到期；管理员显式清理或进程
重启也会回收记录。

### 创建意图

ProtocolAdapter 只随会话标识传递以下创建意图，不产生绑定分类：

- `X-Any2API-Session`、`X-Session-ID`、`Session-Id` / `Session_id`、Claude
  `metadata.user_id.session_id` 和 `conversation_id` 允许未命中时首次创建；
- Codex `previous_response_id` 必须续接已有绑定。未命中时返回 `session_binding_lost`，禁止猜测原
  Credential 或在另一个目标上重建。

可创建 Session 的首个请求继续使用版本化 `Creating` 租约串行确定目标。只有在绑定尚未提交、
下游仍为 `Pending` 且 RetrySafety 允许时，首次创建流程才可安全改用另一个候选；一旦提交为
`Bound`，目标永久固定到绑定被显式清理、TTL 到期或进程退出。

活跃 `Creating` Lease 不按 `affinity.wait_timeout` 或 TTL 回收。其他请求的等待超时只结束该 waiter；
创建者提交或 RAII Drop 才结束 Lease 并唤醒等待者，防止长请求执行期间出现第二个创建者。
每个 `Creating` waiter 必须先取得全局有界 `QueueTicket`，并与普通候选等待、固定 Credential 等待
共享同一个 scheduler epoch；提交、Drop 或清理 `Creating` 时推进该 epoch，不建立会绕过队列上限的
私有等待链。

TTL 到期后，可创建 Session 再次出现时按全新首次请求处理；必须续接的 Response ID 则返回
`session_binding_lost`。这不是对现存绑定的自动重绑。

### 键作用域

显式 Session 的 HMAC 输入作用域固定为入口协议方言加 `ModelRouteId`，并使用 session 用途域；它不
包含 `GatewayApiKey`，因此网关密钥不能影响上游选择。同一个原始 Session 值在不同入口方言或逻辑
Route 下不会意外共享绑定。

Continuation 使用独立 continuation 用途域，只对 Response ID 自身做 HMAC，不叠加 Route scope。
绑定值已经保存 Credential、Route Target、上游模型和协议方言，续接请求必须按该完整目标恢复。
两种键空间的差异只防止标识碰撞，不产生不同的绑定类型、TTL、等待或失败语义。

有状态协议桥的恢复状态与上述目标属于同一条记录，并按 ADR-0076 使用 Pending/Ready/Abort 生命周期；
Protocol 不维护第二个按 Response ID 查询的 History Store。Pending 等待在 Credential 选择和 RPM 预留前
通过统一 QueueTicket 与 scheduler epoch 完成。

### 设置

SettingRegistry 只保留一个开关和两项统一策略参数：

| 设置 | 类型 | 默认值 | 允许范围 |
|---|---|---:|---:|
| `affinity.enabled` | boolean | `true` | `true` / `false` |
| `affinity.ttl` | duration_secs | `86_400` | `1..=2_592_000` |
| `affinity.wait_timeout` | duration_secs | `30` | `1..=86_400` |

三项设置按 PublishedSnapshot revision 捕获并支持现有默认值、覆盖值、生效值和底层删除覆盖语义；Web
只允许保存具体覆盖值，不提供删除覆盖或恢复默认入口。
`affinity.enabled` 只控制允许首次创建的普通显式 Session：关闭时 Runtime 忽略这类标识，不创建、命中
或等待其绑定；Continuation 始终要求命中原绑定。开关关闭不清空 Registry，重新开启后尚未过期的
普通 Session 绑定可继续命中。

Web 将开关直接展示在“设置 → 路由策略”，TTL 与等待超时放在高级设置中。不提供绑定强度或可重绑
模式。

SettingRegistry 不提供其他粘性设置键、别名、双读或模式分支。会话绑定从不持久化，因此不存在运行态
恢复或数据迁移。

### 管理与观测

管理 API 保留清理全部绑定和按路由凭据清理的能力。普通总览只返回统一 `binding_count` 与
`creating_count`，不返回绑定类别、逐 Credential 分布、原始标识、HMAC 样本或绑定内容。

管理与观测不再按绑定类别拆分；首次创建发生安全候选切换时仍属于普通 Attempt 切换，不伪装成
现存绑定变化。

## 后果

- 调度器只需处理“未绑定选择候选”和“已绑定固定目标”两条路径。
- 普通 Session 和 Response ID 复用同一绑定表、TTL、等待与清理实现。
- 配置面只提供一个普通 Session 粘性开关和两项统一策略参数，不包含绑定强度或重绑模式分支。
- 绑定目标暂时不可用时可用性低于自动换号策略，但不会破坏有状态会话或把账号专属状态泄漏给另一个
  Credential；这是明确选择的正确性边界。

## 验证

- Protocol 测试覆盖显式 Session 与 Continuation 两种创建意图、代表性标识来源和
  `previous_response_id` 未命中，并确认 Images 与 Count Tokens 忽略全部会话标识。
- Runtime 测试覆盖并发 `Creating`、单一 TTL、有效访问刷新、固定目标等待、超时不切换、目标不可用不
  切换且失败访问不刷新 TTL、显式清理、Credential 删除后旧 snapshot 可提交而新 revision 解析失败，
  以及重启空状态。
- JSON/SSE 契约覆盖 Response ID 在可见前绑定、普通 Session 首次创建、后续固定完整目标、
  `session_binding_lost` 和提交后禁止切换。
- Settings 与管理 API 测试覆盖三个键、删除覆盖、开关热更新、Continuation 不受开关影响和统一聚合；
  Web 测试覆盖保存具体覆盖值且不存在恢复默认入口。
