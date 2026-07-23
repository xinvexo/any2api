# ADR-0029: Provider Base URL 直接决定访问目标

- 状态：Accepted
- 日期：2026-07-23
- 决策者：maintainer

## 背景

any2api 是个人自托管实例，Provider Endpoint 只能由已认证管理员配置。原设计要求每个 Endpoint 额外维护 `allow_insecure_http` 与 `allow_private_network`，导致填写一个明确的 Base URL 后仍要重复确认协议和地址类别，也把同一意图拆成三个容易不一致的字段。

管理员已经通过 Base URL 明确指定上游目标。项目仍需要防止字符串拼接、客户端改写 authority、自动重定向和凭据跨源泄漏，但不需要替管理员判断自托管 HTTP、loopback、局域网或容器网络是否可以访问。

## 决策

- `ProviderBaseUrl` 只接受结构合法的 `http` 或 `https` URL，必须有 host，禁止 userinfo、query、fragment、零端口和 `.`/`..` 路径片段。
- `http`、`https`、公网、loopback、局域网、link-local 和容器网络地址均不需要额外授权字段；Base URL 直接决定访问目标。
- 从 Domain、SQLite、管理 API、PublishedSnapshot 和 Web 删除 `allow_insecure_http`、`allow_private_network`，不保留兼容字段或隐藏默认值。
- DIRECT 始终本地解析并固定本次请求的目标；`upstream.strict_ssrf=true` 时代理路径也本地解析并固定目标。两种路径都不按公网/私网地址类别过滤管理员配置的 Endpoint。
- Transport 继续禁用自动重定向。客户端的 Host、absolute-form URL、Forwarded 与 X-Forwarded-* 不参与上游 authority 构造；Provider Credential 只注入到 Driver 根据已发布 Base URL 构造的请求。
- 新迁移删除旧授权列，保留已有 Endpoint 的其余配置与身份。

## 备选方案

- 只从 Web 隐藏开关，后端始终写入 `true`：会留下错误领域模型、API 和数据库字段，违反新项目直接完成正确重构的原则，放弃。
- 保留全局“允许 HTTP/内网”设置：仍会让 Base URL 与第二个策略来源冲突，放弃。
- 完全保存自由字符串并在请求时拼接：会重新引入 authority 覆盖、userinfo 和路径拼接风险，放弃。

## 后果

- 自托管兼容服务可以直接使用 `http://127.0.0.1`、局域网 IP、容器主机名或公网 HTTPS 地址，Provider 表单只保留 Base URL。
- 拥有管理权限的人可以让 any2api 携带 Provider Credential 访问任意 HTTP(S) 目标，因此管理员认证与远程管理边界仍是必要保护。
- 严格 DNS 模式继续提供本地解析与目标固定，但不再被描述为内网访问授权。

## 验证

- Domain 测试覆盖 HTTP/HTTPS、公网/私网/loopback 直接接受，以及非 HTTP(S)、userinfo、query、fragment、零端口和路径穿越拒绝。
- Storage 与管理 API 契约验证授权字段已移除，旧数据库迁移后 Endpoint 保持可读写。
- Transport/公开请求契约验证 DIRECT、HTTP 与 SOCKS5 路径可以访问管理员配置的私网 Endpoint，且重定向仍关闭。
- React 测试验证表单只提交 Base URL，不再渲染网络授权开关。
