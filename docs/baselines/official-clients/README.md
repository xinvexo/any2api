# 官方客户端上游基线

本目录保存独立于 any2api 自身 wire fixture 的官方客户端观测证据。JSON 只记录脱敏后的 HTTP/1 Header
顺序、Body 结构和 capture-specific hash，不保存原始请求正文、认证值、动态 UUID、时间戳或临时路径。

## 采集约束

1. 使用已安装发布物的明确版本与 SHA-256，不使用源码猜测或第三方转述。
2. 客户端进程使用清空的环境、独立临时 client home 和空工作目录；只恢复已记录的最小环境变量。
3. Base URL 指向一次性 `127.0.0.1` HTTP recorder，认证使用合成 Token；不访问真实 Provider 或账号。
4. recorder 按 wire 顺序读取 request line、Header 和 Content-Length Body，返回固定本地 400 后退出。
5. raw capture 只在临时目录中用于生成摘要和 SHA-256，核对后删除，不提交仓库。
6. 若宿主注入 originator、代理或认证变量，该次结果作废；必须清空环境后重采。

## 当前覆盖

| Provider | 客户端入口 | 版本 | 平台 | 操作 | 线路 | 日期 |
|---|---|---|---|---|---|---|
| Codex | `codex exec` | 0.147.0 | macOS 26.6.1 / arm64 | Responses | loopback HTTP/1.1 | 2026-08-11 |
| Codex | interactive TUI | 0.147.0 | macOS 26.6.1 / arm64 | Responses | loopback HTTP/1.1 | 2026-08-11 |

两条干净环境基线分别使用 `codex_exec` 与 `codex-tui` persona。继承 Codex Desktop 会话环境的试采因
`CODEX_INTERNAL_ORIGINATOR_OVERRIDE` 改写成 `Codex Desktop`，已经丢弃，不能作为独立 CLI 默认值。

这些基线只支持应用层 HTTP/1 与 JSON 结构比较。Claude、Grok、Kimi、Codex ChatGPT OAuth authority、
TLS、HTTP/2、其他平台和长期流仍未覆盖；不得从这里外推“官方客户端完全一致”或“不可识别”。

`cargo xtask architecture-check` 会校验所有 JSON 的 provenance、SHA-256、脱敏和采集策略。
