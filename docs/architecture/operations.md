# 构建与运维

本文是构建产物、支持平台、进程生命周期、自更新和内存底层组件的当前规范。安装命令、环境变量、反向代理
示例与公开 Release 操作见 [README](../../README.md)。

## 构建入口

Rust 与 React 可以独立开发；完整生产应用由根 Node tooling 组合：

- `pnpm dev` 协调 Vite 与可重建的 Rust 后端；
- `pnpm build` 生成管理 TypeScript bindings、构建临时 Web assets，再把该目录显式传给 Cargo；
- `pnpm package` 复用同一构建流程并产生版本化归档与 SHA-256。

这些是完整应用的便利入口，不禁止直接运行相关 Cargo、pnpm 或测试命令。Cargo 命令保持 Rust-only；
`build.rs` 只读取显式 Web asset 目录或内置的 Rust-only notice page，不启动 Node、不联网、不写源码树。

TypeScript binding 先生成到 target 下的临时目录，再与受版本控制目录做精确同步。构建脚本通过 Cargo JSON
artifact 找到可执行文件，不扫描时间戳或猜测固定 target 路径。

## 平台支持与 CI

官方预构建和进程内安装只支持 Linux AMD64 GNU。macOS 与 Windows 用于开发和底层平台代码验证，不承诺
官方安装包或同等级生产支持。

普通 PR 的必需检查集中在 Linux：Rust fmt/clippy/test、架构语义检查、Web typecheck/lint/test、完整应用构建
和浏览器关键路径。macOS/Windows 只在主分支或定时任务运行 Rust 原生检查，并以 best-effort 结果独立报告；
它们不重复完整前端构建。

## 运行与单实例

`app/any2api` 是唯一 Composition Root。启动顺序包括环境验证、数据目录权限、实例锁、Migration、配置加载与
编译、Runtime/Writer/管理员状态装配、监听器绑定和信号处理。准备完成前不对外宣称健康。

每个数据目录只允许一个进程。SQLite、WAL/SHM、实例锁和文件日志使用限制性权限；Windows 由部署者使用宿主
ACL 限制服务账号。没有内建在线备份或恢复协议，备份在停机后复制整个数据目录。

## 进程生命周期

运行期按 `Running → Draining → Forced` 收敛：

- Running 接受公开请求和允许的新后台任务；
- Draining 停止新工作，等待已跟踪请求、配置发布、日志 Writer 和必要更新阶段在宽限期内完成；
- Forced 取消仍可取消的 future，完成不可中断的本地文件/阻塞收尾后退出。

请求 Body、流式响应、后台任务和管理事件连接各自持有合适的 lifecycle Guard。长连接参与停机等待，但不能
永久阻止空闲内存回收。进程重启后 Runtime 状态从空开始。

## 内嵌 Web

正式应用把当前 Vite 输出编入单个可执行文件，并以预计算 MIME、ETag 和压缩变体提供。SPA deep link 回落到
内嵌 `index.html`；API 路径和资源路径不参与 SPA fallback。`ANY2API_WEB_DIR` 只用于明确的开发覆盖，不能
在生产时隐式读取工作目录中的旧 `web/dist`。

## 官方 Release 与自更新

Release workflow 的显式版本输入决定二进制版本、tag 和资产名；Cargo package version 只是 Workspace 元数据。
发布流程只上传 Linux AMD64 归档及 checksum。

管理员触发的更新固定访问官方仓库，重新解析最新稳定版本，下载固定平台资产，执行大小、checksum、归档结构
与 `--version` 冒烟验证，再原子替换同一路径可执行文件。旧文件只在新进程完成存储、配置、监听器和信号处理
初始化后清理；可观察的早期启动失败允许恢复旧二进制。

更新器只恢复二进制，不回滚 SQLite Migration，也不替代 systemd、Docker 或宿主健康策略。开发构建、外部 Web
目录和非官方平台可以检查版本，但不能进程内安装。Docker 部署通过替换镜像升级。

## 代理、超时和出站

Transport 支持 DIRECT、HTTP 和 SOCKS5。全局代理与对象专属代理在配置发布时解析为明确出口；专属代理失败
不回退。DNS、连接地址和重定向继续接受 SSRF/授权目标检查。客户端与上游读取使用按阶段超时；外层反向代理
必须为长时间无首字节的模型操作提供不少于 README 所述窗口。

## 内存底层组件

底层性能机制按测量结果取舍，不作为固定业务层级。当前保留两个窄边界：

- `payload-buffer` 只让至少 2 MiB 的连续大 payload 使用独立映射；普通对象继续使用通用分配器。代表性
  256 MiB 释放测试中，映射方案能让 RSS 立即回到基线，而 `Vec`/mimalloc 不回落，代价是该路径约慢 3 倍；
- `memory-reclaimer` 隔离平台堆压力释放。它只在无公共请求和无阻塞后台活动的稳定空闲 epoch 运行，不形成
  全局内存准入，也不承诺 RSS 回到冷启动值。

自定义 zstd workspace allocator 没有成立的 RSS 优势，并在 64 KiB、1 MiB、2 MiB、8 MiB 样本中分别比标准
安全 streaming 解码慢约 15%、12%、2%、3%，因此不再保留独立 crate 或 unsafe 分配边界。Server 使用有界的
标准 zstd streaming 解码，并继续负责压缩链深度、输出上限和错误收敛。

这些结论绑定当前负载与依赖版本；后续只有可重复的 RSS 或尾延迟证据足以抵消 unsafe、平台 CI 和维护成本时，
才引入新的专用分配机制。取舍理由见 [ADR-0175](../adr/0175-measured-memory-isolation.md)。

## 可观测与故障处理

console/file tracing、RequestLog、HTTP metadata 日志和管理员实时快照各有独立用途。日志 Writer 有界且不能把
遥测背压传给公开请求；详情安全边界见 [storage-and-security.md](storage-and-security.md#持久化遥测)。

健康端点只报告进程可服务和构建/实例身份，不暴露配置、Secret 或内部调度详情。受认证管理 API 提供当前资源
和运行态。外部 supervisor 根据进程退出、健康和部署策略处理重启；应用不实现复杂容灾或跨进程请求恢复。
