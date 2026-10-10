# 数据库与持久化

关系状态使用 PostgreSQL，模型/助理配置、包、Secret 和产物文件使用受管目录。数据库记录路径、标识、版本与 SHA-256，不把文件 Base64 作为持久工作数据或模型上下文。

## 数据分组

| 数据 | 主要表或存储位置 | 用途 |
| --- | --- | --- |
| 会话和执行 | `execution_sessions`、`execution_turns`、`execution_runs` | owner、输入、状态、版本、时间和执行快照 |
| 助理快照 | `execution_session_agent_profiles`、`execution_run_agent_profiles` | 固定每次执行的助理身份与版本 |
| 工具账本 | `execution_invocations`、`execution_background_observations` | 完整参数/结果、稳定身份、提交与后台状态 |
| 调度 | `execution_scheduler_configuration`、`execution_resource_reservations` | 版本化并发配置与在途共享池占位 |
| 事件和上下文 | `execution_events`、`execution_context_checkpoints`、`execution_context_transforms` | 有序事件、原上下文与投影证据 |
| 模型与用量 | `execution_model_calls`、`execution_model_requests`、`execution_request_parts`、`execution_run_budgets` | 请求、实际用量、预算及按需读取证据 |
| 产物 | `execution_run_artifacts`、`execution_artifact_files`、`execution_artifact_configuration` | 来源 Run、摘要、文件登记和交付配置 |
| 对话交互 | `execution_response_actions`、`execution_session_metadata` 等 | 反馈、置顶、标题、会话元信息 |
| 能力登记 | `capabilities`、`capability_versions`、`capability_permissions`、`capability_health` | 定义、不可变版本、权限和健康 |
| 发现和审计 | `capability_discovery`、`registry_audit` | 实时工具 Schema、服务证据与变更记录 |
| 身份 | `auth_users`、`auth_user_sessions`、`auth_passwords`、`auth_sessions` 等 | 用户、密码哈希、会话、邮箱验证、助理分配和政策 |
| 资料和记忆 | `knowledge_documents`、`knowledge_categories`、`knowledge_libraries`、`knowledge_library_grants`、`personal_memories` | 用户资料、分类、公共库授权及个人记忆 |
| 私有文件 | 配置 generation、包目录、Secret Store、Sandbox、Artifacts | 模型/助理配置、凭据、原文档、任务源码及输出 |

执行 DDL 保留部分历史表；当前调度配置使用单例 `execution_scheduler_configuration`，共享池占位引用 invocation。其他历史表的存在不代表对应执行链已经接通，具体状态见 [功能状态](STATUS.md)。

## 配置与迁移

`EXOHARNESS_DATABASE_URL` 提供连接地址；密码可通过 `EXOHARNESS_DATABASE_PASSWORD_FILE` 单独读取。执行 namespace 使用 `EXOHARNESS_DATABASE_NAMESPACE`，默认 `execution`；App 当前为登记与身份分别使用 `registry`、`auth`。资料表由执行存储的迁移管理，不应臆造独立连接配置。

DDL 位于 `modules/storage/postgres/migrations/`，身份 DDL 位于 `modules/identity/src/schema.rs`。包括执行、登记、发现证据、资料、文档结构和标题的增量迁移。迁移显式执行，使用事务锁，重复执行不清空已有数据。

模型/助理保存通过 `modules/configuration` 的不可变 generation 与 current 指针完成；运行时刷新配置，执行快照固定 revision。凭据不写入公共配置投影或 Run 快照。

## 事务和并发

Session 是 owner 校验和创建 Turn 的锁定边界；同会话不能同时驱动两个活跃 Run。Run 版本对应已提交事件序号。状态、Turn、控制元数据及事件在同事务中提交，任一步失败即回滚。

invocation 准备与领取分开。调用意图包含固定能力 revision、原生参数、摘要和逻辑键。同意图幂等；冲突意图拒绝。MCP admission 在短事务中对能力加 advisory lock，检查并发占用及等待顺序，不在网络调用期间持有数据库锁，也不新增另一套队列数据库。

调度配置写入、Run 准入与共享池领取使用执行 namespace 内统一事务锁，配置保存采用 revision CAS。活跃 Run 统计 running / waiting_authorization，queued 不占执行名额。MCP 池占用从 submitted 账本计算，包含 outcome_unknown；取消 Run 本身不释放外部占位。`0005_scheduler.sql` 为增量迁移，不删除历史记录。

`context_json._exoharness_pro` 保存宿主控制元数据；更新该键时保留其他上下文字段。完整参数/结果保存在调用账本，不在控制元数据里复制一遍。

同步 PostgreSQL 驱动采用连接池；API、Agent 经 `spawn_blocking` 边界访问。当前连接使用 NoTls，适用于本机或受保护私网，不应直接连接未受保护的公网数据库。

## 兼容与备份

历史 Run 的控制数据需按新版格式显式迁移后恢复。迁移校验调用状态和执行参数，由单一版本持有 Run 执行权。详见 [迁移说明](MIGRATION.md)。

备份应同时覆盖 PostgreSQL、私有配置及密钥、包目录、任务与产物文件，并保持同一时间窗口。只有数据库备份无法恢复文件和模型凭据。签名密钥丢失会使既有链接不可验证；数据库恢复演练必须同时验证授权、版本、文件摘要和下载。
