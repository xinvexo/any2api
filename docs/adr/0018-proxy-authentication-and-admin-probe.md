# ADR-0018: 代理认证与管理探测边界

- 状态：Accepted
- 日期：2026-07-21
- 修订：2026-07-30
- 决策者：maintainer

## 背景

HTTP 与 SOCKS5 代理需要支持用户名密码认证，并在管理 Web 中提供受限的连通性测试。密码不能进入普通领域对象的可见字段、普通读取/响应 DTO、日志或连接池 key；管理探测也不能变成可提交任意 URL 的 SSRF 工具。

## 决策

- `ProxyProfile` 只保存可见的认证元数据与单调 `authentication_version`；密码保存在独立 `proxy_passwords` 记录中，使用现有 XChaCha20-Poly1305 Secret Vault 加密。
- 代理密码 AAD 绑定 `ProxyProfileId + authentication_version`。认证状态实际发生设置、替换或清除时增加 `authentication_version` 与 `config_version`；对已经关闭认证状态的重复清除是 no-op；已开始请求的 PublishedSnapshot 继续持有其捕获的认证材料，新请求使用当前 Client 代际。
- 认证关闭时用户名与密码都不存在；认证开启时二者必须成对存在。用户名不能为空、不能包含控制字符或 HTTP Basic 分隔符 `:`，且最长 255 字节；密码长度为 `1..=255` 字节，以同时满足 HTTP Basic 与 SOCKS5 RFC 1929 边界。
- Transport API 使用脱敏 `ProxyCredentials` sidecar，不把密码放进派生 `Debug` 的 `ProxyProfile`、`TransportRequest.headers`、代理 URL 或缓存 key。reqwest Proxy 在 Client 构建时调用 `basic_auth`，同时覆盖 HTTP forward proxy、HTTPS CONNECT 和 SOCKS5 用户名密码认证；认证材料只在受控 Transport 边界编码为代理认证头或握手材料。
- 普通代理创建/元数据更新继续只处理地址与启用状态。认证使用专用设置/替换端点和清除端点；普通响应只包含用户名、是否已配置密码和认证版本，专用写入请求才在受控边界接收密码。
- 管理探测只接受路径中的 `ProxyProfileId`，不接受 `ProviderEndpointId` 或客户端 URL。Runtime 在单一命名常量中固定中立 HTTPS 目标 `https://example.com/`，以 10 秒响应头等待上限发送无 ProviderCredential 的空 GET。普通目标 HTTP 响应头（包括非 2xx）即视为链路可达，但 HTTP forward proxy 的 `407 Proxy Authentication Required` 是代理认证握手拒绝，必须返回脱敏的 `ProxyHandshake + Proxy` 失败；Body 立即丢弃。
- 探测结果只返回 Proxy ID、捕获的配置 revision 与 Proxy config version、延迟、状态码或 `TransportErrorStage`/`TransportFailureScope`。Transport 的 Endpoint 归因在管理探测契约中命名为 `probe_target`，避免与 Provider Endpoint 混淆。响应不返回目标 IP、代理地址、响应正文或底层错误字符串；Web 只展示与当前 Proxy 配置代际完全匹配的结果。
- 管理探测绕过 Credential 调度、RPM 预留和健康结算，不更新熔断、冷却或 RequestLog。
- 严格 SSRF 模式下的本地 DNS 与固定目标连接遵循 ADR-0019；关闭严格模式时 HTTP/SOCKS5 的远端 DNS 是显式配置的代理信任边界。

## 后果

- 代理认证轮换会自然切换连接池代际，已开始请求不会被中途修改，新请求不会复用轮换前密码的 Client。
- 管理员可以在不发送 Provider API Key、不创建 Provider Endpoint 的情况下验证代理连接与认证；目标返回任意普通 HTTP 响应头仍可证明代理链路可达，代理本身返回 407 则表示代理认证失败。
- 测试 API 没有客户端目标参数，因此无法被用作任意 URL 的 SSRF 转发器。固定站点只表示通用公网连通性，不能替代 Provider Credential 的真实 Endpoint 测试。
- 严格 SSRF 的 HTTP CONNECT/SOCKS 固定目标连接与认证材料使用同一实际代理路径，不增加本机直连回退。

## 验证

- Domain/Storage 测试覆盖认证版本、用户名/密码校验、加密往返、认证仍存在时的重启加载、设置/替换/清除、重复清除 no-op、AAD 负向边界和 Debug 脱敏。
- Transport 测试覆盖 HTTP Basic、HTTPS CONNECT Basic、SOCKS5 RFC 1929、HTTP forward/CONNECT 407 代理认证拒绝、错误密码 Fail-Closed 与认证轮换 Client 代际。
- HTTP 契约测试覆盖 DTO 不回显密码、认证写入/清除、无请求体探测和固定目标不受客户端输入影响。
- Web 测试覆盖局部密码状态、保存/清除认证、页面不请求 Provider Endpoint、双胶囊结果以及未测试/测试中/失败的稳定布局。
