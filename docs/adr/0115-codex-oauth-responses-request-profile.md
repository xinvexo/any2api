# ADR-0115：Codex OAuth Responses 出站请求 Profile

- 状态：Accepted
- 日期：2026-08-05
- 决策者：maintainer
- 修订：ADR-0059、ADR-0113

## 背景

OpenAI 公共 Responses API 与 ChatGPT 订阅账号使用的 Codex Responses 后端共享基础方言，但请求契约
并不完全相同。真实请求证明 `https://chatgpt.com/backend-api/codex/responses` 要求显式
`store=false`，并拒绝 `max_output_tokens`；通用 Responses 客户端或从其他协议转换而来的请求可能省略
前者、发送后者，并继续使用 `system` role。原生 Codex Desktop 已经发送符合该后端约束的正文，因此
现有同协议透明直通可以成功。

该差异属于选中 OAuthAccount 后才确定的 Provider Endpoint Profile。把它放进通用 OpenAI Responses
Adapter 会同时改坏 API Key 访问的 OpenAI 公共 API和自定义兼容 Endpoint；按 User-Agent、客户端名称或
请求内容猜测来源又会把路由正确性绑定到可伪造的客户端指纹。

ADR-0113 要求同协议直通不物化完整 JSON 树，并让重试共享不可变入口 Body。兼容 Profile 不能恢复每个
Attempt 的 `serde_json::Value` 深拷贝，也不能就地修改共享请求，导致后续安全切换到 API Key 时沿用已经
裁剪的 OAuth 正文。

## 决策

1. `ProviderDriver` 增加接收 attempt-owned `Bytes` 的出站请求体准备钩子。Runtime 在 ProtocolExchange
   已按实际模型生成 wire Body、但尚未执行 zstd 和 Transport I/O 时调用该钩子；默认实现原样返回同一
   `Bytes`。中央 Runtime 不按 Provider 增加 `match`。
2. Codex Driver 只在当前 Attempt 已选中 `OAuthAccount` 且实际上游操作精确为普通
   `ProtocolOperation::Responses` 时启用该 Profile。Codex API Key、Responses Compact、Chat
   Completions、Images、Claude、Grok 和所有其他 Driver 均保持原样；触发条件不读取 User-Agent、
   `originator` 或客户端产品名。
3. Profile 执行以下有界、已登记的兼容变换：
   - 缺失、为真或类型错误的 `store` 都写成 JSON `false`；
   - 删除 Codex 后端不接受的 `max_output_tokens`、`max_completion_tokens`、`temperature`、`top_p`、
     `truncation`、`context_management` 与 `user`；
   - `service_tier` 只保留字符串 `priority`，其他值删除；
   - 顶层字符串 `input` 等价展开为单个 user `input_text` message；数组 input 中对象型 message 的
     `role=system` 精确改为 `developer`，其他 item 与嵌套内容保持原样；
   - `include` 固定为 `['reasoning.encrypted_content']`；缺失或不是 JSON boolean 的
     `parallel_tool_calls` 补 `true`，但保留客户端显式的 `true` 或 `false`。
4. Profile 不强制修改 `stream`。入口的 buffered/SSE 生命周期、Accept Header、Transport 执行路径和
   响应解析必须保持一致；若未来 Codex 后端要求只接受流式调用，应另行实现明确的上游流聚合能力，禁止
   只改 JSON 字段造成 Runtime 按 JSON 读取 SSE。
5. 除上述精确字段外，未知顶层字段和所有未命中的原始嵌套值继续透明保留。该 Profile 不是通用未知字段
   denylist，也不是 Responses → Responses 的第二个跨协议 Bridge。
6. 实现借用 `RawValue` 只索引顶层字段；合规正文直接返回原 `Bytes` 分配。确需改写时才分配一份新 wire
   Body，并只为字符串 input 或包含精确 `system` role 的 input 数组重建对应片段，禁止物化完整历史树。
7. 每次 Attempt 都从共享不可变 `DecodedRequest` 重新编码，再把本次 attempt-owned Body 交给 Driver。
   Profile 不修改 `DecodedRequest`、`AdapterPayload` 或 continuation；OAuth Attempt 后安全切换到 API Key
   时必须重新得到未经 OAuth 裁剪的原始语义。
8. 删除 token limit 表示 Codex OAuth 后端无法兑现客户端请求的输出上限；这是明确的兼容降级，并由本
   ADR 固定。不能把该行为扩张到支持这些字段的 OpenAI API Key 上游。

## 后果

- 原生 Codex CLI/Desktop 请求继续共享原始 Body，常见成功路径不增加完整正文复制或 JSON 树常驻内存。
- 通用 Responses 客户端可以通过同一个 Codex OAuthAccount 使用 ChatGPT Codex 后端，不再因已知字段差异
  收到连续的 400。
- 兼容行为由最终选中的上游契约决定，不需要识别 CC Switch、Claude CLI 或其他客户端。
- Provider 请求体钩子成为稳定扩展点，但每个新增变换仍必须有真实上游证据和独立 ADR，不能演变成万能
  JSON 中间件。

## 验证

- Codex Provider 单元测试覆盖缺失/错误 `store`、token limit 与其他已登记字段删除、字符串 input、
  `system -> developer`、include、显式 parallel 值保留、未知字段保留和合规 Body 分配复用。
- Provider 单元测试证明相同 Codex Driver 在 `oauth=false`、Responses Compact 与其他操作时逐字节且同
  分配返回正文。
- OAuth 路由契约通过真实 Runtime 选中账号并捕获 TransportRequest，证明规范化发生在 Codex OAuth
  Attempt，认证、模型、代理、item ID 归一化和 RequestLog 来源保持不变。
- fmt、clippy、Provider/Runtime/Contract 测试与 architecture-check 共同验证接口和依赖边界。
