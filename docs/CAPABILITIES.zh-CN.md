# Skill MCP 与沙盒接入

[English](CAPABILITIES.md) | 简体中文

能力生命周期分成登记、授权、准备、验证、执行和交付。导入保存内容与版本；连接测试、环境核验及用户授权分别记录状态。

## Skill

管理端可以上传单文件、文件树或 ZIP。包根应包含 `SKILL.md`；说明、引用资源和脚本保存在不可变包版本中。说明读取是参考数据，不能自授权限、替换模型系统规则或冒充工具执行。

脚本环境核验固定到能力 ID、revision、包摘要和运行镜像 SHA。检查 Python/Node 等解释器及支持的依赖声明，检查器在隔离环境运行，不执行上传脚本、安装 hook 或任意 pip/npm 命令。包更新或换镜像后不能复用旧证明。

新版 Agent 可通过原生 Sandbox 工具挂载授权 Skill 包并执行保存的入口；用户/助理权限、包版本和沙盒就绪均须检查。`script_execution_ready` 的通用受管 Tool 状态与原生 Sandbox 派发是不同接口，不能仅依据一个字段宣布所有脚本都已绑定。依赖缺失仍需受控准备，导入不自动联网安装。

删除操作清理 Skill 登记及对应包内容。历史执行证据、调用引用、审计和生成文件按各自保留策略管理。

## MCP

使用标准 SDK 进行 Streamable HTTP 初始化、能力协商和分页工具发现。管理端支持粘贴 `mcpServers` JSON 及地址导入。Sensitive Header 原值移出 Manifest，调用时恢复完整 Header，不擅自补写或修改认证语义。

发现证据绑定能力 revision，包含工具 Schema、摘要和 serverInfo。版本或权限变化不能复用旧工具缓存。调用依据实时 input Schema，参数保持原生对象/数组；`isError`、structuredContent 和 resource_link 按协议作为事实保存。

当前可登记 stdio 配置，但不允许在宿主机启动。受管 stdio 执行链尚未完整接通，不能把远程 HTTP 支持扩写成所有 MCP 运行方式都已就绪。

## 跨能力数据与发布一致性

工具返回的附件 ID、Artifact ID、任务 ID 和签名 URL 是不同类型的引用。后续调用依据实时 Schema 和实际返回字段传递原始值，保留 URL 签名及来源信息。引用类型变化由能力接口和 Skill 说明同步适配。

能力更新验收包含以下步骤：

1. 部署服务代码或运行镜像，并核对实际进程的挂载路径和版本。
2. 更新已登记的 Skill 包，核对当前 revision 和实际读取的说明。
3. 重新发现 MCP 工具，检查已登记的实时 Schema 与服务一致。
4. 使用保留的输入及产物验证跨能力调用，检查类型、身份、访问权限和输出。
5. 重复提交相同任务引用，确认复用已完成的结果；记录所需的取消和错误路径。

部署代码、登记版本和工具发现证据作为独立检查项保存，更新过程保留原始执行日志。

## 授权与就绪

调用必须同时满足：登记存在、revision 一致、能力启用、声明权限允许、用户政策允许、助理范围允许、执行后端可用。助理选择一个 Skill 不自动授权它可能使用的全部 MCP 或 TTS；这些执行能力需各自允许。

服务返回的业务错误属于服务执行事实；框架不能按业务名猜分辨率、时长、剧情或地址。服务间依赖通过登记连接配置声明，框架只负责解析与授权。

## 进度 后台任务与取消

同步调用使用标准 MCP progressToken 和 progress notifications。服务返回的进度和阶段直接转换为界面事件。有效进度续期 inactivity 等待，其他资源预算继续独立校验。

异步流程可以显式使用 ExoHarness `_meta.exoharness.background_task` 扩展：

```json
{
  "version": 1,
  "id": "opaque-operation-id",
  "status": "working",
  "poll": {"tool": "inspect_operation", "arguments": {"id": "opaque-operation-id"}},
  "cancel": {"tool": "stop_operation", "arguments": {"id": "opaque-operation-id"}},
  "poll_interval_ms": 2500,
  "progress": {"progress": 42, "total": 100, "message": "Service-reported stage"}
}
```

工具名与参数由所属 MCP 声明，控制调用受授权能力和 revision 约束。状态包括 working / completed / failed / cancelled，终态包含真实结果。该扩展使用 ExoHarness 后台任务元数据，与原生 MCP Tasks 协议分别定义。

后端按稳定任务句柄轮询并记录状态，模型仅接收所需终态结果。取消以外部服务确认停止为依据；确认失败时保留未知状态并阻止重复提交。

## 成品文件

MCP 可通过 `resource_link` 或 `structuredContent.artifacts[]` 声明成品；兼容单文件 `structuredContent.result`，文件描述包含 `filename`、`content_type` 和 `url` 或 `download_url`。文件展示直接使用原始工具回执，与模型是否在回答中附链接无关。`isError` 回执和 `artifact_visibility=internal` 的文件不进入最终成品卡片，重复声明按地址与文件名去重。

Harness 保存的文件使用永久签名链接。同源 MCP 文件可按现有受控交付机制保存；其他 MCP 文件保持服务签发的地址与期限。服务自行配置有效期，跨域地址不会因此获得宿主代抓取授权。文件删除和签名密钥轮换可以撤销既有访问。

## 并发

同一 MCP 的 `limits.max_concurrency` 在提交边界生效；当前允许 1–1024。等待发生在实际业务提交之前，跨会话和进程使用持久调用账本；等待不计为再次尝试。后台调用直到可信终态才释放占用，Run 被取消本身不足以证明 GPU 任务已结束。

管理员在登录与通知页面的运行调度配置中创建共享池，并将使用同一资源的 MCP 关联到该池。调用同时遵守 MCP 自身限制和池限制；例如多个能力关联到并发为 1 的 GPU 池，可实现跨服务串行。关联用于限制资源，不授予执行权限。

全局 Agent 和单用户活跃 Run 上限独立于 MCP 限制。默认分别为 1000 和 2；超额运行等待，首次模型请求尚未提交。排队期间可以取消，配置更新对后续准入立即生效；降低限制不终止已经执行的任务。任务服务可持有 4096 个执行或排队 worker。全局上限是准入策略，实际吞吐取决于模型配额、数据库连接和机器资源；GPU 作业继续遵守各 MCP 与共享资源池的限制。

## 沙盒

Python、Shell、JavaScript 和 TypeScript 使用固定 OCI 镜像，逐任务隔离。统一后端配置只读根文件系统、只读包、可写工作区、网络策略、非 root 身份和资源限制。TypeScript 要求镜像内 Node.js 支持原生类型转换。

代码执行要求沙盒就绪，镜像和依赖由管理员准备。主服务环境与任务沙盒分别配置。通用计算、绘图使用独立执行工具；Skill 的专用运行依赖按包声明处理。

实现位置：`modules/{skills,skill-runtime,mcp,secrets,sandbox,tools}`；HTTP 入口见 [API](INTERFACES.md)。完整代码项目流程见 [Coding](CODING_WORKFLOW.md)。
