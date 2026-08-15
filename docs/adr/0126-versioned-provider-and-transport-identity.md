# ADR-0126: 版本化 Provider 应用身份与通用 Transport profile

- 状态：Accepted
- 日期：2026-08-10
- 相关决策：ADR-0130、ADR-0131、ADR-0149
- 决策者：maintainer

## 背景

Provider 数据面与 OAuth 额度面的固定 Header 散落在多个文件。Claude data 声称 `claude-code/2.1.220`，quota 却声称 `2.1.7`；Grok 在所有构建上都固定声称 `macos; aarch64`；Codex data 与 ChatGPT `wham` quota 各有一套未集中说明的 persona。同时所有 Provider 共用同一 Rustls/Hyper/Reqwest 线路行为，但该行为只以分散常量和依赖默认存在。

没有官方抓包基线时，为了“看起来不同”而随机化 UA、TLS 或 HTTP/2 只会创造新的不一致，也无法证明它代表任何原生客户端。可验证的目标是：Provider 应用身份内部一致、surface 差异有依据，通用 Transport 实现诚实且版本可审计。

## 决策

1. Codex、Claude 和 Grok 各自新增 Provider-local `identity` 模块，它是该 Provider data/quota 固定 Header 的唯一真相源。Driver 与 quota protocol 只能消费该 profile，不再复制版本、UA、client identifier、mode 或 surface persona。
2. Claude data/quota 统一为当前已审计值 `claude-code/2.1.220`。它是冻结 contract，不是“永远最新”声明；后续升级必须同时更新 data 与 quota fixture。
3. Grok 保留冻结的 client version 与 `grok-shell/<version> (os; arch)` wire 形状，但 `os/arch` 改为当前 Rust 构建目标。xAI Grok Build 主仓库的 `PlatformInfo::current` 同样读取 `std::env::consts::OS/ARCH`；因此不再在 Linux/x86_64 部署上伪报 macOS/ARM。data 与 quota 共用同一构造器。
4. Codex data fallback 与 `/backend-api/wham` quota 保留显式不同的子 profile。quota 的 `Codex Desktop`/beta/language/fetch/priority 字段属于已有内部 Endpoint 契约，本决策不在没有同 Endpoint capture 时猜测删改。ADR-0131 的 Codex 0.147.0 data capture 证明 `codex exec`、交互 TUI 与 Desktop override 使用不同 identity；因此单一入口值不能替换所有缺省/跨方言请求的 fallback。同方言客户端 persona 继续按 Header ownership 原样投影，固定差异必须在同一 identity 模块中可见。
5. OAuth token surface 继续使用各 Provider 的 client ID、grant 与 content type 契约，默认不借用 data-plane persona UA。如果某 Token Endpoint 必需固定 Header，必须以 Provider 证据追加到对应 identity profile，不能修改通用 form/json helper 污染其他 Provider。Kimi 仍无 persona Header。
6. Transport 使用单一 `generic-rustls-hyper-v3` wire profile，集中定义 policy version、ALPN、HTTP 版本、TCP/H2 keepalive、redirect、retry 和响应编码能力。Client cache key 必须包含该 profile 版本；依赖或 wire 策略改变必须提升版本并更新契约。
7. 该 Transport profile 是 any2api 通用 gateway 实现，不是任何 Provider 原生客户端模仿层。Provider Driver 不能选择 TLS/H2/TCP profile；除非未来有真实、公开且必需的 wire contract 并另立 ADR，Runtime/Transport 不新增 Provider `match`。
8. 禁止 any2api 为隐藏差异而另行随机 UA、cipher/extension 顺序、HTTP/2 SETTINGS 或时序。Rustls 0.23 自身按 `order_seed` 随机排列无顺序要求的 ClientHello extension，这是所选 TLS 栈的真实版本化行为，由 ADR-0130 的 capture contract 如实记录，不扩展为 Provider 模仿层。无官方 capture 时不声称“隐身”或“不可识别”；审计报告继续把 generic profile 标为上游可观测。

## 后果

- Claude data/quota 不再同时声称两个版本。
- Grok UA 的平台与实际构建目标一致，不再是全局固定 macOS/ARM。
- Codex surface 差异仍然可观测，但变为局部、显式且可契约测试的决策，不再是散落常量。
- Codex 0.147.0 的入口专属 persona 与当前 fallback 的差异已由独立基线确认；在缺少通用 fallback 语义时不以另一个入口硬编码覆盖。
- 所有 Provider 仍共用 generic Rust transport profile；这是已接受的上游可观特征，而不是伪造的原生客户端声明。

## 验证

- Provider 单元测试同时生成 data 与 quota headers，断言 Claude UA 一致、Grok version/identifier/mode 一致且 UA 使用当前 target OS/arch、Codex 两个子 profile 的差异精确冻结。
- OAuth token 契约测试继续断言 Provider 各自的 form/json 编码，且没有意外借用 data UA。
- Transport profile snapshot 断言 ID、policy versions、ALPN、HTTP 能力、keepalive、redirect、retry 和 response coding；现有 loopback TLS/H2 测试继续证明 isolation 域与物理连接行为。

## 依据

- xAI Grok Build `PlatformInfo::current` 与 UA 构造（核对 revision `e5478eff1e4050558e12e1328b85e6616632efb6`）：<https://github.com/xai-org/grok-build/blob/e5478eff1e4050558e12e1328b85e6616632efb6/crates/codegen/xai-grok-sampler/src/client.rs#L428-L480>
- xAI Grok Build billing Header（同 revision）：<https://github.com/xai-org/grok-build/blob/e5478eff1e4050558e12e1328b85e6616632efb6/crates/codegen/xai-grok-shell/src/extensions/billing.rs#L204-L229>
