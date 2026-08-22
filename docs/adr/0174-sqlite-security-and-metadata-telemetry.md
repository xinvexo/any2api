# ADR-0174: SQLite 本地安全边界与 metadata-only HTTP 日志

- 状态：Accepted
- 日期：2026-08-22
- 当前事实：[存储、配置与安全](../architecture/storage-and-security.md)
- 替代：[ADR-0170](0170-current-decision-register.md) 的存储、安全和可观测部分
- 被替代：无

## 背景

单节点实例需要持久化配置、必要凭据和本地审计。引入外部数据库或应用级主密钥会扩大运维面；另一方面，HTTP
请求可能携带 API Key、OAuth code、Cookie、查询凭据和用户内容，保存交换内容会把诊断功能变成第二个 Secret
仓库。

## 决策

SQLite 与受限数据目录是本地持久化边界，Schema 使用不可改写的前向 Migration。必要 Provider/OAuth/代理
Secret 可以存在 SQLite，但不得进入日志、普通管理响应、浏览器持久化或导出。

HTTP 系统日志只保存用于审计和故障定位的 metadata；query、Header 和 Body 不采集。升级 Migration 通过重建
日志表物理移除旧库中不再允许保留的内容。RequestLog/Attempt 继续保存有界的模型请求结果与规范 usage，但
不用于计费或恢复。

## 备选方案

- PostgreSQL 或外部 Secret manager：对个人单节点增加部署依赖，且不能替代主机访问控制。
- 应用内主密钥加密所有字段：密钥仍需本机保存或人工注入，增加轮换和灾难恢复协议。
- 保存完整 HTTP 交换并只依赖管理员鉴权：扩大泄露影响、容量与 UI 复杂度，OAuth/认证端点尤其危险。
- 完全不记录 HTTP：失去扫描、错误、取消和远程访问的最低审计能力。

## 后果

系统日志仍能回答谁在何时访问哪个 path、结果与耗时，同时不保存请求内容和凭据。代价是不能从日志重放或查看
完整上游/客户端 payload；深度诊断依赖可复现请求、受控 tracing 和外部抓包。数据目录仍必须由部署者保护。

## 验证

Migration 清理测试、DTO/Repository metadata 契约、Secret 泄露测试、日志容量与保留测试、远程管理安全测试。
