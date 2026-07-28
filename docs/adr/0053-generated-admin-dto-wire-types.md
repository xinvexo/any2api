# ADR-0053: 管理 API 线格式类型由 Rust DTO 生成

- 状态：Accepted
- 日期：2026-07-26
- 决策者：maintainer

## 背景

管理 API 的请求/响应契约不能在两处各自手写一份：

- 后端：`crates/server/src/admin/*/dto/` 中的 serde snake_case DTO；
- 前端：`web/src/features/*/api/*-contracts.ts` 中的 camelCase 接口加逐字段运行时解析器，当前约 2100 行，并配套同等规模的 `.test.ts` 用例维持两份定义一致。

同时维护 Rust DTO、TypeScript 接口、运行时解析器和契约测试会产生字段漂移；线格式类型应从 Rust DTO 直接生成，前端只手写超出结构的语义校验。

## 决策

- 后端管理 DTO 通过 `ts-rs` 的 `#[derive(TS)]` 直接导出 TypeScript 线格式类型（保持 snake_case，与 HTTP 字节一致），输出到 `web/src/shared/api/generated/`。
- 生成物是**线格式单一事实源**：前端解析器的输入参数从 `unknown` 收紧为生成的线类型；camelCase 领域模型与运行时校验（Token 形态、正整数、枚举白名单等安全断言）仍由前端手写，因为它们承载超出结构的语义规则。
- 生成通过 `cargo test --package any2api-server export_bindings`（ts-rs 标准机制）执行，导出目录与整数映射由 `.cargo/config.toml` 的 `TS_RS_EXPORT_DIR`/`TS_RS_LARGE_INT=number` 固定；drift 检查放在 Rust CI job：nextest 全量测试（含 export_bindings）后执行 `git diff --exit-code web/src/shared/api/generated/`，不一致即失败。
- 生成目录禁止手工编辑，纳入 ESLint ignore 与 review 约定；前端 feature 仍只能从自身 `api/` 模块导入，生成类型经由各 feature 的 contracts 文件转发，不改变 feature 边界规则。
- 公开协议入口（OpenAI/Anthropic 兼容面）**不在范围内**：其契约由上游协议规定，继续由协议 adapter 与契约测试拥有。

## 备选方案

- `schemars` 导出 JSON Schema 再经 `json-schema-to-typescript`/`typescript-json-schema` 二段生成：链路多一跳、产物含 Schema 噪音，且仍需 Node 侧生成步骤；在只需要 TS 类型的场景下不如 ts-rs 直接。
- OpenAPI（`utoipa`）全量描述管理 API：要求为每个 handler 补注解，侵入面与维护面显著更高，而当前只需要 TypeScript 线类型。
- 继续手写重复线类型：每个端点的同步成本随 API 面线性增长。

## 影响

- 新增/修改 DTO 时,前端线类型自动跟随，遗漏同步从"运行时测试失败或线上解析异常"提前到"CI drift 检查失败"。
- 手写解析器保留但输入有类型约束，`value.config_revision` 之类的字段访问获得编译期检查，契约测试可以收缩为语义校验（安全断言）而非全字段结构复读。
- `ts-rs` 为 dev-dependency 加 test-only derive，不进入发布二进制；serde 属性（`rename_all`、`skip_serializing_if`）由 ts-rs 原生理解。
- 已标注 `ts-rs` 的 Gateway API Key、总览 Usage 与请求 Usage DTO 使用生成线类型；未标注的管理 DTO 不得伪装成已生成契约。

## 验证

- 生成类型与对应前端领域模型做字段级契约测试，避免隐藏漂移。
- CI drift 检查在故意改动一个 Rust DTO 字段而不重新生成时必须失败。
- 前端 typecheck 在解析器访问不存在字段时必须失败（现状不会）。
