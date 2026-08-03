# ADR-0089：自更新的有界二进制回滚

- 状态：Accepted
- 日期：2026-08-03
- 决策人：项目维护者
- 修订：ADR-0065

## 背景

旧更新流程把校验后的候选直接 rename 到当前可执行路径，然后在优雅停机结束后 `exec` 该路径。旧 inode
没有目录项，候选也没有在目标主机执行过。只处理 `execve` 的同步返回并不能覆盖动态加载失败；一旦
`execve` 成功，新程序仍可能在 Migration、配置编译、端口绑定或 Worker 启动阶段退出，而且已经没有可恢复
的旧二进制。

常驻父进程或双槽部署可以监督任意时长的存活，但会改变单二进制 PID、服务管理器与停机模型。仅增加一个
`--version` 则只能验证动态加载和最小入口，不能证明真实应用启动。需要在不引入第二套进程管理系统的前提下，
明确应用可以可靠负责的回滚窗口。

## 决策

1. `any2api --version` 是唯一参数快路径，只向 stdout 输出 `any2api <build-version>` 后成功退出。它在读取
   启动环境、创建数据目录、获取实例锁、打开 SQLite 和创建 Tokio runtime 前完成；其他参数或组合一律
   Fail-Fast。更新器以清空环境、关闭 stdin/stderr、限制输出和十秒截止时间的子进程执行 staged 候选，
   只有退出码为零且 stdout 与目标 Release 精确一致才进入提交。
2. 更新状态只使用当前可执行文件的两个确定 sibling：`<executable>.update-pending` 和
   `<executable>.previous`。pending 是带格式版本和目标 SemVer 的内部所有权标记；previous 是当前可执行
   inode 的硬链接。创建使用 `create_new`/`hard_link`，任何已有目标都使安装 Fail-Closed，禁止猜测或覆盖
   来源不明的文件。
3. 提交顺序为：写入并同步 pending、硬链接 previous、同步父目录、原子 rename staged 到当前路径、再次
   同步父目录。rename 前失败清理本次状态且当前路径不变；rename 后的提交失败优先以 previous 原子恢复当前
   路径。整个解包、冒烟与该提交序列在进程 TaskTracker 跟踪的同一个 blocking closure 内运行，closure 同时
   拥有临时目录和终态回调；只有当前路径、previous 和 pending 形成完整状态后才在 closure 内请求重启，
   外层异步任务取消不能把替换与重启请求拆开。
4. 旧进程的 `exec` 立即失败时，以 previous 原子覆盖当前路径、清理 pending、同步目录，再执行恢复后的旧
   路径。新进程识别合法 pending，并在启动确认前持有恢复能力；构建版本与 pending 目标不一致时不尝试应用
   启动，直接恢复旧程序。内部 update-restart 标记允许参数、环境和非竞争性实例锁错误在完整 App 装配前
   进入同一恢复路径；若实例锁明确被另一个活进程持有，则不改写磁盘二进制，避免两个进程互相回滚。
5. 启动确认点位于启动参数和环境解析、单实例锁、SQLite Migration 与配置加载、运行时/Provider 编译、管理
   认证、Router 构造、listener bind、OAuth refresh Worker、affinity sweeper 与进程停机信号 handler 全部
   成功之后，进入 HTTP serve 之前。此前返回的可观察错误恢复并执行旧程序；确认时删除 previous，再删除
   pending 并同步目录。
   清理失败只告警并在下次成功启动重试，不能把已经就绪的新服务变成一次人为启动失败。
6. 这一机制不是 supervisor。staged 冒烟覆盖执行格式、动态加载器和目标主机共享库基线；应用自恢复覆盖
   进入 Rust 入口后到确认点的确定性错误。确认后的 panic/崩溃、无法运行任何用户代码的外部故障、`SIGKILL`、
   断电和文件系统损坏仍由 systemd、Docker 或其他部署层处理。Web 的健康版本确认是管理员 UX，不参与
   previous 生命周期，也不能延长应用内回滚窗口。
7. 回滚只恢复二进制，不逆向执行 Migration、不恢复配置或运行态。失败 Migration 应保持其事务语义；若新
   Schema 已成功提交而旧程序无法读取，previous 只能作为诊断/恢复材料，不能保证服务恢复。可能破坏上一
   正式版本读取能力的 Release 必须依靠发布说明、离线数据备份和部署层升级流程，不能把 Web 原地更新描述为
   数据库降级方案。

## 后果

- 候选在替换前已经实际执行，旧二进制在新版本真实启动完成前仍有稳定目录项。
- `execve` 同步失败与大多数启动期回归可以自动恢复，且不会引入常驻父进程、第二端口或持久运行态。
- 二进制目录会在一次更新窗口内多保留一份 inode 和一个小型标记；同名遗留状态会阻止下一次安装，避免覆盖
  可能仍有恢复价值的文件。
- “启动成功”成为后端明确事件，不再由浏览器在线与否决定；同时其时间边界和数据库局限被显式记录。

## 验证

- staged 候选分别覆盖正确版本、错误版本、非零退出、输出超限和超时；失败时当前二进制保持逐字节不变。
- 提交测试证明当前路径是候选、previous 是旧内容、pending 存在，且同名冲突不会被覆盖。
- 生命周期测试证明 blocking 提交不占用 Tokio worker；外层异步任务取消后 Tracker 仍等待该提交及其终态
  回调完整结束。
- 恢复测试覆盖不同 inode 与 hard-link 同 inode 两种中断状态；确认测试覆盖顺序清理及缺失 previous 的幂等
  收敛。
- App 集成测试证明 `--version` 不要求有效启动环境，未知参数在数据目录产生前失败；启动代码只有在 listener、
  必要 Worker 与停机信号 handler 成功后调用确认，并能在确认后立即优雅处理 SIGTERM。
