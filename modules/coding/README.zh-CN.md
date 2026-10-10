# Coding

[English](README.md) | 简体中文

`exoharness-coding` 提供多文件代码任务的路径、检索、精确编辑和补丁算法。文件存取由 Sandbox 完成，工具调用通过 Tools 暴露，项目发布由 Artifacts 处理。

## 算法接口

- [paths.rs](src/paths.rs)：`WorkspacePath::parse` 校验工作区相对路径。
- [search.rs](src/search.rs)：`matches` 检索传入文件的文本，返回路径、行号和匹配摘要。
- [edits.rs](src/edits.rs)：`Document` 与 `ExactEdit` 描述输入，`apply_edits` 按原文匹配生成编辑结果。
- [patch.rs](src/patch.rs)：`parse` 解析补丁，`affected` 列举涉及的文件，`prepare` 验证并生成候选变更。

算法在原始文本上准备候选结果。Sandbox 复核工作区版本后提交文件变更；匹配歧义或陈旧 revision 返回冲突。构建和测试在同一任务工作区执行，失败后保留文件，用后续编辑修复对应位置。

## 测试

[tests/](tests/) 覆盖路径与文本处理、精确编辑、多文件补丁和聚焦检索，使用内存输入：

```sh
cargo test --locked -p exoharness-coding
```

沙盒文件提交见 [Sandbox](../sandbox/README.md)，完整任务流程见 [Coding 工作流](../../docs/CODING_WORKFLOW.md)。
