# ADR-0127：Transport 统一拥有响应 Content-Encoding

- 状态：Accepted
- 日期：2026-08-10
- 决策者：maintainer
- 相关决策：ADR-0126、ADR-0135

## 背景

普通 Reqwest 与严格代理下的 pinned Hyper 是两条 Client 路径。若压缩响应没有在共同边界
解码，Protocol 和 Provider 错误分类会把压缩字节误判为无效 JSON/SSE；只删除 Header 又会
把正文语义破坏。响应表示必须由 Transport 在所有状态、所有 Client 路径统一拥有。

## 决策

1. `generic-rustls-hyper-v3` 声明当前可增量解码的响应编码。Transport 不生成固定
   `Accept-Encoding`；只有同方言请求中客户端提供、且全部 coding 可解码的值才由 ADR-0135
   原样透传。即使请求没有声明协商，上游意外返回受支持编码时仍执行相同解码。
2. 普通 Reqwest 与 pinned Hyper 都在网络 Body 和 read-timeout 包装完成后调用同一个响应
   解码边界，再把 Body 交给 Runtime、Protocol 或 Provider 错误分类。
3. 支持 `gzip`、`br`、`zstd` 与无操作 `identity`，按 HTTP 声明顺序的逆序增量解码。
   名称大小写不敏感，允许多 Header 行与逗号列表；编码链最多四层。空 token、未知 token、
   损坏数据或更深链立即失败。
4. 解码前后同步维护表示元数据。成功建立解码链后删除 `Content-Encoding`、
   `Content-Length`、`Content-Range`、`ETag`、`Digest` 与 `Content-MD5`；禁止只删 Header
   后继续传递压缩字节。
5. 编码 Header 无效或解压流损坏使用 `TransportErrorStage::ReadBody`、
   `TransportFailureScope::Endpoint` 和 `RetrySafety::Ambiguous`。底层网络 Body 原有错误
   保留自己的阶段和归因。
6. 非成功状态透明返回上游 HTTP 状态、允许 Header 和解码后的原始内容字节，不做 JSON
   重序列化。Buffered、错误正文和 SSE 帧上限都作用于解压后的字节；SSE 使用有界输出 chunk
   增量进入现有 frame parser，不整包缓冲。

## 后果

- 所有 Provider、状态和 Client 路径使用同一响应编码语义。
- 下游得到的是解码后的原始内容，不会收到失配的表示 Header。
- 请求协商与响应解码职责分离；跨协议桥不继承源方言的 `Accept-Encoding`。

## 验证

- 单元测试以任意小 chunk 输入真实 gzip、Brotli 和 Zstandard 流，覆盖逆序解码、Header
  清理、损坏/未知编码和底层错误保真。
- loopback 测试覆盖普通/pinned Client、缺失请求协商时的压缩响应，以及 ADR-0135 的
  同方言原值透传。
- 透明上游错误契约覆盖非成功状态、解码后原始字节和不携带失配 Header。
