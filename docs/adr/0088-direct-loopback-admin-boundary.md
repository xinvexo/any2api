# ADR-0088：直接 loopback 管理边界与 IP 地址规范化

- 状态：Accepted
- 日期：2026-08-03
- 决策人：项目维护者
- 修订：ADR-0014、ADR-0050、ADR-0072

## 背景

Server 同时需要两种地址语义：TCP `ConnectInfo` 表示真正连接到 any2api 的对端；可信代理解析得到的
`client_ip` 表示逻辑客户端，用于日志和登录失败限流。旧实现把后者的 `is_loopback()` 同时用于 Setup 与
`admin.remote_enabled=false`。当可信 CIDR 内的主机直连服务并提交
`X-Forwarded-For: 127.0.0.1` 时，解析结果会获得本机权限。

简单改成只检查 TCP peer 仍不正确：同机 Nginx/Caddy 的 peer 通常是 loopback，如果忽略请求已经进入
trusted-proxy 解析这一事实，反代后的所有远程客户端都会继承反代进程的本机权限。

双栈 listener 还有独立的表示问题。操作系统可以把 IPv4 peer 暴露为 `::ffff:a.b.c.d`；直接对它执行
IPv4 CIDR 匹配或 IPv6 `is_loopback()` 会分别漏掉可信代理和 `127.0.0.1`。

## 决策

1. Server 在客户端地址入口先对 TCP peer 执行 `IpAddr::to_canonical()`；XFF 中每个成功解析的地址也在
   信任链处理前执行同一规范化。IPv4-mapped IPv6 因此统一成为原生 IPv4，再参与 CIDR、loopback、
   DTO 与日志持久化。
2. `ClientConnection` 同时保留逻辑 `client_ip`、是否经可信代理、传输安全状态和直接本机权限。直接本机
   权限只在“规范化 TCP peer 是 loopback 且本请求未进入 trusted-proxy 解析”时成立。
3. Setup 和 `admin.remote_enabled=false` 只使用直接本机权限。XFF 中的 loopback 永远不能放宽这两个
   边界；同机可信反代后的请求也不视为直接本机连接。远程初始化继续使用
   `ANY2API_ADMIN_PASSWORD`，本机管理员仍可直接连接 any2api 端口完成 Setup。
4. 逻辑 `client_ip` 继续用于管理员登录失败窗口、RequestLog 与 HttpAccessLog；`X-Forwarded-Proto`
   继续决定反代连接的 HTTPS/Cookie 状态。本决策不把 Gateway Key、调度或 RPM 与客户端地址绑定。
5. 管理会话 DTO 的 `client_loopback` 与明文风险提示使用直接本机语义，使 Web 展示与实际访问边界一致。
6. 缺失 peer 和当前定义为非法的可信代理头继续 Fail-Closed；ADR-0096 允许可信代理请求在 XFF/XFP
   完全缺失时分别降级为 TCP 对端与非安全 HTTP，但连接仍标记为经可信代理，不能获得直接本机权限。

## 后果

- 可信网段中的客户端不能通过伪造 loopback XFF 绕过关闭的远程管理或首次 Setup 限制。
- 同机反向代理仍可提供真实来源与 HTTPS 信息，但不会替远程请求继承 loopback 特权。
- `[::]` 双栈监听得到的 IPv4-mapped peer 与原生 IPv4 在信任和本机判断上保持一致。
- 日志中的逻辑来源和管理权限来源不再被一个 `is_loopback()` 隐式混为同一概念。

## 验证

- Server 单元测试覆盖 mapped loopback、mapped IPv4 可信代理、XFF 地址规范化，以及可信 loopback 代理
  不具有直接本机权限。
- 管理 HTTP 契约使用 IPv4-mapped loopback 完成 Setup，并证明 trusted proxy 提交单一 loopback XFF
  仍不能通过 Setup 或 `admin.remote_enabled=false`。
- 公开请求可信代理契约继续证明规范化后的逻辑地址进入 RequestLog。
