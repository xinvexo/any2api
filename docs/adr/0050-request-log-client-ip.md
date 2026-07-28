# ADR-0050: RequestLog 保存可信解析后的客户端 IP

- 状态：Accepted
- 日期：2026-07-26
- 决策者：maintainer

## 背景

any2api 部署时可能由 Nginx 反代，Nginx 前还可能存在 Cloudflare。RequestLog 目前能关联网关 Key 和最终上游来源，但不能定位请求来自哪个客户端。直接记录任意 `X-Forwarded-For` 或 `CF-Connecting-IP` 会把客户端可伪造文本写入日志；公开面另写一套代理头解析又会与管理面现有的来源判断产生分歧。

## 决策

- 将管理面已有的可信代理解析器提升为 Server 级 `ClientAddressPolicy`，管理鉴权与公开 API 入口复用同一实现。
- TCP 对端不在 `ANY2API_TRUSTED_PROXY_CIDRS` 时，忽略所有转发头并使用 TCP 对端 IP。
- TCP 对端可信时，要求恰好一个有效的 `X-Forwarded-For` 和 `X-Forwarded-Proto`；从 TCP 对端开始按 XFF 右到左剥离连续可信代理，首个不可信地址是客户端地址。缺失、重复、空值或非法 IP/协议一律 Fail-Closed。
- Cloudflare 来源规范化属于前置 Nginx 的职责。any2api 不直接信任或持久化 `CF-Connecting-IP`，也不保存原始 `Forwarded` / `X-Forwarded-*` 文本。
- `PublicRequest` 携带解析后的 `IpAddr` 进入 Runtime，`RequestRecorder` 将其写入最终 RequestLog。该地址不参与上游选择、RPM、会话粘性、重试或鉴权结果。
- 规范首版 Schema 在 `request_logs` 中包含可空 `client_ip TEXT`；新进入模型执行链的公开请求写入规范化 IPv4/IPv6 字符串。Storage 在读写边界使用 Rust `IpAddr` 校验语义。
- 管理请求日志列表与详情 DTO 返回 `client_ip`，Web 在展开详情和独立详情页展示。该字段只存在于已认证的管理响应和浏览器当前查询缓存，不进入普通结构化文件日志。
- 沿用 ADR-0015 的记录范围：鉴权失败、未知公开路由和方法错误不因本决策新增 SQLite RequestLog。

## 后果

- 正确配置可信代理 CIDR 后，日志中的地址在直连、Nginx 以及 Cloudflare + Nginx 链路中具有同一含义。
- 配错可信代理 CIDR 时不会退回相信客户端头：未信任代理只记录其 TCP 地址，已信任代理携带非法头时请求被拒绝。
- 历史日志没有可追溯来源时明确显示未记录，不进行猜测或回填。

## 验证

- Server 测试覆盖直连、可信代理、多跳链、伪造最左值、重复/缺失/非法转发头。
- Storage 测试覆盖 IPv4/IPv6/历史 `NULL` 往返和非法数据库值拒绝。
- 公共请求契约覆盖直连地址持久化、可信代理真实地址持久化以及非法可信代理头的协议兼容 400 响应。
- Web 契约与组件测试覆盖地址解析、未记录状态和详情展示。
