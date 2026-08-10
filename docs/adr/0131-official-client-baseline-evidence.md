# ADR-0131: 官方客户端基线的证据与脱敏契约

- 状态：Accepted
- 日期：2026-08-11
- 决策者：maintainer

## 背景

ADR-0130 已冻结 any2api 自身的 TLS、HTTP/2、HTTP/1 与全部 Registry surface，但这些 fixture 只能证明
gateway 的实际行为，不能回答它与某个官方客户端版本是否相同。审计剩余项要求建立带 Provider、客户端
版本、平台、操作和日期的独立基线。

第一次直接运行 Codex 0.147.0 时，进程继承了 Codex Desktop 注入的
`CODEX_INTERNAL_ORIGINATOR_OVERRIDE`，请求因此显示 `originator: Codex Desktop`。清空宿主环境后重复采集，
`codex exec` 改为 `codex_exec`，交互 TUI 则为 `codex-tui`。这证明宿主环境和入口形态本身就是基线变量；
未隔离的抓包不能作为通用身份依据。

Codex 请求 Body 还包含内置指令、工具说明、临时路径和动态 item/session ID。提交完整 raw capture 会扩大
不必要的信息面，也不能形成跨运行稳定的比较对象。

## 决策

1. 官方客户端采集是显式的 maintainer evidence workflow，不进入 CI，也不访问真实 Provider。目标固定为
   loopback HTTP recorder，认证只使用合成 Token，client home 与工作目录使用独立临时目录。
2. 启动客户端前清空继承环境，只恢复被记录的最小变量。认证、代理、真实 client home、
   `CODEX_INTERNAL_ORIGINATOR_OVERRIDE` 等宿主状态不得进入采集；若客户端入口需要 `TERM`，其值必须写入
   capture metadata。
3. 每份机器可读基线必须记录 schema version、Provider、客户端 product/entrypoint/version、发布物
   SHA-256、distribution、OS/version/build/arch、采集日期、operation、network/auth/environment policy、
   request/body capture hash 和明确局限。
4. Header 按 raw HTTP/1 收到顺序保存，但认证、authority、Content-Length 以及设备/会话/线程/请求关联值
   只保存语义 placeholder。结构化 metadata 只保存字段名和稳定枚举，不保存 UUID、时间戳或原值。
5. Body 不提交原文。基线保存 capture-specific SHA-256、字节数、wire 顶层字段顺序、关键协议字段、
   input item 形状和 client metadata 字段名；hash 用于确认该次证据，不要求不同运行相等。
6. 首批基线固定 Codex 0.147.0、macOS 26.6.1 build 25G76、arm64、Responses、loopback HTTP/1.1，
   分别覆盖 `codex exec` 和交互 TUI。两者共同确认 Header 顺序、Responses 字段形状和入口专属
   originator/UA；不覆盖 ChatGPT OAuth authority、TLS、HTTP/2、其他平台或长期流行为。
7. 基线观测不自动修改生产 persona。当前证据显示 `codex exec` 与 TUI 使用不同身份，因此不能选择其中
   任意一个作为所有缺省/跨方言请求的通用伪装。同方言客户端提供的 Header 继续按 ADR-0125 的
   ownership 规则投影；Provider fallback 只有在取得匹配 surface 的证据或明确改为 gateway identity 时才
   更新。
8. 不为缩小差异新增每请求随机 UA、TLS 参数、HTTP/2 SETTINGS 或时序。基线用于发现协议正确性、版本漂移
   和 ownership 缺口，不用于声称“不可识别”。

## 后果

- Codex 的两个官方入口已有可追溯、无真实账号参与的 HTTP/1 Responses 证据。
- Desktop originator override 不再被误认为独立 CLI 默认值。
- 当前 `codex_cli_rs/0.145.0` fallback 与 0.147.0 入口身份的差异成为已确认、显式保留的边界，而不是在
  缺少跨入口语义时盲目替换为另一个错误 persona。
- Claude、Grok、Kimi、Codex OAuth/TLS/H2 与其他平台仍需要各自独立基线，不能由本次 Codex H1 结果外推。

## 验证

- `cargo xtask architecture-check` 枚举 `docs/baselines/official-clients/*.json`，拒绝缺失元数据、非法
  SHA-256、非 loopback/合成凭据/清空环境策略、未脱敏认证 Header 和空局限列表。
- Codex Header 单元测试覆盖本次观测到的 replayable capability Header 与 Credential-owned
  window/turn/request/session/thread Header，并证明换 Credential 后不会重放关联值。
- Codex OAuth request Profile 单元测试以本次观测的顶层字段形状证明合规 Body 保持原 allocation，
  `reasoning.context`、`text.verbosity`、`prompt_cache_key` 与 `client_metadata` 不被错误删除或重编码。
