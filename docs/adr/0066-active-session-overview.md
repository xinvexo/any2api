# ADR-0066: 总览只展示当前策略下的活动显式会话

- 状态：Accepted
- 日期：2026-07-29
- 决策者：maintainer

## 背景

`AffinityRegistry` 为了调度正确性统一保存普通显式 Session 绑定和 Response ID
Continuation 索引。原总览直接展示整张表的 `binding_count`，会导致一个多轮 Responses
会话随每个 Response ID 增加多条“会话”，也会在 `affinity.enabled=false` 时继续显示旧的
普通绑定和必须保留的 Continuation 索引。该数字回答的是“内存中有多少路由索引”，
而不是管理员在总览中需要的“当前有多少会话正在参与粘性路由”。

## 决策

- 总览会话响应改为只返回 `affinity_enabled`、`active_session_count` 和
  `creating_session_count`。不再返回统一绑定总数、Continuation 索引数、逐 Credential 分布或绑定样本。
- `active_session_count` 只计数 TTL 内的普通显式 Session `Bound` 记录，并且只有当当前
  PublishedSnapshot 的 `affinity.enabled=true` 时才返回实际数量；关闭时返回 `0`。
- `creating_session_count` 只计数普通显式 Session `Creating` 记录；关闭时返回 `0`。
- Response ID Continuation 仍按 ADR-0062/0064 创建、命中、刷新和清理，不受普通粘性开关影响；
  它们只是必须续接的路由索引，不计入活动会话指标。
- 统一 Registry 的调度语义、TTL、目标、等待、清理和容量上限不拆分。绑定只保留一个最小内部
  `Session` / `Continuation` 来源标记，专用于聚合观测，不产生第二套调度分支。
- 协议没有通用的“会话已结束”信号。因此开关开启时，“活动”表示当前策略仍会命中且
  尚未超过 `affinity.ttl` 的显式会话，不表示当前正有 HTTP Body 在传输。页面必须明示该口径。

## 后果

会话粘性关闭时，总览与当前生效策略一致地显示 `0`，但不会为了视觉数字破坏
`previous_response_id` 的续接正确性。开启时，多轮 Responses 产生的多个 Response ID 不再被错计为多个会话。

## 验证

- Runtime 测试覆盖普通 Session 与 Continuation 共存时的分类聚合、TTL 清理和开关过滤。
- 管理 HTTP 契约确认关闭时活动/建立中会话均为 `0`，Continuation 索引仍可完成续接。
- React 契约和组件测试覆盖新响应字段、关闭状态和 TTL 口径说明。
