import test from "node:test";
import assert from "node:assert/strict";
import { remoteMcpInstallIntent, skillCreationIntent, skillInstallIntent, toolPackageInstallIntent } from "../src/capability-install.ts";

test("explicit MCP URL connects without guessing a protocol or saving a credential in chat", () => {
  assert.deepEqual(remoteMcpInstallIntent("帮我安装 MCP https://mcp.example.test/mcp", 0), {
    kind: "url", content: "https://mcp.example.test/mcp",
  });
  assert.equal(remoteMcpInstallIntent("如何安装 MCP？", 0), null);
  assert.throws(() => remoteMcpInstallIntent("帮我安装 MCP https://mcp.example.test/mcp Authorization: secret", 0), /不要把 MCP 密钥发进对话/);
  assert.deepEqual(remoteMcpInstallIntent("帮我安装playwright的mcp服务，我需要使用", 0), { kind: "guidance" });
  assert.deepEqual(remoteMcpInstallIntent("安装 MCP", 0), { kind: "guidance" });
  assert.deepEqual(remoteMcpInstallIntent("帮我安装 MCP https://github.com/example/browser-mcp", 0), {
    kind: "github", content: "https://github.com/example/browser-mcp",
  });
  assert.throws(() => remoteMcpInstallIntent("安装 MCP", 1), /不接受上传文件/);
});

test("mcpServers JSON is intercepted before it can enter a model Turn", () => {
  const content = '{"mcpServers":{"remote":{"url":"https://mcp.example.test/mcp","headers":{"Authorization":"secret"}}}}';
  assert.deepEqual(remoteMcpInstallIntent(content, 0), { kind: "json", content });
  assert.throws(() => remoteMcpInstallIntent(content, 1), /单独发送 JSON 配置/);
});

test("Skill install requires an explicit GitHub repository, not an uploaded file", () => {
  assert.deepEqual(skillInstallIntent("帮我安装技能 https://github.com/example/skill/tree/main/skills/foo", 0), {
    kind: "github", content: "https://github.com/example/skill/tree/main/skills/foo",
  });
  assert.equal(skillInstallIntent("这个技能是什么？", 0), null);
  assert.deepEqual(skillInstallIntent("安装 Skill", 0), { kind: "guidance" });
  assert.deepEqual(skillInstallIntent("帮我安装这个技能", 0), { kind: "guidance" });
  assert.throws(() => skillInstallIntent("安装 Skill https://example.com/skill", 0), /GitHub/);
  assert.throws(() => skillInstallIntent("安装 Skill https://github.com/example/skill", 1), /不接受上传文件/);
});

test("Skill creation request is explained before starting an Agent Run", () => {
  assert.equal(skillCreationIntent('请帮我创建一个名为 "find-skills" 的 Skill：根据任务搜索本地技能目录'), true);
  assert.equal(skillCreationIntent("新建一个搜索技能的 Skill"), true);
  assert.equal(skillCreationIntent("只写一份 Skill 草稿，不安装"), false);
  assert.equal(skillCreationIntent("如何创建 Skill？"), false);
  assert.equal(skillCreationIntent("创建一张图片"), false);
});

test("Tool install accepts one complete script or declared ZIP without sending source to chat", () => {
  const script = { name: "calculate.py", base64: "cHJpbnQoMSk=" };
  assert.equal(toolPackageInstallIntent("帮我安装这个 Tool", [script]), script);
  assert.equal(toolPackageInstallIntent("这个 Tool 怎么用？", [script]), null);
  assert.throws(() => toolPackageInstallIntent("安装 Tool", []), /添加一个脚本文件/);
  assert.throws(() => toolPackageInstallIntent("安装 Tool", [{ name: "script.sh" }]), /未完整读取/);
});
