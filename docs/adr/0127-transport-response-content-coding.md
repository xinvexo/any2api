# ADR-0127: Transport 统一拥有响应 Content-Encoding

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer

## 背景

基线实现删除成功响应的 `Content-Encoding`，但没有解压 Body。普通 Reqwest 与严格代理下的 pinned Hyper 又是两条 Client 路径；只打开 Reqwest 自动解压 feature 会留下行为分叉。上游即使未收到 `Accept-Encoding` 也可能返回压缩 JSON/SSE，届时 Protocol 会把压缩字节误判为无效 JSON 或 SSE。

ADR-0061 要求最终非成功响应透明返回上游状态与原始正文，旧决定还保留匹配的 `Content-Encoding`。但 Transport 一旦主动协商压缩，下游客户端并未参与这次协商；把压缩表示直接转交下游既不安全，也会让 Provider 错误分类读取压缩字节。透明边界因此应定义为“解码后的原始内容字节不重序列化”，而不是保留 hop 间协商得到的压缩表示。

## 决策

1. ADR-0126 的通用线路 profile 升级为 `generic-rustls-hyper-v2`。Transport 在 Provider/Protocol Header 合并完成后覆盖写入 `Accept-Encoding: gzip, br, zstd`；客户端不得选择或扩张该集合。
2. Transport 对所有状态拥有 response content coding。普通 Reqwest 与 pinned Hyper 都在各自网络 Body 和 read-timeout 包装完成后调用同一个增量解码边界，再把 Body 返回 Runtime。
3. 支持 `gzip`、`br`、`zstd` 与无操作的 `identity`，按 HTTP 声明顺序的逆序解码。编码名称大小写不敏感；允许多个 Header 行与逗号列表，但编码链最多四层。空 token、未知 token 或更深链立即失败。
4. 解码路径在把 Body 暴露给 Protocol 或 Provider 错误分类前删除 `Content-Encoding`、`Content-Length`、`Content-Range`、`ETag`、`Digest` 与 `Content-MD5`。未知或损坏编码绝不能只删 Header 后继续传递原始压缩字节。
5. 编码 Header 无效或解压流损坏使用 `TransportErrorStage::ReadBody`、`TransportFailureScope::Endpoint` 和 `RetrySafety::Ambiguous`。底层网络 Body 原有错误仍保留其阶段与 failure scope，不得被解码适配器错误归因为 Endpoint codec 故障。
6. 非成功状态在解码后继续由 Runtime 执行 ADR-0061：原始 HTTP 状态、允许 Header 和解码后的原始内容字节透明返回，不做 JSON 重序列化。该决定局部取代 ADR-0061 对压缩表示与 `Content-Encoding` 的保留。
7. Buffered 与错误正文上限都作用于解压后的字节；SSE 以有界输出 chunk 增量解码后进入现有 frame parser。不得为压缩响应整包缓冲，也不增加第二套 Provider-specific codec 分支。
8. `Accept-Encoding` 顺序和 codec 集合是可观测的 generic gateway wire contract。禁止随机化，也不声称它等于任一官方客户端。

## 后果

- 意外或协商得到的压缩 JSON/SSE 不再进入 Protocol 或 Provider 错误分类形成抽象解析失败。
- 两种 Client 路径、所有 Provider 与 OAuth data/quota/token traffic class 使用同一编码能力。
- 非成功响应继续保持状态与内容透明性，但不再把 hop 间压缩表示转交给下游。
- `generic-rustls-hyper-v1` 被 v2 取代；后续编码集合或顺序变化必须再次提升 profile version。

## 验证

- 单元测试以任意小 chunk 输入真实 gzip、Brotli 和 Zstandard 流，断言增量还原、Header 同步清理、损坏/未知编码分类以及底层 TransportError 保真。
- loopback Client 测试断言线路实际发送固定 `Accept-Encoding`，普通与 pinned 路径都能读取压缩响应。
- 透明上游错误契约断言非成功响应保留状态和解码后的原始内容字节，不携带失配的 Content-Encoding。
