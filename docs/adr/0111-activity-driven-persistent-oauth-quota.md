# ADR-0111：按实际使用触发的 OAuth 额度快照持久化

- 状态：Accepted
- 日期：2026-08-04
- 决策者：maintainer
- 修订：ADR-0034、ADR-0036、ADR-0045、ADR-0046、ADR-0070

## 背景

原有管理面把额度结果只保存在当前浏览器的 React Query 内存中。刷新页面、切换浏览器或进程重启后，即使上一次额度读取已经成功，也必须再次访问 Provider 才能看到结果。另一方面，固定周期扫描全部 OAuthAccount 会持续访问长期未使用的账号，增加无意义的上游请求、代理流量和风控面。

额度展示快照与路由健康并不是同一种状态。前者是已经经过 Provider Driver 校验、不会包含 Token 或原始响应的最后一次成功观测；后者包含耗尽、冷却、认证、RPM、会话和队列等运行态语义。禁止恢复后者不要求丢弃前者。

## 决策

1. 新增独立 `oauth_quota_snapshots` 表，以 `oauth_account_id` 为主键并使用 `ON DELETE CASCADE`。只保存版本化、大小有界、Provider-neutral 的安全额度结果与 `fetched_at`；不得保存原始 Provider 响应、OAuth JSON、Token、请求 Header 或错误正文。该表不是配置真相，写入不增加 config revision，也不进入 `PublishedSnapshot`。
2. 管理 API 拆分读缓存与访问上游：`GET /api/admin/oauth/accounts/{id}/quota` 只读取 SQLite，未曾成功刷新时返回 `null`；`POST /api/admin/oauth/accounts/{id}/quota/refresh` 才执行 Provider 的权威只读查询。手动刷新只有在安全快照成功写入 SQLite 后才返回成功；刷新失败保留此前最后一次成功快照。
3. 持久化快照只用于管理展示。进程启动不得用它恢复或建立额度健康、认证健康、冷却、RPM 窗口、`in_flight`、队列、会话或路由资格；从 SQLite 读取快照也不得调用健康同步。权威上游刷新仍可按 ADR-0070 更新当前进程的路由健康。
4. 真实公共请求选中 OAuthAccount 且 Attempt 已经进入 Transport 后，在该 Attempt 的 buffered 结算或 streaming EOF、错误、断连、Drop 时记录一次账号活动。候选选择失败、RPM 预留失败、请求编码失败和其他尚未进入上游网络的本地失败不得触发额度刷新。
5. 活动刷新由单个生命周期受管 Worker 合并。账号首次活动使用短 debounce，连续活动不无限推迟首次刷新；每个账号受最小自动刷新间隔约束，固定并发上限为 6。刷新进行中出现新活动时最多保留一个后续刷新。失败不按固定周期重试，只保留旧快照；只有新的真实账号活动才再次调度。没有待处理活动时 Worker 不启动轮询或扫描 SQLite/账号列表，进程启动也不为历史账号建立定时任务。
6. 管理额度查询和 reset 是管理面 Provider 操作，不进入数据面 Route 选择，也不预留该账号配置的本地数据面 RPM。它们仍受账号级 reset singleflight、全局代理、严格 SSRF、有界正文、读取超时以及活动 Worker 的并发和最小间隔保护。这样自动刷新不会因为刚完成的数据面请求已经占用 RPM 而必然失败，也不会释放或伪造任何数据面 RPM 名额。
7. 每次成功 upsert 或 reset 后的快照删除都推进进程内 quota change epoch。受认证的 `GET /api/admin/oauth/quota-events` SSE 只发送 `oauth_quota_changed` 失效事件和 keepalive，不携带账号、额度或原始响应，也不持久化或回放。新连接先发送当前 epoch；Web 只使活动的额度缓存失效并重新执行 SQLite GET，不由 SSE 直接访问 Provider。
8. Web 账号卡片挂载后立即读取缓存 GET，因而页面刷新或新浏览器可显示最后一次成功结果及抓取时间。单账号、“刷新全部额度”、失效账号实时诊断和 reset 后刷新统一调用 POST refresh；前端 Query cache 仍只存在于当前浏览器内存，不写 localStorage/sessionStorage。初始读取、SSE 和自动刷新不弹成功通知。
9. Codex reset 成功后删除重置前快照并发出失效事件，再由 Web 执行一次权威 refresh。若刷新失败，界面显示无当前快照而不是继续展示重置前结果。账号删除依赖外键级联清除快照。

自动刷新只覆盖当前具有 Provider 权威 quota 查询能力的 OAuthAccount。ProviderCredential API Key 没有统一、可靠的额度读取契约，不用本地请求日志或 Token 计数伪造余额。

## 备选方案

- 固定周期刷新全部账号：拒绝。长期未使用账号会持续产生无意义上游请求，且周期越短越接近轮询风暴，越长越不能满足活动账号的及时更新。
- 只在浏览器 localStorage 保存：拒绝。不同浏览器和设备不共享，无法由服务端活动更新，也会复制数据生命周期和迁移语义。
- 从 RequestLog 或响应 Token usage 扣减额度：拒绝。Provider 窗口、其他客户端消耗、账单金额和 reset 规则不可见，本地估算不是权威额度。
- 用持久化快照恢复路由健康：拒绝。快照可能过期，且会把展示缓存变成跨重启准入状态，违反运行态清空边界。
- 每个请求同步等待额度刷新：拒绝。它会把管理观测延迟和故障耦合到数据面响应，并延长 streaming Guard 生命周期。

## 后果

- 最后一次成功额度可跨页面、浏览器和进程重启展示，同时明确带有抓取时间；网络失败不会抹掉可用的最后观测。
- 活跃账号在真实请求结束后自动趋近最新额度；闲置账号不会被定时访问。
- 自动查询是额外的 Provider 管理读取，但由按账号合并、最小间隔和全局并发上限约束，且不阻塞公开请求。
- SQLite 增加一张可安全删除和重建的派生快照表；配置发布、OAuth JSON 和运行态恢复语义不变。

## 验证

- Migration 升级测试覆盖既有 OAuthAccount 数据保留、快照表约束和账号删除级联。
- Storage/Runtime 测试覆盖版本化编解码、大小上限、最后成功快照保留、手动刷新写入确认、缓存读取不更新健康和 reset 后删除。
- Worker 测试覆盖无活动不查询、同账号突发合并、最小间隔、跨账号并发上限、失败只在后续活动重试，以及 streaming EOF/错误/Drop 只记录一次活动。
- HTTP 契约覆盖 GET 不访问上游、POST refresh 持久化后可由新请求读取、SSE 鉴权/初始 epoch/变更通知和 DTO 脱敏。
- Web 测试覆盖初始持久化快照、抓取时间、手动/批量 POST refresh、SSE 只重读缓存、reset 清空旧快照和无成功通知。
