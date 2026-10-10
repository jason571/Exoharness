# API 接口

管理端和用户端使用独立 Router，文件端只提供受限读取。默认端口为 9797 / 9798 / 9790，部署可调整为其他端口；接口路径不包含端口。以下是当前业务接口，不复制历史 CLI 或旧路由清单。

## 认证与错误

浏览器先调用登录接口取得 HttpOnly Cookie，再由 `GET /v1/auth/session` 获取会话及 CSRF 信息。Cookie 写请求携带 `x-exoharness-csrf`；API 客户端可使用受限凭据文件中的 Bearer token。`actor`、owner 或助理 ID 不构成身份依据。

管理写操作要求管理员，用户数据接口校验当前 owner。配置、密钥、Trace 等管理路由不登记在用户端。认证层校验 Host/Origin，反向代理要求见 [安全](SECURITY.md)。

通用错误格式：

```json
{"error":{"code":"state_conflict","message":"状态已变化，请刷新后使用最新版本"},"resumable":false}
```

常用状态：400 输入错误，401 未认证，403 未授权，404 当前范围内不存在，409 版本/状态冲突或未知结果，413 上传超限，503 后端未就绪。错误码比人类文字更适合客户端判断。不能把所有 409 都提示成“继续执行”。

写入配置和能力时按请求类型携带 `expected_revision`；陈旧版本不静默覆盖。精确 DTO 以对应处理器的 Deserialize 类型为准，未提供独立 OpenAPI 文档。

## 会话与执行

| 方法 | 路径 | 作用 |
| --- | --- | --- |
| GET / POST | `/v1/sessions` | 列表/创建；列表支持 `q` 搜索 |
| GET / PATCH / DELETE | `/v1/sessions/{session}` | 历史、标题/置顶更新、归档删除；历史支持 `turn_limit`、`before_ordinal` |
| POST | `/v1/sessions/{session}/turns` | 提交真实执行，返回 202 与 Run 信息 |
| GET | `/v1/runs/{run}/events` | SSE 事件，`Last-Event-ID` 为已读序号 |
| POST | `/v1/runs/{run}/cancel` | 停止本次 Run；外部取消是否确认由真实回执决定 |
| POST | `/v1/runs/{run}/resume` | 当前返回 409 `resume_unavailable`，不冒充安全恢复 |
| GET | `/v1/sessions/{session}/response-actions` | 查询已完成回复的用户行为 |
| GET / POST | `/v1/runs/{run}/response-actions` | 复制、点赞、点踩及重新生成相关行为记录 |
| GET | `/v1/chat/configuration` | 当前对话配置安全投影 |
| GET | `/v1/agent-profile`、`/v1/agent-profiles/default` | 当前用户分配助理的公开投影 |

最小调用顺序：

```text
POST /v1/sessions                       {"title":"示例任务"}
POST /v1/sessions/{session}/turns       {"prompt":"计算并绘制结果","attachments":[]}
GET  /v1/runs/{run}/events              Last-Event-ID: 0
GET  /v1/sessions/{session}
```

Turn DTO 可包含 `model`、`capability_ids`、`actor`。服务端依据认证身份执行授权并固定快照。生产聊天使用上述 Session / Turn / Run 链路；`/v1/chat` 和 `/v1/execute/*` 属于受限验证入口。

SSE 将已提交 Chat payload 作为 JSON `data` 发送，并带事件 ID；包含文本、调用、真实进度和终态。断线重连重放已提交事件，不重新执行工具。`complete`、`failed`、`cancelled`、`recoverable` 后结束流。持久状态查询与事件重放是两个接口，流断开不等于任务失败。

## 能力与导入

以下接口在管理端：

| 方法 | 路径 | 作用 |
| --- | --- | --- |
| GET | `/v1/capabilities`、`/v1/capabilities/overview` | 登记记录及授权/健康/就绪状态 |
| GET | `/v1/assistant-capabilities` | 助理可授权目录，包括原生沙盒入口 |
| GET / DELETE | `/v1/capabilities/{id}` | 详情/删除 |
| GET | `/v1/capabilities/{id}/versions`、`/health`、`/runtime` | 版本、健康和运行环境信息 |
| PUT | `/v1/capabilities/{id}/enabled` | 按版本启停 |
| GET | `/v1/capabilities/{id}/permissions` | 权限决策 |
| PUT | `/v1/capabilities/{id}/permissions/{permission}` | Allow / Deny / Pending |
| GET / POST | `/v1/capabilities/{id}/preparation` | 运行依赖核验及显式准备；权限独立校验 |
| POST | `/v1/capabilities/{id}/validate` | MCP 初始化/发现或 Skill 完整性检查 |
| GET | `/v1/capabilities/{id}/tools` | 当前已验证原生工具 Schema |
| GET | `/v1/capabilities/{id}/secrets` | 认证配置状态，不返回值 |
| PUT / DELETE | `/v1/capabilities/{id}/secrets/{name}` | 轮换/删除秘密引用 |
| POST | `/v1/imports/manifest` | `content` 中粘贴完整 MCP JSON |
| POST | `/v1/imports/mcp-http` | 地址型 MCP 导入 |
| POST | `/v1/imports/skill/preview` | 只读预览包及版本变化 |
| POST | `/v1/imports/skill`、`/v1/imports/skill/package` | 上传 Skill 文件树/ZIP |
| GET | `/v1/capabilities/{id}/skill-instructions`、`/skill-files` | 完整说明及包资源信息 |

示例 MCP 内容使用占位地址，不携带真实密钥：

```json
{"mcpServers":{"example":{"type":"streamableHttp","url":"https://mcp.example.com/mcp"}}}
```

导入解析不启动服务；敏感 Header 剥离到 Secret Store。ToolNode 登记类型存在，但独立 Tool 执行准备尚未接通；不能将其与已接通的原生沙盒执行混淆。

## 模型、助理与语音

| 方法 | 路径 | 作用 |
| --- | --- | --- |
| GET | `/v1/model-catalog` | 管理模型/provider 类型与默认配置 |
| POST | `/v1/model-configuration` | 管理统一模型保存入口 |
| POST | `/v1/model-providers`、`/v1/models` | 分别保存 Provider 和模型 |
| DELETE | `/v1/model-providers/{name}`、`/v1/models/{name}` | 删除配置 |
| PUT | `/v1/model-providers/{name}/enabled`、`/v1/models/{name}/enabled` | 启停 |
| PUT | `/v1/models/{name}/default` | 设置类型默认值 |
| POST | `/v1/models/{name}/test` | 类型化模型协议测试 |
| GET / POST | `/v1/agent-profiles` | 管理助理列表/创建 |
| GET / PUT / DELETE | `/v1/agent-profiles/{id}` | 管理详情、更新/删除 |
| PUT | `/v1/agent-profiles/{id}/enabled`、`/default` | 管理启停和默认值 |
| POST / GET | `/v1/agent-assets`、`/v1/agent-assets/{id}` | 管理上传/授权读取头像 |
| GET | `/v1/speech-input/configuration` | 用户/管理端 ASR 就绪配置 |
| POST | `/v1/speech-input` | 用户/管理端上传录音并取得转写，不将音频送入普通 LLM 上下文 |

实际协议支持见 [模型配置](MODEL_CONFIGURATION_DESIGN.md)。

## 用户、资料和管理

| 方法 | 路径 | 范围 |
| --- | --- | --- |
| POST | `/v1/auth/login`、`/v1/auth/logout` | 各自端口的登录/注销 |
| GET | `/v1/auth/session`、`/v1/auth/status` | 当前身份/认证状态 |
| POST | `/v1/auth/registration-code`、`/v1/auth/register` | 用户端验证码/注册 |
| GET | `/v1/account` | 用户端账号信息 |
| PUT | `/v1/account/profile`、`/v1/account/password` | 用户修改个人资料/密码 |
| GET | `/v1/user-capabilities`、`/v1/user-capabilities/install-jobs` | 用户能力视图；当前 installation_ready=false |
| GET | `/v1/management/users`、`/v1/management/users/{id}` | 管理用户列表/详情 |
| PUT | `/v1/management/users/{id}/assistant`、`/capability-policy` | 管理分配助理/用户政策 |
| PUT | `/v1/auth/passwords/{role}` | 管理角色密码 |
| GET / PUT | `/v1/auth/registration-email` | 管理注册邮件模板 |
| GET / PUT | `/v1/scheduler-configuration` | 管理并发上限、共享池及 MCP 关联；PUT 携带当前 `revision` |
| GET | `/v1/management/overview`、`/v1/audit` | 管理总览/审计 |
| GET | `/v1/trace/sessions`、`/v1/trace/runs`、`/v1/trace/runs/{run}` | 管理跨用户追踪列表与详情 |
| GET | `/v1/trace/runs/{run}/requests/{logical}/{attempt}` | 管理请求清单；可按 `part` 读取该请求所属的完整证据部件 |
| GET / POST | `/v1/knowledge/categories`、`/v1/knowledge/documents`、`/v1/memories` | 用户自身范围内的查询/创建 |
| PUT / DELETE | `/v1/knowledge/categories/{id}`、`/v1/memories/{id}` | 更新/删除 |
| GET / PUT / DELETE | `/v1/knowledge/documents/{id}` | 资料详情/更新/删除 |
| POST | `/v1/knowledge/documents/import` | 上传资料文件 |
| GET | `/v1/knowledge/search`、`/v1/knowledge/documents/{id}/blocks[/{block}]` | 查询、结构块清单/按版本读取 |
| GET / POST | `/v1/knowledge/libraries` | 管理资料库 |
| GET / PUT / DELETE | `/v1/knowledge/libraries/{library}` | 管理库详情/更新/删除 |
| PUT | `/v1/knowledge/libraries/{library}/grants` | 管理库授权 |

管理资料库提供 `/v1/knowledge/libraries/{library}/categories...` 与 `/documents...` 的对应操作。方括号表示可选路径段，发送请求时使用实际路径。字段、格式和读取范围见 [资料与记忆](KNOWLEDGE_DOCUMENT_IMPORT.md)。

Trace 列表支持 `limit` / `offset`；Run 列表要求 `session_id`。详情支持 `event_after` / `event_limit` 读取时间轴，不要求一次加载所有事件。请求 `part` 必须属于该 Run/请求，不能作为任意文件读取接口。

调度 GET 返回 `revision`、`max_active_runs_per_user`（1–128，默认 2）、`max_global_agent_runs`（1–1024，默认 1000）、`resource_pools`、`mcp_resource_assignments`、`updated_at`（毫秒）和只读 `mcp_capabilities`。PUT 提交前六项，不提交只读目录；版本冲突返回 409 `state_conflict`。池结构为 `{id,name,max_concurrency}`，MCP 关联结构为 `{capability_id:pool_id}`，关联必须指向已登记 MCP 和存在的池。配置保存不改变权限。

排队通过 SSE `resource_queue` 事件提供 `state: waiting | acquired` 和 `message`；Run 等待不包含工具调用 ID，MCP 等待包含 `tool_call_id`。等待不请求模型、不增加执行尝试次数。总览的 scheduler 字段返回真实活跃数、等待数、用户/全局上限以及各池占用。

## 文件端

| 方法 | 路径 | 作用 |
| --- | --- | --- |
| GET | `/health` | 文件服务健康 |
| GET | `/artifacts/{id}/{filename}` | 签名下载/Range |
| GET | `/artifacts/{id}/{filename}/metadata`、`/preview` | 文件元数据或受限预览 |
| GET | `/projects/{id}/{signature}/{*path}` | 发布清单内的多文件静态预览 |
| GET | `/workspace-files/{id}/{filename}`、`/preview` | 兼容文件路径 |

文件服务生成资源路径和签名 query。客户端使用返回地址，访问范围限定于已登记文件和授权项目清单。

源码路由分别位于 `modules/api/src/{chat,conversations,registry,skills,mcp,configuration,identity,knowledge,media}/`，Trace 在 `trace.rs`。`/v1/local/*` 与 `/local-validation` 是开发验证接口；不能仅因为它们的基础设施状态字段存在，就认定实际聊天路由未接通。
