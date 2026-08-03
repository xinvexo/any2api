# ADR-0065: 经校验的 GitHub Release 自动更新

- 状态：Accepted
- 日期：2026-07-29
- 修订：2026-08-03
- 决策者：maintainer

## 背景

any2api 以单个内嵌管理 Web 的 Rust 二进制发布，官方 Release 首版只有 Linux AMD64 GNU 资产。管理员
需要从 Web 查看版本、检查更新并完成更新，但不能让浏览器提供下载地址，也不能让未经校验的归档覆盖
正在运行的程序。更新后的进程还必须遵守已有请求 drain、后台任务收尾和单实例锁边界。

## 决策

- 新增独立 `updater` Adapter crate。它拥有 GitHub Release 查询、有界下载、checksum 校验、受限归档
  解包和可执行文件替换；`server` 只暴露管理 DTO/Handler，`app` 负责装配更新器和重启信号。
- 管理 API 提供 `GET /api/admin/about`、`POST /api/admin/update/check`、
  `POST /api/admin/update/install` 和 `GET /api/admin/update/status`。四者都需要管理员会话；两个 POST
  继续使用统一 CSRF 校验。Install 只接受任务并返回 `202 Accepted`，Status 返回阶段、目标版本、下载
  字节进度或稳定失败码。
- 仓库固定为 `https://github.com/xinvexo/any2api`。检查操作读取最新正式 Release，要求 `v<SemVer>` Tag
  和 `any2api-v<SemVer>-linux-amd64.tar.gz`、同名 `.sha256` 同时存在，不接受客户端版本、仓库或 URL。
- 安装端重新检查最新 Release，不复用浏览器提交的结果。归档下载受字节上限约束，checksum 必须匹配；
  tar.gz 只允许唯一的根目录普通文件 `any2api`，拒绝额外成员、链接和路径穿越。
- GitHub Client 使用 10 秒连接超时和 30 秒无进展读取超时，后者在每次成功读取后重置。Release 元数据与
  小型 checksum 请求仍分别保留 15 秒、30 秒总时限；最大 128 MiB 的归档不设固定总下载时限，慢速但
  持续前进的下载不得在 300 秒被截断。连续 30 秒没有读取进展、任务被 Forced 取消、响应超过字节上限或
  最终大小不等于 Release 元数据时仍明确失败；取消不保存或恢复部分归档。
- 安装仅对 `x86_64-unknown-linux-gnu` release 构建和内嵌 Web 启用。检查和仓库链接在其他环境仍可用；
  Docker 不获得 Docker socket 或镜像管理能力。
- 新二进制先写到当前可执行文件同目录，设置可执行权限并 `sync_all`；随后以有界 `--version` 子进程验证
  动态加载、进程入口和编译版本与目标 Release 完全一致。提交使用更新器所有权标记与同目录
  `<executable>.previous` 硬链接保留旧 inode，目录同步后才以同文件系统 rename 原子替换。已有同名状态
  文件时 Fail-Closed，不覆盖来源不明的文件；提交前失败删除自身半成品并继续运行旧二进制。安装任务本身
  不触碰数据目录或 SQLite Migration。
- 临时目录命名固定为 `.any2api-update-<6 位 ASCII 字母数字>`。更新器初始化和每次安装前扫描可执行文件
  父目录，只删除目录及全部已知条目均由当前有效 UID 持有、且内容为空或仅包含普通文件
  `release.tar.gz` / `any2api.new` 的精确匹配项；不得跟随符号链接，也不得删除近似名称、其他类型、其他
  所有者或带未知条目的目录。初始化清理为告警式 best-effort；安装前已确认残留的清理 I/O 失败会终止
  本次安装。
- 管理员确认安装后，更新器原子创建至多一个进程内安装任务；Release 重检、下载、校验、替换和重启均由
  该任务持有，不再借用 Install HTTP 请求生命周期。异步下载任务与最终提交任务都通过唯一
  `ProcessLifecycle` 的后台 TaskTracker 注册，不得直接使用脱管的 `tokio::spawn`；进入 Draining 后拒绝接受
  新安装，Forced 时可以取消仍处于下载或 checksum 校验阶段的 future。
- checksum 通过后，异步任务不经过新的 `.await`，一次性把临时目录、归档/候选/当前路径、目标版本与终态
  回调移入 TaskTracker 的 blocking closure。该 closure 连续执行解包、权限与文件同步、候选冒烟、previous
  提交和原子替换；成功后的 `restarting` 状态与重启请求、预期错误后的 `failed` 状态也在 closure 内完成。
  已登记 closure 不因外层 future 被 Forced Drop 而取消，且在真正返回前始终保留 Tracker 计数；因此既不阻塞
  Tokio worker，也不会在磁盘提交成功和重启请求之间形成取消窗口。任务状态只在内存中保存，不跨进程恢复。
- 替换成功后设置一次性重启信号。Axum 完成本次响应并按现有预算 drain，后台任务和存储完成收尾，Tokio
  runtime 关闭后，外层进程使用启动时捕获的路径和原参数 `exec` 新程序。`exec` 立即失败时原子恢复
  previous 并执行旧程序。新程序只有在参数/环境、单实例锁、SQLite Migration、配置与 Provider 编译、
  管理认证、Router、listener、必要后台 Worker 及停机信号 handler 全部成功后才确认启动并清理
  pending/previous；确认前的可观察启动失败同样恢复并执行旧程序。若活动请求、受管后台任务、遥测或 SQLite
  等关键收尾失败则仍沿用致命退出，不以更新为由跳过资源生命周期边界；文件日志的有界 best-effort flush
  按 ADR-0090 不属于关键收尾，不能取消已经请求的重启。
- `--version` 在任何环境读取、SQLite、实例锁或 Tokio runtime 副作用前返回当前编译版本；未知参数和参数
  组合在同一边界 Fail-Fast。应用内回滚只覆盖启动确认前的二进制：不逆向执行 SQLite Migration，不承诺
  修复 `SIGKILL`、介质故障或确认后的崩溃，也不取代部署层 supervisor。完整边界由 ADR-0089 收紧。
- 公共 `/api/health` 增加当前运行中二进制的 `application_version` 并返回 `Cache-Control: no-store`。版本本身不是 Secret；这一窄字段允许
  浏览器在管理员会话随进程重启丢失后，确认响应者确实是目标版本，而不是把旧进程恢复或一次网络成功误判
  为安装成功。
- Web 不自动检查或静默安装。管理员显式检查后看到最新版本和 Release 链接，有更新时再显式触发安装；
  页面不常驻展示环境能力，不支持的环境只在安装请求后提示。安装一经触发即覆盖为不可关闭的全屏模态状态：
  下载显示确定进度，随后显示安装和重启阶段；服务端明确失败时提供重新安装或返回。若连续 90 秒无法取得
  活动更新状态且健康响应也不是目标版本，则进入不宣称成功/失败的“无法确认”状态，提供继续等待或返回；
  继续等待只恢复轮询，不重复提交安装。Web 观察到精确目标版本健康响应后短暂显示完成并自动刷新。浏览器只
  在 `sessionStorage` 保存预期目标版本，用于误刷新后恢复锁定界面；明确失败或无法确认时清除，使刷新能够
  解锁。下载进度和任务状态始终重新读取服务端内存状态，不持久化到浏览器。详细状态机见 ADR-0091。

## 后果

官方单二进制部署可以在管理面完成可验证更新，并保持进程 PID、启动参数和服务管理器关系。非官方平台
或开发模式不会被不匹配的 Release 覆盖。checksum 与 Release 同属 GitHub 信任边界，不替代代码签名；
若未来增加签名资产，应追加 ADR 扩展验证链，而不是弱化现有 SHA-256 检查。

更新器在启动窗口内保留上一版二进制，候选至少已经证明能在目标主机执行版本快路径；这能恢复 `exec`
立即失败和进入应用后、确认前的大多数确定性启动失败。它不是常驻监督器，也不是数据库降级系统；跨 Schema
回滚能力必须由 Release 的 Migration 设计和部署备份保证，不能从 previous 文件的存在推导出来。

`SIGKILL`、断电或 OOM 遗留的 archive/候选工作区会在后续启动或安装前收敛，同时严格命名、所有权和
内容检查避免把相邻部署文件当作更新残留删除。

## 验证

- Updater 单元测试覆盖 SemVer/资产选择、checksum、大小上限、额外归档成员、`--version` 超时/版本拒绝、
  previous/pending 提交、启动确认清理和原子回滚。
- 本地流式 HTTP 测试覆盖总时长超过旧总 deadline 比例但每次读取持续前进时成功，以及单次读取停滞超过
  无进展时限时返回 `DownloadFailed`；既有大小上限和精确最终大小断言继续生效。
- 临时工作区测试覆盖精确残留清理，以及近似名称、普通文件、符号链接、所有者不匹配和未知内容均保持不变。
- Updater 状态测试覆盖单任务准入、请求返回后任务继续、下载进度单调、失败状态、Draining 拒绝新安装、
  blocking 提交内的成功/失败终态与替换后的重启请求。App 生命周期测试使用受控阻塞点证明提交 closure 不占用
  Tokio worker，外层更新 future 在 Forced 后收敛而 closure 仍由 Tracker 持有，释放阻塞点后才完整结束。
- 管理契约测试使用假 UpdateService 覆盖关于、检查、任务接受、状态、错误映射和缺失服务；健康契约验证
  运行版本字段。
- Web 契约与组件测试覆盖响应校验、有更新、不可关闭的全屏阶段、真实字节进度、失败出口、目标版本健康
  确认与自动刷新，以及连续不可达后“继续等待/返回”恢复且不重复安装。
- 发布工作流验证输入是稳定 SemVer，并用它生成 Tag、固定资产名、checksum 以及编译进二进制的正式版本；
  Cargo package version 不限制也不参与产品版本，本地开发构建固定使用 `0.0.0-dev`。
- App 进程集成测试验证 `--version` 不启动服务且非法参数 Fail-Fast；启动装配在 listener、必要 Worker
  与停机信号 handler 成功后显式确认更新，确认前错误路径触发 previous 恢复。
