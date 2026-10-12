// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

/** Interface copy only. User messages, documents and tool payloads are not translated. */
const entries = `
能力受限|Capability limited
正在处理任务，已完成内容会保留。|Processing the task. Completed work is retained.
目前没有获得完成这项任务所需的技能权限，请联系管理员开放相应技能。已完成内容已保留。|The required Skill permission is unavailable. Please ask your administrator to enable it. Completed work is retained.
所需能力暂时不可用，暂时无法完成这项任务。已完成内容已保留。|The required capability is temporarily unavailable. Completed work is retained.
已有任务的最终结果暂时无法确认。已完成内容已保留，无需重复提交。|The existing task's final result is not yet confirmed. Completed work is retained; no resubmission is needed.
当前能力暂时无法处理完整任务，请缩小未完成部分的范围。已完成内容已保留。|The current capability cannot handle the full task. Please reduce the remaining scope. Completed work is retained.
目前可用的能力暂时无法完成这项任务，已完成内容已保留。可联系管理员检查所需技能与权限。|The available capabilities cannot currently complete this task. Completed work is retained. Your administrator can check the required Skills and permissions.
运行总览|Overview
指定服务|Selected services
：连接 HTTP MCP|: Connect HTTP MCP
：认证值会自动保存到本机密钥保险箱|: Credentials are saved automatically in the local secret store
新建分类|New category
添加新模型|Add model
交付|Deliver
执行与交付|Execution & delivery
进行中|In progress
等待恢复|Awaiting resume
执行失败|Execution failed
查看对话|View conversation
查看文件|View file
发送消息后，可在这里查看执行状态、预览和下载文件。|After sending a message, view task status, preview results and download files here.
这里显示已加载历史中最近 50 次执行；更早的文件可在对应历史消息中查看。|The latest 50 runs in loaded history are shown here. Find earlier files in their original messages.
点击插入可用内容|Select to insert available content
技能中心|Capabilities
技能导入|Import capabilities
模型配置|Models
智能助理|Assistants
资料管理|Knowledge
运行追踪|Run trace
操作记录|Audit log
系统设置|System settings
用户管理|Users
管理员|Administrator
管理员视图|Administrator view
管理控制台|Management console
只读用户|Read-only user
退出|Sign out
退出登录|Sign out
登录|Sign in
登录 ExoHarness|Sign in to ExoHarness
欢迎回来|Welcome back
登录后继续你的专属对话。|Sign in to continue your conversations.
账号|Account
邮箱|Email
邮箱或用户名|Email or username
密码|Password
显示密码|Show password
隐藏密码|Hide password
显示确认密码|Show confirmation password
隐藏确认密码|Hide confirmation password
确认密码|Confirm password
创建账号|Create account
已有账号？|Already have an account?
还没有账号？|New to ExoHarness?
使用邮箱注册|Register with email
用户名|Username
邮箱验证码|Email verification code
发送验证码|Send code
返回登录|Back to sign in
验证|Verify
6 位数字|6 digits
验证码将发送到你的邮箱，验证完成后才能注册。|A verification code will be sent to your email. Verify it to register.
创建账号，开启属于你的创作空间。想法、素材与作品，都在同一个地方延续。|Create an account to keep your ideas, materials and creations in one place.
从今天开始，|Start today.
把灵感实现。|Bring your ideas to life.
让每一个想法，|Give every idea
都有回响。|a voice.
看见每一次执行，|Follow every task.
掌控每一项能力。|Manage every capability.
管理模型、能力与运行状态，让智能体在清晰的边界内可靠工作。|Manage models, capabilities and execution within defined permissions.
管理员可修改系统配置；只读用户可查看获授权的管理页面。正式聊天请使用用户端。|Administrators manage configuration. Read-only users can view authorized pages. Use the user portal for chat.
输入管理员密码或只读用户密码，系统会自动进入对应权限。|Enter the administrator or read-only password to access your assigned role.
当前管理密码|Current administrator password
登录密码|Sign-in password
主导航|Main navigation
展开菜单|Expand menu
收起菜单|Collapse menu
打开管理菜单|Open management menu
关闭管理菜单|Close management menu
执行进度|Execution progress
正在同步 Registry|Syncing registry
正在同步会话记录|Syncing conversations
暂无会话记录|No conversations yet
正在同步操作记录|Syncing audit records
邮件服务已连接|Mail service connected
输入结构|Input structure
模型输入结构（不保存原文）|Model input structure (original not stored here)
未记录（不等于实际计费节省）|Not recorded (not equivalent to billing savings)
模型输入证据|Model input evidence
模型配置|Model configuration
系统说明|System instructions
工具 Schema|Tool schema
已脱敏|Redacted
媒体转为引用|Media references
技能资源去重|Deduplicated Skill resources
移除未执行的错误参数|Removed rejected arguments
局部修复上下文|Local repair context
保留阶段结果引用|Retained checkpoint references
移出未选用的技能|Removed unselected Skills
移出已替代的检查点|Removed superseded checkpoints
选择本步工具|Selected tools for this step
整理历史上下文|Compacted history
当前工作流上下文|Workflow context
当前参数修复上下文|Argument repair context
大结果保存为引用|Persisted large-result reference
恢复保存的上下文|Restored context
选取待整理内容|Selected history for compaction
移除 Schema 未引用的定义（保留约束）|Removed unreachable schema definitions (constraints retained)
选取进展检查内容|Selected loop-review context
移出未提交的回复|Removed unsubmitted response
纠正未实际执行的回复|Corrected unexecuted response
运行信息|Run information
结束详情|Completion details
运行状态与预算|Run status and budget
结束原因与关联错误|Completion reason and related errors
用量对账异常|Usage reconciliation mismatch
运行轨迹已同步|Trace synchronized
正在读取运行轨迹|Loading trace
运行中|Running
运行中 · 自动刷新|Running · Auto-refresh
未提供|Not provided
未记录|Not recorded
Provider 未提供|Not reported by provider
实际 Token|Reported tokens
上下文估算|Context estimate
耗时|Duration
用户请求|User request
运行时|Runtime
解析与规划|Analysis and planning
请求|Request
响应|Response
发起能力调用|Invoke capability
已返回|Returned
等待返回|Awaiting response
处理调用结果|Process tool result
生成最终响应|Generate final response
直接响应|Direct response
参数校验失败 · 未执行|Argument validation failed · Not executed
调用前校验|Pre-execution validation
旧运行或尚未发起模型请求，暂无逐次 Token 数据|No per-request token data for this run yet
接收请求|Request received
事件已记录|Event recorded
查看原始事件|View raw event
创建运行|Run created
运行状态|Run status
排队中|Queued
助手开始响应|Assistant response started
助手输出|Assistant output
本轮响应结束|Turn response ended
准备调用|Prepare invocation
准备调用（尚未提交）|Invocation prepared (not submitted)
提交调用|Submit invocation
已提交调用|Invocation submitted
调用能力|Invoke capability
调用结算|Invocation resolved
调用已结算|Invocation resolved
查看返回结果|View result
查看调用参数|View arguments
能力返回|Capability result
能力返回错误|Capability error
恢复状态|Recovery status
状态更新|Status update
执行阶段|Execution phase
执行资源|Execution resources
能力选择|Capability selection
执行错误|Execution error
依赖连接|Dependency connections
展开全文|Expand text
收起全文|Collapse text
（空白片段）|(Whitespace)
该 Run 没有持久化事件|No persisted events for this run
参数摘要|Argument digest
类型|Type
调用类型|Call type
状态 / 停止原因|Status / Stop reason
首事件时延|Time to first event
总耗时|Total duration
输入 Token（不含缓存）|Input tokens (excluding cache)
输出 Token|Output tokens
缓存 Token|Cached tokens
工具 Schema（估算）|Tool schemas (estimate)
系统提示（估算）|System prompt (estimate)
用户消息（估算）|User messages (estimate)
历史助理消息（估算）|Assistant history (estimate)
工具结果（估算）|Tool results (estimate)
摘要与执行引用（估算）|Summaries and execution references (estimate)
Skill 说明 / 引用（摘要内估算）|Skill instructions / references (within summaries)
Checkpoint（摘要内估算）|Checkpoints (within summaries)
局部修复（摘要内估算）|Local repairs (within summaries)
大结果引用（摘要内估算）|Large-result references (within summaries)
工具参数（助理消息内估算）|Tool arguments (within assistant messages)
Schema 投影减少（估算）|Schema projection savings (estimate)
正在读取请求清单…|Loading request manifest…
请求清单与变化|Request manifest and changes
这次历史请求未保存输入清单，仍可查看原有用量和结构指标。|This historical request has no input manifest; usage and structure metrics remain available.
输出摘要|Output summary
模型输出摘要（不重复保存原文）|Model output summary (original retained separately)
未匹配依赖地址，保留服务自身配置|No dependency address matched; keeping the service configuration
尚未设置|Not set
活跃任务|Active tasks
正在排队|Queued
失败 / 可恢复|Failed / Recoverable
期间执行次数|Runs in period
期间 Token|Tokens in period
任务|Task
最新状态|Latest status
提交时间|Submitted
追踪|Trace
查看|View
执行资源尚未启用|Execution resources disabled
执行次数（含重试）|Runs (including retries)
成功尝试|Successful attempts
失败 / 可恢复尝试|Failed / recoverable attempts
已结束运行耗时|Completed run duration
模型 / 服务商|Model / Provider
请求次数|Requests
累计请求耗时|Total request duration
用量缺失|Missing usage
全部用户|All users
未指定助理|Unassigned assistant
当前任务按最新执行计数，用量包含历史重试|Tasks count the latest run; usage includes retries
执行资源与能力检查为全局|Resources and capability checks are global
每 30 秒更新|Refreshes every 30 seconds
能力范围及声明依赖检查通过|Capability scope and declared dependencies verified
执行连接或内容尚未就绪|Execution connection or content not ready
查看助理|View assistant
此范围内暂无记录|No records in this range
此用户在所选时间范围内暂无执行记录。|No runs for this user in the selected period.
所选时间范围内暂无执行记录。|No runs in the selected period.
运行总览暂未开放，请稍后再试。|Overview unavailable. Try again later.
当前账号无权查看运行总览。|This account cannot access the overview.
暂时无法读取运行记录，请稍后重试。|Unable to load run records. Try again later.
暂时无法刷新，以下为上次读取的记录。|Unable to refresh. Showing previously loaded records.
运行记录暂不可用。|Run records unavailable.
尚未登记能力|No capabilities registered
所有已登记能力均已就绪|All registered capabilities are ready
助理已停用|Assistant disabled
全部项目|All projects
停用服务|Disable service
启用服务|Enable service
无需额外权限|No additional permissions required
无额外权限声明|No additional permissions declared
这项能力不需要额外权限，可以直接验证。|This capability needs no additional permissions and can be tested directly.
这项能力所需的权限已经授权。|The required permissions have been granted.
永久删除这项 Skill 的登记、全部历史版本和服务器包文件。此操作无法恢复，操作记录仍会保留。|Permanently delete this Skill registration, all revisions and server package files. This cannot be undone. Audit records are retained.
从服务列表移除这项登记，并删除它保存在本机的认证信息。操作记录仍会保留。|Remove this registration and its locally stored credentials. Audit records are retained.
彻底删除 Skill|Permanently delete Skill
API Key 已配置；页面和 API 均不会回显|API key configured; its value is never returned by the page or API
启用这个 LLM；保存后自动停用同类型的其他配置，不影响其他类型|Enable this LLM; saving disables other configurations of the same type without affecting other types
已启用 · 对话可用|Enabled · Available in chat
已启用 · 语音输入|Enabled · Voice input
测试 API 连接|Test API connection
生成短音频测试|Test short audio synthesis
测试转录接口|Test transcription API
用于聊天录音转文字，不消耗 LLM Token|Transcribes chat recordings without using LLM tokens
输出长度由模型服务决定|Output length determined by the model service
更换头像|Change avatar
配置已保存|Configuration saved
删除这个助理|Delete this assistant
还没有资料库。在右侧创建后，即可分类并导入文件。|No knowledge libraries yet. Create one on the right to organize and import files.
系统操作|System action
其他|Other
更新权限|Update permissions
启用能力|Enable capability
停用能力|Disable capability
移除能力|Remove capability
更新能力|Update capability
添加能力|Add capability
恢复旧版本|Restore previous revision
完成验证|Validation completed
开始调用|Invocation started
调用完成|Invocation completed
调用失败|Invocation failed
已移除的能力|Removed capability
系统|System
控制台|Console
自动任务|Automated task
未知来源|Unknown source
邮件服务未配置|Mail service not configured
当前使用默认内容|Using default content
此功能尚未接入新框架|This feature is not available yet
个会话|chats
关闭菜单|Close menu
管理菜单|Management menu
了解任务进展、资源占用和使用情况。|View task progress, resource allocation and usage.
任务与状态|Tasks and status
任务指标|Task metrics
会话记录|Conversations
会话内各轮消息|Conversation turns
会话累计 Token|Total conversation tokens
会话追踪|Conversation trace
会话总数|Conversation count
对话总数|Conversation count
能力概况|Capability summary
用户统计|User statistics
模型消耗|Model usage
用量统计|Usage
统计维度|Group by
统计范围|Period
最近 24 小时|Last 24 hours
最近 7 天|Last 7 days
最近 30 天|Last 30 days
最近 90 天|Last 90 days
本轮 Token|Tokens this turn
本轮耗时|Turn duration
输入 Token|Input tokens
输出 Token|Output tokens
缓存 Token|Cached tokens
费用|Cost
模型请求|Model requests
能力调用|Tool calls
运行摘要|Run summary
有执行记录|With executions
全部|All
全部记录|All records
全部追踪|All traces
搜索会话、最近消息、状态|Search conversations, recent messages or status
搜索用户|Search users
用户|User
用户名称|User name
刷新|Refresh
刷新列表|Refresh list
最近 200 个会话 · 补充消息自动归组|Latest 200 conversations · Follow-up messages grouped automatically
每轮任务显示最新一次执行，优先显示活跃任务，最多 50 项。点击查看直接打开该次 Trace，历史尝试仍保留。|Shows the latest execution for each turn, with active tasks first, up to 50 items. Open a task to view its trace. Previous attempts remain available.
资源池占用来自调度器，不等同于 GPU 显存检测。已排队任务会在限制内自动执行。|Resource allocation is reported by the scheduler, not GPU memory monitoring. Queued tasks start within the configured limits.
按实际日志统计，最多展示用量最高的 50 项。未上报 Token 的模型请求单独标注；未配置价格时不估算费用。|Usage is calculated from logs for the top 50 items. Requests without token reports are marked separately. Costs require configured prices.
同一会话的消息集中展示，按轮查看模型消耗、能力调用与执行结果。|Messages are grouped by conversation. View model usage, tool calls and results for each turn.
集中管理 Skill、MCP 与本地工具，查看可用性、权限和启用状态。|Manage Skills, MCP services and local tools, including readiness, permissions and activation.
从左侧选择后，可以检查可用性、处理权限并决定是否启用。|Select a capability to check readiness, manage permissions and enable it.
先检查，再启用|Check before enabling
添加能力|Add capability
选择一项能力|Select a capability
全部能力|All capabilities
能力列表|Capability list
能力类别|Capability category
能力类型|Capability type
能力类型筛选|Filter capability types
能力范围|Capability scope
能力管理|Capability management
能力 ID|Capability ID
能力|Capability
能力变更|Capability changes
能力授权范围|Capability permissions
能力与助理检查|Capability and assistant checks
可用性|Readiness
当前可用|Available
已启用|Enabled
启用|Enable
禁用|Disable
已禁用|Disabled
验证可用性|Check availability
验证记录|Validation history
尚未验证|Not checked
尚未设置|Not configured
尚未修改|Unchanged
删除登记|Delete registration
删除能力|Delete capability
权限|Permissions
认证信息|Credentials
工具说明|Tool description
技能说明|Skill instructions
技能完整说明|Full Skill instructions
当前不可变 revision 的 SKILL.md，只读展示。|Read-only SKILL.md for the current immutable revision.
复制说明|Copy instructions
复制 JSON|Copy JSON
点击“验证可用性”，检查这项能力当前是否可以正常使用。|Select Check availability to test this capability.
认证内容只保存在本机，不会显示在页面、配置详情或 API 响应中。可在这里直接更新、轮换或删除。|Credentials are stored locally and omitted from pages, configuration details and API responses. Update, rotate or delete them here.
从服务列表移除这项登记，并删除它保存在本机的认证信息。操作记录仍会保留。|Remove this registration and its local credentials. Audit records are retained.
返回技能中心|Back to capabilities
前往技能中心|Go to capabilities
在这里接入 Skill、MCP 和本地工具；导入后的准备与检查结果会显示在下方。|Import Skills, MCP services and local tools. Preparation and validation results appear below.
先选择能力类型，页面只显示该类型需要的配置。|Choose a capability type to view its configuration fields.
常见配置都可以直接使用|Common configurations are supported
需要修改配置？重新导入同名能力会创建新 revision|To update a capability, import the same name to create a new revision.
添加 MCP 服务|Add MCP service
连接 MCP|Connect MCP
连接 MCP 服务|Connect MCP service
接入外部服务与工具|Connect services and tools
MCP 服务|MCP services
MCP 配置 JSON|MCP configuration JSON
可以直接粘贴现有配置。Authorization、Token 等敏感值会在登记前从配置中剥离，单独保存且不再回显。|Paste an existing configuration. Sensitive values such as Authorization and tokens are extracted before registration, stored separately and never displayed again.
支持一次添加多个服务|Add multiple services at once
Token 会单独保存到本机密钥保险箱，不进入 MCP 配置或列表 API|Tokens are stored in the local secret store, separately from MCP configuration and list responses.
粘贴 JSON|Paste JSON
载入 JSON 文件|Load JSON file
格式化|Format
填入示例|Insert example
从剪贴板粘贴|Paste from clipboard
等待粘贴配置|Waiting for configuration
粘贴后会立即检查 JSON 格式和服务数量|Pasted JSON is checked for syntax and service count.
中可包含多个服务|may contain multiple services
服务名称|Service name
填写服务地址|Enter service URL
MCP 服务地址|MCP service URL
填写正在运行的 MCP 地址|Enter a running MCP service URL
添加 HTTP MCP|Add HTTP MCP
适合已经提供 Streamable HTTP 地址的服务。|For services with an existing Streamable HTTP endpoint.
这个 MCP 需要访问网络|This MCP requires network access
添加本地 MCP|Add local MCP
选择本地 MCP 项目|Select local MCP project
已有配置可以直接粘贴；也可以手动填写服务地址，或选择本地 MCP 项目。|Paste a configuration, enter a service URL, or select a local MCP project.
三种方式任选一种。ExoHarness 会自动识别常见的 Python、Node.js 或 Shell 启动方式。|Choose one input method. ExoHarness detects common Python, Node.js and Shell entrypoints.
等待选择启动文件、ZIP 或目录|Select an entrypoint, ZIP or directory
选择启动文件|Select entrypoint
选择项目 ZIP|Select project ZIP
选择项目文件夹|Select project folder
未选择 ZIP|No ZIP selected
未选择文件夹|No folder selected
未选择目录|No directory selected
入口模块或文件|Entrypoint module or file
解释器或程序|Interpreter or program
工具名称前缀|Tool name prefix
允许全部已启用|Allow all enabled
自动准备并启用|Prepare and enable automatically
仅授权|Authorized only
准备过程不会执行生成、发送消息等业务操作|Preparation does not run generation, messaging or other business operations.
自动准备并启用：允许标准连接和启动权限，安装声明的受支持依赖并检查；不覆盖已拒绝权限，不执行 Skill 安装脚本。取消勾选则仅登记。|Allow standard connection and startup permissions, prepare supported declared dependencies, and validate readiness. Denied permissions remain unchanged. Skill installation scripts are not executed. Clear this option to register only.
添加 Skill|Add Skill
加入工作方法与配套资料|Add instructions and supporting resources
添加一套工作说明和配套资料，Agent 会在合适的任务中按需使用。|Add task instructions and resources for the Agent to load when needed.
三种方式任选一种。单个 .md 文件会自动作为 Skill 入口；包含参考资料或模板时，目录或 ZIP 的根目录需有 SKILL.md。|Choose one input method. A single .md file becomes the Skill entrypoint. Folders and ZIP packages with resources must contain SKILL.md at the root.
等待选择 Skill 文件|Select a Skill file
选择单个 Skill Markdown|Select Skill Markdown file
选择 Skill ZIP|Select Skill ZIP
选择 Skill 文件夹|Select Skill folder
尚未选择文件|No file selected
未选择文件|No file selected
检查并添加 Skill|Validate and add Skill
添加本地工具|Add local tool
接入脚本或代码包|Import script or code package
把一个脚本或代码包添加为 Agent 可调用的工具。添加完成后需要先验证再启用。|Register a script or code package as an Agent tool. Validate it before enabling.
三种方式任选一种。支持 Python、Node.js 和 Shell；添加过程中不会运行代码。|Choose one input method. Python, Node.js and Shell are supported. Importing does not execute code.
可以选择启动文件，也可以添加完整目录或 ZIP 包。|Select an entrypoint, a complete folder or a ZIP package.
选择工具脚本|Select tool script
选择代码包 ZIP|Select code ZIP
选择代码包文件夹|Select code folder
代码包运行方式|Code runtime
工具名称|Tool name
工具组名称|Tool group name
工具组说明|Tool group description
这个服务能做什么？|What does this service do?
想让它做什么？|What should it do?
输入参数格式|Input parameters
使用 JSON Schema 描述 Agent 调用工具时可以填写的参数|Describe accepted tool arguments with JSON Schema.
需要的权限（可选）|Required permissions (optional)
多个权限使用逗号分隔|Separate permissions with commas
验证并注册|Validate and register
导入完成|Import complete
确认后会创建新的不可变版本；当前启用状态和已有权限决定会保留。|Confirmation creates a new immutable revision while retaining activation status and permission decisions.
按能力类型配置模型。启用后，LLM 负责对话推理，TTS 等生成模型可由用户端按需调用。|Configure models by capability type. Enabled LLMs handle conversations; TTS and other models are available for appropriate tasks.
模型列表|Model list
模型类型|Model type
模型名称|Model name
模型信息|Model details
模型设置|Model settings
模型|Model
LLM · 大语言模型|LLM · Language model
TTS · 文字转语音|TTS · Text to speech
ASR · 语音转文字|ASR · Speech to text
添加模型配置|Add model configuration
API 协议|API protocol
API 地址|API endpoint
接口连接|Connection
接口配置名称|Connection name
配置名称|Configuration name
配置名称用于区分保存的配置；模型名称会直接发送给 API 服务。|The configuration name identifies the saved entry. The model name is sent to the API.
填写模型服务提供的模型名称|Enter the model name provided by the service
OpenAI 兼容接口|OpenAI-compatible API
Anthropic 接口|Anthropic API
保持现有协议|Keep current protocol
连接 LLM 服务所需的信息。|Connection details for the LLM service.
不同 TTS 接口使用不同名称，保存新接口不会覆盖已配置的服务和密钥。|Use a separate name for each TTS connection. Saving a new connection preserves existing services and credentials.
用于区分多套配置，支持中文|Identifies configurations; Chinese names are supported
用于正式用户端的对话与任务|For user conversations and tasks
启用这个 LLM；保存后自动停用其他 LLM|Enable this LLM; saving disables other LLMs
已提供常用默认值，点击展开后可以修改。|Common defaults are provided. Expand to edit them.
LLM 高级配置|Advanced LLM settings
思考模式|Reasoning mode
关闭思考|Disable reasoning
默认关闭|Disabled by default
最大输出 Token|Maximum output tokens
输出长度控制|Output length
不发送输出上限（由模型服务决定）|Omit output limit (provider default)
不发送上限时，模型服务自身的输出限制仍然有效。仅适用于支持省略该参数的兼容接口。|The provider output limit still applies. Available only for APIs that support omitting this parameter.
上下文窗口|Context window
默认 0.7；Anthropic 最大为 1|Default 0.7; maximum 1 for Anthropic
默认 0.9，范围 0.01–1|Default 0.9; range 0.01–1
默认 40；OpenAI 官方接口不发送此参数|Default 40; omitted for the official OpenAI API
火山引擎 TTS · SSE v3|Volcengine TTS · SSE v3
阿里云千问 TTS · 非实时 HTTP|Alibaba Qwen TTS · HTTP
阿里云 Qwen-Audio TTS · 实时 WebSocket|Alibaba Qwen-Audio TTS · WebSocket
音色|Voice
服务商提供的音色标识|Provider voice identifier
ExoHarness 中稳定的声音角色标识|Stable voice profile identifier in ExoHarness
语言|Language
对应请求中的 language_type；单一语种建议明确选择|Maps to language_type. Select a language for single-language content.
Auto · 自动识别|Auto · Detect language
Chinese · 中文|Chinese
English · 英文|English
Japanese · 日文|Japanese
Korean · 韩文|Korean
French · 法文|French
German · 德文|German
Russian · 俄文|Russian
Italian · 意大利文|Italian
Spanish · 西班牙文|Spanish
Portuguese · 葡萄牙文|Portuguese
音频格式|Audio format
PCM（保存时封装为 WAV）|PCM (saved as WAV)
采样率|Sample rate
实时 WebSocket 默认 22050 Hz|Default 22050 Hz for real-time WebSocket
OpenAI 兼容 · 音频转录|OpenAI-compatible · Audio transcription
Bearer Token（可选）|Bearer token (optional)
请输入 API Key|Enter API key
保存配置|Save configuration
开始测试|Test connection
当前启用配置|Active configuration
配置 ID|Configuration ID
LLM、TTS 与 ASR 分组显示；已启用模型可在用户端使用，点击配置载入左侧修改。|Models are grouped by LLM, TTS and ASR. Enabled models are available in the portal. Select an entry to edit it.
每种类型可以保存多个配置，并独立启用一个；不同类型互不影响。|Save multiple configurations for each type and enable one independently.
为不同任务配置助理|Configure assistants for different tasks
独立设置身份、可用能力和发布状态。能力选择由后端执行检查，不只是提示词。|Configure identity, permissions and publication separately. Capability access is enforced by the backend.
创建智能助理|Create assistant
助理列表|Assistant list
助理|Assistant
选择后编辑当前助理；只有点击“创建智能助理”才会新增。|Select an assistant to edit it. Create assistant adds a new one.
形象与名称|Appearance and name
基本信息|Basic information
配置版本|Configuration revision
显示名称|Display name
创建后不可修改|Immutable after creation
用于正式用户端的新对话|For new portal conversations
设为新对话默认助理|Default assistant for new conversations
头像|Avatar
头像预览|Avatar preview
选择头像|Choose avatar
移除图片|Remove image
文字头像|Text avatar
PNG、JPEG 或 WebP，最大 2 MiB|PNG, JPEG or WebP, up to 2 MiB
未选择图片或图片无法加载时显示|Displayed when no image is selected or loading fails
头像上传后按内容保存；旧会话仍保留创建时的助理快照。|Avatars are saved by content. Existing conversations retain their original assistant snapshot.
简短说明|Short description
用户在对话界面看到的助理说明|Assistant description shown in chat
身份提示词|Identity prompt
输入智能助理的身份、职责、边界与交互风格|Enter the assistant identity, responsibilities, limits and communication style
定义助理是谁、服务对象、能力边界和表达方式；具体能力知识应放入 Skill。|Define identity, audience, scope and communication style. Put task-specific instructions in Skills.
身份提示词属于部署配置，不会写入 Harness 代码。|The identity prompt is deployment configuration, separate from Harness code.
按类别选择助理可以使用的能力，修改后保存生效。|Select authorized capabilities by category and save changes.
Skill 范围|Skill scope
MCP 范围|MCP scope
Tool 范围|Tool scope
MCP 选择|MCP selection
Tool 选择|Tool selection
模型能力|Model capabilities
模型能力范围|Model capability scope
模型能力选择|Model capability selection
全部已启用|All enabled
指定能力|Selected capabilities
不使用|None
只看已选|Selected only
搜索能力名称|Search capability names
搜索名称、描述或 ID|Search name, description or ID
按需读取已授权的技能；选择“指定能力”后，只有勾选的技能可供此助理使用。|Authorized Skills are loaded on demand. Selected capabilities limits access to checked Skills.
MCP 默认不授权。勾选并保存后才允许调用；全局启用不代表已分配给助理。|MCP access requires explicit selection and saving. Globally enabled services are not automatically assigned to an assistant.
选择此助理可调用的已登记工具。执行仍遵守系统权限与沙盒限制。|Choose registered tools for this assistant. System permissions and sandbox limits still apply.
单独授权 TTS、ASR、图片生成等执行能力；不决定对话 LLM。全部已启用会自动包含后续新增且已接通的模型；指定能力只允许勾选项。|Authorize TTS, ASR, image generation and other model capabilities separately from the chat LLM. All enabled includes future connected models; Selected capabilities includes checked entries only.
可关联技能列表|Related Skills
重新读取工具列表|Reload tools
默认对话模型|Default chat model
使用当前模型|Use current model
跟随全局默认|Use global default
用于理解与回答的 LLM。TTS、ASR、图片生成等执行权限请在“能力范围 → 模型能力”中单独配置。|Select the LLM for conversations. Configure TTS, ASR and image permissions under Capability scope → Model capabilities.
软件工程 · 私有项目沙盒|Coding · Private project sandbox
启用软件工程|Enable coding
允许此助理在用户私有项目中读写代码、运行构建和测试、查看差异并恢复检查点。不依赖特定 Skill；仅在任务需要时加载工具。|Allow code editing, builds, tests, diffs and checkpoint recovery in private projects. Tools are loaded as needed, independently of any Skill.
隔离运行环境|Isolated runtime
请选择已启用的沙盒服务|Select an enabled sandbox service
仅授权该服务声明的软件工程工具，不会自动授予其他 MCP 工具。执行始终在隔离容器中，不能访问宿主机、凭据或网络；额外依赖由管理员准备。|Only the service's declared coding tools are authorized. Execution is isolated from the host, credentials and network. Administrators prepare additional dependencies.
运行与用户端发布|Activation and publication
运行与发布|Activation and publication
启用这个智能助理|Enable this assistant
发布到用户端，允许注册用户在新对话中选择|Publish for registered users in new conversations
身份与任务指引保留在会话快照中；收紧能力范围会对后续调用和恢复检查生效。停用助理会禁止继续调用能力，不会删除已有成果。|Identity and task instructions are saved in the conversation snapshot. Restricted permissions apply to future calls and recovery. Disabling an assistant blocks further calls while preserving existing results.
对话推理使用系统模型配置；并发与预算继续服从系统上限。默认助理始终在用户端可用。|Conversations use system model configuration and concurrency and budget limits. The default assistant is available in the portal.
资料库|Knowledge library
资料分类|Categories
分类|Category
资料名称|Document name
资料内容|Document content
资料库说明（可选）|Library description (optional)
第一步 · 新建资料库|Step 1 · Create library
第二步 · 分类与导入|Step 2 · Categorize and import
第三步 · 授权助理|Step 3 · Authorize assistants
先建立资料库，再分类并导入文件，最后授权助理在相关对话中检索使用。用户私人资料独立保存。|Create a library, organize and import documents, then authorize assistants to retrieve them. Personal documents are stored separately.
先选择资料库|Select a library first
保存资料库|Save library
删除资料库|Delete library
新建|New
每个资料库可单独授权助理。|Each library has separate assistant permissions.
分类可选。先选择分类，再导入文件；新建分类时可选上级，最多三级。|Categories are optional. Select a category before importing. New categories support up to three levels.
请先保存上方资料库；保存后即可创建分类并导入文件。|Save the library before adding categories or importing files.
选择文件导入|Import files
查看与编辑分类|View and edit categories
管理分类（可选）|Manage categories (optional)
分类名称|Category name
输入分类名称|Enter category name
新分类名称|New category name
创建并选中|Create and select
上级分类|Parent category
顶层分类|Root category
顶层分类（第一级）|Root category (level 1)
保存分类|Save category
未分类|Uncategorized
手动添加文字资料|Add text document
标题|Title
正文|Content
标签与来源（可选）|Tags and source (optional)
标签（逗号分隔，最多 8 个）|Tags (comma-separated, up to 8)
来源链接（可选，不自动抓取）|Source URL (optional, not fetched)
来源链接（可选，仅记录出处，不自动抓取）|Source URL (optional, recorded only)
保存资料|Save document
只允许已启用的助理使用本库；修改授权后，新请求立即生效。|Only enabled assistants can use this library. Permission changes apply to new requests immediately.
保存授权|Save permissions
支持 DOCX、文字型 PDF（最大 4 MB），以及 UTF-8 文字文件（最大 2 MB）。DOCX 表格完整保留，PDF 不确定的表格保留版面文字并提示；暂不支持扫描件或图片 OCR。提取文字最多 1 MB。|Supports DOCX and text PDFs (up to 4 MB), and UTF-8 text files (up to 2 MB). DOCX tables are preserved. Uncertain PDF tables retain layout text with a warning. Scans and OCR are not supported. Extracted text is limited to 1 MB.
支持 DOCX、文字型 PDF（最大 4 MB），以及 UTF-8 文字文件（最大 2 MB）；每次最多 10 个。DOCX 表格完整保留，PDF 不确定的表格仅保留版面文字并提示。暂不支持扫描件或图片 OCR。提取文字最多 1 MB。|Supports up to 10 DOCX or text PDF files (4 MB each), or UTF-8 text files (2 MB each). DOCX tables are preserved. Uncertain PDF tables retain layout text with a warning. Scans and OCR are not supported. Extracted text is limited to 1 MB.
查看完整调用路径、各阶段耗时以及脱敏后的输入与返回数据。|Inspect the call flow, timing, and redacted inputs and outputs.
事件时间线|Event timeline
事件展示与翻页|Timeline and pagination
整理视图|Grouped view
原始事件|Raw events
最新事件|Latest events
当前显示部分事件，可翻页或跳到最新事件；运行状态与调用结果不受分页影响。|A page of events is shown. Browse pages or jump to the latest events. Pagination does not affect execution state or results.
连续文字合并展示，原始事件完整保留|Text chunks are grouped; original events are retained
模型请求证据|Model request details
逐次请求显示实际 Token，并对照发送前的上下文估算|Compare reported tokens with context estimates for each request
选择请求组成部分|Select request component
调用路径|Call flow
点击能力或结果节点检查数据|Select a tool or result node to inspect data
调用数据|Call data
调用详情|Call details
关闭调用详情|Close call details
调用参数|Arguments
返回结果|Result
记录类型|Record type
搜索能力或操作|Search capabilities or operations
查看能力的添加、权限、验证、调用与启停记录。|View registration, permission, validation, call and activation records.
管理运行调度、共享资源、登录密码与注册通知。|Manage scheduling, shared resources, passwords and registration notifications.
运行与资源调度|Scheduling and resources
全局 Agent 并发|Global Agent concurrency
单个用户活跃 Run|Active runs per user
所有用户与管理端任务共享。默认 1000。|Shared by all users and administrator tasks. Default: 1000.
跨不同会话并行；同一会话不并行。默认 2。|Different conversations may run concurrently. Turns in one conversation are sequential. Default: 2.
同一会话始终按顺序执行；不同会话可在账号与全局上限内并行。共享资源池可让多个 MCP 共同竞争同一组设备。|Turns run sequentially within a conversation. Other conversations share account and global limits. Multiple MCP services can share a resource pool.
MCP 资源声明|MCP resource declarations
共享资源池|Shared resource pools
添加资源池|Add resource pool
未分配资源池的 MCP 继续使用自身 Manifest 中的并发限制。|MCP services without a pool use their manifest concurrency limits.
例如多项 MCP 共用同一块 GPU 时，将它们指向同一个池；池的并发设为设备可安全承载的任务数。|Assign MCP services using the same GPU to one pool. Set its concurrency to the device's safe capacity.
保存调度配置|Save scheduling
生成文件与下载|Generated files and downloads
视频、音频等生成文件由 ExoHarness 保存，播放与下载使用同一有效期；无需依赖原服务的临时链接。文件仍仅向有权查看该对话的用户开放。|ExoHarness stores generated files with a shared playback and download lifetime, independently of upstream temporary URLs. Conversation access permissions still apply.
访问有效期|Access lifetime
永久有效（默认）|No expiry (default)
指定天数|Custom duration
有效天数|Days valid
保存文件配置|Save file settings
设置对已保存和新生成的文件立即生效，从文件登记时间计算。到期只限制访问，不删除文件；改回永久有效后可继续访问。删除对话时，其生成文件也会删除。|Applies to saved and new files from their registration time. Expiry restricts access without deleting files. Setting no expiry restores access. Deleting a conversation also deletes its generated files.
修改登录密码|Change sign-in password
选择用户|Select user
管理员拥有全部配置与管理权限。|Administrators have full configuration and management access.
新密码|New password
确认新密码|Confirm new password
更新密码|Update password
更新管理员密码|Update administrator password
更新后，该用户类型已有的登录会话会立即退出。|Existing sessions for this user type are signed out immediately.
注册邮件|Registration email
邮件标题|Email subject
邮件正文|Email body
保存邮件内容|Save email content
设置新用户收到的验证码邮件标题和正文。保存后，新发送的邮件会立即使用这里的内容。|Configure the subject and body of verification emails. New emails use the saved content immediately.
是必填的验证码位置；邮件按纯文本发送。|is the required verification-code placeholder. Emails are sent as plain text.
查看对话记录，设置默认助理、能力权限和用户安装的能力。|View conversations and manage assigned assistants, permissions and installed capabilities.
昵称、用户名或邮箱|Nickname, username or email
选择一个用户|Select a user
用户详情、默认助理和执行对话会显示在这里。|User details, assigned assistant and conversations appear here.
默认助理|Default assistant
修改助理只影响新会话；关闭能力使用权限会阻止该用户新的对应调用。用户安装的能力默认私有，仅不含凭据、不可执行的通用 Skill 可由管理员按用户分享；分享不越过助理授权。完整执行记录可进入 Trace。|Assistant changes apply to new conversations. Revoked capability access blocks new calls. Installed capabilities are private by default; administrators may share non-executable Skills without credentials. Assistant permissions still apply. Execution records are available in Trace.
管理当前助理|Manage current assistant
助理配置|Assistant configuration
选择用户查看对话与助理配置|Select a user to view conversations and assistant configuration
今天想一起做什么？|What would you like to create?
制作宣传视频、设计海报、创作音乐，或撰写文档与 PPT——告诉我你的主题和要求。|Create promotional videos, posters, music, documents or presentations. Tell me your topic and requirements.
从一段灵感出发，创作视频、音乐、图像与文档。你的下一件作品，从这里开始。|Turn an idea into videos, music, images or documents. Start your next creation here.
创作|Create
想象|Imagine
新对话|New chat
新会话 · 发送后自动保存|New chat · Saved after your first message
对话|Chat
对话导航|Chat navigation
对话记录|Chat history
搜索历史对话|Search chat history
开始新对话|Start new chat
关闭对话记录|Close chat history
折叠对话侧栏|Collapse sidebar
展开对话侧栏|Expand sidebar
账号菜单|Account menu
我的账号|My account
系统配置|System configuration
模型与能力由管理员统一配置|Models and capabilities are managed by the administrator
添加参考文件|Attach reference files
添加文件|Attach files
发送消息|Send message
发送消息…|Send message…
停止|Stop
停止当前运行|Stop current run
AI 生成内容可能有误，请核对重要信息|AI-generated content may be inaccurate. Verify important information.
Enter 发送 · Shift + Enter 换行|Enter to send · Shift + Enter for a new line
释放文件添加到对话|Drop files to attach to chat
加载更早的消息|Load earlier messages
滚动到最新消息|Scroll to latest message
回到最新消息|Back to latest message
个人资料|Profile
我的能力|My capabilities
我的资料|My documents
我的记忆|My memory
设置|Settings
账号设置|Account settings
修改密码|Change password
外观|Appearance
外观风格|Theme
明亮|Light
暗色|Dark
切换为暗色模式|Switch to dark theme
切换为明亮模式|Switch to light theme
关于我们|About
版本|Version
版本信息|Version information
积分查询|Credits
积分查询即将开放|Credits are coming soon
即将开放|Coming soon
聊天宠物|Chat companion
选择聊天宠物|Choose chat companion
选择聊天中的陪伴角色，也可以随时关闭浮动宠物。|Choose a chat companion or turn off the floating companion.
了解当前版本与 ExoHarness。|About this version of ExoHarness.
ExoHarness 是连接智能助理、模型与工具的通用执行平台。通过对话组织任务，让能力调用、执行进度和生成结果清晰可见。|ExoHarness connects assistants, models and tools. Organize tasks in chat and follow tool calls, progress and results.
昵称或登录名|Nickname or username
当前密码|Current password
再次输入新密码|Re-enter new password
修改后，所有设备上的登录会话都会退出。|Changing your password signs you out on all devices.
已安装的能力|Installed capabilities
发现已有 Skill|Discover Skills
搜索 Skill|Search Skills
使用范围|Access scope
MCP 添加方式|MCP input method
MCP 地址、mcpServers JSON 或 GitHub 项目|MCP URL, mcpServers JSON or GitHub project
管理员授权后，可连接远程 MCP 地址、粘贴地址型 mcpServers JSON，或从公开 GitHub 项目安装本地 MCP。Skill 只从公开 GitHub 项目安装；用户安装 Skill/MCP 不接受上传文件。远程 MCP 所需的认证信息在对应能力下填写，页面只显示“已配置”。|With administrator permission, connect a remote MCP URL, paste URL-based mcpServers JSON, or install a local MCP from public GitHub. Skills can be installed from public GitHub only. User installations do not accept uploaded files. Enter credentials in the capability settings; only configuration status is displayed.
安装 MCP|Install MCP
添加与管理资料|Manage documents
尚无分类。|No categories yet.
编辑|Edit
再点一次确认删除|Click again to confirm deletion
还没有资料。上传后，助理可在你的对话中检索并使用相关内容；这里用于查看和管理。|No documents yet. After uploading, your assistant can find and use relevant content in your conversations. View and manage your documents here.
资料已删除|Document deleted
还没有记忆。只会保存你主动填写的内容。|No memories yet. Only content you explicitly enter is saved.
记忆已删除|Memory deleted
助理可在对话中检索并使用相关资料。|Your assistant can find and use relevant documents during chat.
管理你主动确认的长期偏好。|Manage long-term preferences you explicitly confirm.
保护你的账号安全。|Protect your account.
管理你的显示名称。|Manage your display name.
查看安装进度，管理自己的 MCP 认证信息。|View installation progress and manage your MCP credentials.
分类已保存|Category saved
资料已保存|Document saved
每次最多导入 10 个文件。|Import up to 10 files at a time.
没有找到匹配资料。|No matching documents found.
记忆已保存|Memory saved
个人资料已保存|Profile saved
两次输入的新密码不一致|The new passwords do not match
密码已更新，请重新登录|Password updated. Please sign in again.
请求失败|Request failed
已关闭|Disabled
检查中|Checking
状态暂不可用|Status currently unavailable
重新检查|Check again
Authorization 已配置 · 更新|Authorization configured · Update
填写 Authorization|Enter Authorization
Authorization 值|Authorization value
填写服务提供的完整 Header 值|Enter the complete header value provided by the service
保存并重新检查|Save and check again
认证信息已保存，正在重新检查连接。|Credentials saved. Checking the connection again.
删除认证信息|Delete credentials
认证信息已删除，MCP 已停用；如服务支持免认证，可重新检查连接。|Credentials deleted and MCP disabled. If the service supports access without authentication, check the connection again.
已配置，可更新|Configured; can be updated
待配置|Not configured
仅在此处填写，不要发送到对话|Enter credentials here only, not in chat
正在检查 MCP，完成后即可填写密钥|Checking MCP. Enter credentials after the check completes.
公开 GitHub 来源 · 固定提交安装|Public GitHub source · Installed from a pinned commit
安装中|Installing
取消下载|Cancel download
当前可查看已登记的个人能力；新版自助安装功能尚未接入，不能在此提交安装。|View your registered capabilities here. Self-service installation is not connected in this version.
管理员尚未开放能力安装权限；已安装能力仍按使用权限和助理授权检查。|Your administrator has not enabled capability installation. Installed capabilities still follow user and assistant permissions.
尚未安装私有能力。|No private capabilities installed yet.
安装任务|Installation tasks
MCP 已登记，正在检查来源、运行环境与工具；检查通过后才会启用。|MCP registered. Its source, runtime and tools are being checked. It will be enabled after the checks pass.
助理可在你的对话中检索并使用相关资料；资料只属于当前账号，不会自动分享给其他用户。|Your assistant can retrieve relevant documents during chat. Documents are private to your account.
管理我的资料|Manage my documents
检索我的资料|Search my documents
长期偏好|Long-term preferences
仅保存你明确填写的长期偏好。可随时停用、修改或删除；不会自动从聊天中提取。|Save preferences you explicitly enter. Disable, edit or delete them at any time. Preferences are not extracted automatically from chat.
希望记住的偏好|Preferences to remember
启用这条记忆|Enable this memory
保存记忆|Save memory
返回对话|Back to chat
模型请求中|Waiting for model
正在深度思考...|Thinking deeply...
正在深度思考…|Thinking deeply…
正在思考...|Thinking...
正在思考…|Thinking…
正在回答...|Responding...
正在生成...|Generating...
正在生成…|Generating…
正在调用能力|Calling tool
正在连接|Connecting
正在连接服务|Connecting to service
服务在线|Service online
服务不可用|Service unavailable
正在读取|Loading
正在读取…|Loading…
正在读取 Skill…|Loading Skill…
正在读取对话…|Loading conversations…
正在读取用户…|Loading users…
正在读取能力…|Loading capabilities…
正在读取运行日志…|Loading run logs…
正在读取运行轨迹|Loading trace
正在读取操作记录|Loading audit log
正在读取权限状态。|Loading permissions.
正在读取邮件服务|Loading email settings
正在读取配置|Loading configuration
正在打开设置…|Opening settings…
正在检测语音服务…|Checking speech service…
正在检查|Checking
正在检查沙盒依赖。|Checking sandbox dependencies.
正在检查沙盒依赖|Checking sandbox dependencies
检查沙盒依赖|Check sandbox dependencies
检查不会执行上传脚本；通过检查不等于获得脚本执行授权。缺少依赖会明确列出，不回退宿主机执行。|Validation does not execute uploaded scripts or grant execution permission. Missing dependencies are listed. Execution remains isolated.
检查已登记能力和助理允许范围，不自动执行服务或安装依赖。|Check registered capabilities and assistant permissions without executing services or installing dependencies.
沙盒运行环境|Sandbox runtime
沙盒镜像|Sandbox image
本地工具|Local tools
独立工具|Independent tools
等待模型调用|Waiting for model request
执行资源|Execution resources
调用|Call
查看技术详情|View technical details
查看输入内容|View input
敏感字段已由服务端脱敏|Sensitive fields are redacted by the server
选择一个会话|Select a conversation
名称|Name
说明|Description
上一个|Previous
下一个|Next
上一页|Previous page
下一页|Next page
查询 / 刷新|Query / Refresh
选择本地项目|Select local project
变更范围|Change scope
试用工具|Try tool
推荐|Recommended
保存后不会回显明文|Plaintext is not displayed after saving
输入后安全保存，不会再次显示|Stored securely and not displayed again
搜索|Search
搜索能力|Search capabilities
搜索中…|Searching…
正在搜索…|Searching…
没有找到相关对话|No matching conversations
发送第一条消息后，会话会保存在这里。|Conversations appear here after your first message.
置顶|Pin
取消置顶|Unpin
重命名|Rename
删除|Delete
复制|Copy
已复制|Copied
点赞|Like
踩|Dislike
重新生成|Regenerate
继续运行|Resume
继续执行|Resume
等待授权|Awaiting authorization
可恢复|Recoverable
排队中|Queued
正在执行|Running
运行中|Running
准备中|Preparing
已完成|Completed
完成|Completed
成功|Succeeded
失败|Failed
已失败|Failed
已取消|Cancelled
取消|Cancel
已停止|Stopped
等待中|Waiting
未开始|Not started
闲置|Idle
空闲|Idle
正在录音|Recording
开始录音|Start recording
停止录音|Stop recording
语音输入|Voice input
下载图片|Download image
下载视频|Download video
下载音频|Download audio
下载文件|Download file
下载|Download
预览|Preview
打开预览|Open preview
今天|Today
昨天|Yesterday
过去三天|Last 3 days
更早|Earlier
更多|More
显示更多|Show more
未命名会话|Untitled chat
新对话名称，按回车保存|New chat name; press Enter to save
请先停止正在运行的对话|Stop the active run first
编码|Coding
正在保存…|Saving…
保存|Save
关闭|Close
读取失败|Loading failed
无数据|No data
暂时没有数据|No data yet
暂时没有记录|No records yet
暂无记录|No records yet
暂无数据|No data yet
暂未配置|Not configured
已配置|Configured
不可用|Unavailable
可用|Available
未就绪|Not ready
已就绪|Ready
未启用|Disabled
更新检查|Update check
保存中|Saving
搜索结果|Search results
全部已选|All selected
清除|Clear
复制成功|Copied
显示|Show
隐藏|Hide
关闭宠物|Disable companion
启用宠物|Enable companion
返回|Back
选择文件|Choose file
未选择|Not selected
打开|Open
连接|Connect
工具|Tool
需要登录|Sign-in required
身份|Identity
文档|Document
音频|Audio
图片|Image
视频|Video
运行|Run
详细信息|Details
已保存|Saved
默认|Default
新用户|New user
搜索技能|Search Skills
全选|Select all
取消全选|Clear selection
0 个模型|0 models
0 个助理|0 assistants
例如：主力模型|Example: primary model
例如：压缩指定图片|Example: compress selected images
例如：品牌, 门店|Example: brand, store
例如：品牌规范|Example: brand guidelines
例如：处理项目中的图片文件|Example: process project images
例如：查询东京未来三天的天气|Example: weather in Tokyo for the next three days
例如：查询公司内部知识库|Example: search the company knowledge base
例如：读取本地项目数据|Example: read local project data
例如 小澜|Example: Xiaolan
输入要查找的词句|Enter search terms
配置助理|Configure assistant
配置|Configure
了解当前版本|About this version
注册|Register
注册用户|Registered users
搜索历史|Search history
ExoHarness 技能中心|ExoHarness capabilities
ExoHarness 运行总览|ExoHarness overview
ExoHarness 对话|ExoHarness chat
智能助理配置|Assistant configuration
输入|Input
输出|Output
事件|Events
结果|Result
错误|Error
状态|Status
模型响应|Model response
工具调用|Tool call
工具返回|Tool result
登录失败|Sign-in failed
正在登录…|Signing in…
正在登录|Signing in
登录中…|Signing in…
正在注册…|Registering…
验证码已发送|Verification code sent
密码至少 8 位|Password must be at least 8 characters
请填写邮箱和密码|Enter your email and password
请填写密码|Enter your password
请填写邮箱|Enter your email
两次输入的密码不一致|Passwords do not match
邮箱、用户名或密码不正确|Incorrect email, username or password
登录尝试过多，请稍后再试|Too many sign-in attempts. Try again later.
语音服务不可用|Speech service unavailable
语音服务可用|Speech service available
连接失败|Connection failed
取消请求已发送|Cancellation requested
本次运行失败|Run failed
任务完成|Task completed
任务失败|Task failed
任务已取消|Task cancelled
执行已停止|Execution stopped
需要授权|Authorization required
模型响应未完成|Incomplete model response
连接正常|Connected
已连接|Connected
未连接|Disconnected
等待|Waiting
加载中|Loading
用户端|User portal
管理端|Management console
安全|Security
资料|Documents
能力检查|Capability checks
返回首页|Back to home
提交|Submit
活动任务|Active tasks
授权|Authorize
拒绝|Deny
允许|Allow
消息|Message
无|None
未知|Unknown
请稍候|Please wait
按回车发送|Press Enter to send
关闭菜单|Close menu
打开菜单|Open menu
`
  .trim().split('\n').map(line => line.split('|') as [string, string]);

// Built-in companion copy is scoped to the gallery, never user-owned names.
export const companionCatalog: ReadonlyMap<string, string> = new Map(`
团子|Blob companions
小女孩|Girl companions
芽芽|Sprout
焰焰|Flame
墨墨|Ink
波波|Bubble
核核|Core
团团|Bun
小芽|Mint
小橙|Ember
小夜|Night
小汐|Tide
小芯|Chip
小棉|Cotton
头顶小芽的绿团子|Green companion with a little sprout
蓬尾小狐|Fluffy-tailed fox
夜色小猫|Night-colored cat
会发光的水滴|Glowing water droplet
带天线的小机器人|Little robot with an antenna
垂耳小兔|Floppy-eared bunny
薄荷绿短发，元气校服|Short mint hair and a lively school uniform
橙发狐耳，活泼校服|Orange hair, fox ears and a lively school uniform
紫发猫耳，安静校服|Purple hair, cat ears and a quiet school uniform
蓝发水滴，清爽校服|Blue hair, droplet accent and a fresh school uniform
银发星标，科技校服|Silver hair, star accent and a tech-inspired school uniform
粉发兔夹，甜美校服|Pink hair, bunny clip and a sweet school uniform
`.trim().split('\n').map(line => line.split('|') as [string, string]));

export const catalog: ReadonlyMap<string, string> = new Map(entries);

export function english(source: string): string {
  const direct = catalog.get(source);
  if (direct !== undefined) return direct;
  // Localize UI templates while retaining their names, IDs, URLs and numbers.
  // Only known whole templates are matched; arbitrary content is not translated.
  const patterns: [RegExp, (...parts: string[]) => string][] = [
    [/^更新于 (.+)$/, time => `Updated at ${time}`],
    [/^上次保存：(.+)$/, time => `Last saved: ${time}`],
    [/^(用户筛选|筛选)：(.+)$/, (kind, value) => `${kind === '用户筛选' ? 'User filter' : 'Filter'}: ${value}`],
    [/^第 (\d+) 轮 · 第 (\d+) 次执行$/, (turn, attempt) => `Turn ${turn} · Attempt ${attempt}`],
    [/^第 (\d+) 轮(?: · (本轮执行|重试 (\d+)))?$/, (turn, suffix, retry) => `Turn ${turn}${suffix ? retry ? ` · Retry ${retry}` : ' · Current run' : ''}`],
    [/^(\d+) 轮消息 · ([\d,]+) Token$/, (turns, tokens) => `${turns} turns · ${tokens} tokens`],
    [/^(\d+) 轮消息 · (\d+) 次执行$/, (turns, runs) => `${turns} turns · ${runs} runs`],
    [/^(\d+) 次模型请求 · (\d+) 次能力调用$/, (requests, calls) => `${requests} model requests · ${calls} capability calls`],
    [/^(\d+) 次请求 · (\d+) 次重试 · ([\d,]+) Token$/, (requests, retries, tokens) => `${requests} requests · ${retries} retries · ${tokens} tokens`],
    [/^(\d+) 次模型请求$/, count => `${count} model requests`],
    [/^(\d+) 次(能力调用|请求|重试)$/, (count, kind) => `${count} ${({ '能力调用':'capability calls','请求':'requests','重试':'retries' } as Record<string,string>)[kind]}`],
    [/^相比上次请求：新增 (\d+) 项，移除 (\d+) 项，复用 (\d+) 项。(逻辑输入可核验。|含脱敏或省略内容。) 复用指日志存储去重，不代表模型没有接收或消耗 Token。(?: 已记录 (\d+) 个上下文变换，可核验变换前后的来源。)?$/, (added, removed, retained, exact, changes) => `Compared with the previous request: ${added} added, ${removed} removed, ${retained} retained. ${exact === '逻辑输入可核验。' ? 'Logical input is verifiable.' : 'Includes redacted or omitted content.'} Retention refers to journal deduplication, not avoided model input or tokens.${changes ? ` ${changes} context transformations recorded with verifiable sources.` : ''}`],
    [/^(\d+) 次请求(未提供用量|仅提供部分用量|尚无结束记录)$/, (count, kind) => `${count} requests ${({ '未提供用量':'without usage','仅提供部分用量':'with partial usage','尚无结束记录':'without completion records' } as Record<string,string>)[kind]}`],
    [/^含历史累计 ([\d,]+) Token$/, tokens => `Includes ${tokens} historical tokens`],
    [/^变换 (\d+)$/, sequence => `Transformation ${sequence}`],
    [/^原 (\S+) (\d+)$/, (kind, index) => `Original ${kind} ${index}`],
    [/^([\d,]+) 字节$/, bytes => `${bytes} bytes`],
    [/^(模型配置|系统说明|消息|工具 Schema) (\d+)$/, (label, index) => `${english(label)} ${index}`],
    [/^运行(成功|失败|已取消|可恢复|运行中)$/, status => `Run: ${english(status)}`],
    [/^(\d+) 个文字片段已合并$/, count => `${count} text fragments grouped`],
    [/^查看 (\d+) 条原始事件$/, count => `View ${count} raw events`],
    [/^(\d+) 条展示 · (\d+) 条原始事件(?: · 当前事件 (#.+) \/ 最新 #(\d+))?$/, (shown, raw, range, latest) => `${shown} displayed · ${raw} raw events${range ? ` · Current events ${range} / Latest #${latest}` : ''}`],
    [/^(调用|模型请求) (\d+) \/ (\d+)$/, (kind, index, total) => `${kind === '调用' ? 'Invocation' : 'Model request'} ${index} / ${total}`],
    [/^(读取|写入) (.+)$/, (action, value) => `${action === '读取' ? 'Read' : 'Write'} ${english(value)}`],
    [/^(\d+) 个依赖连接 · (\d+) 个已授权服务$/, (bindings, services) => `${bindings} dependency connections · ${services} authorized services`],
    [/^(\d+) 轮消息 · 更新于 (.+)$/, (turns, time) => `${turns} turns · Updated at ${time}`],
    [/^(\d+) 项能力 · (\d+) 项未就绪$/, (total, blocked) => `${total} capabilities · ${blocked} not ready`],
    [/^(\d+) 项能力$/, count => `${count} capabilities`],
    [/^共 (\d+) 项$/, count => `${count} items`],
    [/^共 (\d+) 条记录$/, count => `${count} records`],
    [/^显示最近 (\d+) 条记录$/, count => `Showing the latest ${count} records`],
    [/^显示 (\d+) 条 · 共 (\d+) 条$/, (shown, total) => `Showing ${shown} of ${total} records`],
    [/^显示 (\d+) 项 · 共 (\d+) 项$/, (shown, total) => `Showing ${shown} of ${total} items`],
    [/^(\d+) 个配置$/, count => `${count} configurations`],
    [/^(\d+) 个会话$/, count => `${count} chats`],
    [/^(\d+) 个$/, count => `${count} total`],
    [/^第 (\d+) 版$/, revision => `Revision ${revision}`],
    [/^(Skill|MCP 服务|Tool) · (v[^ ·]+) · 第 (\d+) 版$/, (kind, version, revision) => `${kind === 'MCP 服务' ? 'MCP service' : kind} · ${version} · Revision ${revision}`],
    [/^(\d+) (分钟|秒)$/, (value, unit) => `${value} ${unit === '分钟' ? 'min' : 'sec'}`],
    [/^(\d+) 次未上报$/, count => `${count} unreported`],
    [/^每用户最多 (\d+) 个活跃 Run$/, count => `Up to ${count} active runs per user`],
    [/^(\d+)\/(\d+) 已授权$/, (allowed, total) => `${allowed}/${total} authorized`],
    [/^(助理|配置名称|协议|接口|Profile|音色|语言|项目)：(.+)$/, (label, value) => `${({'助理':'Assistant','配置名称':'Configuration','协议':'Protocol','接口':'Endpoint','Profile':'Profile','音色':'Voice','语言':'Language','项目':'Project'} as Record<string,string>)[label]}: ${value}`],
    [/^(修改智能助理|更新当前助理) · (.+)$/, (action, name) => `${action === '修改智能助理' ? 'Edit assistant' : 'Update assistant'} · ${name}`],
    [/^上下文 ([\d,]+) · (最大输出 ([\d,]+) tokens|输出长度由模型服务决定)$/, (context, output, max) => `Context ${context} · ${max ? `Maximum output ${max} tokens` : english(output)}`],
    [/^(.+)：执行连接或内容尚未就绪$/, name => `${name}: Execution connection or content not ready`],
    [/^(\d+) 项可用性提醒 · 展开查看$/, count => `${count} availability notices · Expand to view`],
  ];
  for (const [pattern, format] of patterns) {
    const match = source.match(pattern);
    if (match) return format(...match.slice(1));
  }
  // Model metadata and overview summaries are rendered as a single text node.
  if (source.includes('\n')) return source.split('\n').map(english).join('\n');
  if (source.includes(' · ')) return source.split(' · ').map(english).join(' · ');
  if (source.includes(' → ')) return source.split(' → ').map(english).join(' → ');
  if (source.includes(' / ')) return source.split(' / ').map(english).join(' / ');
  const title = source.match(/^(.+) · ExoHarness$/);
  if (title && catalog.has(title[1])) return `${catalog.get(title[1])} · ExoHarness`;
  const decorated = source.match(/^([←＋：\s]*)(.+?)([→\s]*)$/);
  if (decorated && catalog.has(decorated[2])) return decorated[1] + catalog.get(decorated[2]) + decorated[3];
  const example = source.match(/^例如[：:\s]*(.+)$/);
  if (example) return `Example: ${example[1]}`;
  const count = source.match(/^(\d+)\s*个(模型|助理)$/);
  if (count) return `${count[1]} ${count[2] === '模型' ? 'models' : 'assistants'}`;
  const action = source.match(/^(打开对话|对话操作|删除对话)[：:](.*)$/);
  if (action) return `${({'打开对话': 'Open chat', '对话操作': 'Chat actions', '删除对话': 'Delete chat'} as Record<string, string>)[action[1]]}: ${action[2]}`;
  return source;
}
