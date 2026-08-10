# ADR-0004: 单节点代理 Transport 与连接池代际

- 状态：Superseded by ADR-0123
- 日期：2026-07-18
- 决策者：maintainer

> 2026-08-10：ADR-0123 取代了本文“多个 Credential 可按相同代理策略共享连接池”的身份边界。本文其余 fail-closed、超时、代理和错误归因决策继续有效。

## 背景

any2api 需要在同一进程中支持 DIRECT、HTTP 和 SOCKS5 出口。多个 Credential 可以共享同一代理连接池，代理配置热更新后已开始请求仍要继续使用其捕获的 Client，同时退役配置代际不能永久占用内存。专属代理失败必须 fail-closed，不能回退全局代理或本机直连。

## 决策

- 首版 Transport 使用 `reqwest` + Rustls，并只通过 `transport::api` 暴露 any2api 自有请求、响应和错误类型。Rustls 使用宿主系统证书根，避免把另一套固定根证书打包进二进制，也允许自托管环境使用其显式安装的企业根证书。
- Client Builder 禁用系统代理、Cookie Store、自动重定向和 `reqwest` 内建协议重试；Provider 认证头只允许逐请求注入，不进入 Client 默认 Header。所有重试必须由 Runtime 的 Attempt、RetrySafety 和预算状态机决定。
- DIRECT 明确调用 `no_proxy()`；HTTP 使用结构化 `http://host:port` 代理 URL；SOCKS5 使用 `socks5h://host:port`，默认由远端代理解析目标域名。
- Runtime 在调用 Transport 前完成 `Credential DIRECT -> global proxy -> local DIRECT` 解析。Transport 只执行传入的实际 `ProxyProfile`，没有代理回退分支。
- Client 缓存键包含 `ProxyProfileId + config_version + ProxyKind`，以及连接超时、TLS 策略版本、HTTP 版本策略、池空闲超时、每目标空闲连接上限和池策略版本。相同完整策略代际共享连接池；代理或网络策略变化后使用新 key，已开始请求继续持有其捕获 Client 的 `Arc`。
- 缓存使用有界强引用 LRU。淘汰只移除 Manager 的缓存引用，不中断仍持有 Client 的请求。
- 每次 `TransportManager::execute` 开始时把 Client 代际中的 `connect_timeout` 转换为一个绝对 deadline。手工 DNS、Client 获取或构造、TCP、HTTP CONNECT/SOCKS5 握手与 TLS 共享该 deadline，不允许各阶段重新获得完整 timeout。同步 Client 构造不可被异步 timer 中断，但构造返回后必须先检查同一 deadline，过期时不得启动 I/O。直到请求 Body 首次被连接层消费前都属于连接阶段；边界到达后停止连接 deadline，再启动请求级 `read_timeout` 等待响应头。
- 请求 Body 为内存中的 `Bytes`，响应 Body 为错误类型化的异步字节流，用于 JSON 和 SSE 转发。
- 连接建立前失败标记为 `DefinitelyNotSent`；收到响应头前的非连接错误和响应 Body 错误保守标记为 `Ambiguous`。
- `reqwest` 不能稳定暴露 DNS、TCP、代理握手和上游 TLS 的全部细分来源，因此失败阶段与健康归因分离。`TransportError` 同时携带 `TransportErrorStage` 与 `TransportFailureScope::{Endpoint, Proxy, Unattributed}`；DIRECT DNS/TCP 明确归 Endpoint，普通 HTTP 代理的可验证连接失败归 Proxy，明确识别的 HTTP forward/CONNECT 407 代理认证拒绝归 `ProxyHandshake + Proxy`，其余 CONNECT/SOCKS/目标 TLS 无法可靠区分时仍归 Unattributed。CONNECT 407 的识别依赖当前锁定的 reqwest/hyper-util 错误来源文本，并由真实回归测试钉住；Runtime 只惩罚明确归因的健康对象，Unattributed 对 Endpoint/Proxy 均保持 neutral。

## 后果

- 单节点场景不需要为每个 Credential 创建独立 Client，连接池数量由代理配置代际而不是 API Key 数量决定。
- 系统环境变量中的代理不会改变 DIRECT 语义，HTTP/SOCKS5 失败也不会静默绕过指定出口。
- 系统证书库中的受信任根会影响上游 TLS 信任边界；这是自托管部署的显式宿主策略，不由 Provider 或请求动态修改。
- 一次 `TransportManager::execute` 最多发送一次网络请求；任何再次尝试都必须返回 Runtime 并创建可观测的 Attempt。
- SOCKS5h 是显式信任远端 DNS 的边界；ADR-0019 定义可热更新的严格 SSRF 模式，在该模式下使用本地解析与固定目标连接。
- 代理认证已由 ADR-0018 与 ADR-0074 固定为 SQLite 明文持久化、脱敏 sidecar 与逐 Client 配置；密码不写入日志、普通读取 DTO、`TransportRequest.headers`、代理 URL 或连接池 key，仅在受控 Client/握手边界编码为代理认证材料。
- TransportManager 是独立可测试模块，并由 Runtime 装配进入 Model Route、GatewayApiKey 鉴权后的公开协议请求链路。

## 验证

- 模块网络测试覆盖 DIRECT、HTTP absolute-form、HTTPS 经 HTTP CONNECT 完成 TLS 隧道、SOCKS5h 远端 DNS、禁重定向、流式响应和 Client 代际缓存，并验证 CONNECT 后 Endpoint TLS 失败不会误归因到 Proxy。
- 超时测试使用停滞的 TLS、HTTP CONNECT 与 SOCKS5 握手，验证普通 reqwest 与严格固定目标连接器都在同一个绝对 `connect_timeout` 内结束；请求 Body 首次消费后的响应头停滞则只由 `read_timeout` 结束。
- fail-closed 测试使用可直连本地目标与不可用显式代理，确认目标端口没有收到连接。
- 契约测试只通过 `transport::api` 重复验证显式代理失败绝不回退 DIRECT。
