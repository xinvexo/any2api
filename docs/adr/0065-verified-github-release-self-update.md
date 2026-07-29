# ADR-0065: 经校验的 GitHub Release 自动更新

- 状态：Accepted
- 日期：2026-07-29
- 决策者：maintainer

## 背景

any2api 以单个内嵌管理 Web 的 Rust 二进制发布，官方 Release 首版只有 Linux AMD64 GNU 资产。管理员
需要从 Web 查看版本、检查更新并完成更新，但不能让浏览器提供下载地址，也不能让未经校验的归档覆盖
正在运行的程序。更新后的进程还必须遵守已有请求 drain、后台任务收尾和单实例锁边界。

## 决策

- 新增独立 `updater` Adapter crate。它拥有 GitHub Release 查询、有界下载、checksum 校验、受限归档
  解包和可执行文件替换；`server` 只暴露管理 DTO/Handler，`app` 负责装配更新器和重启信号。
- 管理 API 提供 `GET /api/admin/about`、`POST /api/admin/update/check` 和
  `POST /api/admin/update/install`。三者都需要管理员会话；两个 POST 继续使用统一 CSRF 校验。
- 仓库固定为 `https://github.com/xinvexo/any2api`。检查操作读取最新正式 Release，要求 `v<SemVer>` Tag
  和 `any2api-v<SemVer>-linux-amd64.tar.gz`、同名 `.sha256` 同时存在，不接受客户端版本、仓库或 URL。
- 安装端重新检查最新 Release，不复用浏览器提交的结果。归档下载受字节上限约束，checksum 必须匹配；
  tar.gz 只允许唯一的根目录普通文件 `any2api`，拒绝额外成员、链接和路径穿越。
- 安装仅对 `x86_64-unknown-linux-gnu` release 构建和内嵌 Web 启用。检查和仓库链接在其他环境仍可用；
  Docker 不获得 Docker socket 或镜像管理能力。
- 新二进制先写到当前可执行文件同目录，设置可执行权限并 `sync_all`，随后以同文件系统 rename 原子替换。
  失败时删除暂存目录并继续运行旧二进制；任何步骤都不触碰数据目录或 SQLite Migration。
- 下载和校验仍可取消；进入最终解包后不再创建脱离请求生命周期的后台任务，也不在替换与重启请求之间
  增加等待点，避免请求取消后磁盘二进制已替换但当前进程没有重启。
- 替换成功后设置一次性重启信号。Axum 完成本次响应并按现有预算 drain，后台任务和存储完成收尾，Tokio
  runtime 关闭后，外层进程使用启动时捕获的路径和原参数 `exec` 新程序。若收尾失败则沿用致命退出，
  不以更新为由跳过资源生命周期边界。
- Web 不自动轮询或静默安装。管理员显式检查后看到最新版本和 Release 链接，有更新时再显式触发安装；
  页面不常驻展示环境能力，不支持的环境只在安装请求后提示，安装成功后提示服务正在重启。

## 后果

官方单二进制部署可以在管理面完成可验证更新，并保持进程 PID、启动参数和服务管理器关系。非官方平台
或开发模式不会被不匹配的 Release 覆盖。checksum 与 Release 同属 GitHub 信任边界，不替代代码签名；
若未来增加签名资产，应追加 ADR 扩展验证链，而不是弱化现有 SHA-256 检查。

## 验证

- Updater 单元测试覆盖 SemVer/资产选择、checksum、大小上限、额外归档成员和原子替换。
- 管理契约测试使用假 UpdateService 覆盖关于、检查、安装、错误映射和缺失服务。
- Web 契约与组件测试覆盖响应校验、有更新、安装请求后的环境不支持提示和重启提示。
- 发布工作流在构建前验证输入版本与 Cargo metadata 完全一致，并继续生成固定资产名和 checksum。
