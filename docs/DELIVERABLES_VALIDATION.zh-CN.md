# 成品交付修复与验收

[English testing guide](TESTING.md) | 简体中文

日期：2026-10-10

## 问题

会话 `session-94b7a252166ed3c9eae36d0ca6b488e8` 的 MCP 回执为成功，视频时长 15.083 秒；下载地址位于 `structuredContent.result.download_url`。原成品收集器只读取 `structuredContent.artifacts` 和 `resource_link`，导致聊天响应 `artifacts` 为空。最终回答未附 Markdown 链接时，播放器和下载入口均不展示。媒体工作流的最终视频也使用单文件 `result` 描述，因此存在相同缺口。

MiniMax 的部署配置另有 `H3_MCP_ARTIFACT_URL_TTL_SECONDS=3600`，前一轮媒体工作流的签名修改没有覆盖该服务。

## 实现

通用收集器支持三种明确的文件声明：MCP `resource_link`、`structuredContent.artifacts[]`、`structuredContent.result` 单文件对象。单文件对象必须提供文件名、MIME 与 HTTP(S) 地址；纯文字、嵌套提示词、缺少字段的对象均不转换成文件。错误回执与内部文件标记保持隐藏。同一地址与文件名在多个声明中去重。

此次仅修改成品投影适配；工具发现、模型调用、权限、执行 ID 和 GPU 准入逻辑保持原实现。受控远程保存的同源要求保留。

MiniMax 签名支持 `expires=0`，默认 TTL 改为 0，服务器部署配置同步为 0。有限时长签名仍按原期限失效，签名绑定任务、文件名与期限。修改 URL 参数不能绕过校验。

## 历史恢复

从最新成功 Run 的原始 MCP 回执提取明确的最终文件，复核 MiniMax 持久任务状态、媒体文件元数据、文件存在性、SHA-256 和公网 Range 访问后，为 13 条未归档聊天记录补回永久成品卡片。受影响的目标会话包含在内。

恢复仅更新聊天响应的成品投影。原始工具参数、回执、Token、最终回答和 Run 状态保持不变。每次更新检查原响应仍与备份一致，任一冲突回滚整批事务。未调用模型、生成器、TTS 或视频渲染。

## 验证

- 工具模块 29 项单元测试通过；文件描述测试覆盖视频、音乐、图片、PPTX、DOCX、PDF、去重、内部文件和无效链接。
- MiniMax 签名 3 项、原配置 5 项测试通过；永久链接在推进时钟后有效，有限链接到期与签名篡改均被拒绝。签名测试也在服务器运行通过。
- Rust Clippy 检查通过，本机发布编译完成。
- 公网域名 `media.clhh.tech` 上的真实 MP4、MP3、PNG、PPTX、DOCX 下载检查通过；所有选中链接的 `expires` 均为 0。PPTX/DOCX ZIP 完整性及主 XML 解析通过。
- 实际发布的 Chat 页面资源加载服务器成品：视频解码为 1344×768、约 15.08 秒，音乐能读取有效时长，图片可解码，PPT/Word 下载卡片存在。刷新后保留且无重复卡片。此浏览器测试使用隔离会话接口夹具与真实服务器文件，未冒充用户身份或提交生成任务。
- 服务器暂无 PDF 成品样本，PDF 已覆盖适配单元测试，未宣称完成真实 PDF 下载验收。

## 发布与备份

执行文件 SHA-256：`13d1a9b9b74272b66c718a92ee701bf61ce27ee507da11f0e52003acad39dba9`。

服务器备份：`/mnt/data/ExoHarnessPro/backups/media-delivery-20261010-084506`。包含原二进制、页面、发布清单、MiniMax 签名源码、部署配置、`execution_pro` PostgreSQL dump 与历史聊天投影备份。更新仅替换发布二进制、页面资源和 MiniMax 签名相关源码/配置，保留服务器启动脚本及其余服务地址。主服务与 MiniMax 服务已重启，8787/8788/8790 健康检查通过；未构建镜像、未在服务器编译。
