# ADR-0113：同协议请求共享原始 JSON

- 状态：Accepted
- 日期：2026-08-04
- 决策者：maintainer
- 修订：ADR-0101、ADR-0115

## 背景

ADR-0101 已经取消 Attempt 与 OAuth replan 对 `DecodedRequest` 的深拷贝，但 JSON 入口仍先把完整 Body
解析为 `serde_json::Value`，随后又为上游构造一份完整 wire Body。等待上游响应头和首个 SSE 事件时，
同一请求会同时持有原始审计前缀、整棵 JSON 对象树和重编码字节。JSON 节点越密集，`Value` 的字符串、
数组与 Map 分配相对线协议字节的放大越明显；这种成本按活跃请求数线性增长，和高并发代理希望维持的
小型每请求状态相冲突。

当前首版公开模型名固定等于上游模型名，多数 Responses、Chat Completions、Messages 与 Images JSON
请求又是同协议直通。为这些请求完整物化树，只为读取少量顶层路由字段并重新写回相同正文，没有必要。
显式 Responses → Chat Completions Bridge 确实需要结构化遍历，应只让选择该桥的请求承担该成本。

## 决策

1. JSON 入口使用 `RawJsonPayload` 保存一个共享 `Bytes` 和顶层字段的范围索引。索引通过
   `serde_json::value::RawValue` 的借用反序列化建立，因此仍完整验证 JSON 和根对象类型，但不为未知嵌套
   字段构造 `Value` 树，也不复制其字符串。
2. `model`、`stream`、会话字段、思考级别和远程压缩标记只解析各自需要的顶层原始字段。HeaderMap 在完成
   入口检查后直接移动到 `DecodedRequest`，不为相同所有者再克隆一份。
3. 同协议编码时，如果最终上游模型等于入口模型、当前操作不需要删除 `stream` 且顶层字段名唯一，直接
   克隆共享 `Bytes` 句柄作为 Transport Body；该操作不复制正文。确实需要替换模型、删除字段或消除重复
   顶层字段时，只按顶层索引把未修改的原始字段片段写入一个新的有界 wire Body，禁止先物化完整树。
   ADR-0115 登记的 Codex OAuth Responses 出站 Profile 在当前 Attempt 已选中后可以对 attempt-owned Body
   再做一次借用式顶层改写；它不得修改或替换共享入口 payload。
4. Responses 可移植 item ID 归一化继续发生在路由前。实现使用 `RawValue` 增量扫描顶层 `input` 数组和每个
   item 的顶层 `type`/`id`；只有发现已知类型的非法字符串 ID 时才重建正文并删除该字段。未知 item、嵌套
   ID、非字符串 ID 和非数组 input 保持原样，禁止为归一化解析完整历史树。
5. `AdapterPayload::Json(Value)` 继续用于桥接生成的结构和测试夹具；入口解码使用独立
   `AdapterPayload::RawJson`。显式 ProtocolBridge 可以按需把 Raw JSON 物化为一次 `Value`，随后沿用桥的
   有界 continuation 与转换状态；直通路径不得触发这次物化。
6. Raw JSON 的 Debug 只报告字节数和字段数。原始 Body、模型内容、会话标识和 Secret 不得进入 Debug、
   tracing 或模型 RequestLog；HttpAccessLog 的原始交换例外保持不变。

## 不变量

- 非对象、非法 JSON、缺失/空模型、非法 stream 和已有 affinity 字段类型继续在入口以相同协议错误拒绝。
- 同协议直通保留全部未知字段和原始嵌套值；通用协议层只允许模型替换、非流式 `stream` 删除与 Responses
  item ID 归一化。选中上游后只有独立 ADR 登记的 Provider Endpoint Profile 可以改写 attempt-owned
  Body；当前唯一例外是 ADR-0115 的 Codex OAuth Responses Profile。
- 原始 Body 只由共享引用持有；Attempt、replan 与直接 ProtocolExchange 不得修改它。Provider Endpoint
  Profile 需要改写时生成新的 attempt-owned Body，后续 Attempt 仍从原始共享 payload 开始。
- Bridge 物化只发生在已经选定的显式跨协议路径，不能反向让所有直通候选预付结构化内存。

## 后果

- 常见直通请求从“线协议 Body + Value 树 + 新 wire Body”缩减为一份共享 Body 和少量顶层索引；等待慢
  上游时的每请求常驻成本更接近正文大小，而不是 JSON 节点数量。
- 需要改写 Responses ID 的请求会短暂同时持有旧、新正文，但不会持有整棵历史 `Value`；改写完成后旧
  Bytes 立即释放。显式 Bridge 仍承担真实转换所需的结构化成本。
- 原始 JSON 直通不会自动执行递归 canonicalization。顶层重复字段不进入零拷贝直通，而是按与原
  `serde_json::Value` 路径一致的最终字段语义折叠后再发送，避免本地路由字段与上游解析产生歧义；嵌套
  非规范 JSON 仍保持其线协议值，客户端不应依赖重复字段的未定义解释。

## 验证

- Protocol 单元与 Registry 契约覆盖非法 JSON、非对象、模型/stream、affinity、远程压缩、未知字段、模型
  替换和非流式字段删除，并用指针相等证明无需改写的同协议 Body 共享原始分配。
- Responses 回归枚举全部已知 item 类型，覆盖合法/非法 ID、未知类型、嵌套 ID、非字符串 ID、非数组 input
  与多次编码不修改入口 payload。
- Responses → Chat Completions 契约覆盖 Raw JSON 的按需物化、首次请求和 continuation 恢复。
- 使用独立测试进程对 120,000 个 Responses message item、13,080,041-byte wire Body 和 1 MiB 审计捕获前缀
  各运行五次并取中位峰值 RSS；Darwin arm64 debug test profile 从改造前 `228,966,400` bytes 降到改造后
  `19,415,040` bytes，下降 `91.52%`（`11.79×`）。该一次性探针只用于前后对照，不进入长期测试套件。
