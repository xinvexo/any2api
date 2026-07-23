# ADR-0019: 严格 SSRF 的本地 DNS 与固定目标连接

- 状态：Accepted
- 日期：2026-07-21
- 决策者：maintainer

## 背景

当前 Transport 已经对 DIRECT 连接执行本地 DNS 解析、逐个校验 A/AAAA 结果并把连接固定到本次解析结果；HTTP 代理和 SOCKS5 默认使用代理侧解析（HTTP forward/CONNECT 由代理解析目标，SOCKS5h 由 SOCKS 服务端解析目标）。这种默认行为适合需要避免本地 DNS 泄漏的个人部署，但本机无法证明代理最终连接到的目标 IP。

Provider Endpoint 的 Base URL 是管理员明确配置的受信任目标。需要一个可选的严格本地解析模式，使管理员能够要求所有上游出口都由 any2api 在本地解析并固定目标地址，且不能因为 HTTP CONNECT 或 SOCKS5 而重新把域名解析权交给远端代理。

## 决策

### 1. 统一解析策略

- `DIRECT` 始终使用 any2api 的本地解析路径，无论严格模式开关是否开启。字面 IP 不执行 DNS。
- 严格模式关闭时，HTTP 代理和 SOCKS5 保持既有受信远端 DNS 语义：HTTP forward/CONNECT 把原始域名交给代理，SOCKS5 使用远端域名地址类型（等价于 `socks5h`）。管理界面必须显示这一信任边界。
- 严格模式开启时，HTTP forward、HTTPS CONNECT 和 SOCKS5 均先由 any2api 解析 Endpoint 主机，然后只使用本次解析所得地址发起连接。代理仍只负责到达目标地址，不再负责解析 Endpoint 主机。
- 严格模式通过 `upstream.strict_ssrf` SettingRegistry 设置控制，默认 `false`，可热更新并显示默认值、覆盖值和生效值。

### 2. A/AAAA 解析与目标固定

- 对域名收集所有 A/AAAA 结果并去重。管理员配置可以解析到公网、loopback、link-local、私网或其他地址类别，不在 Transport 中追加第二套授权判断。
- 解析为空或无法取得有效端口统一归类为 `Dns + Endpoint + DefinitelyNotSent`。
- 解析结果属于本次 Transport 请求的不可变计划。连接池 key 包含主机、端口、协议和完整解析地址集合；DNS 结果变化会进入新的 Client 代际，旧连接不会被新解析结果复用。
- 严格模式不会在连接失败后悄悄重新解析或回退远端 DNS。一次连接计划只使用其捕获的地址；是否开始新的上游 Attempt 由 Runtime 的 RetrySafety 和预算决定。
- 多地址连接使用解析器给出的稳定顺序并去重。DIRECT 连接器可以在同一连接建立阶段按顺序尝试地址；HTTP CONNECT、SOCKS5 和 HTTP forward 的目标地址在本次请求中固定为首个可构造的解析地址，连接失败作为连接前失败返回，不在 Transport 内隐藏多次上游请求。

### 3. HTTP forward 与 CONNECT

- 严格 HTTP forward 请求把绝对 URI 的 authority 改为已验证 IP（IPv6 使用括号），同时显式保留原始 `Host` header。这样代理依据 IP 建立连接，Provider 仍能按原始 Host 做虚拟主机路由。
- 严格 HTTPS CONNECT 发送 `CONNECT <verified-ip>:<port>`，隧道建立后 TLS 使用原始域名作为 SNI 和证书校验名；隧道内 HTTP 请求继续使用原始 `Host` header。Provider URL 的 scheme、路径和 query 语义不被客户端输入覆盖。
- HTTP 代理认证只在 CONNECT/forward 受控边界注入；Provider Credential 不进入代理握手。代理返回 407 归类为 `ProxyHandshake + Proxy + RejectedBeforeExecution`。

### 4. SOCKS5

- 严格模式使用 SOCKS5 的 IP 地址类型发送已验证地址，禁止 SOCKS5h/域名地址类型；TLS 仍使用原始域名作为 SNI，普通 HTTP 请求仍保留原始 Host。
- SOCKS5 代理自身的 host/port 是用户显式配置的代理信任边界，不作为 Provider Endpoint 目标参与 SSRF 地址校验；代理连接失败或认证/命令失败只归因 Proxy。

### 5. 重定向、Host/SNI 与 DNS rebinding

- Transport 继续禁用自动重定向。未来若显式支持重定向，每一次跳转都必须重新解析，并且跨 authority 不得转发 Provider Credential；本 ADR 不开启重定向。
- 客户端提供的 `Host`、absolute-form URL、`Forwarded` 和 `X-Forwarded-*` 不参与目标 authority 构造。只有已发布 Endpoint 的结构化 URL决定原始 Host/SNI。
- Client 连接池只能复用同一 ProxyProfile 配置代际、同一 Endpoint scheme/authority 和同一已验证地址集合的连接。这样 DNS rebinding 不会把旧 IP 连接误当成新解析结果；已建立的旧连接在请求结束前继续使用其原代际。

### 6. 与远端 DNS 模式的关系

- `upstream.strict_ssrf=false` 时，DIRECT 仍本地解析并固定目标；HTTP/SOCKS5 远端 DNS 模式把解析权交给用户配置的代理。
- 严格模式控制解析位置和目标固定，不是地址类别授权开关；Base URL 指向的公网或内网目标都可访问。

## 后果

- 严格模式可以防止 Provider 域名经 HTTP CONNECT/SOCKS5 改由代理重新解析，并把本次请求固定到本地解析结果；它不阻止管理员配置私网目标，且会增加本地 DNS 查询并可能暴露 DNS 请求。
- HTTP CONNECT 的 TLS SNI/证书校验仍保持域名语义，避免把连接固定到 IP 后破坏虚拟主机 TLS。
- 严格代理路径使用固定目标连接器；连接池代际按解析结果隔离。连接前地址失败仍由 Runtime 决定是否启动下一次 Attempt，Transport 不伪装成一次请求的隐式重试。
- 不提供 DNS pinning 的跨重启恢复；进程重启后重新读取配置并重新解析，所有运行态连接和健康状态清空。

## 验证

- Domain/Settings 契约测试覆盖严格模式默认值、覆盖值和恢复默认。
- Transport 测试覆盖 DIRECT 私网访问、HTTP forward IP authority + Host、HTTPS CONNECT IP target + 原始 SNI/Host、SOCKS5 IP address type、DNS 结果代际隔离和 407 归因。
- 真实 HTTP 契约测试验证严格模式下不可达专属代理不会回退 DIRECT，远端 DNS 模式仍保持现有行为。
