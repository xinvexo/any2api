# ADR-0001: ProviderEndpoint 的结构化 URL 与显式网络授权

- 状态：Superseded by ADR-0029
- 日期：2026-07-18
- 决策者：maintainer

## 背景

本 ADR 记录了最初的显式网络授权方案。管理员填写的 Base URL 改为直接决定访问目标，HTTP/内网授权字段已由 ADR-0029 移除；以下内容仅保留为历史决策记录。

参考项目通常把 Provider 地址保存为自由字符串，并在请求阶段直接拼接路径。这样会把 query/fragment、userinfo、重定向和私网访问边界推迟到网络执行，容易造成凭据外送和 SSRF。any2api 需要允许个人部署自托管 Provider，同时默认保持公网 HTTPS 的安全基线。

## 决策

- `ProviderEndpoint` 使用强类型 `ProviderBaseUrl`，只接受 `http`/`https`，必须有 host，禁止 userinfo、query、fragment、零端口和 `.`/`..` 路径片段。
- 保存时去除多余尾斜杠但保留固定路径前缀；协议模块未来只能通过结构化 URL API 追加固定路径，不能用任意字符串覆盖 authority。
- `allow_insecure_http` 和 `allow_private_network` 是 Endpoint 级、相互独立的显式授权。HTTP+私网地址必须同时开启两个开关。
- 字面 loopback、私网、link-local、multicast、未指定、共享地址、文档/保留地址，以及 localhost/metadata/local 命名空间默认拒绝。
- 本切片只做无网络 I/O 的结构化和字面主机校验。域名 DNS A/AAAA 最终校验、连接 IP 固定、重定向重新校验属于 Transport 执行边界。

## 备选方案

- 继续保存 `String` 并在请求时拼接：实现简单，但无法保证配置发布后不产生危险 URL，放弃。
- 默认允许 HTTP/私网，再依赖全局 SSRF 设置：会让一个 Endpoint 的风险影响所有 Provider，且与项目“按 Endpoint 显式授权”约束冲突，放弃。
- 在领域层执行 DNS 查询：会引入网络 I/O、不可预测延迟和 DNS rebinding 的时序问题，放到 Transport 的受控拨号层。

## 后果

结构化模型让管理 API、SQLite 和 Web 使用同一组字段与校验；自托管用户仍可逐个 Endpoint 开启 HTTP/私网。Provider URL 的 DNS 信任边界尚未在本切片完成，实际发请求前必须由 Transport 再次校验并限制重定向。

## 验证

- Domain 单元测试覆盖 scheme、userinfo、query/fragment、端口、字面 IP、metadata/local 域名和 provider/dialect 配对。
- Storage/Publisher 测试覆盖 revision、热更新和重启读取。
- Admin 契约测试覆盖风险开关、非法 URL、冲突和 loopback 限制。
- `cargo fmt/clippy/test/build/xtask architecture-check` 与 Web typecheck/lint/test/build 全部通过。
