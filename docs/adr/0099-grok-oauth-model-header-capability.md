# ADR-0099：Grok OAuth 模型 Header 的 UTF-8 字节语义

- 状态：Accepted
- 日期：2026-08-03
- 决策者：maintainer
- 修订：ADR-0040

## 背景

Grok OAuth 数据面通过 `x-grok-model-override` 携带最终上游模型。Claude 审查报告认为
`HeaderValue::from_str` 会拒绝非 ASCII，因此通用 `UpstreamModelName` 允许 Unicode 会让请求在本地
失败，并且原实现还把组装失败错误标成来自上游的 `InvalidResponse`。

复现结果否定了报告的核心前提。本项目锁定的 `http 1.4.2` 会按 UTF-8 取得 Rust `str` 的原始字节，
校验实现接受 32–126 以及 128–255，只拒绝控制字节和 DEL；`"本地/Grok"` 可以成功构造 Header，字节
与 `str::as_bytes()` 完全一致。xAI 官方 Grok Build 当前锁定 `http 1.4.0`，同样把模型字符串直接交给
Reqwest 的 Header API，因而走相同的原始 UTF-8 字节路径。报告描述的问题在当前依赖和官方客户端上
不可复现。

仍需修正原实现的意图和错误语义：不能依赖 `from_str` 文档中与实际 1.4.x 校验实现不一致的 ASCII
表述，也不能把本地请求 Header 组装错误归为上游响应错误。

## 决策

1. 保持 `PublicModelName` 与 `UpstreamModelName` 的 Unicode 领域契约不变，不增加 Grok OAuth 专属的
   ASCII 配置限制。
2. `x-grok-model-override` 使用 `HeaderValue::from_bytes(model.as_bytes())`，明确规定 Rust 模型字符串按
   UTF-8 原始字节写入。不得再做 percent、base64 或其他二次编码。
3. HTTP 字段值中的高位字节按 RFC 9110 的 `obs-text` 传输。这里的 UTF-8 语义来自模型字符串与 xAI
   官方客户端的同构实现；any2api 不尝试把任意不透明字节反解为模型名。
4. 领域层已拒绝空白边界、控制字符和 DEL，因此正常发布的 `UpstreamModelName` 都能通过 Header
   校验。不得为强类型已经排除的状态增加重复的配置发布校验或 Provider 能力层。
5. `ProviderDriver` 的请求 Header 接口仍接收原始 `&str`，供内部适配和契约测试使用；若调用方绕过
   领域类型传入控制字节，Grok 返回 `UnsupportedOAuthModel`，不得使用响应侧的 `InvalidResponse`。
6. Grok API Key 路由仍不发送 `x-grok-model-override`，模型继续由 JSON Body 表达。

## 依据

- [RFC 9110 §5.5](https://www.rfc-editor.org/rfc/rfc9110.html#section-5.5) 的 `field-vchar`
  包含 `obs-text`；接收方应把这类字节视作不透明数据。
- [xAI 官方 Grok Build sampler](https://github.com/xai-org/grok-build/blob/main/crates/codegen/xai-grok-sampler/src/client.rs)
  直接把模型字符串设置为 `x-grok-model-override`，没有额外编码层；其当前 lockfile 使用
  `http 1.4.0`。
- [`http 1.4.2` 的 HeaderValue 实现](https://github.com/hyperium/http/blob/v1.4.2/src/header/value.rs)
  对 `from_str` 与 `from_bytes` 使用同一逐字节校验，并允许 128–255；`from_bytes` 使本项目选择的字节
  语义显式可见。

## 后果

- Grok OAuth 可以完整保留非 ASCII 上游模型名，不会引入与其他路由不同的模型命名规则。
- 线协议字节由明确的 UTF-8 转换决定，不依赖 percent/base64 的供应商外约定。
- 非法控制字节即使绕过领域层，也会得到请求侧模型错误，不再伪装成无效上游响应。

## 验证

- Provider 单测验证 Grok OAuth 的 Unicode 模型 Header 与 `str::as_bytes()` 完全一致，控制字节返回
  `UnsupportedOAuthModel`，同一 Unicode 模型在 API Key 模式下不生成该 Header。
- Registry 契约从实际注册的 Grok Driver 验证 Unicode Header 字节，防止未来恢复 ASCII 假设。
