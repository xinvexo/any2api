# ADR-0050: RequestLog 保存可信解析后的客户端 IP

- 状态：Accepted
- 日期：2026-07-26
- 决策者：maintainer

> 可信代理列表的配置来源已由 ADR-0072 改为热更新的 `network.trusted_proxy_cidrs`；转发头缺失与重复策略由 ADR-0096 修订。

## 背景

any2api 部署时可能由 Nginx 反代，Nginx 前还可能存在 Cloudflare。RequestLog 需要保存可信解析后的客户端地址；直接记录任意 `X-Forwarded-For` 或 `CF-Connecting-IP` 会把客户端可伪造文本写入日志，公开面另写一套代理头解析又会与管理面来源判断产生分歧。

## 决策

- 将管理面已有的可信代理解析器提升为 Server 级 `ClientAddressPolicy`，管理鉴权与公开 API 入口复用同一实现。
- TCP 对端和 XFF 中的地址先通过 `IpAddr::to_canonical()` 规范化；IPv4-mapped IPv6 以原生 IPv4 参与 CIDR 匹配、loopback 判断与持久化。TCP 对端不在当前 PublishedSnapshot 的 `network.trusted_proxy_cidrs` 时，忽略所有转发头并使用规范化 TCP 对端 IP。
- TCP 对端可信时，多行 `X-Forwarded-For` 按收到顺序合并为一条逻辑地址链，再从 TCP 对端开始向右到左剥离连续可信代理，首个不可信地址是客户端地址；XFF 完全缺失时使用规范化 TCP 对端。`X-Forwarded-Proto` 完全缺失时按不安全 HTTP 处理；它出现时必须是唯一的 `http` 或 `https`。空值、非法 IP、非法/重复协议仍 Fail-Closed，完整依据见 ADR-0096。
- Cloudflare 来源规范化属于前置 Nginx 的职责。any2api 不直接信任或持久化 `CF-Connecting-IP`，也不保存原始 `Forwarded` / `X-Forwarded-*` 文本。
- `PublicRequest` 携带解析后的 `IpAddr` 进入 Runtime，`RequestRecorder` 将其写入最终 RequestLog。该地址不参与上游选择、RPM、会话粘性、重试或鉴权结果。
- Schema 使用必填的 `request_logs.client_ip TEXT NOT NULL`；进入模型执行链的每个公开请求都写入规范化 IPv4/IPv6 字符串。Storage 在读写边界使用 Rust `IpAddr` 校验语义。
- 管理请求日志列表与详情 DTO 返回必填 `client_ip`，Web 在展开详情和独立详情页展示。该字段只存在于已认证的管理响应和浏览器当前查询缓存，不进入普通结构化文件日志。
- `HttpAccessLog.client_ip` 保持可空，因为最外层系统日志中间件可能在地址解析失败时仍需记录本地失败；这不放宽 RequestLog 的必填约束。
- 管理面的直接本机权限不从该逻辑客户端地址派生；Setup 和关闭远程管理时的访问边界由 ADR-0088 单独定义。
- 沿用 ADR-0015 的记录范围：鉴权失败、未知公开路由和方法错误不因本决策新增 SQLite RequestLog。

## 后果

- 正确配置可信代理 CIDR 后，日志中的地址在直连、Nginx 以及 Cloudflare + Nginx 链路中具有同一含义。
- 配错可信代理 CIDR 时不会退回相信客户端头：未信任代理只记录其 TCP 地址，已信任代理完全缺少头时保守记录 TCP 地址并按 HTTP 处理，携带非法头时请求被拒绝。
- 每条 RequestLog 都有可解析的规范客户端地址，不需要未记录占位或回填逻辑。

## 验证

- Server 测试覆盖直连、可信代理、多跳链、伪造最左值、重复/缺失/非法转发头。
- Storage 测试覆盖 IPv4/IPv6 往返、`NULL` 约束拒绝和非法数据库值拒绝。
- 公共请求契约覆盖直连地址持久化、可信代理真实地址持久化以及非法可信代理头的协议兼容 400 响应。
- Web 契约与组件测试覆盖必填地址解析和详情展示。
