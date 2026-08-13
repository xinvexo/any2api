# ADR-0149：恢复凭据无关的上游请求面以保住 prompt cache

- 状态：Accepted
- 日期：2026-08-13
- 决策者：maintainer
- 修订：ADR-0123、ADR-0125、ADR-0139 的数据面部分；废除 ADR-0148

## 问题

2026-08-11 上线的指纹边界批次（ADR-0123 按凭据隔离传输、ADR-0125 Credential-owned Header、随后
ADR-0139 的 Codex OAuth 请求 zstd 重压缩）使生产环境 Codex Responses 的 prompt cache 命中率从
93–96% 跌至 75–84%。生产 RequestLog 给出两条独立证据：

1. 换凭据重试的请求命中率从 97–98% 崩到 18–25%——ADR-0125 在换号 Attempt 上删除
   `session-id`/`thread-id` 等标识，上游按这些标识路由缓存分片。
2. 8/10 之前"切换账号"与"不切换"的命中率相同（93–96%），即上游缓存按会话标识与请求前缀路由、
   跨账号连续；8/11 起切换账号的请求跌到 ~75%，未切换保持 92%+——数据面按凭据拆分连接池
   （ADR-0123）打断了缓存路由的连接亲和。

结论：上游 prompt cache 不按账号隔离；任何"发往不同凭据的同一请求在字节或线路上不一致"的改动
都会直接兑换成缓存损失。ADR-0148 的软亲和只是对该自伤的症状补丁，同时削弱了按请求负载均衡。

## 决策

1. Codex/Claude/Grok 的会话、设备、请求关联与分布式追踪 Header（installation、session、thread、
   window、conversation、agent、request ID、`traceparent`/`tracestate`、Claude `x-stainless-*`
   前缀等）改回 `Replayable`：换 Credential 的 Attempt 照常投影。仅上游为特定账号签发或校验的值
   保持 `CredentialOwned`（`x-oai-attestation`、`anthropic-usage-limit`、
   `x-anthropic-additional-protection`）或 `BoundTurnState`（`x-codex-turn-state`）。
2. 数据面传输隔离改为单一共享身份 `TransportIsolationKey::shared_data_plane()`：所有凭据的推理
   请求在相同代理与线路 profile 下共享连接池。OAuth Token/Quota 控制面与诊断保持 ADR-0123 的
   按凭据代际隔离不变。
3. 停用请求正文 zstd 重压缩：所有上游请求发送 identity JSON。响应侧解码能力不变。
4. 删除 ADR-0148 的 prompt cache 局部性软路由（注册表、选路提示与生命周期挂钩），未绑定选择
   回到纯稳定轮询。会话粘性（ADR-0062/0064）语义不变。

## 后果

- 同一逻辑请求的两个 Authorization 会重新共享客户端 session/请求/追踪标识与物理连接；这是上游
  缓存路由的必要输入，属于已知且接受的可观测关联特征。审计口径相应回退。
- 负载均衡恢复为纯按请求轮询，无任何隐藏路由状态；缓存连续性由字节与线路一致性保证，不再依赖
  调度亲和。
- 凭据轮换不再触发数据面连接池换代（共享池无凭据代际）；认证材料本就逐请求注入，不受影响。

## 验证

- Provider 单元测试与 golden header 契约断言换号投影保留会话标识、attestation/turn-state 仍被删除。
- 契约测试断言换凭据重试携带同一 `x-client-request-id`/`x-codex-installation-id`/`traceparent`。
- `supports_encoding` 测试断言请求压缩全面关闭。
- 生产验证口径：部署后按 RequestLog 观察 `cache_read_tokens/input_tokens` 日命中率应回到 93%+，
  且"切换账号"与"未切换"两组命中率重新收敛。
