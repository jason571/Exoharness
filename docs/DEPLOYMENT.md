# 构建与部署

ExoHarness 支持原生二进制和挂载发布包的容器启动。源码编译使用本机 Rust、Python 和 Node.js 工具链；OCI 沙盒和容器托管数据库使用 Docker Engine。代码及页面更新重新发布二进制，运行依赖变化时更新镜像。首次部署步骤见 [英文首页](../README.md) 和 [中文指南](../README.zh-CN.md)。

## 构建

需要 Rust/Cargo 1.88+、Python 3.11+、Node/npm，以及锁文件对应的依赖。离线构建只使用已缓存依赖，不自动下载工具链或构建镜像。

```sh
bash scripts/package-release.sh --offline
# 同一构建入口
bash scripts/build.sh --offline
# Rust 调试构建（省略前端打包）
bash scripts/build.sh --offline --skip-web --profile dev --output /tmp/exoharness-debug
```

前端构建先于 Rust，避免编入旧资源。当前页面使用二进制内嵌资源；只替换 release/web 不保证进程已加载新页面，页面更新也需重新打包并重启相应二进制。

```text
release/
  bin/exoharness
  web/
  scripts/start-native.sh
  scripts/start-docker.sh
  scripts/local-service.py
  scripts/docker-service.py
  build-manifest.json
  README.txt
```

发布包包含二进制、页面资源、启动脚本和清单。数据库、配置 generation、用户文件和凭据保存在独立私有数据目录。发布到另一主机前，检查 CPU 架构、动态库和 glibc ABI 兼容性。

## 原生启动

```sh
bash release/scripts/start-native.sh prepare-db
bash release/scripts/start-native.sh start
bash release/scripts/start-native.sh status
bash release/scripts/start-native.sh restart
bash release/scripts/start-native.sh stop
```

`prepare-db` 使用已缓存 PostgreSQL 镜像创建本项目测试容器/卷；缺镜像停止，不自动 pull/build。可改为已有 PostgreSQL，无须用 Docker 部署数据库。

`access` 显示本地访问凭据，输出不可公开；`set-admin-password` 隐藏输入、更新数据库并撤销旧会话。`data-path` 显示实际私有数据位置。运行目录支持 `EXOHARNESS_LOCAL_DATA_DIR`，在无法保持 Unix 权限的 WSL 挂载盘上使用 Linux 私有目录，不放宽密钥权限检查。

原生日志在实际数据目录的 `service.log`。脚本凭 executable、启动标识和发布摘要验证进程所有权，不只按 PID 杀进程；重启主服务不应重启数据库或业务 MCP。

## 配置项

| 变量 | 用途 |
| --- | --- |
| `EXOHARNESS_BIND` | 管理监听；默认 `127.0.0.1:9797` |
| `EXOHARNESS_PORTAL_BIND` | 用户监听；默认 `127.0.0.1:9798` |
| `EXOHARNESS_MEDIA_BIND` | 文件监听；默认 `127.0.0.1:9790` |
| `EXOHARNESS_MEDIA_PUBLIC_BASE_URL` | 对外下载/预览地址 |
| `EXOHARNESS_DATABASE_URL` | PostgreSQL 地址 |
| `EXOHARNESS_DATABASE_PASSWORD_FILE` | 独立密码文件 |
| `EXOHARNESS_DATABASE_NAMESPACE` | 执行 schema，默认 execution |
| `EXOHARNESS_LOCAL_CREDENTIALS_FILE` | 私有本地访问凭据 |
| `EXOHARNESS_CONFIGURATION_DIR` | 模型/助理私有配置目录 |
| `EXOHARNESS_CAPABILITY_DIRECTORY` | 不可变包及相关能力数据 |
| `EXOHARNESS_SANDBOX_IMAGE` | 已准备的任务运行镜像，运行时固定摘要 |
| `EXOHARNESS_MODEL_REQUEST_TIMEOUT_SECONDS` | 模型请求超时配置 |

App 使用三个独立 IPv4 loopback 端口。通过生产代理部署为 8787 / 8788 / 8790 时，设置对应监听变量。配置项以本表和启动代码为准；公网接入条件见 [安全](SECURITY.md)。

## 容器启动

从源码原生编译 release 后，构建主服务运行镜像：

```sh
docker build -f runtime/server/Dockerfile -t exoharness-server:0.1 .
docker run --rm --network none --read-only exoharness-server:0.1 --version
export EXOHARNESS_RUNTIME_IMAGE=exoharness-server:0.1
```

`runtime/server/Dockerfile.dockerignore` 限制构建上下文，只允许执行文件及构建元数据。主服务镜像与 `runtime/sandbox/Dockerfile` 的任务执行镜像独立。任务沙盒镜像的构建、登记和离线传输见 [沙盒准备](../README.zh-CN.md#准备沙盒执行镜像)。

先停原生监听，再启动容器方式：

```sh
bash release/scripts/start-native.sh stop
bash release/scripts/start-docker.sh start
bash release/scripts/start-docker.sh status
bash release/scripts/start-docker.sh restart
bash release/scripts/start-docker.sh stop
```

使用已缓存兼容镜像，`EXOHARNESS_RUNTIME_IMAGE` 可以指定。启动前以无网络的 `--version` 检验 ABI，失败不自动换镜像或下载。主服务只读挂载二进制等固定资源，私有可变目录单独授权；默认不把宿主 Docker Socket 暴露给 API 容器。

Docker 主服务采用 host network，需要 Linux Docker Engine，或开启 host networking 的 Docker Desktop。容器内外部沙盒执行代理尚待接通；需要 OCI 代码执行能力时采用原生主服务模式。默认容器配置通过独立挂载隔离主服务数据。

## 模型和外部服务

在管理页面配置并测试模型，导入 Skill/MCP 后检查授权和就绪。视频、音乐、图片、TTS、ASR 及渲染服务是独立依赖，不包含在主服务二进制中。

源码树 `scripts/start-asr-local.py` 复用已有 ASR 项目、镜像和权重。该脚本属于本地开发环境，独立发布包另行配置 ASR 服务。ASR 验收使用已知文本录音核对转写结果。

## 更新与回退

1. 检查运行中任务和未知 invocation，避免通过重启隐藏外部作业。
2. 校验发布清单和目标 ABI；备份执行文件、数据库及关联私有数据。
3. 替换发布文件，不覆盖数据目录，重启三端服务。
4. 检查健康、认证、配置、事件、代表性下载及实际任务。
5. 回退必须核对数据格式，不因换回二进制而重复提交未知调用。

更新时保留执行日志和结果未知的调用记录，按现有恢复策略继续处理。测试命令见 [测试](TESTING.md)。
