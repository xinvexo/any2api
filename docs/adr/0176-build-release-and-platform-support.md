# ADR-0176: 完整应用构建与平台支持等级

- 状态：Accepted
- 日期：2026-08-22
- 当前事实：[构建与运维](../architecture/operations.md)
- 替代：[ADR-0170](0170-current-decision-register.md) 的构建与发布部分
- 被替代：无

## 背景

应用由 Rust 服务和 React 管理面组成。Cargo 仍需支持快速 Rust-only 工作流，而正式发布必须保证当前 Web 与
后端契约进入同一个产物。项目只发布 Linux AMD64，却曾在多个平台重复完整前端构建。

## 决策

根 Node tooling 是完整开发、构建和打包的组合入口；Cargo 保持 Rust-only，开发者可以直接运行任一层相关命令。
正式产物是内嵌 Web 的单一二进制，Release workflow 只发布 Linux AMD64 GNU。

普通 PR 在 Linux 完成必需验证和一次完整应用构建。macOS/Windows 作为开发/原生底层 best-effort 平台，在
主分支或定时任务验证 Rust，不重复完整前端构建。进程内更新遵守与官方发布相同的平台边界。

## 备选方案

- Cargo `build.rs` 自动调用 Node：破坏 Rust-only 构建、离线性和可预测性。
- 提交 `web/dist`：引入持久化产物和源码漂移。
- 所有平台每个 PR 完整构建：成本与官方支持等级不匹配。
- 分发 Web 旁车目录：增加部署文件和版本错配风险。

## 后果

正式二进制的前后端版本一致，Rust 开发循环不依赖 Node。非 Linux 问题可能稍晚在主分支/定时任务发现；在正式
发布其他平台前，需要提升相应 CI、打包、签名和更新支持等级。

## 验证

Linux 完整应用构建、生成绑定差异检查、Release 版本/归档/checksum 测试和非发布平台原生检查。
