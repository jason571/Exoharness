# Coding 工作流

Coding 支持代码任务的工作区、检索、局部修改、检查和交付，网页是其中一种场景。语言、业务与视觉规范放在按需读取的 Skill 中；Coding 不另建 Agent 循环，不绕过模型、授权、预算或日志。

## 流程

创建或复用任务工作区 → 聚焦检索/读取 → 写入或原子补丁 → 执行保存的检查 → 依据真实诊断局部修复 → 检查新 revision → 发布最终文件。

Coding 工具 Schema 在任务需要时加载。工作文件持久保存，失败后基于已有文件进行局部修复。

| 工具 | 主要输入 | 语义 |
| --- | --- | --- |
| `coding_search` | workspace_id、query | 返回相关片段、行号、revision、SHA-256 和分页信息 |
| `coding_apply_patch` | workspace_id、expected_revision、patch | 多文件 Add/Update/Move/Delete；一次 CAS 原子提交 |
| `coding_check` | workspace_id、expected_revision、name、runtime、entrypoint | 在沙盒运行已保存的检查，不用内联代码覆盖源码 |
| `coding_publish` | workspace_id、expected_revision、files | 项目 ZIP，可选静态 HTML 预览 |

通用 `workspace_*` 和 `sandbox_run` 继续用于单文件、文档、绘图等任务，不强制打包成网站。

## 修改与失败

路径必须是工作区相对路径；拒绝目录穿越、绝对路径和符号链接越界。补丁锚点要求明确，旧 revision、歧义或部分失败拒绝整批写入。幂等操作不能重新修改已经提交的文件。

检查返回非零退出码时，保留源文件和日志并修复失败部分。检查与发布使用独立操作，发布读取已生成文件。权限拒绝、存储冲突、取消和结果未知按独立状态处理。

默认断网、固定镜像、只读包挂载和资源限制；缺依赖返回明确诊断，不自动联网安装，也不回退宿主机 Shell。旧版 Task / Worker / Worktree / Review / Merge 完整协作流程没有迁入当前 Coding，不能照搬旧 CLI 命令。

## 交付

最终发布清单决定哪些文件可下载；未选择的中间文件仍私有。多页面静态预览保留相对 CSS、JS 和媒体资源，只开放清单成员。CSP 隔离预览，不提供后端服务执行。

项目发布支持最多 128 个文件，单文件上限 8 MiB、总量上限 32 MiB，补丁上限 4 MiB。模型单次输出长度由模型配置和上游接口共同决定。

实现分布：`modules/coding` 纯策略，`sandbox/src/task/coding.rs` 工作区提交，`tools/src/coding*.rs` Schema/派发，`artifacts/src/project.rs` 发布，`api/src/media/project.rs` 预览。

验收覆盖多文件、检查失败后的局部修改、Unicode、版本冲突、取消、ZIP 内容及桌面和手机浏览器资源加载。
