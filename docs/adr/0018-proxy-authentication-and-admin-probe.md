# ADR-0018: 代理认证与管理探测边界

- 状态：Accepted
- 日期：2026-07-21
- 决策者：maintainer

## 背景

现有 DIRECT、HTTP 与 SOCKS5h Transport 已进入公开请求链路，但 ProxyProfile 只保存 host/port。需要补齐 HTTP Basic 与 SOCKS5 用户名密码认证，并在管理 Web 中提供可验证配置的测试操作。密码不能进入普通领域对象的可见字段、普通读取/响应 DTO、日志或连接池 key；管理探测也不能变成可提交任意 URL 的 SSRF 工具。

严格 SSRF 本地 DNS 会改变 HTTP CONNECT 与 SOCKS5 的目标固定语义，风险和实现范围明显大于认证接线，因此不与本决策合并。

## 决策

- `ProxyProfile` 只保存可见的认证元数据与单调 `authentication_version`；密码保存在独立 `proxy_passwords` 记录中，使用现有 XChaCha20-Poly1305 Secret Vault 加密。
- 代理密码 AAD 绑定 `ProxyProfileId + authentication_version`。认证状态实际发生设置、替换或清除时增加 `authentication_version` 与 `config_version`；对已经关闭认证状态的重复清除是 no-op；旧 PublishedSnapshot 继续持有旧认证材料，新请求使用新 Client 代际。
- 认证关闭时用户名与密码都不存在；认证开启时二者必须成对存在。用户名不能为空、不能包含控制字符或 HTTP Basic 分隔符 `:`，且最长 255 字节；密码长度为 `1..=255` 字节，以同时满足 HTTP Basic 与 SOCKS5 RFC 1929 边界。
- Transport API 使用脱敏 `ProxyCredentials` sidecar，不把密码放进派生 `Debug` 的 `ProxyProfile`、`TransportRequest.headers`、代理 URL 或缓存 key。reqwest Proxy 在 Client 构建时调用 `basic_auth`，同时覆盖 HTTP forward proxy、HTTPS CONNECT 和 SOCKS5 用户名密码认证；认证材料只在受控 Transport 边界编码为代理认证头或握手材料。
- 普通代理创建/元数据更新继续只处理地址与启用状态。认证使用专用设置/替换端点和清除端点；普通响应只包含用户名、是否已配置密码和认证版本，专用写入请求才在受控边界接收密码。
- 管理探测只接受当前 PublishedSnapshot 中的 `ProxyProfileId` 与 `ProviderEndpointId`。探测发送无 ProviderCredential 的空 GET；普通上游 HTTP 响应头（包括 401、403、404）即视为链路可达，但 HTTP forward proxy 的 `407 Proxy Authentication Required` 是代理认证握手拒绝，必须返回脱敏的 `ProxyHandshake + Proxy` 失败；Body 立即丢弃。
- 探测结果返回 Proxy/Endpoint ID、捕获的配置 revision 与两项资源 config version、延迟、状态码或 `TransportErrorStage`/`TransportFailureScope`。不返回目标 IP、代理地址、响应正文或底层错误字符串；Web 只展示与当前配置代际完全匹配的结果。
- 管理探测绕过 Credential 调度和健康结算，不占用生成/辅助并发，不更新熔断、冷却或 RequestLog。
- 严格 SSRF 本地 DNS 保留为下一独立 ADR。本决策完成时 HTTP/SOCKS5 仍使用受信远端 DNS 边界；后续 ADR-0019 已实现可热更新的本地解析与固定目标模式。

## 后果

- 代理认证轮换会自然切换连接池代际，旧请求不会被中途修改，新请求不会复用旧密码 Client。
- 管理员可以在不发送 Provider API Key 的情况下验证代理连接与认证；上游返回 401、403 或 404 仍可证明代理链路可达，代理本身返回 407 则表示代理认证失败。
- 测试 API 不能临时提交任意 URL，只能复用已由管理员保存并通过结构化校验的 ProviderEndpoint；该 Endpoint 可以按 ADR-0029 指向公网或内网目标。
- 本切片自身不解决代理远端 DNS 的目标 IP 证明问题；该边界后来由 ADR-0019 的自定义 HTTP CONNECT/SOCKS 固定目标连接策略补齐。

## 验证

- Domain/Storage 测试覆盖认证版本、用户名/密码校验、加密往返、认证仍存在时的重启加载、设置/替换/清除、重复清除 no-op 和 Debug 脱敏；代理专用错误 ID/version AAD 的负向测试留待 Vault 契约继续收紧。
- Transport 测试覆盖 HTTP Basic、HTTPS CONNECT Basic、SOCKS5 RFC 1929、HTTP forward/CONNECT 407 代理认证拒绝、错误密码 Fail-Closed 与认证轮换 Client 代际。
- HTTP 契约测试覆盖 DTO 不回显密码、认证写入/清除和受限 ProviderEndpoint 探测。
- Web 测试覆盖局部密码状态、保存认证、默认测试目标和行内结果展示；清除认证与切换目标的细粒度行为测试留在后续 UI 收紧切片。
