# ADR-0123：按用途与模式隔离 Transport/TLS 状态

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer
- 相关决策：ADR-0149

## 背景

Transport 同时承载数据面、OAuth 控制面和诊断请求。认证材料逐请求注入，但 Token、额度
与诊断的连接生命周期不能因推理数据面共用连接池而互相泄漏。另一方面，均衡模式需要保持
跨 Credential 的数据面线路连续性，以维持上游 prompt cache；粘性模式则需要遵守已绑定
会话的账号隔离。

## 决策

1. 每个 `TransportRequest` 都携带强类型 `TransportIsolationKey`。它至少区分
   `RoutingCredentialId`、路由代际、认证代际和 `TransportTrafficClass`，禁止由进程级
   默认值隐式推导。临时 OAuth 登录交换和无凭据代理诊断使用不持久化的临时 key。
2. `TransportTrafficClass` 固定为 `DataPlane`、`OAuthToken`、`OAuthQuota` 和 `Diagnostic`。
   OAuth Token/Quota/Diagnostic 按账号与认证代际隔离 Client、连接池、TLS session store 和
   HTTP/2 stream namespace；同一账号同一代际同一用途可以复用。
3. `DataPlane` 的隔离由快照中的 `affinity.enabled` 决定：均衡模式使用
   `TransportIsolationKey::shared_data_plane()`，相同代理与线路 profile 的推理请求共享
   Client/连接池；粘性模式按已绑定 Credential、路由代际和认证代际隔离。模式切换只影响
   新开始的请求，已捕获快照的请求继续使用自己的 key。
4. Client cache key 必须包含完整 isolation key、代理/网络 generation 和当前 generic
   wire profile 版本。每个新 Client 独立创建 TLS config，禁止通过 clone 共享 session ticket
   store。旧快照或旧代际持有的 Client 可以完成已开始请求，但不得命中新代际缓存。
5. 禁用或删除 Credential 后不再产生新的 DataPlane Attempt；禁用 OAuthAccount 仍可按
   OAuthToken/Quota 规则执行 Token 保活和额度操作。进程重启清空所有 Client、连接、TLS
   ticket、队列和其他运行态状态。

## 后果

- 控制面仍具有明确的账号代际隔离；数据面均衡模式不会因隐藏的 per-Credential 连接池
  拆分而损失缓存连续性。
- 粘性模式的账号隔离和会话语义保持不变；共享数据面 Client 不改变认证逐请求注入或
  Gateway Key 剥离。
- Transport 只比较 opaque key，不增加 Provider 分支，也不接触 Secret。

## 验证

- Transport 测试覆盖四种 traffic class、代理/代际变化、TLS session store 独立性和
  generic profile version。
- Runtime/HTTP 测试覆盖均衡模式共享 DataPlane、粘性模式隔离 DataPlane，以及 OAuth
  Token/Quota/Diagnostic 始终按账号代际隔离。
- 生命周期测试覆盖旧快照完成请求、删除/禁用后的新请求拒绝和重启清空运行态。
