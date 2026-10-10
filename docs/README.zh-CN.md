# 文档索引

[English](README.md) | 简体中文

ExoHarness 技术文档涵盖系统架构、接口、数据存储、执行策略和部署运维。功能范围与当前限制集中列于 [功能状态](STATUS.md)。

| 文档 | 读者与内容 |
| --- | --- |
| [商业授权](../COMMERCIAL-LICENSE.zh-CN.md) | AGPL 条款与单独商业授权方式 |
| [架构](ARCHITECTURE.md) | 模块职责、扩展生命周期、执行链与核心不变量 |
| [API 接口](INTERFACES.md) | 管理、用户、文件三端的路由、认证、请求和事件 |
| [数据库](DATABASE.md) | 表分组、事务、配置文件、迁移与备份边界 |
| [上下文与 Token](CONTEXT_AND_TOKENS.md) | 日志与模型投影、按需能力、checkpoint、统计与防循环 |
| [能力接入](CAPABILITIES.md) | Skill/MCP 导入、准备、授权、Schema、进度和并发 |
| [模型配置](MODEL_CONFIGURATION_DESIGN.md) | LLM/TTS/ASR 配置、协议支持及助理选择 |
| [助理配置](ASSISTANT_CONFIGURATION_DESIGN.md) | 身份、头像、能力授权与用户分配 |
| [Coding 工作流](CODING_WORKFLOW.md) | 多文件代码生成、局部修复、检查和交付 |
| [资料与记忆](KNOWLEDGE_DOCUMENT_IMPORT.md) | 分类、DOCX/文字型 PDF、表格与检索限制 |
| [文件交付](MEDIA_DELIVERY_DESIGN.md) | 产物、签名、音视频播放与网页预览 |
| [界面与语言](WEB_CONSOLE.md) | 页面资源、中文/英文、数据边界及浏览器测试 |
| [部署](DEPLOYMENT.md) | 原生构建、发布包、单机/容器启动与配置 |
| [安全](SECURITY.md) | 租户隔离、凭据、沙盒、网络与公网接入 |
| [测试](TESTING.md) | 单元、数据库、协议、OCI、浏览器和真实模型验收 |
| [数据迁移](MIGRATION.md) | 历史配置和数据复用，旧 Run 格式边界 |
| [功能状态](STATUS.md) | 已接通能力与真实剩余限制 |
| [开源检查](OPEN_SOURCE_CHECKLIST.md) | 私有数据排除、许可证、发布与检查顺序 |
| [发布包说明](RELEASE.txt) | 独立 release 的启动说明模板 |

`legacy-api-inventory.json` 保存历史版本的路由清单，供迁移核对。当前接口见 [API 文档](INTERFACES.md)，路由实现在 `modules/api/src/`。

权限、调用回执、文件交付、上下文、能力导入及测试规范按主题组织；历史路由清单用于迁移核对。
