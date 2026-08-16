# ADR-0154：Codex memory 请求稳定 Prompt Cache

- 状态：Accepted
- 日期：2026-08-15
- 决策者：maintainer
- 关联：ADR-0115（Codex OAuth Responses 出站 Profile）、ADR-0149（凭据无关请求面）、ADR-0161（移除 Responses WebSocket 入口）

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
`compact`、`review`、`prewarm` 等。Responses WebSocket 入口已移除，memory 请求统一经 HTTP/SSE
入口；`x-openai-memgen-request` Header 不作为判定信号，继续使用请求体中的 `client_metadata`。

## 决策

1. **默认规则成文**：`prompt_cache_key` 一律原值转发（含 Responses→Chat 桥接路径），本 ADR
   登记唯一例外。稳定 key 只解决 cache routing/grouping；GPT-5.6 memory 还必须显式标出固定
   instructions 的缓存边界。
2. **生效条件**（全部满足才改写，任一歧义即原样放行）：选中 Codex Driver 且实际上游操作为
   Responses——与凭据是 OAuth 还是 API Key、与公开模型名均无关；请求体
   `client_metadata["x-codex-turn-metadata"]` 为字符串且可解析为 JSON 对象，其
   `request_kind` 等于 `"memory"` 或 `"memory_consolidation"`；请求体 `instructions` 为非空
   字符串；实际上游模型必须是支持该能力的 GPT-5.6 系列（当前按 `gpt-5.6` 或
   `gpt-5.6-*` 识别）。请求体不是 JSON 对象、模型不在能力范围、元数据缺失、类型不符、解析
   失败，或待重写字段不是可表达的 JSON 形状——一律不改写。误命中普通回合会把全部会话并到
   同一 key 上、冲击单 key 分片速率上限，因此宁可漏修不可误伤。
3. **key 派生**：`SHA-256("codex-memory/v1" \0 实际上游模型 \0 effort \0 instructions)`，
   `reasoning.effort` 缺失时以 `0x01` 哨兵字节代替，字段间以 `\0` 分隔防拼接歧义；取摘要前
   16 字节设置 UUIDv4 version/variant 位后格式化为 UUID 字符串。版本号在哈希原文内，换代改
   盐即可；对外形态与 Codex 自发的 session UUID 无差异，不携带网关标识（与 ADR-0149 保持
   正常客户端形态的姿态一致）。
4. **固定前缀的 Responses wire 形态**：Responses 顶层 `instructions` 字符串本身不能承载
   `prompt_cache_breakpoint`。因此只在目标请求中移除该顶层字段，并把其原始字符串语义等价
   转换为 `input` 的首个 developer message：

   该形态遵循 OpenAI 的 [Prompt Caching 指南](https://developers.openai.com/api/docs/guides/prompt-caching/)
   与 [Responses Create schema](https://developers.openai.com/api/reference/resources/responses/methods/create/)。

   ```json
   {
     "input": [
       {
         "type": "message",
         "role": "developer",
         "content": [
           {
             "type": "input_text",
             "text": "<固定 memory instructions>",
             "prompt_cache_breakpoint": {"mode": "explicit"}
           }
         ]
       },
       {"type": "message", "role": "user", "content": "<动态 rollout>"}
     ],
     "prompt_cache_key": "<稳定 UUID>",
     "prompt_cache_options": {"mode": "explicit"}
   }
   ```

   原有数组 `input` 项按原顺序置于 developer message 之后；原有字符串 `input` 等价包装为
   user message。breakpoint 永远位于固定 instructions 的最后一个稳定 content block，而不是
   动态 rollout 后。`prompt_cache_options` 缺失时补写，已有对象（例如合法的 `ttl: "30m"`）
   保留其他字段但确定性地覆盖 `mode` 为 `explicit`；非对象或无法解析的值 fail-closed。
5. **改写顺序与边界**：稳定 cache rewrite 在 OAuth 出站 Profile（ADR-0115）之前执行，API Key
   与 OAuth 都得到同一 memory 前缀；Profile 只做其既有兼容规范化，不移动 breakpoint。HTTP
   Responses 入口经过这条共享准备路径；Responses→Chat 桥接的实际
   upstream operation 不是 Responses，因此继续原值转发。
6. **与 ADR-0149 的一致性**：派生 key 和 wire 重写都是请求内容的确定函数，不含任何凭据身份；
   换凭据重试的 Attempt 仍发送同一语义和同一 key，不引入 session sticky routing。Luna/Sol、
   max/low、不同 instructions 版本天然得到不同 key；同一 memory 固定提示跨任务复用同一 key。

## 不做

- 不开会话粘性、不固定账号、不删除 key、不注入 retention。
- 不做 memory 请求串行化或单飞；先修 key，部署后仍有明显冷启动并发穿透再议。
- 不按 effort 推测凭据能力黑名单；只按实际上游 GPT-5.6 模型能力边界启用 explicit wire 形态。
- Chat Completions 桥接路径不适用该例外，bridge 继续原值转发。

## 后果

- memory 请求跨任务共享固定 instructions 前缀的缓存分片：稳定 key 提供路由/分组，显式
  breakpoint 再把约 30 KB instructions 固定为可复用前缀；同一 rollout 重复提取时可继续深度
  命中。上限仍受 rollout 正文互不相同影响，不会接近普通回合的 93%+。
- 普通回合（`request_kind` 为 `turn`、缺失或不可解析）绝不改写；部署后普通回合命中率保持
  既有水平是误伤哨兵指标。
- 非 OAuth 的 Codex API Key 直通路径此前零改写；命中本规则的 memory 请求现在会经历一次
  字段级重写（顶层 instructions 转换、input/prompt_cache_options/prompt_cache_key 键序
  规范化），语义和动态 rollout 保持不变，字节仍逐 Attempt 确定。

## 验证

- 单测：固定输入的 golden key；同 instructions/model/effort 不同 session 和 rollout 得到同一
  key；三元组任一变化 key 变化；两个 rollout 的 developer instructions block 与 breakpoint
  完全一致、动态 input 仍各自不同；目标请求包含 `prompt_cache_options.mode=explicit`；
  `request_kind` 为 `turn`/缺失/畸形、模型不支持、`instructions` 缺失或为空时原样转发；缺
  `prompt_cache_key` 时补写；已有 key/options 的覆盖与保留行为确定；API Key 上下文同样生效；
  OAuth 上下文与出站 Profile 叠加后仍产出相同 wire 语义。
- 生产验证口径：RequestLog 过滤 memory 请求（Luna low），`cache_read_tokens/input_tokens`
  覆盖率应显著抬升且每请求出现 instructions 量级的保底命中；普通回合命中率不回退。
