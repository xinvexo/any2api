# 官方客户端脱敏观测证据

本目录只保存独立于 any2api wire fixture 的机器可读外部证据，不定义当前架构或运行时行为。证据如何影响
Provider/Protocol 契约见 [协议主题文档](../../architecture/protocol-bridges.md#证据和验证)，采集与 Secret
边界见 [存储安全主题文档](../../architecture/storage-and-security.md#secret-边界)。

每份 JSON 自包含客户端 provenance、采集条件、脱敏后的请求摘要和适用局限。目录中的 JSON 文件集合
就是当前证据清单，不在说明文档中重复维护易过期的版本、平台或覆盖表。

`cargo xtask architecture-check` 会校验全部证据的 Schema、SHA-256、脱敏和隔离采集策略。
