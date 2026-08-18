# ADR-0166: Node 统一拥有开发、构建与打包生命周期

- 状态：Accepted
- 日期：2026-08-18
- 决策者：maintainer
- 取代：先前的构建时 Web 资源同步决策

## 背景

any2api 是 React/TypeScript 与 Rust 组成的单体应用。此前开发、Web 构建、内嵌资源同步、Rust 编译、
归档和校验和分散在 Web package script、Node 脚本、Rust xtask 与 GitHub workflow 中。相同的完整
应用构建被多处手写，CI 可以先成功构建当前前端、再因为仓库中的旧资源快照而失败；普通 Cargo 测试
还会隐式改写前端 TypeScript 生成目录。

普通 Cargo 构建也缺少明确的 Web 资源新鲜度契约：构建脚本只能看到某个目录存在，无法证明它来自
本次 Vite 构建。开发脚本则在源码变化时先杀死正在运行的后端，再启动新的编译；编译失败会让整个
开发服务消失，密集变化还可能产生重启竞争与遗留进程。

应用级生命周期需要一个跨平台编排所有阶段的所有者，同时保持 Vite、Cargo 与最终 Rust 可执行文件
各自清晰的职责。

## 决策

### 所有权与公开命令

- 仓库根的 pnpm/Node 工具是应用级生命周期的唯一所有者，公开入口收敛为 `pnpm dev`、
  `pnpm build` 和 `pnpm package`。
- Vite 只拥有前端开发服务器、HMR 与前端生产构建；Cargo 只拥有 Rust 编译、测试与目标平台产物；
  编译完成的 Rust 可执行文件只拥有运行时服务。
- Rust 构建脚本不得启动 Node、pnpm 或 Vite，不得联网或修改工作树。Rust 辅助工具不得反向编排
  完整应用生命周期。
- Node 的内部阶段模块可以被 E2E 与 CI 复用，但不是新的平行公开入口。任何阶段失败都立即终止当前
  build/package，不得复用不明来源的旧产物伪装成功。

### DEV 生命周期

`pnpm dev` 同时启动 Vite 与一个 Node 管理的 Rust 构建/运行监督器：

- 前端源码变化只由 Vite HMR 处理，不触发 Rust 重编译；Rust 工作区、Migration 与相关构建配置变化
  经过 debounce 后推进单调 epoch。
- 任意时刻最多运行一个 Cargo build。编译期间到达的新变化只标记当前结果过期并合并到下一轮；过期
  的成功结果不能替换运行实例。
- 最新 epoch 编译成功后，监督器通过 Cargo JSON 消息取得实际可执行文件，先准备新 runtime，再有界
  停止旧 runtime 并替换。编译失败时保留上一个成功 runtime；初次编译失败时仍保留 Vite、监视器与
  后续修复机会。
- 后端意外退出不进入无界 crash loop，等待下一次 Rust 源码变化；Vite 意外退出表示前端开发面已失效，
  整个开发会话以非零状态结束。
- POSIX 使用独立进程组终止子树，Windows 使用等价的进程树终止；正常退出、信号、编排异常与强制超时
  都走同一幂等清理路径。停止旧 runtime 后必须等待其进程树退出，不能依赖固定 sleep 猜测端口释放。

### TypeScript bindings 生命周期

- 管理 API 线格式仍由 Rust DTO 通过 ts-rs 生成，但只有一个带 `ignored` 标记的显式导出测试负责枚举
  全部根 DTO 并导出。
- Node 在 DEV 初始化、相关 DTO 输入变化和完整 BUILD 前调用该导出器。导出先写入全新临时目录，再与
  `web/src/shared/api/generated/` 做 exact sync：增加新文件、只更新内容变化的文件并删除失效文件。
- 普通 `cargo test`、`cargo nextest`、`cargo check` 与 `cargo build` 不得写入前端源码树；生成目录和整数
  映射环境只由显式 Node 阶段传入。
- 生成的 TypeScript 文件继续提交，用于代码审查和无需 Rust 的前端类型检查；它们不是生产 Web bundle。

### BUILD 与内嵌资源契约

`pnpm build` 完成一次当前源码的完整应用构建：

1. 显式生成并 exact sync TypeScript bindings；
2. 让 Vite 直接输出到 `target` 下的新临时 staging，不经过源码树中的生产资源目录；
3. 只接受普通文件和规范化相对路径，要求 `index.html` 存在，为每个文件记录字节数和 SHA-256，并按
   排序后的条目计算 bundle digest；
4. 将资源根与版本化 manifest 原子发布到内容寻址目录；
5. 通过显式环境变量把本次 manifest 路径传给 Cargo，并从 Cargo JSON 编译消息解析真实可执行文件。

manifest 至少包含 schema 版本、bundle digest，以及按路径排序的 `{ path, size, sha256 }`。Rust
`build.rs` 在环境变量存在时统一校验 manifest、资源集合、大小、摘要、`index.html` 与文件类型，再生成
`include_bytes!` 清单；缺失、篡改、额外文件或非法路径都必须让编译失败。内容寻址目录让并发或增量构建
可以复用已验证 bundle，又不会把部分写入误认为完成产物。

生产 HTML/JS/CSS 不提交到应用源码树，也不在构建时改写 Git 工作区。`web/dist` 只可以作为独立前端
命令的临时输出，不是完整应用构建的输入或真相来源。

不带 manifest 的普通 Cargo-only 构建使用仓库中明确维护的最小 `rust-only` 占位页，并打印构建警告。
该页面必须清楚说明当前二进制不含管理 Web；Cargo 不得静默嵌入上次完整构建留下的 bundle。这样 Rust
质量门禁无需 Node，同时不会把陈旧资源误表述为当前前端。

### PACKAGE 与平台产物

- `pnpm package` 直接复用同一个 `buildApplication` 实现，不通过 shell 再调用另一个公开 build 命令，
  然后才执行分发归档与 SHA-256 checksum。
- host 或显式 `--target <triple>` 在入口处解析一次，形成包含 Cargo target、操作系统、架构、可执行文件
  后缀、分发标签和是否原生的 descriptor；后续构建与归档只消费该 descriptor。
- 可执行文件路径来自 Cargo JSON `compiler-artifact`，不得拼接固定 `target/release` 路径。原生构建运行
  `--version` 校验成品；跨平台构建无法执行时保留显式版本和产物结构校验。
- BUILD 只产出完整应用二进制。PACKAGE 额外在独立 `dist` 中生成稳定命名的归档和同名 checksum，归档
  根只包含最终可执行文件并保留可执行权限。Linux AMD64 继续使用更新器约定的
  `any2api-v<version>-linux-amd64.tar.gz` 与 `<archive>.sha256`。
- 签名、公证、安装器和多平台矩阵是 PACKAGE 之后的可扩展分发阶段，不进入本次核心构建抽象。

### CI、Release 与 E2E

- Rust-only 质量 job 可以直接运行 Cargo，并验证明确的占位资源路径；它不宣称产出当前完整应用。
- Web job 运行 TypeScript、lint、单元测试和独立 Vite build。完整应用 job、浏览器 E2E 与 Release 从
  仓库根调用 Node 生命周期，不复制资源准备或 Cargo artifact 解析逻辑。
- Release 只负责校验 tag/version、安装平台工具、调用 `pnpm package` 并上传 `dist`；归档和 checksum
  由 PACKAGE 阶段生成。
- Playwright E2E 使用共享的应用 build primitive 构建真实二进制，从独立临时工作目录启动且不设置
  外部 Web 目录，以验证本轮 manifest 对应的内嵌资源。

## 备选方案

- 在 `build.rs` 中调用前端工具：会让普通 Cargo 隐式依赖 Node、允许构建脚本联网或改写工作区，拒绝。
- 提交最近一次完整 Web bundle 供 Cargo 回退：无法证明与当前源码一致，仍会产生哈希文件 churn 和
  “成功嵌入旧 UI”的假象，拒绝。
- 由 Rust xtask 统一调用 Vite 与 Cargo：会反转工具所有权，并让跨平台进程管理和 pnpm workspace
  生命周期落入 Rust 辅助工具，拒绝。
- DEV 每次变化先终止旧后端再编译：实现简单但编译错误会中断可用服务，密集变化会放大竞态，拒绝。
- PACKAGE 复制一套独立 build 流程：容易让本地 build、E2E 和 Release 漂移，拒绝。

## 后果

- 开发者在仓库根获得三个语义明确的入口；前端 HMR、Rust 热重载与完整分发不再互相冒充。
- 完整构建总能证明其 Web bundle 来自本轮源码，普通 Cargo 则明确表明自己只有占位 Web。
- 生产资源不再造成源码树 churn；代价是完整 BUILD 需要 Node/pnpm，且构建工具需要维护 manifest 校验、
  Cargo JSON 解析和跨平台进程树收尾。
- bindings 的更新成为显式、可复现的生成阶段；普通 Rust 测试恢复为无源码副作用。
- “保留生成快照并在构建时同步”的旧决策被完整取代；ADR-0027 和 ADR-0053 按本决策修订。

## 验证

- Node 单元测试只覆盖需要状态协调的 DEV 合并/部署行为和实际复现过的进程树清理。
- `pnpm build` 的真实执行验证 bindings、Vite、manifest、Cargo JSON 产物与 standalone 内嵌首页；
  `pnpm package` 的真实执行验证归档名称、内容和 checksum，不在生产流程中重复解读刚生成的归档。
- Cargo-only、Web-only、E2E 与 Release 分别从各自公开边界验证；不为同一个内部步骤在多层复制测试。
