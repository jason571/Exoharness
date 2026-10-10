# Skills

`exoharness-skills` 管理 Skill 包的导入、版本、说明和资源读取。包内容按摘要保存，能力登记记录引用对应 revision；执行时读取固定版本的包快照。

## 主要接口

- [service.rs](src/service.rs)：`Skills::preview` 检查导入内容，`import` 保存包并登记版本，`instructions` 和 `instruction_view` 读取说明，`files` 列举资源，`remove` 处理删除。
- [package.rs](src/package.rs)、[archive.rs](src/archive.rs)：解码输入、检查包路径和归档结构。
- [metadata.rs](src/metadata.rs)、[normalization.rs](src/normalization.rs)：解析 `SKILL.md` 元信息并规范化导入结构。
- [snapshot.rs](src/snapshot.rs)：`PackageSnapshot` 固定执行使用的包身份和资源。
- [storage.rs](src/storage.rs)：保存包文件与摘要。

导入过程读取、校验并保存内容，脚本运行另行经过环境检查与执行授权。资源读取遵循包内路径和当前访问范围；删除由登记记录和包引用共同决定。Skill 的任务规范、参考资料和脚本属于包内容。

## 测试

[tests/packages.rs](tests/packages.rs) 检查包格式与路径，[tests/service.rs](tests/service.rs) 验证登记、资源和版本。服务用例需要独立 PostgreSQL：

```sh
cargo test --locked -p exoharness-skills
```

环境准备见 [测试与验收](../../docs/TESTING.md)，接入方式见 [能力接入](../../docs/CAPABILITIES.md)。
