# Artifacts

[English](README.md) | 简体中文

`exoharness-artifacts` 保存执行产物并生成统一访问地址。文件记录包含 owner、来源 Run、名称、媒体类型、大小和 SHA-256；二进制内容保存在独立文件目录。

## 主要接口

- [service.rs](src/service.rs)：`Artifacts` 提供文件发布、签名 URL、授权读取及健康检查。
- [files.rs](src/files.rs)、[types.rs](src/types.rs)：文件处理、`PublishedArtifact` 和错误类型。
- [signing.rs](src/signing.rs)：签名生成、到期与验证。
- [remote.rs](src/remote.rs)：`RemoteFile`、`same_service_origin` 和远程产物导入，校验来源与下载条件。
- [project.rs](src/project.rs)：多文件静态项目发布和签名预览。

沙盒输出、模型音频和 MCP 产物经执行适配器交给本服务。下载地址使用配置的公开基址。静态项目保留资源目录及相对路径，API 文件端负责响应头、Range 和浏览器预览。

## 测试

[tests/](tests/) 覆盖文件保存、来源校验、远程导入、签名和项目资源。用例需要独立 PostgreSQL，远程下载使用本地测试服务：

```sh
cargo test --locked -p exoharness-artifacts
```

相关说明：[文件交付](../../docs/MEDIA_DELIVERY_DESIGN.md)、[数据库](../../docs/DATABASE.md)。
