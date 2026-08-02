# ADR-0084：面向慢上游的宽松等待默认值

- 状态：Accepted
- 日期：2026-08-02
- 决策者：maintainer
- 修订：ADR-0016、ADR-0017

## 背景

第三方 API Key 上游的响应稳定性和延迟通常弱于 Provider 官方 OAuth 数据面。普通请求原先默认只允许
`15s` 等待响应头、`5s` 等待首个 SSE 事件、`60s` 等待提交后的下一个流 chunk，并且全部提交前
Attempt 只有 `20s` 总预算。部分可正常完成的上游会在两分钟以上才返回首个结果，旧默认会在上游仍在
工作时提前终止请求；本地 RPM 或固定会话等待 `30s` 也无法覆盖一个完整的 60 秒滚动窗口或短暂健康恢复。

## 决策

1. `upstream.read_timeout` 默认改为 `300s`。它仍只限制等待响应头或 buffered body 下一个 chunk 的连续
   空闲时间，每次成功读取后重新计时。
2. `stream.precommit.max_duration` 与 `stream.postcommit.idle_timeout` 默认均改为 `300s`。前者等待首个
   可接受 SSE 事件；后者只限制提交后的连续静默，不限制持续有上游数据的流总时长。
3. `retry.precommit_total_budget` 默认改为 `600s`，为慢 Attempt、候选切换和既有短退避保留共同的绝对
   边界。最大尝试次数、Credential 切换次数和同 Credential 重试次数保持不变。
4. `scheduler.queue_timeout` 与 `affinity.wait_timeout` 默认均改为 `180s`，使 RPM 窗口、短暂冷却和固定
   Credential 恢复有足够等待时间，仍受统一 QueueTicket 容量与取消约束。
5. 以上是统一 SettingRegistry 默认值，同时适用于 ProviderCredential 与 OAuthAccount。Runtime 不按
   凭据来源增加第二套调度或请求执行分支；默认值由更慢的合法上游需求决定。
6. Transport 的连接阶段继续使用独立 `10s` 默认 deadline。DNS、TCP、代理握手或 TLS 在请求体发送前
   明确失败时应快速释放并选择其他候选，不随服务端生成耗时一起放宽。
7. 冷却、熔断、重试次数和 `max_waiting_requests` 默认值保持不变。SQLite 中已有的显式覆盖值不迁移、
   不删除，继续优先于新的编译默认值。

## 后果

- 两分钟以上才产生首个结果的合法上游不会再因一分钟以内的默认窗口被提前判定不可用。
- 已提交 SSE 只要每次连续静默不超过五分钟，可以运行任意更长时间；该值不是总请求时长上限。
- 完全静默或失效的上游会更久才释放当前请求，这是提高慢上游成功率的明确取舍；连接前确定性故障仍会
  快速切换，RPM、队列容量和取消继续提供有界资源生命周期。
- 管理员此前显式设置的较短超时保持原值；只有没有覆盖值的实例自动采用新默认。

## 验证

- Domain 测试固定六项新默认值及其 SettingDefinition 元数据。
- 管理 API 契约验证 Web 可见的默认值和生效值与 SettingRegistry 一致。
- 既有虚拟时间与 HTTP 契约继续用显式短覆盖验证读取、首事件、流空闲、队列和固定会话超时，避免测试
  依赖真实的长等待。
