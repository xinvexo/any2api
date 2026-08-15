# ADR-0154：Codex memory 请求派生稳定 prompt_cache_key

- 状态：Accepted
- 日期：2026-08-15
- 决策者：maintainer
- 关联：ADR-0115（Codex OAuth Responses 出站 Profile）、ADR-0149（凭据无关请求面）、ADR-0151（Responses WebSocket 入口）

## 问题

Codex 桌面端每个回合会异步发起后台记忆提取（Memory Phase 1 与后续 consolidation）：固定约
30 KB 的 memory instructions 加上动态 rollout 历史，最多 8 个任务并发，模型与推理力度由
`memories.extract_model`/low 决定。Codex 客户端把这些请求的 `prompt_cache_key` 设为当前任务
的 session 标识——对普通回合正确（同一会话的增长前缀聚在同一分片），但对 memory 请求粒度
错误：真正跨请求稳定的是全局固定的 instructions 前缀，而 key 每个任务都换，固定前缀永远
冷启动，并发进一步放大 miss。

生产 24 小时实测（RequestLog）：60 条成功的 Luna low memory 请求只有 17 条读到缓存；总输入
约 231 万 Token，缓存读取仅约 12.5 万（5.4%）。命中与未命中均匀分布在全部 5 个 Luna 凭据上，
与账号无关；any2api 侧 `prompt_cache_key` 一直原值转发，未破坏任何标识。客户端不提供该 key
的配置项，网关是唯一能修正粒度的位置。

Codex 0.147 二进制确认请求携带结构化回合元数据：`client_metadata["x-codex-turn-metadata"]`
是 JSON 字符串，字段含 `request_kind`，枚举包含 `turn`、`memory`、`memory_consolidation`、
`compact`、`review`、`prewarm` 等。生产 memory 流量全部经 WebSocket 入口（ADR-0151），升级
后没有每请求 Header，因此 `x-openai-memgen-request` Header 不可作为判定信号；请求体的
`client_metadata` 在 HTTP 与 WebSocket 两条入口上形态一致。

## 决策

1. **默认规则成文**：`prompt_cache_key` 一律原值转发（含 Responses→Chat 桥接路径），本 ADR
   登记唯一例外。
2. **生效条件**（全部满足才改写，任一歧义即原样放行）：选中 Codex Driver 且实际上游操作为
   Responses——与凭据是 OAuth 还是 API Key、与公开模型名均无关；请求体
   `client_metadata["x-codex-turn-metadata"]` 为字符串且可解析为 JSON 对象，其
   `request_kind` 等于 `"memory"` 或 `"memory_consolidation"`；请求体 `instructions` 为非空
   字符串。请求体不是 JSON 对象、元数据缺失、类型不符、解析失败——一律不改写。误命中普通
   回合会把全部会话并到同一 key 上、冲击单 key 分片速率上限，因此宁可漏修不可误伤。
3. **key 派生**：`SHA-256("codex-memory/v1" \0 实际上游模型 \0 effort \0 instructions)`，
   `reasoning.effort` 缺失时以 `0x01` 哨兵字节代替，字段间以 `\0` 分隔防拼接歧义；取摘要前
   16 字节设置 UUIDv4 version/variant 位后格式化为 UUID 字符串。版本号在哈希原文内，换代改
   盐即可；对外形态与 Codex 自发的 session UUID 无差异，不携带网关标识（与 ADR-0149 保持
   正常客户端形态的姿态一致）。
4. **只写一个字段**：`prompt_cache_key` 存在则替换、缺失则补写；`instructions`、`input`、
   输出 Schema 与其余字段不动。该规则在 OAuth 出站 Profile（ADR-0115）之前独立应用，二者
   叠加时行为不变。
5. **与 ADR-0149 的一致性**：派生 key 是请求内容的确定函数，不含任何凭据身份；换凭据重试
   的 Attempt 仍发送完全相同的字节。Luna/Sol、max/low、不同 instructions 版本天然得到不同
   key；同一 memory 提示跨任务复用同一 key。

## 不做

- 不开会话粘性、不固定账号、不删除 key、不注入 retention。
- 不做 memory 请求串行化或单飞；先修 key，部署后仍有明显冷启动并发穿透再议。
- 不按 effort 推测凭据能力黑名单（独立议题，与本修复无关）。
- Chat Completions 桥接路径不适用该例外，bridge 继续原值转发。

## 后果

- memory 请求跨任务共享固定 instructions 前缀的缓存分片：覆盖率预期从 ~5% 抬到约 20% 保底
  （instructions ≈ 30 KB），同一 rollout 重复提取时深度命中；上限受限于 rollout 正文互不
  相同，不会接近普通回合的 93%+。
- 普通回合（`request_kind` 为 `turn`、缺失或不可解析）绝不改写；部署后普通回合命中率保持
  既有水平是误伤哨兵指标。
- 非 OAuth 的 Codex API Key 直通路径此前零改写；命中本规则的 memory 请求现在会经历一次
  字段级重写（键序规范化），字节仍逐 Attempt 确定。

## 验证

- 单测：固定输入的 golden key；同 instructions/model/effort 不同 session 得到同一 key；
  三元组任一变化 key 变化；`request_kind` 为 `turn`/缺失/畸形、`instructions` 缺失或为空
  时原样转发；缺 `prompt_cache_key` 时补写；API Key 上下文同样生效；OAuth 上下文与出站
  Profile 叠加后仍产出派生 key。
- 生产验证口径：RequestLog 过滤 memory 请求（Luna low），`cache_read_tokens/input_tokens`
  覆盖率应显著抬升且每请求出现 instructions 量级的保底命中；普通回合命中率不回退。
