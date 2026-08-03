# ADR-0094: 健康竞争失败时精确回滚尚未开始的 RPM 预留

- 状态：Accepted
- 日期：2026-08-03
- 决策者：maintainer

## 背景

候选选择先做 Credential、Endpoint 与 Proxy 健康预检查，再在线性化点为 Credential 预留滚动 60 秒 RPM 名额并增加 `in_flight`，随后取得真正随 Attempt 持有的健康 Guard。预检查与 Guard 获取之间存在不可消除的竞争：Endpoint 可能刚被其他 Attempt 打开，或唯一 Half-Open 探针可能被并发请求取得。

旧行为在这次 Guard 获取失败后只释放 `in_flight`，已经写入窗口的 RPM 时间戳仍保留 60 秒。等待循环每次重选都可能再次遇到同一竞争，使没有开始上游 I/O 的选择失败消耗真实吞吐。

简单改成“先取得健康 Guard，再预留 RPM”并不合适：Half-Open Guard 获取成功后若发现 RPM 已满，释放探针会推进全局 scheduler epoch；等待者可能被自己刚释放的探针立即唤醒，重复取得探针、发现 RPM 已满并再次释放，形成无上游 Attempt 的唤醒空转。它还会让健康 Guard 在固定等待优先级和 RPM 线性化点之外短暂占用。

## 决策

- 保留当前顺序：健康预检查、原子选择并预留 RPM、取得 Attempt 健康 Guard。健康预检查只用于减少无效尝试，真正的 Guard 获取仍关闭检查后的竞争窗口。
- 有限 RPM 的每次窗口预留分配进程内唯一 `RateReservation`，包含单调 ID 与预留时间。滚动窗口存储该记录而不是裸时间戳；无限 RPM 不创建令牌。
- `RoutingPermit` 持有可选预留令牌。只有候选选择内部在健康 Guard 获取失败、`SelectedCandidate` 尚未形成且任何上游 I/O 尚未开始时，才可调用消耗 Permit 的显式 `rollback_before_attempt`。
- 回滚在同一个 `CredentialRuntimeHandle` 窗口 Mutex 下按唯一 ID 删除自己的记录。插入、容量判断和删除因此共享同一串行化边界；回滚不能删除其他并发 Attempt 的名额，也不能使任何成功预留时的窗口计数超过 RPM。
- 回滚随后把 `in_flight` 精确减少一次，并只在确实删除窗口记录后、释放窗口锁之后推进统一 scheduler epoch，使已排队请求立即重新选择。令牌若已经因极端停顿超过 60 秒而被其他窗口操作裁剪，删除是安全 no-op，Permit 仍正常释放。
- 普通 `RoutingPermit::drop` 永远不回滚 RPM。健康 Guard 成功取得并形成 `SelectedCandidate` 后，预留即代表一次上游 Attempt；后续请求构造、Transport、响应、取消、流式 Drop 或 RetrySafety 结果仍保留到 60 秒自然到期。
- generation 轮询与固定会话选择共用 `RouteCandidate::acquire_health_with_rpm_reservation`，避免两条路径出现不同回滚语义。
- 本决策不增加可配置项、不持久化令牌，也不按 RetrySafety 建立第二套窗口状态机。

## 后果

- 健康预检查后的 Half-Open/健康竞争不再虚耗 Credential RPM，等待循环也不会在每次竞争失败后累积窗口记录。
- 并发请求可能在预留与紧随其后的回滚之间短暂观察到保守占用；回滚后的 epoch 会使等待策略立即重选。该短窗口不会超发，也不把回滚扩展到已经进入 Attempt 的不确定阶段。
- 每个有限 Credential 最多保存其 RPM 上限对应的 `RateReservation`；相较原时间戳只增加一个 `u64` ID，仍受 `100_000` 上限约束。

## 验证

- RateWindow 单元测试验证按唯一令牌精确删除、重复删除无效、剩余记录与 60 秒到期时间不被改写。
- 既有并发测试继续验证滚动窗口从不超过 RPM、到期可重新准入和热更新不重置有限窗口。
- generation 选择回归测试构造健康预检查后 Half-Open 探针被抢占的确定性竞争，验证失败候选的 `requests_in_window` 与 `in_flight` 都回到零，并继续选择同 tier 健康候选。
- 固定与 generation 路径都调用同一个“健康获取或精确回滚”边界；正常 Attempt、失败、取消与流结束测试继续验证 RPM 不归还。
