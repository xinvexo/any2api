# ADR-0157：GatewayApiKey 使用标准 `sk-` 前缀

- 状态：Accepted
- 日期：2026-08-11
- 决策人：maintainer
- 相关决策：ADR-0006、ARCHITECTURE.md 第 9.8 节

## 背景

any2api 的本地 `GatewayApiKey` 原来使用 `a2k_v1_` 前缀。部分 OpenAI/Codex
客户端只接受或默认展示 `sk-` 形状的访问密钥，因此本地网关密钥的格式与客户端
输入约定不一致。

这只是入口凭据的字符串格式变化。`GatewayApiKey` 仍然是 any2api 实例访问凭据，
不是 Provider API Key、OAuth Token 或官方 Codex 凭据；它不会被转发给 Provider，
也不能选择或绑定上游账号。

## 决策

1. 新建和轮换的 GatewayApiKey 使用 `sk-` 加 32 个 CSPRNG 随机字节的 URL-safe
   Base64 无填充编码，总长度为 46 个 ASCII 字符。
2. 当前运行时只接受 `sk-` 形状。旧 `a2k_v1_` token 不在认证入口提供双轨兼容，
   避免当前 Secret 格式继续分叉。
3. 新增 Migration 0020 重建 `gateway_api_keys` 的 CHECK 约束。若数据库中存在旧
   `a2k_v1_` 记录，Migration 在任何 DDL 或数据修改前 fail-closed；管理员必须先
   显式删除旧记录或重新初始化数据库后再生成 `sk-` Key。迁移不自动改写、合并或
   保留旧 token，也不改变 Gateway Key 与上游凭据的隔离关系。
4. `PublishedSnapshot` 入口先执行当前 `sk-` 格式校验；旧前缀不会作为客户端 token
   被重新接受。
5. Web 契约解析器、Rust domain/storage/runtime 契约测试和内嵌资源必须同步使用
   `sk-`。管理 API 继续返回明文 GatewayApiKey，Provider Secret 与 OAuth JSON
   的处理不变。

## 后果

- Codex 风格客户端可以直接使用新生成的 GatewayApiKey 字符串。
- 已有 `a2k_v1_` Key 不会被自动转换；包含这类记录的数据库会在 Migration 0020
  进入 DDL 前失败。管理员必须先显式删除旧 Key（或重新初始化数据库），再让服务
  生成新的 `sk-` Key。
- 前缀相似不改变凭据职责：上游仍只看到调度器选中的 Provider API Key 或 OAuth
  Token，GatewayApiKey 继续在 Provider Driver 之前被剥离。

## 验证

- Domain/runtime/storage 测试覆盖 `sk-` 长度、字符集、生成、轮换和认证。
- Migration 测试使用代表性旧记录验证在任何 DDL 前拒绝旧格式，并验证空库能完成
  新 CHECK 约束的发布。
- Web typecheck、lint、unit test、embedded build/check 与 Rust 全工作区门禁继续
  通过。
