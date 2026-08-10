# ADR-0132：按 Provider 排列 OAuth 控制面请求起始时刻

- 状态：Accepted
- 日期：2026-08-11
- 决策人：maintainer
- 修订：ADR-0033、ADR-0036、ADR-0078、ADR-0111、ADR-0123

## 背景

ADR-0123 已经按 OAuthAccount、路由/认证代际和 traffic class 隔离 Client、TCP/TLS、HTTP/2 与 TLS resumption，但多个 OAuthAccount 仍按永久架构固定继承同一个全局 DIRECT/代理出口。连接隔离不会改变公网出口，也不会阻止多个独立 Client 在同一时刻建立连接。

当前定时 Token Worker、activity-driven quota Worker 和 Web 批量额度操作都允许最多 6 个账号并发。批量导入账号经常具有相近过期时间，并发数据面请求也会在相同 debounce 边界触发额度活动；本地受控 Transport 测试确认同一 Provider 的最多 6 个请求可以在同一调度 tick 同步进入 Transport。账号级 singleflight 只合并同一账号，不能约束跨账号起始突发。

这里的目标是减少无业务必要的同步控制面突发、降低 Token/额度 Endpoint 的瞬时压力，并维持明确的账号和连接隔离。它不隐藏共享出口，不改变应用、TLS 或 HTTP 指纹，也不尝试规避 Provider 的认证、配额或滥用策略。

## 决策

1. OAuth Runtime 创建一个服务生命周期的 `OAuthControlPlanePacer`，登录网络阶段、Device Code 轮询、authorization-code exchange、定时/按需 Token refresh、quota 查询/补充查询/reset 全部使用同一个实例。
2. Pacer 按 `ProviderKind` 分域；Codex、Claude、Grok 彼此不互相阻塞。同一 Provider 的每次 `TransportManager::execute` 开始前取得 FIFO 起始门闩，固定最小间隔为 500 ms。该值是单节点控制面保护常量，不进入 SettingRegistry。
3. 门闩只持有到允许本次 Transport 调用开始的时刻，随后立即推进下一可用时刻并释放；不持有响应头、Body 读取、解析或配置发布生命周期。因此慢请求仍可与后续请求有界重叠，现有最多 6 个 Worker Future 的资源上限继续有效。
4. 等待者取消时不得预留未来空槽。实现必须在 Provider 门闩内等待，到达实际起始时刻后才推进状态；取消自动释放门闩，下一等待者可使用原时刻。
5. 账号级 refresh/quota singleflight、Token version CAS、quota operation gate、自动 Worker debounce/最小账号间隔、网络 timeout、Retry-After 与公共请求绝对 precommit budget 保持不变。Pacer 不创建第二套账号调度器，不预留或释放数据面 RPM。
6. Data plane 明确不进入该 Pacer。OAuthAccount 的公开推理请求继续只服从统一路由、RPM、健康、粘性、重试和 Transport isolation；禁止借控制面保护增加隐藏的账号并发上限。
7. 不增加随机 jitter、伪造设备身份、随机 Header、TLS 参数或 per-account 出口。多个 OAuthAccount 继续固定继承同一个全局出口；需要停止某账号全部 Provider 通信时仍必须删除账号。

## 后果

- 同一 Provider 批量到期、批量额度刷新和并发 401 修复不再在同一 tick 同步起跑。
- 最多 6 个响应仍可并发在途，避免一个 30 秒慢 Token/额度响应串行阻塞所有账号。
- 一条多阶段 quota 操作的后续补充请求也服从相同起始间隔，因而不会在另一账号请求之间形成不受控旁路。
- 登录、刷新和额度操作之间会出现最多由队列长度决定的额外有界等待；取消不会留下幽灵预留。公共请求仍受原绝对 deadline 约束，无法容纳等待时按既有错误路径终止。
- 该变更不能改变或隐藏共享公网 IP；它只消除 any2api 自己制造的同步控制面突发。

## 验证

- paused-time 单元测试证明同 Provider 连续许可至少相隔 500 ms，不同 Provider 可同时开始。
- 取消排在门闩上的 Future 后，下一等待者不会被已取消任务多推迟一个间隔。
- 定时刷新受控 Transport 测试证明第二账号在第一请求仍在途时可以开始，但不能早于最小起始间隔。
- quota、401 refresh 和登录现有契约继续通过，证明所有路径仍使用账号级隔离、DIRECT/全局代理、严格 SSRF 和原有错误分类。
