# ExoHarness

[English](README.md) | 简体中文 · [在线体验](https://agent.flyang.art/) · [演示服务管理端](https://harness.flyang.art/) · [技术文档](docs/README.md)

**基于 Rust 的可扩展 Agent Harness 框架，提供可自行部署的完整 Web 控制台。**

版权所有 © 2026 Jason Yang · [联系作者](mailto:flyangqn@gmail.com)

ExoHarness 为智能体提供通用运行基础：模型接入、上下文管理、能力发现、授权、工具执行、进度、持久化和追踪。开发者与管理员通过接入 Skill、MCP 服务和模型，为助理组合不同的业务能力。

框架可用于研究分析、软件工程、业务操作、内容创作等不同领域。多用户 Web 应用为这些智能体提供统一的聊天入口、管理控制台和文件服务；每个助理的实际能力由接入服务及授权范围决定。

## 界面演示

[打开用户端](https://agent.flyang.art/) · [打开演示管理端](https://harness.flyang.art/) · [下载发布包](https://github.com/jason571/Exoharness/releases)

**聊天、结果预览与文件下载**

![ExoHarness 用户端：任务回复、流程预览和报告下载](docs/assets/demo-chat.png)

**管理端：任务状态、执行资源与用量**

![ExoHarness 管理端：运行总览与任务追踪入口](docs/assets/demo-management.png)

截图使用实际前端和示例数据，不代表真实模型或 MCP 执行。运行 `node scripts/capture-demo.mjs` 可复现截图，需要 Node 22+ 和 Google Chrome。在线执行任务需要配置模型并授权相应能力。

## 下载并运行 Linux 发布包

首个公开预览版适用于 **Linux x86_64、glibc 2.39 或更新版本**，例如 Ubuntu 24.04。安装 Python 3.11+ 和 Docker Engine，在 [Releases](https://github.com/jason571/Exoharness/releases) 下载 `.tar.gz` 与 `SHA256SUMS`：

```bash
sha256sum --check SHA256SUMS --ignore-missing
tar -xzf exoharness-v0.1.0-linux-x86_64-gnu.tar.gz
cd exoharness-v0.1.0-linux-x86_64-gnu
docker pull postgres:18.6-alpine3.23
bash scripts/start-native.sh prepare-db
bash scripts/start-native.sh start
bash scripts/start-native.sh access
```

管理端为 `http://127.0.0.1:9797/`，用户端为 `http://127.0.0.1:9798/`。`access` 显示本地私有凭据。执行任务前配置模型和助理；用户注册需配置 SMTP。模型凭据、能力包、外部 MCP 服务及 OCI 执行镜像需另行准备。详细步骤见包内 `README.zh-CN.txt` 和 [构建部署说明](docs/DEPLOYMENT.zh-CN.md)。

## 如何扩展能力？

核心框架与业务能力分工明确：

- **Skill**：提供可复用的工作方法、任务规范、参考资料及可选脚本。
- **MCP 服务**：提供工具接口，将智能体连接到 API、业务应用、数据源和专业执行服务。
- **模型连接**：提供推理、语音及已支持协议的其他模型能力。
- **Harness**：统一处理发现、上下文、权限、调用记录、执行保护、进度和结果交付。

增加兼容的 Skill 或 MCP，准备其执行依赖并授予权限后，即可沿用现有的发现与执行链，无需为每个业务场景修改 Agent 主循环。协议支持范围见 [能力接入](docs/CAPABILITIES.md) 和 [模型配置](docs/MODEL_CONFIGURATION_DESIGN.md)。

同一套部署可以配置职责不同、工具权限不同的助理。开发者也可以接入自己的服务，通过框架的执行契约与 API 组合新的应用和工作流。

## 应用示例

以下场景展示能力组合的使用方式，作为接入后的应用示例。其他领域的 Skill 和 MCP 同样可以在框架中组合使用。

| 示例场景 | 请求示例 | 可能的交付及依赖 |
| --- | --- | --- |
| 联网研究与数据分析 | 搜索美国近年 GDP，并绘制趋势图 | 搜索资料、图表或表格；需要授权搜索工具及沙盒执行。 |
| 办公文档 | 生成娱乐行业分析报告和 PPT | PPTX、DOCX 或 PDF 下载；需要相应 Skill 及沙盒文档库。 |
| 网页与代码 | 制作中英文文旅网站，包含首页和详情页 | 多文件项目、网页预览和 ZIP；使用 Coding 模块及沙盒。 |
| 图片与海报 | 为公司设计宣传海报 | 图片预览及下载；需要图片生成服务与设计规范。 |
| 音乐与视频 | 生成带解说的宣传视频，或为上传的歌曲制作 MV | 音视频成品；需要生成、配音及合成服务，具体流程由能力包提供。 |
| 教学内容 | 为小学生解释一道题并生成动画 | 讲解音频和视频；需要教学 Skill、TTS 与动画渲染服务。 |

每个示例需要安装并授权对应能力；具体业务范围随接入的 Skill、MCP 和模型扩展。根目录 `skills/` 和 `mcp/` 作为独立项目维护，部署后通过管理端导入。

## 一次任务如何执行？

1. 管理员配置模型、导入能力，为用户分配助理和权限。
2. 用户通过流式聊天提出需求，可以附带参考文件。
3. 助理发现已授权能力，并加载当前步骤需要的工具 Schema。
4. MCP 或沙盒执行实际任务，进度和调用记录写入数据库。
5. 用户获得任务回复或结果文件，需要时查看预览并下载；管理员通过 Trace 检查请求、参数、结果、错误和 Token 用量。

## 在线体验与交流

打开 [ExoHarness 在线体验](https://agent.flyang.art/)，注册并登录后即可使用已授权的助理与能力。请勿在公开演示环境上传敏感资料。

演示服务管理端：[harness.flyang.art](https://harness.flyang.art/)。

扫码添加微信，交流部署、反馈问题或申请加入技术讨论群。

<img src="docs/assets/wechat-group.jpg" alt="ExoHarness 技术交流微信二维码" width="300" />

GitHub 默认首页为英文，可通过顶部入口切换到本中文说明。完整技术文档默认提供英文版，并可在各文档页切换到中文。

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

三个 HTTP 监听默认绑定 `127.0.0.1`，端口可通过环境变量配置。源码按模块组织于 `modules/`；模块测试位于各模块的 `tests/`，脚本和浏览器测试位于 `scripts/tests/`。

## 技术栈与实现

| 层级 | 实现 | 源码位置 |
| --- | --- | --- |
| 核心框架与服务端 | Rust：Agent 循环、扩展、上下文、工具、权限、数据库访问、HTTP/SSE API 和文件服务 | `modules/` |
| 管理端与聊天页面 | TypeScript 编译为 JavaScript，配合 HTML、CSS 及已有 JavaScript 页面代码；页面在浏览器运行，后端仍是 Rust | `web-console/src/`、`web/` |
| 构建、部署与迁移 | Python 脚本，Shell 入口 | `scripts/` |
| 测试 | Rust 单元/协议/集成测试、Python 场景及脚本测试、Node.js 浏览器和界面测试 | `modules/*/tests/`、`scripts/tests/`、`web-console/tests/` |
| 任务沙盒 | Docker OCI 容器，包含 Python、Bash、Node.js、文档库及媒体工具 | `modules/sandbox/`、`runtime/sandbox/` |
| 数据库 | PostgreSQL，可独立运行或使用 Docker | `modules/storage/` |

## 环境要求

Python 不只是测试依赖，需要区分宿主机工具环境与任务沙盒：

- **Rust 核心服务**：编译后是独立执行文件，直接启动二进制进行基本聊天和远程 MCP 调用不依赖宿主机 Python。
- **宿主机 Python 3.11+**：用于随附的构建、打包、数据库准备、启动、迁移和测试脚本。
- **沙盒 Python**：执行用户任务中的 Python 代码、文档处理和隔离文档导入，与宿主机环境分开。
- **宿主机 Node.js**：用于编译前端；已编译的发布包不需要宿主机 Node.js 来提供页面。
- **外部 MCP / 模型服务**：按各自的部署方式准备依赖，安装宿主机 Conda 包不会自动改变任务沙盒。

| 环境 | 用途 |
| --- | --- |
| Linux / WSL2 | 当前构建和运行脚本的目标环境 |
| Rust / Cargo 1.88+ | 编译核心服务，使用 `Cargo.lock` |
| 宿主机 Python 3.11+ | 构建、发布、数据库准备、启动、迁移及测试脚本 |
| Node.js 22+ / npm | TypeScript 检查及前端打包，使用 `package-lock.json` |
| C/C++ 链接工具、pkg-config、OpenSSL 开发库 | Rust 原生依赖的编译与链接 |
| PostgreSQL | 用户、能力登记、会话、执行日志及资料存储 |
| Docker Engine | OCI 任务沙盒；也可托管 PostgreSQL 和主服务 |
| 沙盒内 Python / Node.js | 隔离执行任务代码、文档处理及资料导入 |

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

建议使用 Docker 托管 PostgreSQL 和任务沙盒。当前完整能力部署采用“原生主服务＋Docker 数据库与沙盒”；主服务也可容器化运行，适合聊天、管理和远程 MCP，容器内启动任务沙盒的执行代理尚未接通。

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

主服务运行镜像与任务沙盒镜像分开。前者运行 Rust 框架及 API，后者隔离执行用户任务。先完成源码编译和 `prepare-db`，再从仓库根目录构建主服务镜像：

```sh
docker build -f runtime/server/Dockerfile -t exoharness-server:0.1 .
docker run --rm --network none --read-only exoharness-server:0.1 --version
export EXOHARNESS_RUNTIME_IMAGE=exoharness-server:0.1

bash release/scripts/start-native.sh stop
bash release/scripts/start-docker.sh start
bash release/scripts/start-docker.sh status
bash release/scripts/start-docker.sh restart
bash release/scripts/start-docker.sh stop
```

`runtime/server/Dockerfile` 将已编译的二进制装入 Ubuntu 运行镜像。专用 Docker context 白名单只允许二进制及构建元数据，私有配置、密钥、用户数据和能力包不进入构建上下文。源码编译仍使用本机工具链。

启动脚本复用 `prepare-db` 生成的数据目录，挂载当前 release 和独立私有配置，不自动构建或拉取镜像。启动前检查二进制 ABI；容器使用 host network、非 root、只读根目录和受限挂载。需要 Linux Docker Engine，或启用 host networking 的 Docker Desktop。

当前主服务容器支持聊天、管理及远程 MCP 接入。OCI 任务执行后端依赖宿主 Docker CLI，容器模式的外部沙盒执行代理待接通。需要完整 Python、Shell 和 Coding 执行能力时采用原生主服务。主服务容器与任务沙盒具有独立执行边界。

查看主服务容器日志：

```sh
docker logs --tail 100 exoharness-pro-local-service
```

将主服务与沙盒镜像分发到另一台主机：

```sh
docker save exoharness-server:0.1 exoharness-sandbox:0.1 -o exoharness-images.tar
# 目标主机执行
docker load -i exoharness-images.tar
```

仅修改代码或页面时，重新编译 release 并重启即可，启动脚本会挂载新执行文件，无需重建运行镜像。运行依赖变化时重建对应镜像；跨主机部署需匹配 CPU 架构和系统库 ABI。

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

核心代码采用 [GNU Affero 通用公共许可证第 3 版](LICENSE)（`AGPL-3.0-only`）。遵循 AGPL 条款（包括修改后的网络服务向用户提供对应源码）即可免费使用。企业若需要在不履行 AGPL 义务的情况下使用代码，例如集成到闭源商业产品，应另行购买商业授权。联系邮箱：[flyangqn@gmail.com](mailto:flyangqn@gmail.com)。

外部 Skill、MCP、运行镜像、模型权重和第三方依赖仍遵循各自许可证。详见[双授权说明](COMMERCIAL-LICENSE.md)。
