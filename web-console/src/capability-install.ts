export type McpInstallIntent = { kind: "url" | "json" | "github"; content: string } | { kind: "guidance" };
export type SkillInstallIntent = { kind: "github"; content: string } | { kind: "guidance" };

const INSTALL_COMMAND = /^\s*(?:(?:请|帮我|给我|为我|我要|我想|我需要)\s*)?(?:安装|添加|连接|注册|导入|install|add|connect|import)/i;
const CREATE_SKILL_COMMAND = /^\s*(?:(?:请|帮我|给我|为我|我要|我想|我需要)\s*)*(?:创建|新建|制作|create\b|make\b)/i;

// Creating a registered Skill is distinct from drafting SKILL.md text. The
// portal must explain the installation policy before starting an Agent Run;
// the server remains the authority at the actual import/commit boundary.
export function skillCreationIntent(prompt: string): boolean {
  if (!CREATE_SKILL_COMMAND.test(prompt) || !/(?:\bskills?\b|技能)/i.test(prompt)) return false;
  return !/(?:草稿|只写|只输出|不安装|不导入|不启用|示例)/.test(prompt);
}

function githubUrl(text: string): string | null {
  const urls = text.match(/https?:\/\/[^\s<>"']+/gi) || [];
  if (urls.length !== 1) return null;
  try {
    const url = new URL(urls[0]);
    return url.protocol === "https:" && url.hostname === "github.com" ? url.href : null;
  } catch {
    return null;
  }
}

export function remoteMcpInstallIntent(prompt: string, attachmentCount: number): McpInstallIntent | null {
  const text = prompt.trim();
  if (text.startsWith("{") && /"mcpServers"|"mcp_servers"/.test(text)) {
    if (attachmentCount) throw new Error("安装 MCP 时请单独发送 JSON 配置，不要同时添加其他附件。");
    return { kind: "json", content: text };
  }
  if (!INSTALL_COMMAND.test(text) || !/\bmcp\b/i.test(text)) return null;
  if (attachmentCount) throw new Error("用户安装 MCP 不接受上传文件；请提供地址型 JSON 或公开 GitHub 仓库链接。");
  if (/(?:authorization|bearer|api[_-]?key|token|密钥|令牌)/i.test(text)) {
    throw new Error("不要把 MCP 密钥发进对话；请在设置 → 我的能力中安全填写认证信息。");
  }
  const urls = text.match(/https?:\/\/[^\s<>"']+/gi) || [];
  if (!urls.length) return { kind: "guidance" };
  if (urls.length !== 1) throw new Error("一次只安装一个 MCP，请分别提交地址。");
  const repository = githubUrl(text);
  return repository ? { kind: "github", content: repository } : { kind: "url", content: urls[0] };
}

export function skillInstallIntent(prompt: string, attachmentCount: number): SkillInstallIntent | null {
  if (!INSTALL_COMMAND.test(prompt) || !/(?:\bskills?\b|技能)/i.test(prompt)) return null;
  if (attachmentCount) throw new Error("用户安装 Skill 不接受上传文件；请提供公开 GitHub 仓库链接。");
  const repository = githubUrl(prompt);
  if (!repository) {
    if (/https?:\/\//i.test(prompt)) throw new Error("用户 Skill 仅支持公开的 HTTPS GitHub 仓库或 Skill 子目录链接。");
    return { kind: "guidance" };
  }
  return { kind: "github", content: repository };
}

export function toolPackageInstallIntent<T extends { name: string; base64?: string }>(
  prompt: string,
  attachments: T[],
): T | null {
  if (!INSTALL_COMMAND.test(prompt) || !/(?:\btool\b|工具)/i.test(prompt)) return null;
  const attachment = attachments[0];
  const extension = attachment?.name.toLowerCase().split(".").pop();
  if (attachments.length !== 1 || !["py", "js", "mjs", "cjs", "ts", "tsx", "mts", "cts", "sh", "bash", "zip"].includes(extension || "")) {
    throw new Error("安装 Tool 请添加一个脚本文件，或带根目录 TOOL.json 的 ZIP 包。");
  }
  if (!attachment.base64) throw new Error("Tool 包未完整读取，请重新添加。");
  return attachment;
}
