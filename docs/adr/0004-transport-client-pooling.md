# ADR-0004: 单节点代理 Transport 与连接池代际

- 状态：Accepted
- 日期：2026-07-18
- 决策者：maintainer

## 背景

any2api 需要在同一进程中支持 DIRECT、HTTP 和 SOCKS5 出口。多个 Credential 可以共享同一代理连接池，代理配置热更新后旧请求仍要继续使用旧 Client，同时历史配置不能永久占用内存。专属代理失败必须 fail-closed，不能回退全局代理或本机直连。

## 决策

- 首版 Transport 使用 `reqwest` + Rustls，并只通过 `transport::api` 暴露 any2api 自有请求、响应和错误类型。Rustls 使用宿主系统证书根，避免把另一套固定根证书打包进二进制，也允许自托管环境使用其显式安装的企业根证书。
- Client Builder 禁用系统代理、Cookie Store、自动重定向和 `reqwest` 内建协议重试；Provider 认证头只允许逐请求注入，不进入 Client 默认 Header。所有重试必须由 Runtime 的 Attempt、RetrySafety 和预算状态机决定。
- DIRECT 明确调用 `no_proxy()`；HTTP 使用结构化 `http://host:port` 代理 URL；SOCKS5 使用 `socks5h://host:port`，默认由远端代理解析目标域名。
- Runtime 在调用 Transport 前完成 `Credential DIRECT -> global proxy -> local DIRECT` 解析。Transport 只执行传入的实际 `ProxyProfile`，没有代理回退分支。
- Client 缓存键包含 `ProxyProfileId + config_version + ProxyKind`，以及连接超时、TLS 策略版本、HTTP 版本策略、池空闲超时、每目标空闲连接上限和池策略版本。相同完整策略代际共享连接池；代理或网络策略变化后使用新 key，旧请求继续持有旧 Client 的 `Arc`。
- 缓存使用有界强引用 LRU。淘汰只移除 Manager 的缓存引用，不中断仍持有 Client 的请求。
- 首版请求 Body 为内存中的 `Bytes`，响应 Body 为错误类型化的异步字节流，满足 JSON 和后续 SSE 转发需要。
- 连接建立前失败标记为 `DefinitelyNotSent`；收到响应头前的非连接错误和响应 Body 错误保守标记为 `Ambiguous`。
- `reqwest` 不能稳定暴露 DNS、TCP、代理握手和上游 TLS 的全部细分来源，因此失败阶段与健康归因分离。`TransportError` 同时携带 `TransportErrorStage` 与 `TransportFailureScope::{Endpoint, Proxy, Unattributed}`；DIRECT DNS/TCP 明确归 Endpoint，普通 HTTP 代理的可验证连接失败归 Proxy，CONNECT/SOCKS/目标 TLS 无法可靠区分时归 Unattributed。Runtime 只惩罚明确归因的健康对象，Unattributed 对 Endpoint/Proxy 均保持 neutral。

## 后果

- 单节点场景不需要为每个 Credential 创建独立 Client，连接池数量由代理配置代际而不是 API Key 数量决定。
- 系统环境变量中的代理不会改变 DIRECT 语义，HTTP/SOCKS5 失败也不会静默绕过指定出口。
- 系统证书库中的受信任根会影响上游 TLS 信任边界；这是自托管部署的显式宿主策略，不由 Provider 或请求动态修改。
- 一次 `TransportManager::execute` 最多发送一次网络请求；任何再次尝试都必须返回 Runtime 并创建可观测的 Attempt。
- SOCKS5h 是显式信任远端 DNS 的边界；未来严格 SSRF 模式必须禁止该模式并引入本地解析与固定目标连接。
- 当前代理尚无用户名和密码字段；后续增加代理认证时必须通过 Secret Vault 和逐 Client 代理配置完成，不能写入日志或普通 DTO。
- TransportManager 已可独立执行和测试，但在 Model Route、GatewayApiKey 和公开协议 Handler 完成前尚未进入客户端请求链路。

## 验证

- 模块网络测试覆盖 DIRECT、HTTP absolute-form、HTTPS 经 HTTP CONNECT 完成 TLS 隧道、SOCKS5h 远端 DNS、禁重定向、流式响应和 Client 代际缓存，并验证 CONNECT 后 Endpoint TLS 失败不会误归因到 Proxy。
- fail-closed 测试使用可直连本地目标与不可用显式代理，确认目标端口没有收到连接。
- 契约测试只通过 `transport::api` 重复验证显式代理失败绝不回退 DIRECT。
