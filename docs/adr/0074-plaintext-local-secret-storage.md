# ADR-0074: SQLite 明文 Secret 与部署信任边界

- 状态：Accepted
- 日期：2026-07-31
- 决策者：maintainer

## 背景

any2api 是个人使用、单节点、自托管的程序，主要运行在内网；公网部署由 Nginx、Caddy 等反向代理提供 TLS 和网络入口保护。SQLite 与数据目录权限是本地持久化保护边界，不额外维护第二份密钥状态。已经进入仓库的数据库 Migration 仍不可改写，因此当前明文 Schema 通过新的前向 Migration 落地，但旧 Secret 格式不是数据兼容目标。

## 决策

- Provider API Key、代理密码、Gateway API Key 与 OAuth Provider JSON 统一原样明文存入 SQLite。应用层不实现加密、解密、主密钥、加密密钥轮换、密文 Schema、密文 DTO 或历史密文兼容分支。
- 代码中的 `Secret`、`SecretBytes` 与 `secrecy` 封装只用于限制 Debug、日志和非必要复制，不表示存储值经过加密，也不能成为后续引入应用层加密的隐式扩展点。
- 管理员密码继续只保存 Argon2id PHC 摘要；管理 Session、CSRF Token 和 OAuth 登录临时状态继续只保存在内存。这些单向验证材料不属于可恢复的持久化 Secret。
- Provider API Key、代理密码和 OAuth Token 仍不得进入普通读取 DTO、日志、Debug、URL、浏览器持久化或导出端点。Gateway API Key 继续按产品要求在已认证管理响应中完整展示，但不得写入日志。
- Gateway API Key 是 256 位随机 token。公开入口使用无密钥 SHA-256 摘要进行索引和常量时间验证；`hash_version` 标识摘要格式。
- Provider Secret 指纹使用带稳定域前缀的 SHA-256，只用于管理展示和变更识别，不作为认证或唯一约束。API Key 和 Gateway Token 的高随机熵是该摘要模型的前提。
- Unix 数据目录强制为 `0700`；SQLite、WAL、SHM、实例锁和应用日志强制为 `0600`。既有路径迁移必须只处理当前用户拥有的普通文件/目录，不跟随符号链接。非 Unix 平台依赖只授予服务账户访问的数据目录 ACL。
- 权威配置数据库使用 `synchronous=FULL`。如果后续基准证明遥测写入成为瓶颈，再单独迁移可丢遥测数据库；不能先降低配置事务耐久性。
- 保留既有 Migration 及其 checksum，追加 `0003` 形成当前明文 Schema；新数据库执行完整迁移链得到当前结果。
- 不提供旧 Secret 格式的读取、转换、密钥输入、环境变量或双轨 Repository。`0003` 只允许旧 Secret 表为空；首条语句是只读拒绝断言，发现任意旧 Gateway Key、Provider Credential 或代理密码时，必须在任何 DDL 或数据修改前失败，由维护者清空并重新初始化开发数据库。

## 后果

- 配置集中在 SQLite 中；迁移时必须先停止 any2api，再离线复制包含数据库及其 sidecar 的完整数据目录，不能在 WAL 运行期间只复制主库文件。
- 能读取 SQLite 的本机主体也能读取全部上游和网关凭据。这是明确接受的部署信任边界；反向代理保护网络传输，不改变本地文件权限要求。
- 管理响应、启动参数和 Repository 不识别旧存储形态；迁移历史中的旧字段不能泄漏到当前产品契约。
- Secret 的日志脱敏、客户端认证头剥离、Provider/Gateway Key 隔离、代理 Fail-Closed 和管理认证要求保持不变。

## 验证

- Storage 测试覆盖空库执行完整迁移链、上一 Migration 的空 Secret 表升级，以及旧 Gateway Key、Provider Credential、代理密码分别非空时在任何 Schema 变化前拒绝；同时覆盖 Secret 写入读取和摘要/指纹版本。
- Unix 集成测试从宽松初始权限出发，验证数据目录收紧为 `0700`，数据库、WAL、SHM、实例锁和日志收紧为 `0600`。
- 配置与管理契约测试继续证明 Provider API Key、代理密码和 OAuth Token 不进入读取响应或日志，Gateway API Key 管理可见性保持不变。
