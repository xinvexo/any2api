# ADR-0165: 构建时自动重新生成内嵌 Web 资源

- 状态：Accepted
- 日期：2026-08-18
- 决策者：maintainer

## 背景

`web/src` 是前端源码真相，但 Vite 会为每次构建生成带内容哈希的文件名。若 CI 只执行普通
`pnpm build`，再把 `web/dist` 与 checkout 中的 `app/any2api/web-assets` 做只读比较，任何前端改动
都会在构建完成后因为旧快照失败。CI 也不应为了通过检查而自动提交或推送工作树变更。

## 决策

- `cargo xtask package` 始终从当前 Web 源码执行生产构建、同步 `web-assets`、复核同步结果，再编译 Rust
  release 二进制；不再提供只读 `--check-assets` 打包模式。
- GitHub CI、E2E 和 Release 在 Rust 编译前使用同一自动同步流程。生成目录可以在 checkout 中保留最近快照，
  但构建不得把快照是否与源码提交完全一致当作门禁，也不要求每次前端变更同时提交生成文件。
- `pnpm check:embedded` 保留为开发者显式诊断；它不修改工作树，也不作为 CI/Release 的独立阻断步骤。
- Rust `build.rs` 继续只读取构建阶段已经生成的 `web-assets`，不启动 Node、pnpm、Vite，不把前端构建
  隐式塞入普通 Cargo 编译。需要当前前后端源码的二进制必须走 `cargo xtask package`。

## 后果

- GitHub 构建使用当前提交的前端源码，不会因为哈希文件名或旧嵌入快照而在构建末尾失败。
- 本地 `cargo build` 仍是 Rust-only 入口，使用 checkout 中最近一次生成快照；完整应用构建统一使用
  `cargo xtask package`。
- CI 工作区会在构建过程中产生生成文件，但不会尝试将它们写回分支；发布产物直接使用本次构建生成的资源。

## 验证

- Web CI 运行 `pnpm build:embedded`，不再运行独立的 `check:embedded` 门禁。
- Release 运行 `cargo xtask package --target x86_64-unknown-linux-gnu`。
- `cargo xtask package` 在同步后保留一次只读复核，确保最终嵌入目录完整且无非法文件。
