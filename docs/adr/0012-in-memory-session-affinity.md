# ADR-0012: 进程内会话粘性与固定 Credential 等待

- 状态：Accepted
- 日期：2026-07-19
- 决策者：maintainer

## 背景

Codex `previous_response_id` 可能引用上游 Credential 保存的服务端状态，普通 Codex/Claude 会话也需要尽量保持在同一 Credential。当前数据面已经具备同协议路由、原子 Permit、QueueTicket 与 SSE 预提交边界，但每次请求仍独立执行最低负载率选择。

会话绑定不能持久化，也不能与 `GatewayApiKey` 建立关系。实现还必须避免两个并发首请求为同一软会话分别选择不同 Credential，并保证硬绑定或 `strict` 请求在等待容量时优先于该 Credential 的普通请求。

## 决策

- ProtocolAdapter 在解码时只提取架构规定的显式会话标识，不根据 Prompt、System Prompt 或消息内容生成会话哈希：
  1. Codex `previous_response_id`；
  2. `X-Any2API-Session`；
  3. `X-Session-ID`；
  4. `Session-Id` / `Session_id`；
  5. Claude `metadata.user_id` 内的 `session_id`；
  6. `conversation_id`。
- 原始会话标识只停留在协议解码到 Runtime 哈希这一小段生命周期。`AffinityRegistry` 启动时生成进程级随机 HMAC-SHA256 密钥，分别使用硬绑定和软绑定域分离计算不可逆键；日志、管理 DTO 和 Debug 不显示原始值。
- `AffinityRegistry` 属于稳定 RuntimeRegistry，跨连续 PublishedSnapshot 复用；`AffinityPolicy` 按值进入 PublishedSnapshot。所有硬/软绑定、Creating 状态和 HMAC 密钥在进程重启后清空。
- 软会话未命中时，短 Mutex 事务内创建版本化 `Creating` 租约；网络 I/O 不持锁。其他同会话请求等待该租约提交或撤销，不启动第二个创建者。请求失败、取消或 Drop 时由租约清理并唤醒等待者。
- 软 `prefer` 命中时先等待绑定 Credential，达到 `affinity.soft.prefer_wait_timeout` 后才原子转为新的 Creating 并重新负载均衡；`strict` 只等待原 Credential。硬绑定只允许原 Credential、Route Target、上游模型和协议方言。
- 固定 Credential 等待仍取得全局 QueueTicket。每个 CredentialRuntimeHandle 维护固定等待者计数；普通选择在该 Credential 存在固定等待者时暂不竞争其新释放槽位，固定请求使用专用 acquire 路径。该优先级只影响同一 Credential，不阻塞其他 Credential。
- Codex 非流式成功响应的顶层 `id`，以及 SSE `response.created.response.id`，在向客户端可见前写入硬绑定。绑定容量耗尽或身份冲突时不得暴露该 Response ID。
- `/v1/responses/compact` 只使用显式软会话，不创建硬绑定；`/v1/messages/count_tokens` 不参与任何会话粘性。
- 绑定表使用明确的进程内容量上限，并在插入压力出现时先清理过期项；不会引入后台持久化、恢复或复杂缓存服务。
- 管理 API 提供运行时统计、截断 Session Hash 样本、按 Credential 清理和全部清理。Affinity 页面复用统一 SettingRegistry 修改六项 `affinity.*` 设置。

## 首个切片边界

- 本切片不同时实现健康、冷却、熔断或多 Attempt 重试。
- 硬绑定/`strict` 目标缺失、禁用或代理不可用时返回 `session_binding_lost`；不会猜测或切换 Credential。
- `prefer` 只在绑定目标不再是有效候选，或绑定 Credential 等待超时后重绑。上游 Attempt 一旦开始，本切片不进行自动切换。
- TTL 使用访问时刷新语义；设置热更新后，下一次访问按当前 PublishedSnapshot 的 TTL 判断。

## 后果

- 会话选择进入独立 affinity 模块，不向中央 Provider `match` 或 Axum Handler 增加业务分支。
- 固定等待优先级在容量线性化点实现，普通调度器不需要维护第二套队列。
- 进程重启后的旧 `previous_response_id` 必然得到明确错误，符合不恢复运行态的项目边界。
- 后续健康与重试切片可以在同一绑定目标和固定 acquire API 上增加冷却等待，不需要重写协议或管理页面。

## 验证

- Protocol 测试覆盖提取优先级、Claude Code 两种 `metadata.user_id` 形式、无 Prompt 哈希兜底和 Response ID 提取。
- Runtime 测试覆盖软命中、并发 Creating、prefer 超时重绑、strict/硬绑定不切换、固定等待优先、TTL、清理与重启空状态。
- HTTP 契约覆盖 Codex JSON/SSE 硬续接、Claude 软粘性、未知旧 Response ID 和管理清理 API。
- Web 单元测试覆盖 affinity 设置、统计、截断 Hash、按 Credential 清理和全部清理；真实浏览器验收覆盖 1440 桌面与 390×844 窄屏布局。
