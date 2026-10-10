# 架构设计

[English](ARCHITECTURE.md) | 简体中文

ExoHarness 使用一个应用组合入口、一个扩展宿主和一条 Agent 主循环。管理端、用户端和文件端共享执行事实，但使用不同的路由及授权边界。

```text
管理端 / 用户端          文件端
        │                  │
       API ───────────── Artifacts
        │
 Agent + Tools ── ModelRuntime / MCP / Skills / Sandbox
        │                                  │
   Contracts + Core                  Coding 工作区
        │
 PostgreSQL、配置 generation、Secret Store、私有文件

App 负责配置、扩展注册、依赖注入、监听与关闭，不承载第二套业务循环。
```

## 模块职责

| 目录 | 职责 |
| --- | --- |
| `modules/contracts` | 原生模型/工具协议、执行状态、快照、调用身份、校验和重复调用策略 |
| `modules/core` | 扩展依赖图、类型化服务、资源所有权、生命周期和上下文投影 |
| `modules/agent` | 模型请求、待办调用、预算、有限修正、阶段投影和停止决策 |
| `modules/tools` | 授权目录、能力发现/激活、实时 Schema、执行适配与工作流依赖解析 |
| `modules/api` | HTTP、身份适配、两端业务路由、SSE、Trace 和文件读取 |
| `modules/app` | 组装服务、三端监听、后台任务与优雅关闭 |
| `modules/registry` | 能力版本、权限、健康、发现证据和审计 |
| `modules/configuration` | 模型与助理 generation、校验、私有配置和头像 |
| `modules/model-runtime` | LLM 流式协议、模型测试、语音合成/识别适配 |
| `modules/identity` | 用户、注册、登录会话、密码、安装政策和助理分配 |
| `modules/skills` / `skill-runtime` | 不可变包、说明与资源读取、版本固定的运行环境核验 |
| `modules/mcp` / `secrets` | 标准 MCP、Schema、真实进度、后台句柄、私有凭据 |
| `modules/sandbox` | 独立 OCI 执行、持久工作区、限制、取消和回执 |
| `modules/coding` | 路径、精确编辑、补丁和聚焦检索的纯策略 |
| `modules/artifacts` | 文件持久化、来源、摘要、签名和项目发布 |
| `modules/storage/postgres` | 持久执行/登记/资料事务；同步操作经 blocking 边界运行 |
| `modules/storage/sql-compat` | 身份等模块使用的 PostgreSQL SQL 兼容访问层 |
| `web-console` / `web` | 前端源码与构建资源 |

Contracts 不依赖 HTTP、存储或供应商；Core 不反向依赖业务模块。Agent 通过窄端口调用模型和派发器。HTTP 适配器不复制业务状态机，业务代码不得通过访问整个 API 状态容器绕过模块接口。

## 扩展与生命周期

扩展声明 `ExtensionSpec { id, provides, requires }`，宿主在启动任何扩展前检查重复、缺失、循环和多提供者歧义。类型化 `Service::KEY` 限定发布/解析范围，扩展只能访问声明的依赖。

初始化资源登记所有权，关闭按逆依赖顺序等待释放。部分启动失败也要清理，失败的释放不能被当作成功删除。宿主提供静态注册及生命周期管理，不提供任意动态代码加载、热替换或完整在途代际租约。

## 一次执行

1. 认证后取得用户会话及分配的助理，保存 Turn、Run 和固定模型/能力快照。
2. Agent 根据当前任务发现授权能力，按需读取 Skill 和激活实时工具 Schema。
3. 调用前检查版本、授权、就绪和原生 JSON；准备稳定 invocation。
4. 事务内领取调用资格、更新提交状态和事件；提交事务后才访问外部服务。
5. 可信适配器保存精确回执；MCP 进度直接进入事件流，后台查询不调用模型。
6. 成功产物进入统一文件服务；模型依据交付事实回答，完整调用数据留在日志。

Run admission 在首次模型请求前检查全局和当前用户容量。默认全局 1000、每用户 2，同会话仍串行；超额 Run 保持 queued，通过持久事件显示排队，不调用模型。管理员修改配置后，等待任务采用最新上限，已运行任务不中断。

MCP admission 使用持久 invocation 账本，在短事务内同时检查能力 `limits.max_concurrency` 和关联共享资源池。不同 MCP 可关联同一 GPU 池；未关联能力保留自身限制。等待和取消未提交调用不消耗尝试次数。已提交或结果未知的调用保持占用，直到可信终态回执确认。池关联调整不会释放已领取的在途占位。

## 核心不变量

- 能力导入、运行依赖准备、权限授权和实际执行分别校验。
- 执行日志持久保存调用事实；模型上下文根据日志与当前步骤构建。
- 工具调用/结果按身份成对处理，不截断 JSON、代码或关键工作数据。
- 持久 checkpoint 可以精简模型输入，但不能扩大授权或证明未知调用已经成功。
- 已完成副作用不重复提交；传输断开不能直接判断业务失败。
- 模型请求、参数修正、工具重试分别计数，原样继续不重置预算。
- 取消后晚到回执可以保存，但不能重新启动已结束的 Run。

安全恢复和历史格式兼容的当前边界见 [功能状态](STATUS.md)；数据设计见 [数据库](DATABASE.md)。
