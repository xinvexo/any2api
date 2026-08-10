# ADR-0123: Credential 代际与流量用途隔离 Transport/TLS 状态

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer
- 取代：ADR-0004 中“相同代理策略的多个 Credential 共享连接池”的部分

## 背景

ADR-0004 原本把连接池身份限制在 ProxyProfile 与 Transport policy。Provider 认证虽然安全地逐请求注入，但 `RoutingCredentialId`、认证代际和请求用途都不进入 Client cache key。结果是同一 Provider origin 和代理下，不同 API Key/OAuthAccount 可以命中同一个 reqwest Client，并在同一 TCP/TLS/HTTP2 connection 上切换 Authorization。共享的 Rustls `ClientConfig` clone 还会让不同缓存 Client 复用同一个 TLS session ticket store。

本地 loopback TLS/H2 实验证明了两件事：两个不同 Authorization 实际只产生一次 TCP accept；两个不同缓存 Client 的第二次 TLS 握手仍可显示为 resumed。这不仅是上游可观测差异，也让账号、连接故障、GOAWAY/flow-control 与认证生命周期边界不一致。

## 决策

### 1. 强类型 TransportIsolationKey

每个 `TransportRequest` 必须显式携带 `TransportIsolationKey`，且没有进程级共享默认值。持久化路由凭据的 key 由以下字段组成：

```text
RoutingCredentialId
+ routing_generation
+ authentication_version
+ TransportTrafficClass
```

`RoutingCredentialId` 保留 ProviderCredential/OAuthAccount source tag，因此 UUID 偶然相同也不能碰撞。`routing_generation` 隔离停用后重新启用等路由身份变化；`authentication_version` 隔离 API Key 轮换与 OAuth Token refresh。

尚未产生 OAuthAccount 的登录交换和不绑定 Credential 的代理诊断使用进程内唯一的临时 key。临时 key 不持久化，不包含 OAuth code、device code、Token、代理密码或客户端 Session ID。

### 2. 流量用途是隔离身份的一部分

固定流量类别为：

- `DataPlane`：公开推理、图片和消息请求；
- `OAuthToken`：OAuth authorization/device/refresh token 交换；
- `OAuthQuota`：额度读取、补充查询和额度 reset；
- `Diagnostic`：ProviderCredential 与 Proxy 管理测试。

不同类别即使 account、origin 和 proxy 完全相同，也不得共享 Client、连接池、TLS connection 或 H2 stream namespace。Quota 同一次操作的主查询、补充查询和 reset 可以在同一账号/代际/类别内复用。

### 3. Client 与 TLS resumption 同时隔离

`TransportClientKey` 必须包含完整 `TransportIsolationKey`。只有 isolation、Proxy/config generation、网络选择和全部 Transport policy 都相同，才允许命中同一个 Client。

宿主 trust roots 继续只加载一次并作为只读数据共享，但每个新 `TransportClient` 必须由 roots 新建独立 Rustls `ClientConfig`。禁止 clone 一个进程级 `ClientConfig` 给不同 Client，因为 clone 会共享 session store。这样新的 isolation key 既不能复用旧 HTTP pool，也不能恢复旧 TLS session。

### 4. 生命周期

配置快照和 Attempt 继续捕获不可变的 credential generation。Secret/Token 轮换或重新启用会生成新的 key；Manager 首次收到同一 Credential 的更高路由/认证代际时，移除该 Credential 较旧代际的缓存引用。旧快照、已经开始的请求和流式 Body 可以继续持有旧 Client，不强行中断；迟到的旧快照也绝不能命中新代际 Client。删除/禁用后若没有新请求触发代际清理，旧 Client 只会继续受有界 LRU 与 idle timeout 管理。进程重启清空全部 key、pool 和 ticket 状态。

禁用或删除 Credential 后当前 PublishedSnapshot 不再产生新的 data-plane Attempt。OAuthAccount 的 `enabled=false` 仍按既有架构参与 Token refresh，因此它可以继续使用自己的 `OAuthToken` 隔离域，但不能借此重新进入 `DataPlane`。

## 后果

- Client 数量从“Proxy/config policy 数量级”上升到“活跃 Credential generation × traffic class × policy 数量级”，但仍受现有有界 LRU 限制；这是强账号隔离的明确成本。
- 同一 Credential/认证代际/traffic class 仍可获得 H1 keep-alive、H2 multiplexing 与 TLS resumption 性能。
- Account A/B 不再共享 TCP/TLS/H2 connection；data/token/quota/diagnostic 也不再隐式共池。
- Transport 只比较 opaque isolation value，不增加 Codex/Claude/Grok 分支，也不接触认证 Secret。
- 只改 Client cache key 不够；TLS config factory 必须同时避免跨 Client session-store clone。

## 验证

- loopback TLS/H2 测试用两个 RoutingCredentialId 请求同一 origin/proxy，断言 Client `Arc` 不同且 TCP accept 为 2；
- 相同 RoutingCredentialId、routing/auth generation 和 traffic class 的两个请求仍断言 Client `Arc` 相同且 TCP accept 为 1；
- 更高 routing/auth generation 第一次取得 Client 后，旧代际缓存引用被移除，但旧请求持有的 `Arc` 仍有效；
- 相同账号但 `DataPlane`/`OAuthQuota` 不同，断言 Client 与物理连接分离；
- 两个不同 isolation key 强制建立两条 TLS 连接，断言两次握手均为 full，而不是 `Full -> Resumed`；
- Runtime 测试断言 data、token、quota 与 diagnostic 调用点选择正确 traffic class，Token version 变化会改变 isolation key；
- 现有 DIRECT/HTTP/SOCKS5、严格 SSRF、代理认证、timeout、fail-closed 和流式 Body 测试必须继续通过。
