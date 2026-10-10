# ExoHarness

[简体中文](README.md) | [English](README.en.md) · [在线体验](https://agent.clhh.tech/) · [技术文档](docs/README.md)

ExoHarness 是基于 Rust 的通用 Agent 执行平台，提供多用户聊天、智能助理配置、Skill/MCP 接入、隔离代码执行和统一文件交付。执行过程持久化到 PostgreSQL，管理端可查看模型请求、工具参数、运行事件和生成文件。

## 在线体验与交流

打开 [ExoHarness 在线体验](https://agent.clhh.tech/)，注册并登录后即可使用已授权的助理与能力。请勿在公开演示环境上传敏感资料。

微信群用于技术交流、部署讨论和问题反馈。二维码有效期见图片，过期后会更新。

<img src="docs/assets/wechat-group.jpg" alt="ExoHarness 技术讨论群二维码" width="300" />

English 首页已提供项目简介和文档入口，完整英文文档将陆续补充。

## 功能

- **管理端**：模型与助理配置、能力导入与验证、用户管理、资料管理、运行总览、Trace 与审计。
- **用户端**：流式聊天、历史搜索与会话管理、语音输入、个人资料与记忆、生成文件预览和下载。
- **能力接入**：Skill 包版本管理、标准远程 MCP、实时 JSON Schema 校验、用户与助理授权、独立凭据存储。
- **执行管理**：持久化 Session、Turn、Run 和调用日志；固定执行快照；参数修正预算、重复调用保护、进度与取消。
- **Coding**：多文件项目检索、原子补丁、构建与测试、项目 ZIP 交付和静态网页预览。
- **沙盒**：Python、Shell、JavaScript、TypeScript 通过 OCI 容器执行，支持工作文件复用、资源限制和取消。
- **文件服务**：统一保存图片、音频、视频和文档，记录来源及 SHA-256，支持签名访问与 Range 播放。
- **界面语言**：管理端和用户端支持中文、英文，语言选择保存在当前浏览器。用户消息、模型回复及工具原始数据保持原文。

文档、PPT、网页、课件、音乐、视频和 MV 由已授权的 Skill、MCP 或模型能力提供。部署相应任务时，需同时准备其外部服务和依赖。具体支持范围见 [功能状态](docs/STATUS.md)。

本仓库发布核心框架、前端、测试、部署脚本和技术文档。`skills/` 与 `mcp/` 的具体能力包独立维护，不随本仓库分发；`modules/skills` 和 `modules/mcp` 是框架接入模块，包含在本仓库中。部署后通过管理端导入所需能力。

## 运行结构

```text
浏览器 ── 管理端 9797 / 用户端 9798
                    │
                 ExoHarness
                    ├── PostgreSQL：用户、会话、执行日志、资料
                    ├── 配置与能力目录：模型、助理、Skill 包、凭据
                    ├── OCI 沙盒：代码、脚本、工作文件
                    └── MCP / 模型 API：外部执行能力
浏览器 ── 文件端 9790：下载、媒体播放、网页预览
```

三个 HTTP 监听默认绑定 `127.0.0.1`，端口可配置为 8787、8788、8790。源码按模块组织于 `modules/`；模块测试位于各模块的 `tests/`，脚本和浏览器测试位于 `scripts/tests/`。

## 环境要求

| 环境 | 用途 |
| --- | --- |
| Linux / WSL2 | 当前构建和运行脚本的目标环境 |
| Rust / Cargo 1.88+ | 编译核心服务，使用 `Cargo.lock` |
| Python 3.11+ | 构建、发布和启动脚本 |
| Node.js 22+ / npm | TypeScript 检查及前端打包，使用 `package-lock.json` |
| C/C++ 链接工具、pkg-config、OpenSSL 开发库 | Rust 原生依赖的编译与链接 |
| PostgreSQL | 用户、能力登记、会话、执行日志及资料存储 |
| Docker Engine | OCI 任务沙盒；也可托管 PostgreSQL 和主服务 |

主服务可直接运行二进制，数据库可采用独立 PostgreSQL。代码执行使用 Docker OCI 后端，启用沙盒时需 Docker Engine。运行服务的普通用户需拥有 Docker 使用权限；该权限仅授予受信任的服务账号。

Debian / Ubuntu 编译依赖：

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev python3 ca-certificates
rustc --version
cargo --version
python3 --version
node --version
npm --version
```

Rust、Node.js 和 Docker 按各自发行方式安装。执行下列命令前进入仓库根目录。

## 从源码编译

首次构建：

```sh
git clone git@github.com:jason571/Exoharness.git
cd Exoharness
cargo fetch --locked
npm --prefix web-console ci
bash scripts/package-release.sh
```

依赖已缓存时：

```sh
bash scripts/package-release.sh --offline
```

构建脚本先编译前端，再编译 Rust，最后生成完整 `release/`。页面资源内嵌于二进制，更新前端后需重新打包并重启服务。源码编译使用本机工具链，独立于 Docker。

```text
release/
  bin/exoharness
  web/
  scripts/
    start-native.sh
    start-docker.sh
    local-service.py
    docker-service.py
  build-manifest.json
  README.txt
```

发布包包含执行文件、页面资源、启动脚本和校验清单。数据库、配置、凭据、用户文件及 Skill 包通过独立数据目录保存。跨主机发布时，目标系统的 CPU 架构、动态库和 glibc ABI 需与编译产物兼容。

## 准备数据库

本地启动脚本使用 Docker 管理独立 PostgreSQL 容器和数据卷。先显式获取脚本使用的镜像：

```sh
docker pull postgres:18.6-alpine3.23
bash release/scripts/start-native.sh prepare-db
bash release/scripts/start-native.sh data-path
bash release/scripts/start-native.sh access
bash release/scripts/start-native.sh set-admin-password
```

`prepare-db` 创建数据库、私有配置和随机访问凭据，PostgreSQL 默认映射至 `127.0.0.1:9799`。`access` 的凭据输出仅供部署人员使用，`set-admin-password` 使用隐藏输入修改密码。

服务数据目录可通过 `EXOHARNESS_LOCAL_DATA_DIR` 指定。WSL 下，私有文件放在支持 Unix 权限的 Linux 文件系统中；密钥文件使用 `0600`，目录使用 `0700`。

## 准备沙盒执行镜像

`runtime/sandbox/Dockerfile` 提供基础执行镜像，包含 Python、Bash、Node.js 24 和常用绘图、Office、PDF 处理依赖。Python 依赖版本见 `runtime/sandbox/requirements.txt`。TypeScript 使用 Node.js 原生类型转换。

```sh
docker build -t exoharness-sandbox:0.1 runtime/sandbox
docker run --rm --network none --entrypoint python exoharness-sandbox:0.1 --version
docker run --rm --network none --entrypoint node exoharness-sandbox:0.1 --version
bash release/scripts/start-native.sh set-runtime-image --image exoharness-sandbox:0.1
```

特定 Skill 的额外依赖需加入镜像并重新构建。已验证的既有镜像也可通过 `set-runtime-image` 接入。框架固定镜像摘要，每个任务使用独立容器、只读根文件系统、只读 Skill 包和可写工作区；默认网络关闭，并限制 CPU、内存、PID、运行时间和输出。

离线分发镜像：

```sh
docker save exoharness-sandbox:0.1 -o exoharness-sandbox.tar
# 将 tar 文件复制到目标主机后
docker load -i exoharness-sandbox.tar
```

代码与页面更新重新编译发布包，运行依赖变化时重建执行镜像。视频、音乐、ASR 和课件等独立 MCP 的镜像、权重及 GPU 配置由对应服务管理。

## 原生运行

使用前述本地数据库配置：

```sh
bash release/scripts/start-native.sh start
bash release/scripts/start-native.sh status
bash release/scripts/start-native.sh restart
bash release/scripts/start-native.sh stop
```

- 管理端：<http://127.0.0.1:9797>
- 用户端：<http://127.0.0.1:9798>
- 文件服务：<http://127.0.0.1:9790>

`start` 启动三端主服务；`restart` 和 `stop` 仅管理主服务进程。数据库和外部 MCP 独立运行。日志在实际数据目录的 `service.log`。主服务以普通用户运行，OCI 沙盒要求该用户具有 Docker 使用权限。

### 使用已有 PostgreSQL

系统安装或独立部署的 PostgreSQL 可直接连接。先创建数据库和角色：

```sql
CREATE ROLE exoharness LOGIN PASSWORD '<database-password>';
CREATE DATABASE exoharness OWNER exoharness;
```

在归服务账号所有的私有目录中创建 `credentials.json` 与 `postgres-password`，两文件权限均为 `0600`，目录权限为 `0700`。`credentials.json` 格式如下，部署时替换占位值：

```json
[
  { "id": "admin", "role": "admin", "token": "<random-administrator-token-at-least-32-characters>" },
  { "id": "tester", "role": "user", "token": "<different-random-user-token-at-least-32-characters>" }
]
```

`postgres-password` 填写数据库角色密码。以下示例使用 `/var/lib/exoharness`，该目录需归运行服务的普通用户所有：

```sh
export EXOHARNESS_DATABASE_URL=postgresql://exoharness@127.0.0.1:5432/exoharness
export EXOHARNESS_DATABASE_PASSWORD_FILE=/var/lib/exoharness/postgres-password
export EXOHARNESS_LOCAL_CREDENTIALS_FILE=/var/lib/exoharness/credentials.json
export EXOHARNESS_CONFIGURATION_DIR=/var/lib/exoharness/configuration
export EXOHARNESS_CAPABILITY_DIRECTORY=/var/lib/exoharness/capabilities
export EXOHARNESS_DATABASE_NAMESPACE=execution
export EXOHARNESS_SANDBOX_IMAGE=exoharness-sandbox:0.1
./release/bin/exoharness
```

服务启动时初始化所需表。无代码执行需求时可省略 `EXOHARNESS_SANDBOX_IMAGE`。直接启动方式使用上述环境配置；`start-native.sh` 使用 `prepare-db` 创建的本地配置。长期运行可由 systemd 托管。

## Docker 运行

主服务容器通过挂载发布文件运行，基础镜像需事先缓存：

```sh
docker pull ubuntu:24.04
bash release/scripts/start-native.sh stop
bash release/scripts/start-docker.sh start
bash release/scripts/start-docker.sh status
bash release/scripts/start-docker.sh restart
bash release/scripts/start-docker.sh stop
```

脚本复用 `prepare-db` 生成的数据目录，`EXOHARNESS_RUNTIME_IMAGE` 可指定兼容基础镜像。启动前检查二进制 ABI；容器使用 host network、非 root、只读根目录和受限挂载。需要 Linux Docker Engine，或启用 host networking 的 Docker Desktop。

当前主服务容器支持聊天、管理及远程 MCP 接入。OCI 任务执行后端依赖宿主 Docker CLI，容器模式的外部沙盒执行代理待接通。需要完整 Python、Shell 和 Coding 执行能力时采用原生主服务。主服务容器与任务沙盒具有独立执行边界。

## 模型和能力配置

登录管理端后：

1. 添加 LLM、TTS、ASR 等模型连接，填写凭据并测试接口。
2. 导入 Skill 包、MCP JSON 或服务地址，检查权限和依赖后启用。
3. 配置助理身份、对话模型和 Skill/MCP/模型能力授权。
4. 为用户分配默认助理和使用权限。
5. 在用户端执行代表性任务，在 Trace 中核对调用及最终文件。

页面右上角可切换中文、英文。语音输入需独立 ASR 服务，注册邮件需 SMTP。文件对外地址使用 `EXOHARNESS_MEDIA_PUBLIC_BASE_URL`，例如 `https://media.example.com`。公网通过反向代理提供 HTTPS；当前认证 Origin 和数据库 NoTls 限制见 [安全说明](docs/SECURITY.md)。

## 测试与更新

```sh
export EXOHARNESS_PRO_TEST_SANDBOX_IMAGE=exoharness-sandbox:0.1
bash scripts/test.sh --unit-only
npm --prefix web-console run typecheck
node scripts/tests/browser_i18n.mjs
```

浏览器测试使用 Chrome。`--unit-only` 排除 PostgreSQL 集成测试，仍包含脚本中的隔离文档解析用例，因此需要预置沙盒镜像和 Docker。完整数据库测试使用独立测试环境，详见 [测试说明](docs/TESTING.md)。更新前检查活动任务，备份数据库和私有数据，校验发布清单及 ABI。替换发布包并重启后，检查登录、模型连接、能力调用、事件流及文件播放。独立数据目录在更新时保留。

## 技术文档

- [架构](docs/ARCHITECTURE.md) / [API](docs/INTERFACES.md) / [数据库](docs/DATABASE.md)
- [上下文与 Token](docs/CONTEXT_AND_TOKENS.md) / [能力接入](docs/CAPABILITIES.md) / [Coding](docs/CODING_WORKFLOW.md)
- [部署](docs/DEPLOYMENT.md) / [安全](docs/SECURITY.md) / [测试](docs/TESTING.md) / [完整索引](docs/README.md)
- [贡献约定](CONTRIBUTING.md) / [开源检查清单](docs/OPEN_SOURCE_CHECKLIST.md)

核心代码采用 [MIT License](LICENSE)。外部 Skill、MCP、运行镜像和模型权重按各自许可证使用；第三方依赖及资源保留原有许可。
