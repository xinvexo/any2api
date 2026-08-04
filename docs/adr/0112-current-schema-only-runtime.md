# ADR-0112：运行时只面向当前 Schema

- 状态：Accepted
- 日期：2026-08-04
- 决策人：项目维护者

## 背景

SQLite 曾直接保存 CLIProxyAPI 风格的 Provider 文档，并让 Token Endpoint 响应解析器兼任数据库读取器，
因此运行时长期理解 `type`、`expired`、`last_refresh` 和 Grok `sub` 等外部字段。CLIProxyAPI/Sub2API
导入仍是当前产品功能，但只应作为导入边界的外部输入协议。

## 决策

1. any2api 历史格式转换只存在于顺序 SQL Migration；生产 Rust/TypeScript 不实现旧字段别名、双轨读取、
   启动期重写、废弃转发层或浏览器存储迁移。
2. `oauth_accounts.oauth_json` 只保存 `access_token`、`refresh_token`、`id_token`、`account_id` 和
   `email`。后四项可省略或为 `null`；`provider_kind` 与 `expires_at` 使用同一行已有的强类型列，不在 JSON
   重复保存。Provider current-document decoder 拒绝未知字段；Storage 只负责大小、JSON 对象和必需 access
   token 的持久化边界校验。
3. Migration `0013` 把既有 Provider 文档一次性转换为当前五字段文档，保留 Token、账号身份和安全邮箱；
   既有 Migration 与 checksum 不修改。
4. Token Endpoint 响应继续由具体 Driver 解析；SQLite 文档使用独立 current-document codec。配置编译器
   只调用后者，并从账号列提供 Provider 与绝对过期时间。
5. CLIProxyAPI/Sub2API 导入器继续在 Provider 边界生成 `OAuthTokenMaterial`，随后立即写成当前文档；外部
   wrapper、别名和时间格式不进入 SQLite 读取路径。主题等浏览器状态也只接受当前声明值。

## 后果与验证

- 生产代码只有一种 OAuthAccount 文档结构，外部导入协议与内部持久化不再共用解析器。
- 以后修改文档格式时先追加 SQL Migration，再一次性修改 codec 与调用点。
- 代表性 Migration 升级测试验证 Codex/Grok 字段映射、元数据保留和完整迁移链；现有登录、刷新和导入
  契约验证三条入口生成并消费同一当前格式，不再为同一字段矩阵重复增加各层单测。
