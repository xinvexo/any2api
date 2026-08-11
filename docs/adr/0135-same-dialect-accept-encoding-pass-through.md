# ADR-0135: 同方言 Accept-Encoding 受控透传

- 状态：Accepted
- 日期：2026-08-11
- 决策者：maintainer

## 背景

ADR-0127 为了统一普通 Reqwest 与 pinned Hyper 的响应解码，要求 Transport 在每个请求上强制覆盖 `Accept-Encoding: gzip, br, zstd`。这解决了压缩响应进入 JSON/SSE decoder 的问题，但也给所有 data、OAuth 和诊断 surface 增加了客户端未必发送的固定线路特征。实际同方言请求已经保留鉴权剥离后的客户端 Header，强制生成一套新协商值既不是透明代理，也不是 Provider 必需契约。

响应解码能力和请求协商所有权是两个不同问题。any2api 必须继续解码自己能够理解的压缩响应，因为 SSE、错误分类、模型恢复和遥测都需要读取内容；这不要求网关替客户端主动声明固定压缩偏好。

## 决策

1. 通用线路 profile 升级为 `generic-rustls-hyper-v3`。Transport 不再补写或覆盖 `Accept-Encoding`，Provider Driver 也不得声明另一套固定值。
2. 只有入口方言与实际上游方言相同时，Runtime 才在 Provider 白名单投影之外把客户端 `Accept-Encoding` 复制到最终 TransportRequest。保留原始值、token 顺序、权重参数和同名多 Header 结构；客户端缺失时上游也保持缺失。
3. 跨协议桥不透传源方言的编码协商。OAuth token/quota、代理测试和其他没有数据面客户端 Header 的请求同样不借用或生成该字段。
4. Transport 在网络 I/O 前统一校验最终值。每个逗号项的 coding 必须是当前可增量解码的 `gzip`、`br`、`zstd` 或无操作的 `identity`；允许原样保留参数。任一空项、通配符或不支持 coding 都删除整组 Header，禁止过滤部分项后静默改变客户端偏好。
5. 响应 Content-Encoding 继续由 ADR-0127 的统一边界拥有。普通与 pinned Client 无论请求是否声明 `Accept-Encoding`，都解码受支持的响应编码并同步删除失效表示元数据；未知、损坏和过深编码链仍 Fail-Closed。
6. `Accept-Encoding` 仍在 Provider 通用 Header 投影禁用列表内，避免 Provider 白名单、凭据 Header 或桥接逻辑重复拥有它。唯一复制点位于 Runtime 完成协议、Provider 和凭据 Header 合并之后，唯一校验点位于 Transport 发送之前。

本决策局部取代 ADR-0127 的“Transport 固定覆盖请求 Accept-Encoding”条款，不改变其响应解码、错误归因和透明正文规则。

## 后果

- 官方或其他客户端没有发送 `Accept-Encoding` 时，any2api 不再制造该线路特征。
- 同方言客户端明确发送且本地能够解码的协商值保持原样，不被固定 gateway 值替换。
- 任意未知编码不会诱导上游返回 Runtime 无法解析的表示；代价是这类非受支持协商值不会被转发。
- HTTP/1 raw fixture 与全 surface matrix 会明确显示缺省请求不再包含该 Header；TLS 和 HTTP/2 连接策略保持不变。

## 验证

- Runtime 单元测试覆盖同方言多值原样复制、缺失保持缺失和跨方言不复制。
- Transport 单元测试覆盖受支持值不变、无 Header 不补写以及空项、通配符和未知 coding 整组删除。
- 普通与 pinned loopback Client 测试使用显式客户端 `gzip` 协商并验证增量解码；另验证缺省真实线路没有 `Accept-Encoding`。
- conformance fixture 提升到 v3，全 surface raw matrix 审核删除固定协商字段后的真实差异。
