# ADR-0023: 负载均衡运行态检查与轻量调度计数

- 状态：Accepted
- 日期：2026-07-22
- 决策者：maintainer

## 背景

Credential 最大并发、最低负载率选择、QueueTicket、会话固定等待和分层健康已经在 Runtime 中运行，但 Web 的“负载均衡”仍是占位页。管理面需要解释当前容量为何满载、某个 Credential 为何未被选择，以及请求是否正在排队；不能为此复制调度状态、持久化运行态或引入指标服务。

## 决策

- `RuntimeRegistry` 基于当前 `PublishedSnapshot` 生成只读 `BalancingRuntimeSnapshot`。快照读取现有原子容量、固定等待者、辅助请求占用、QueueCoordinator 与健康对象，不取得 Permit、不推进健康状态。
- 每个稳定 `CredentialRuntimeHandle` 增加 Relaxed 原子计数：普通 Attempt 成功选中、辅助请求成功选中、满载过滤、Credential/模型健康过滤、Endpoint 健康过滤和 Proxy 健康过滤。
- “成功选中”只在 Credential Permit 与当前 Attempt 所需健康 Permit 全部取得后计数；后续本地编码失败或上游失败不撤销该次选择。
- 普通、固定会话与辅助选择都使用同一计数入口。候选在等待重选、健康竞态或 CAS 竞争中可以被多次检查，因此过滤值明确表示调度过滤事件，不表示唯一请求数，也不参与计费、配额、冷却或调度决策。
- 健康快照按每个 Credential 实际被启用 Route Target 引用的 upstream model 枚举，并分别检查 Credential+模型、Endpoint 与解析后的实际 Proxy。状态只返回 `available`、`cooling`（剩余毫秒）或 `unavailable`；不暴露内部失败时间、地址或 Secret。
- 管理 API `GET /api/admin/balancing` 在当前管理员认证与 no-store 边界下返回配置 revision、scheduler epoch、队列策略、Provider 汇总、Credential 运行态与模型健康。
- Web `/balancing` 展示只读运行态并复用 `SettingsManagement keyPrefix="scheduler."` 编辑现有调度设置；不新增另一套配置表单或轮询协议。
- 所有运行态和计数仅保存在内存。配置热更新复用 Credential 句柄时保留；Credential 删除、服务重启或新进程启动后清空。

## 备选方案

- 从 RequestLog 聚合选择比例：历史写入允许丢弃且异步落库，无法精确表达当前容量与过滤原因，放弃。
- 引入 Prometheus/时序数据库或持久化计数表：超出个人单节点首版需求，并会产生恢复和保留策略，放弃。
- 把健康压缩成 Credential 单一状态：会错误放大模型级 429、Endpoint 故障或代理故障，违背现有健康作用域，放弃。
- 只展示 `in_flight/max_concurrency`：无法解释固定等待、辅助请求和调度过滤，信息不足，放弃。

## 后果

热路径每次成功选择或候选过滤增加一个 Relaxed 原子加法，不增加锁、I/O 或持久化。过滤计数可能因等待重选而高于客户端请求数，UI 必须持续使用“过滤次数”而非“失败请求”。健康快照在管理员读取时按当前 Credential×上游模型遍历，规模受个人实例配置限制，不进入公开请求路径。

## 验证

- Runtime 测试覆盖选择/过滤计数、固定等待者、热更新句柄复用、分层健康和重启清零。
- Server 契约覆盖空配置、Credential 容量与队列策略 DTO、管理员认证/no-store。
- Web 测试覆盖加载、空态、Credential 容量、过滤语义、分层健康、刷新错误保留旧数据与 scheduler 设置组合。
- Playwright 核心页面集合加入 `/balancing`，覆盖真实服务直达与 390px 无水平溢出。
