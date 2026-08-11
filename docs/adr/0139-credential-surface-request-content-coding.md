# ADR-0139：请求 Content-Encoding 按上游认证面收敛

- 状态：Accepted
- 日期：2026-08-12
- 决策者：maintainer
- 修订：ADR-0127 与 ADR-0135 未覆盖的请求 Body 压缩边界

## 背景

公开 Codex/OpenAI JSON 入口允许客户端使用 `Content-Encoding: zstd`。Server 会先解压，再由
ProtocolAdapter 校验和规范化 JSON。旧实现把入口编码连同请求一起保留到 Runtime，并仅按“Provider 是
Codex、入口与上游同方言”判断是否把最终 Body 重新压缩为 zstd。

这个判断错误地把 Provider 类型当成了 Endpoint 能力。Codex OAuthAccount 的数据面固定指向已审计的
官方 Codex backend，而 Codex `ProviderCredential` 可以指向任意管理员配置的 OpenAI 兼容 API Key
Endpoint。兼容 Endpoint 通常只承诺 JSON 协议，不承诺接受 zstd 请求 Body。实际故障中，自定义 Endpoint
收到带 `Content-Encoding: zstd` 的压缩字节后直接按 JSON 解析，返回 `invalid JSON request body`；相同
请求发送到官方 Codex OAuth 数据面则成功。

HTTP `Content-Encoding` 描述单个 hop 的表示。客户端选择压缩 client → any2api，并不等于授权或证明
any2api → upstream 也应使用同一编码。上游请求压缩必须属于选中的具体认证面，而不是 Provider kind、
协议方言或入口 Header 的隐式属性。

## 决策

1. Server 继续只在已支持的 JSON 入口接受单层 zstd，并同时执行压缩前与解压后大小限制。未知、重复、
   损坏或超限编码继续按当前协议错误拒绝。
2. ProtocolAdapter 与 Provider Driver 始终接收解压后的规范 JSON。入口 `RequestBodyEncoding` 只作为当前
   Attempt 可参考的候选编码，不得直接透传 `Content-Encoding` 或原压缩字节。
3. `ProviderDriver::supports_request_body_encoding` 必须根据当前选中的认证面作出明确声明。当前 Codex
   只有 OAuthAccount 固定官方数据面支持对同方言 Responses 请求重新压缩为 zstd。
4. Codex API Key `ProviderCredential` 无论指向官方 API URL 还是自定义兼容 Endpoint，都发送 identity
   JSON 并删除入口 `Content-Encoding`。不通过 URL 猜测能力，也不新增管理员可配置的压缩开关。
5. 跨协议请求始终发送 identity JSON。其他 Provider 在没有独立、已验证契约前继续不声明请求 Body
   压缩能力。
6. Runtime 仍只在 Driver 明确返回支持时压缩最终重编码 Body，并重新建立匹配的
   `Content-Encoding: zstd` 与长度语义；否则删除该 Header。原始入口压缩 bytes 永不透传。

## 后果

- 自定义 OpenAI 兼容 Endpoint 不再收到其未声明支持的 zstd Body，恢复普通 JSON 兼容性。
- 官方 Codex OAuth 数据面继续保留已验证的同方言 zstd 行为。
- Codex API Key Endpoint 即使碰巧支持 zstd，也会多使用少量上行带宽；这是避免对任意管理员 Endpoint
  作错误能力推断的确定性代价。
- 新增请求压缩能力时，必须在具体认证面提供证据并更新 Driver 契约，不能只扩张中央 Runtime 分支。

## 验证

- Provider 单元测试断言：同方言 Responses + zstd 在 Codex OAuth context 下为支持，在 Codex API Key
  context 下为不支持。
- 同一测试覆盖 identity 编码、跨方言与 OAuth Chat context 均不触发 zstd 重压缩。
- Runtime 既有测试继续证明 zstd 编码器产生可解码的完整帧；最终是否调用编码器只由 Driver 能力决定。
