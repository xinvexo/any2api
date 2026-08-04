# ADR-0108：公共健康响应只保留状态与应用版本

- 状态：Accepted
- 日期：2026-08-04
- 决策者：maintainer
- 修订：ADR-0065

## 背景

`GET /api/health` 不要求管理员会话，既作为部署层存活探针，也被 Web 自更新流程用于在管理会话随重启失效后
确认目标二进制版本。现有响应除 `status` 和 `application_version` 外，还返回配置 revision、调度 epoch、
停机阶段、活动 HTTP 请求数和受管后台任务数。

全仓消费者审计表明，自更新只读取 `application_version`，存活探针只需要成功状态；但系统总览确实显示活动
HTTP 请求数与受管后台任务数，并通过公共健康端点每 15 秒读取。配置 revision 与调度 epoch 已由受认证的
`GET /api/admin/balancing` 返回，配置写接口也返回 revision。停机阶段只用于进程内收尾与结构化停机日志；进入
Draining 后 listener 会停止接受连接，因此 HTTP 端点不能形成可靠的停机观测协议。

这些内部值不是 Secret，但远程部署公开健康端点后，外部调用者可以高频推断配置变更、调度/健康变化和当前负载。
没有产品消费者时继续暴露它们没有对应收益。

## 决策

1. 公共 `GET /api/health` 的 JSON 契约精确为 `status` 与 `application_version`。`status` 在 Handler 能返回时固定为
   `ok`；版本继续使用编译进当前二进制的产品版本。
2. 端点继续返回 `Cache-Control: no-store`，不增加认证、重定向、缓存验证或更新专用旁路。Web 更新确认仍只在
   `application_version` 精确等于本次目标版本时成功。
3. 删除公共响应中的 `config_revision`、`scheduler_epoch`、`shutdown_phase`、`active_requests` 和
   `background_tasks`。前两项继续由受认证的 Balancing DTO 返回；活动请求与后台任务作为固定大小的 `process`
   汇总迁入同一 DTO，系统总览与调度区复用一个 Query cache。停机阶段没有展示消费者且经 HTTP 不可可靠观测，
   只由 `ProcessLifecycle` 和停机日志内部使用。
4. 健康 Handler 不再读取 `AppState`、PublishedSnapshot 或 Runtime。未来若出现真实的管理诊断需求，应单独定义
   受认证、有界且语义稳定的 DTO，不能把 TaskTracker 长度或调度唤醒计数重新塞回公共存活协议。

## 后果

部署探针和自更新确认行为不变，公共响应不再提供可用于推断配置变化、调度活动或瞬时负载的内部计数。配置、调度
与总览所需进程计数通过管理员 Balancing API 读取；两个总览区从两次轮询收敛为一次受认证查询。停机收尾继续
依赖服务端结构化日志和进程退出结果。

这是新项目内部协议的直接收窄，不保留旧字段兼容层。客户端若错误依赖未承诺的内部字段会明确失败，而不是得到
长期维护但没有可靠语义的观测值。

## 验证

- Server/契约测试断言公开健康响应只有两个字段、版本正确且带 `Cache-Control: no-store`。
- Web 更新契约继续只解析 `application_version`，目标版本确认与有界无法确认状态机不变。
- 管理 Balancing 契约继续验证其认证边界以及 `config_revision`、`scheduler_epoch`、活动请求和后台任务计数。
- Web 总览测试验证状态区与调度区共享 Balancing 查询，不再访问公共健康端点；手动刷新仍同时更新进程与会话状态。
