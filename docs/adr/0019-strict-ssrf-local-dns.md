# ADR-0019: 严格 SSRF 的本地 DNS 与固定目标连接

- 状态：Accepted
- 日期：2026-07-21
- 决策者：maintainer

## 背景

当前 Transport 已经对 DIRECT 连接执行本地 DNS 解析、逐个校验 A/AAAA 结果并把连接固定到本次解析结果；HTTP 代理和 SOCKS5 默认使用代理侧解析（HTTP forward/CONNECT 由代理解析目标，SOCKS5h 由 SOCKS 服务端解析目标）。这种默认行为适合需要避免本地 DNS 泄漏的个人部署，但本机无法证明代理最终连接到的目标 IP。

Provider Endpoint 允许显式授权普通 HTTP 和内网地址。需要一个可选的严格 SSRF 模式，使管理员能够要求所有上游出口都由 any2api 在本地解析并校验目标地址，且不能因为 HTTP CONNECT 或 SOCKS5 而重新把域名解析权交给远端代理。

## 决策

### 1. 统一解析策略

- `DIRECT` 始终使用 any2api 的本地解析路径，无论严格模式开关是否开启。字面 IP 不执行 DNS，但仍执行同样的地址策略校验。
- 严格模式关闭时，HTTP 代理和 SOCKS5 保持既有受信远端 DNS 语义：HTTP forward/CONNECT 把原始域名交给代理，SOCKS5 使用远端域名地址类型（等价于 `socks5h`）。管理界面必须显示这一信任边界。
- 严格模式开启时，HTTP forward、HTTPS CONNECT 和 SOCKS5 均先由 any2api 解析 Endpoint 主机；解析结果全部经过 Endpoint 的 `allow_private_network` 策略校验，然后只使用已验证地址发起连接。代理仍只负责到达目标地址，不再负责解析 Endpoint 主机。
- 严格模式通过 `upstream.strict_ssrf` SettingRegistry 设置控制，默认 `false`，可热更新并显示默认值、覆盖值和生效值。

### 2. A/AAAA 校验与目标固定

- 对域名收集所有 A/AAAA 结果，去重后逐个校验。未显式允许内网时，任一 loopback、link-local、multicast、未指定、私网、metadata 或其他非公网地址都会使本次解析失败；不会过滤危险地址后继续选择其余地址。
- 对字面 IP 执行同样校验。解析为空、无法取得有效端口或地址校验失败统一归类为 `Dns + Endpoint + DefinitelyNotSent`。
- 解析结果属于本次 Transport 请求的不可变计划。连接池 key 包含主机、端口、协议和完整已验证地址集合；DNS 结果变化会进入新的 Client 代际，旧连接不会被新解析结果复用。
- 严格模式不会在连接失败后悄悄重新解析、回退远端 DNS 或切换到未验证地址。一次连接计划只使用其捕获的地址；是否开始新的上游 Attempt 由 Runtime 的 RetrySafety 和预算决定。
- 多地址连接使用解析器给出的稳定顺序并去重。DIRECT 连接器可以在同一连接建立阶段按顺序尝试地址；HTTP CONNECT、SOCKS5 和 HTTP forward 的目标地址在本次请求中固定为首个可构造的已验证地址，连接失败作为连接前失败返回，不在 Transport 内隐藏多次上游请求。

### 3. HTTP forward 与 CONNECT

- 严格 HTTP forward 请求把绝对 URI 的 authority 改为已验证 IP（IPv6 使用括号），同时显式保留原始 `Host` header。这样代理依据 IP 建立连接，Provider 仍能按原始 Host 做虚拟主机路由。
- 严格 HTTPS CONNECT 发送 `CONNECT <verified-ip>:<port>`，隧道建立后 TLS 使用原始域名作为 SNI 和证书校验名；隧道内 HTTP 请求继续使用原始 `Host` header。Provider URL 的 scheme、路径和 query 语义不被客户端输入覆盖。
- HTTP 代理认证只在 CONNECT/forward 受控边界注入；Provider Credential 不进入代理握手。代理返回 407 归类为 `ProxyHandshake + Proxy + RejectedBeforeExecution`。

### 4. SOCKS5

- 严格模式使用 SOCKS5 的 IP 地址类型发送已验证地址，禁止 SOCKS5h/域名地址类型；TLS 仍使用原始域名作为 SNI，普通 HTTP 请求仍保留原始 Host。
- SOCKS5 代理自身的 host/port 是用户显式配置的代理信任边界，不作为 Provider Endpoint 目标参与 SSRF 地址校验；代理连接失败或认证/命令失败只归因 Proxy。

### 5. 重定向、Host/SNI 与 DNS rebinding

- Transport 继续禁用自动重定向。未来若显式支持重定向，每一次跳转都必须重新解析、重新执行 Endpoint 网络授权，并且跨 authority 不得转发 Provider Credential；本 ADR 不开启重定向。
- 客户端提供的 `Host`、absolute-form URL、`Forwarded` 和 `X-Forwarded-*` 不参与目标 authority 构造。只有已发布 Endpoint 的结构化 URL决定原始 Host/SNI。
- Client 连接池只能复用同一 ProxyProfile 配置代际、同一 Endpoint scheme/authority 和同一已验证地址集合的连接。这样 DNS rebinding 不会把旧 IP 连接误当成新解析结果；已建立的旧连接在请求结束前继续使用其原代际。

### 6. 与远端 DNS 模式的关系

- `upstream.strict_ssrf=false` 不代表关闭 SSRF 保护：DIRECT 仍校验本地解析结果，Endpoint 的字面 URL 和显式网络授权仍在配置发布与 Transport 执行时生效。
- 远端 DNS 模式明确记录为用户配置的代理信任边界，管理界面必须提示本机无法验证代理最终目标 IP。
- 严格模式不是全局“允许内网”开关；`allow_private_network` 仍按 Provider Endpoint 独立决定是否接受私网结果。

## 后果

- 严格模式可以防止恶意或被 rebinding 的 Provider 域名经 HTTP CONNECT/SOCKS5 绕过本机地址策略，但会增加本地 DNS 查询并可能暴露 DNS 请求。
- HTTP CONNECT 的 TLS SNI/证书校验仍保持域名语义，避免把连接固定到 IP 后破坏虚拟主机 TLS。
- 严格代理路径使用固定目标连接器；连接池代际按解析结果隔离。连接前地址失败仍由 Runtime 决定是否启动下一次 Attempt，Transport 不伪装成一次请求的隐式重试。
- 不提供 DNS pinning 的跨重启恢复；进程重启后重新读取配置并重新解析，所有运行态连接和健康状态清空。

## 验证

- Domain/Settings 契约测试覆盖严格模式默认值、覆盖值、恢复默认和 Endpoint 私网授权组合。
- Transport 测试覆盖 DIRECT 全地址校验、HTTP forward IP authority + Host、HTTPS CONNECT IP target + 原始 SNI/Host、SOCKS5 IP address type、混合 A/AAAA 危险结果 fail-closed、DNS 结果代际隔离和 407 归因。
- 真实 HTTP 契约测试验证严格模式下不可达专属代理不会回退 DIRECT，远端 DNS 模式仍保持现有行为。
