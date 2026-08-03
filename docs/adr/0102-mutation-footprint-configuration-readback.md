# ADR-0102: 配置 Mutation 按影响面回读

- 状态：Accepted
- 日期：2026-08-03
- 决策者：maintainer

## 背景

配置发布必须在 SQLite 提交前形成完整 `StoredConfiguration`，由 Runtime 完成能力校验和
`PublishedSnapshot` 预编译。此前每个 mutation 会在同一个 `BEGIN IMMEDIATE` 事务内调用两次完整配置加载：
第一次取得当前真相并准备领域修改，写入和递增 revision 后再完整加载一次作为候选。两次加载都会遍历所有
Provider Credential、Gateway API Key、OAuthAccount 和代理 Secret，并重新计算持久化摘要。即使只修改一个
代理名称，也会重复读取所有无关表和验证所有无关 Secret，配置规模增大后发布成本随全量 Secret 数量重复增长。

直接用 mutation 的内存结果拼出候选虽然最快，却会取消 SQLite 写后回读，无法发现 SQL 写入遗漏、值截断、
级联或物化结果与领域候选不一致。把完整候选校验降为局部校验也会破坏同 revision 的网关鉴权、路由和设置快照。

## 决策

1. 每个配置 mutation 在 `BEGIN IMMEDIATE` 事务起点完整加载当前配置一次。该加载继续验证全部持久化
   Secret 摘要、领域值和现有交叉引用；损坏数据保持 Fail-Closed。
2. mutation 写入并递增 revision 后，从同一事务视图回读明确的写入影响面，而不是第二次加载全部配置：
   - Proxy 配置或认证：回读 Proxy 与密码；用新 Proxy 重新构建 Provider Credential 和 OAuthAccount 的
     交叉引用，但复用第一次已经验证的无关 Secret；
   - Provider Endpoint：回读 Endpoint；发生 Credential generation 更新时回读 Credential 与 Secret，
     否则用新 Endpoint 重建其领域配置；同时回读或重建受影响的 Model Route，并在裁剪发生时回读 Setting；
   - Provider Credential：回读 Credential 与 Secret，并回读发生物化变更的 Model Route/Setting；
   - OAuthAccount：回读账号与 OAuth 材料，并回读可能被模型 allowlist 裁剪的 Setting；
   - Gateway API Key：只回读 Key 行并重建验证器；
   - Setting：只回读 Setting override。
   revision 始终从写后事务视图回读。
3. 未被写入影响的聚合只能复用本次事务起点已经完整验证的不可变值。任何跨聚合引用的目标发生变化时，
   必须调用现有 Domain 构造器重建相关配置，不能仅替换字段绕过交叉引用校验。
4. 写后回读值继续与 mutation 预期值逐组件核对，失配返回不包含 Secret 内容的
   `ConfigurationWriteMismatch` 并回滚。新增 mutation、外键级联、触发器或物化写入时，必须同步扩充其影响面
   与测试；不能依赖当前实现“碰巧没有修改其他表”。
5. Storage 最终仍返回包含全部聚合和 Secret 材料的完整 `StoredConfiguration`。Runtime 对整份候选执行现有
   Provider 能力校验和 `PreparedPublishedSnapshot::compile`；成功后才 Commit、无失败 reconcile 并单次切换
   `ArcSwap`。本决策只减少重复持久化读取与摘要计算，不引入局部快照、缓存真相或提交后失败点。

## 备选方案

- 保留两次完整加载：最简单，但无关 mutation 会持续支付第二次全表扫描和全 Secret 摘要成本。
- 完全信任内存 mutation 结果：省去写后读取，却削弱已存在的持久化一致性防线，不采用。
- 在进程中缓存 SQLite 配置作为下一次 mutation 的真相：缓存可能与启动、修复或事务状态分叉，也没有必要；
  串行事务中的第一次完整加载已经提供稳定基线。
- 为每个 Secret 建立额外可变缓存或增量计数器：增加同步状态和失效规则，个人单节点规模下复杂度不成比例。

## 后果

- 修改一个小聚合仍会完整验证事务起点和完整编译候选，但不再第二次读取所有无关表或重复计算其摘要。
- 修改 Secret 所属聚合仍会在写后回读时验证该聚合的新摘要；优化不会降低 Secret 写入的校验强度。
- 影响面成为 mutation 的显式正确性契约，Schema 或写路径扩展必须同步维护。

## 验证

- Storage 测试覆盖每类 mutation 的写后组件一致性、交叉引用重建、revision 和回滚语义。
- 既有损坏 Secret/配置测试继续证明事务起点完整加载 Fail-Closed；Runtime 发布测试继续证明完整候选预编译失败
  时 SQLite revision 与 PublishedSnapshot 都不变化。
- 影响面模块测试证明 Gateway mutation 会重新验证被写入 Key 的摘要，而 Proxy mutation 不会再次验证事务起点
  已经确认且不在其写入影响面内的 Gateway Key；候选事务测试同时确认后者返回的仍是完整鉴权配置。

## 本地基准

在 Apple M4（arm64、macOS 26.6、Rust 1.90.0）上使用最新 Schema，直接预置 10,000 个具有有效明文与
SHA-256 摘要的 Gateway API Key，以 Setting override mutation 测量 Commit 前候选事务准备段。两组都包含
同一次事务起点完整加载、Setting 写入和 revision 递增；基线在写后再次调用完整配置加载，优化组只回读
revision 与 Setting。Runtime 的完整能力校验、快照编译和 Commit 对两组相同，未计入这项只比较 Storage
差异的基准。release 构建预热后取 7 次中位数：

| 路径 | 中位耗时 | 相对结果 |
| --- | ---: | ---: |
| 全量加载 + 全量写后回读 | 5.498 s | 1.00× |
| 全量加载 + Setting 影响面回读 | 2.748 s | 2.00× faster |

可复现命令：

```bash
cargo test -p any2api-storage --release \
  configuration::readback_benchmark_tests::large_setting_publish_compares_full_and_impact_readback \
  -- --ignored --nocapture
```

该结果不意味着所有 mutation 都固定加速两倍：修改 Gateway Key 或 Provider Credential 时仍必须回读并验证
对应 Secret 聚合，影响面越大，节省比例越低。它证明小聚合修改不再支付第二次无关全量摘要校验。
