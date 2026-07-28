# ADR-0062: 统一固定会话绑定

- 状态：Accepted
- 日期：2026-07-29
- 决策者：maintainer
- 取代：ADR-0012 的绑定分类、模式、自动重绑、分组设置和管理聚合

## 背景

现有实现把普通显式 Session 和 Codex `previous_response_id` 分成两类绑定，并进一步提供可切换模式。
这些选项扩大了 AffinityRegistry 状态机、设置、管理 DTO 和测试矩阵，但用户真正需要的约束只有一个：
同一会话一旦确定上游执行目标，后续请求必须回到完全相同的目标，不能因负载、限流、冷却或代理故障
悄然换账号。

普通显式 Session 与 `previous_response_id` 的区别只是“未命中时能否首次创建”。这属于输入意图，
不是两种绑定强度，也不需要两套注册表、TTL 或失败规则。

## 决策

### 单一绑定对象

`AffinityRegistry` 只保存一种 `SessionBinding`：

```text
session_key
  -> Credential
  + Route Target
  + upstream model
  + protocol dialect
  + last-seen timestamp
```

绑定一旦进入 `Bound`，后续请求只能使用该完整目标。RPM 用尽时等待原 Credential；目标处于冷却、
代理不可用、被禁用或不再支持该模型时保留绑定并返回明确的 any2api 本地错误。等待超时也返回本地
错误，不得切换 Credential、Route Target、模型或方言，不得删除旧绑定并自动建立新绑定。上游已经
返回的非 2xx 状态、正文和协议错误仍按透明上游错误契约返回，不因会话绑定生成替代消息或类型。

Credential 删除随配置发布原子清理它的全部绑定，并阻止在途 `Creating` Lease 或 Continuation 提交
把已删除目标重新写回。删除后普通显式 Session 可按全新首次请求重新创建，Continuation 未命中则返回
`session_binding_lost`。

### 创建意图

ProtocolAdapter 只随会话标识传递以下创建意图，不产生绑定分类：

- `X-Any2API-Session`、`X-Session-ID`、`Session-Id` / `Session_id`、Claude
  `metadata.user_id.session_id` 和 `conversation_id` 允许未命中时首次创建；
- Codex `previous_response_id` 必须续接已有绑定。未命中时返回 `session_binding_lost`，禁止猜测原
  Credential 或在另一个目标上重建。

可创建 Session 的首个请求继续使用版本化 `Creating` 租约串行确定目标。只有在绑定尚未提交、
下游仍为 `Pending` 且 RetrySafety 允许时，首次创建流程才可安全改用另一个候选；一旦提交为
`Bound`，目标永久固定到绑定被显式清理、Credential 删除、TTL 到期或进程退出。

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

### 单一设置

SettingRegistry 只保留：

| 设置 | 类型 | 默认值 | 允许范围 |
|---|---|---:|---:|
| `affinity.ttl` | duration_secs | `86_400` | `1..=2_592_000` |
| `affinity.wait_timeout` | duration_secs | `30` | `1..=86_400` |

两项设置按 PublishedSnapshot revision 捕获并支持现有默认值、覆盖值、生效值和恢复默认语义。
Web 将它们放在“设置 → 路由策略”的高级设置中，不提供粘性开关、绑定强度或可重绑模式。

项目仍处于首个正式版本之前，因此直接删除旧六个设置键及其类型、DTO、展示和测试，不增加别名、
双读、迁移映射或兼容分支。保存过旧键覆盖值的开发数据库不受兼容，必须按 pre-v1 规则重建；
会话绑定本身从不持久化，因此没有运行态数据迁移。

### 管理与观测

管理 API 保留清理全部绑定和按路由凭据清理的能力。普通总览只返回统一 `binding_count` 与
`creating_count`，不返回绑定类别、逐 Credential 分布、原始标识、HMAC 样本或绑定内容。

管理与观测不再按绑定类别拆分；首次创建发生安全候选切换时仍属于普通 Attempt 切换，不伪装成
现存绑定变化。

## 后果

- 调度器只需处理“未绑定选择候选”和“已绑定固定目标”两条路径。
- 普通 Session 和 Response ID 复用同一绑定表、TTL、等待与清理实现。
- 配置面从六项会话设置收敛为两项，不保留无法触发当前产品价值的模式分支。
- 绑定目标暂时不可用时可用性低于自动换号策略，但不会破坏有状态会话或把账号专属状态泄漏给另一个
  Credential；这是明确选择的正确性边界。

## 验证

- Protocol 测试覆盖显式 Session 与 Continuation 两种创建意图、代表性标识来源和
  `previous_response_id` 未命中。
- Runtime 测试覆盖并发 `Creating`、单一 TTL、访问刷新、固定目标等待、超时不切换、目标不可用不
  切换、清理、Credential 删除和重启空状态。
- JSON/SSE 契约覆盖 Response ID 在可见前绑定、普通 Session 首次创建、后续固定完整目标、
  `session_binding_lost` 和提交后禁止切换。
- Settings、管理 API 和 Web 测试覆盖两个新键、旧键不存在、统一聚合、保存/恢复默认和旧 deep link
  重定向。
