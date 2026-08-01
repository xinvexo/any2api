# ADR-0063: SQLite Migration 历史不可改写并只追加前向脚本

- 状态：Accepted
- 日期：2026-07-29
- 修订日期：2026-07-31
- 决策者：maintainer

## 背景

项目曾尝试以“首个正式版本前”为由改写 `0001_initial.sql`、删除开发期 Migration，并要求已有数据目录重建。这会让使用任意仓库版本创建的数据目录无法通过正常启动升级，也使 Migration checksum 无法保护已经执行过的历史。仓库中的编号 Migration 本身就是兼容边界，是否已经正式发布不改变这一事实。

## 决策

- `0001_initial.sql` 以及以后所有已经进入仓库的编号 Migration 和 checksum 一经创建即冻结，后续不得改写或删除。
- 每次 Schema 变化追加编号连续的前向 SQL 文件，并把新文件的 SHA-256 写入 `migrations/checksums.toml`。
- 当前 Schema 由空数据库顺序执行完整迁移链得到；旧 Migration 可以包含已经被后续 Migration 删除的字段或结构。
- Storage 生产代码只面向完整迁移后的最新 Schema，不增加旧字段、双轨 Row、兼容查询或运行时版本分支。
- 每个改变既有 Schema 的 Migration 同时提供升级测试：先建立上一 Migration 的 Schema 和代表性数据，再运行完整 Migrator，验证数据保留、最终结构和 Migration 记录。
- 默认必须保留已有数据；只有独立 ADR 明确拒绝某个旧格式时，才允许在任何修改前拒绝非空旧记录。不得静默删除，也不得以此为理由改写历史 Migration。
- `xtask architecture-check` 继续检查迁移编号连续、文件与 checksum 一一对应且内容匹配。

## 后果

- 已有数据目录可以通过正常启动顺序升级，不因后续 Schema 变化而被要求删除数据库。
- 旧 Migration 不再单独代表最新领域模型；架构文档、领域类型和完整迁移后的数据库共同描述当前实现。
- 修复任何既有 Migration 都必须追加新脚本，不能修改历史 SQL 或替换 checksum。
- ADR-0074 对旧 Secret 格式作出一次明确例外：保留 Schema 历史，但不保留旧格式转换代码或数据兼容。

## 验证

- 架构门禁拒绝编号断裂、缺失/多余 checksum 或内容哈希变化。
- Storage 测试覆盖空库执行完整迁移链，以及从上一 Migration 带代表性数据升级到最新版本。
