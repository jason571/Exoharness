import { readKnowledgeTextFile, knowledgeImportWarnings } from "./knowledge-import";
import {
  isChatPetEnabled,
  mountChatPetGallery,
  setChatPetEnabled,
} from "./pet";

if (window.parent !== window) document.body.classList.add("settings-embedded");

interface PortalAccount {
  username: string;
  email: string;
  display_name: string;
}

interface PortalSession {
  account: PortalAccount;
}

interface PortalAuth {
  session: { account: PortalAccount } | null;
  ready: Promise<PortalSession>;
  fetch(path: string, options?: RequestInit): Promise<Response>;
}

interface PortalCapability {
  id: string;
  name: string;
  kind: string;
  revision: number;
  owner: boolean;
  status: "installing" | "available" | "unavailable";
  status_message: string;
  next_action: string | null;
  authorization_update_supported: boolean;
  authorization_configured: boolean;
  environment_secrets: { name: string; configured: boolean }[];
  can_recheck: boolean;
}

interface PortalCapabilityCatalog {
  installation_ready?: boolean;
  capabilities: PortalCapability[];
  policy: { install_mcp: boolean; install_skills: boolean; install_tools: boolean };
}

interface PortalInstallJob {
  id: string;
  capability_id?: string | null;
  kind: "skill" | "mcp";
  url: string;
  status: "queued" | "downloading" | "registering" | "preparing" | "ready" | "blocked" | "cancelled";
  message: string;
}

interface KnowledgeDocument {
  id: string;
  title: string;
  excerpt: string;
  content?: string;
  source_uri: string | null;
  sha256: string;
  revision: number;
  category_id: string | null;
  category_path: string | null;
  tags: string[];
}

interface KnowledgeCategory { id: string; name: string; parent_id: string | null; path: string; revision: number; }

interface PersonalMemory {
  id: string;
  content: string;
  enabled: boolean;
  source: string;
  revision: number;
}

declare global {
  interface Window {
    ExoAuth: PortalAuth;
  }
}

const profileForm = document.querySelector<HTMLFormElement>("#portal-profile-form");
const passwordForm = document.querySelector<HTMLFormElement>("#portal-password-form");
const errorView = document.querySelector<HTMLElement>("#settings-error");
const petToggle = document.querySelector<HTMLInputElement>("#settings-pet-enabled");
const petGallery = document.querySelector<HTMLElement>("#settings-pet-gallery");
const petStatus = document.querySelector<HTMLElement>("#settings-pet-status");
const mcpInstallForm = document.querySelector<HTMLFormElement>("#portal-mcp-install-form");
const capabilityList = document.querySelector<HTMLElement>("#settings-capability-list");
const installJobList = document.querySelector<HTMLElement>("#settings-install-jobs");
let capabilityPolling: number | undefined;
let capabilityRetryCount = 0;

function showMessage(selector: string, message: string): void {
  const view = document.querySelector<HTMLElement>(selector);
  if (!view) return;
  view.textContent = message;
  view.classList.remove("hidden");
}

function clearDocumentForm(): void {
  document.querySelector<HTMLFormElement>("#portal-knowledge-form")?.reset();
  document.querySelector<HTMLInputElement>("#settings-document-id")!.value = "";
  document.querySelector<HTMLInputElement>("#settings-document-revision")!.value = "";
}

function clearCategoryForm(): void {
  document.querySelector<HTMLFormElement>("#portal-category-form")?.reset();
  document.querySelector<HTMLInputElement>("#settings-category-id")!.value = "";
  document.querySelector<HTMLInputElement>("#settings-category-revision")!.value = "";
}

async function loadCategories(): Promise<void> {
  const records = await jsonRequest("/v1/knowledge/categories") as KnowledgeCategory[];
  const parent = document.querySelector<HTMLSelectElement>("#settings-category-parent")!;
  const documentCategory = document.querySelector<HTMLSelectElement>("#settings-document-category")!;
  const parentValue = parent.value;
  const documentValue = documentCategory.value;
  parent.replaceChildren(new Option("顶层分类", ""));
  documentCategory.replaceChildren(new Option("未分类", ""));
  records.forEach((record) => {
    if (record.path.split("/").length < 3) parent.add(new Option(record.path, record.id));
    documentCategory.add(new Option(record.path, record.id));
  });
  parent.value = parentValue;
  documentCategory.value = documentValue;
  const list = document.querySelector<HTMLElement>("#settings-category-list")!;
  list.replaceChildren();
  if (!records.length) list.append(capabilityElement("p", "尚无分类。"));
  records.forEach((record) => {
    const card = capabilityElement("article", "", "portal-knowledge-item");
    card.append(capabilityElement("strong", record.path));
    const actions = capabilityElement("div", "", "portal-knowledge-actions");
    actions.append(
      actionButton("编辑", async () => {
        document.querySelector<HTMLInputElement>("#settings-category-id")!.value = record.id;
        document.querySelector<HTMLInputElement>("#settings-category-revision")!.value = String(record.revision);
        document.querySelector<HTMLInputElement>("#settings-category-name")!.value = record.name;
        parent.value = record.parent_id || "";
        document.querySelector<HTMLInputElement>("#settings-category-name")!.focus();
      }),
      deleteButton("删除", async () => {
        await jsonRequest(`/v1/knowledge/categories/${encodeURIComponent(record.id)}`, { method: "DELETE" });
        clearCategoryForm();
        await Promise.all([loadCategories(), loadKnowledge()]);
      }),
    );
    card.append(actions);
    list.append(card);
  });
}

function clearMemoryForm(): void {
  document.querySelector<HTMLFormElement>("#portal-memory-form")?.reset();
  document.querySelector<HTMLInputElement>("#settings-memory-id")!.value = "";
  document.querySelector<HTMLInputElement>("#settings-memory-revision")!.value = "";
  document.querySelector<HTMLInputElement>("#settings-memory-enabled")!.checked = true;
}

function actionButton(label: string, callback: () => Promise<void>): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "button secondary";
  button.textContent = label;
  button.addEventListener("click", async () => {
    button.disabled = true;
    try { await callback(); } catch (error) { showError(error); } finally { button.disabled = false; }
  });
  return button;
}

function deleteButton(label: string, callback: () => Promise<void>): HTMLButtonElement {
  const button = actionButton(label, async () => {
    if (button.dataset.confirm !== "yes") {
      button.dataset.confirm = "yes";
      button.textContent = "再点一次确认删除";
      window.setTimeout(() => { button.dataset.confirm = ""; button.textContent = label; }, 5000);
      return;
    }
    await callback();
  });
  return button;
}

async function loadKnowledge(): Promise<void> {
  const list = document.querySelector<HTMLElement>("#settings-document-list");
  if (!list) return;
  const records = await jsonRequest("/v1/knowledge/documents") as KnowledgeDocument[];
  list.replaceChildren();
  if (!records.length) {
    list.append(capabilityElement("p", "还没有资料。上传后，助理可在你的对话中检索并使用相关内容；这里用于查看和管理。"));
    return;
  }
  records.forEach((record) => {
    const card = capabilityElement("article", "", "portal-knowledge-item");
    card.append(capabilityElement("strong", record.title), capabilityElement("p", record.excerpt));
    if (record.category_path || record.tags.length) card.append(capabilityElement("small", [record.category_path, ...record.tags].filter(Boolean).join(" · ")));
    if (record.source_uri) card.append(capabilityElement("small", `来源：${record.source_uri}`));
    const actions = capabilityElement("div", "", "portal-knowledge-actions");
    actions.append(
      actionButton("编辑", async () => {
        const full = await jsonRequest(`/v1/knowledge/documents/${encodeURIComponent(record.id)}`) as KnowledgeDocument;
        document.querySelector<HTMLDetailsElement>("#settings-manual-editor")!.open = true;
        document.querySelector<HTMLInputElement>("#settings-document-id")!.value = full.id;
        document.querySelector<HTMLInputElement>("#settings-document-revision")!.value = String(full.revision);
        document.querySelector<HTMLInputElement>("#settings-document-title")!.value = full.title;
        document.querySelector<HTMLTextAreaElement>("#settings-document-content")!.value = full.content || "";
        document.querySelector<HTMLInputElement>("#settings-document-source")!.value = full.source_uri || "";
        document.querySelector<HTMLSelectElement>("#settings-document-category")!.value = full.category_id || "";
        document.querySelector<HTMLInputElement>("#settings-document-tags")!.value = full.tags.join(", ");
        document.querySelector<HTMLInputElement>("#settings-document-title")!.focus();
      }),
      deleteButton("删除", async () => {
        await jsonRequest(`/v1/knowledge/documents/${encodeURIComponent(record.id)}`, { method: "DELETE" });
        if (document.querySelector<HTMLInputElement>("#settings-document-id")?.value === record.id) clearDocumentForm();
        await loadKnowledge();
        showMessage("#settings-knowledge-message", "资料已删除");
      }),
    );
    card.append(actions);
    list.append(card);
  });
}

async function loadMemories(): Promise<void> {
  const list = document.querySelector<HTMLElement>("#settings-memory-list");
  if (!list) return;
  const records = await jsonRequest("/v1/memories") as PersonalMemory[];
  list.replaceChildren();
  if (!records.length) {
    list.append(capabilityElement("p", "还没有记忆。只会保存你主动填写的内容。"));
    return;
  }
  records.forEach((record) => {
    const card = capabilityElement("article", "", "portal-knowledge-item");
    card.append(capabilityElement("strong", record.enabled ? "已启用" : "已停用"), capabilityElement("p", record.content));
    const actions = capabilityElement("div", "", "portal-knowledge-actions");
    actions.append(
      actionButton("编辑", async () => {
        document.querySelector<HTMLInputElement>("#settings-memory-id")!.value = record.id;
        document.querySelector<HTMLInputElement>("#settings-memory-revision")!.value = String(record.revision);
        document.querySelector<HTMLTextAreaElement>("#settings-memory-content")!.value = record.content;
        document.querySelector<HTMLInputElement>("#settings-memory-enabled")!.checked = record.enabled;
        document.querySelector<HTMLTextAreaElement>("#settings-memory-content")!.focus();
      }),
      deleteButton("删除", async () => {
        await jsonRequest(`/v1/memories/${encodeURIComponent(record.id)}`, { method: "DELETE" });
        if (document.querySelector<HTMLInputElement>("#settings-memory-id")?.value === record.id) clearMemoryForm();
        await loadMemories();
        showMessage("#settings-memory-message", "记忆已删除");
      }),
    );
    card.append(actions);
    list.append(card);
  });
}

function showError(error: unknown): void {
  if (!errorView) return;
  errorView.textContent = error instanceof Error ? error.message : String(error);
  errorView.classList.remove("hidden");
}

async function jsonRequest(path: string, options: RequestInit = {}): Promise<any> {
  const response = await window.ExoAuth.fetch(path, options);
  const payload = await response.json().catch(() => null);
  if (!response.ok) throw new Error(payload?.error?.message || "请求失败");
  return payload;
}

function renderPetStatus(): void {
  if (!petToggle || !petStatus) return;
  petStatus.textContent = petToggle.checked ? "已启用" : "已关闭";
  petStatus.dataset.state = petToggle.checked ? "enabled" : "disabled";
}

function capabilityElement<K extends keyof HTMLElementTagNameMap>(tag: K, text: string, className = ""): HTMLElementTagNameMap[K] {
  const item = document.createElement(tag);
  item.textContent = text;
  item.className = className;
  return item;
}

function renderCapability(capability: PortalCapability): HTMLElement {
  const card = capabilityElement("article", "", "portal-capability-item");
  const heading = capabilityElement("div", "", "portal-capability-heading");
  const identity = capabilityElement("div", "", "portal-capability-identity");
  identity.append(
    capabilityElement("strong", capability.name),
    capabilityElement("small", `${capability.kind === "mcp_server" ? "MCP" : capability.kind === "skill" ? "Skill" : "Tool"} · revision ${capability.revision}`),
  );
  heading.append(
    identity,
    capabilityElement("span", capability.status === "installing" ? "检查中" : capability.status === "available" ? "可用" : "不可用", `portal-capability-status is-${capability.status}`),
  );
  card.append(heading, capabilityElement("p", capability.status_message || "状态暂不可用"));
  if (capability.can_recheck) {
    const button = capabilityElement("button", "重新检查", "button secondary portal-capability-recheck");
    button.type = "button";
    button.addEventListener("click", async () => {
      button.disabled = true;
      try {
        await jsonRequest(`/v1/user-capabilities/${encodeURIComponent(capability.id)}/recheck`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ expected_revision: capability.revision }),
        });
        showMessage("#settings-mcp-message", `已重新检查「${capability.name}」，结果会在此更新。`);
        await refreshCapabilities();
      } catch (error) {
        showError(error);
      } finally {
        button.disabled = false;
      }
    });
    card.append(button);
  }
  if (capability.authorization_update_supported) {
    const disclosure = document.createElement("details");
    disclosure.className = "portal-capability-credentials";
    disclosure.open = capability.next_action === "provide_authorization";
    disclosure.append(capabilityElement("summary", capability.authorization_configured ? "Authorization 已配置 · 更新" : "填写 Authorization"));
    const form = document.createElement("form");
    form.className = "portal-mcp-authorization-form stack-form";
    const label = capabilityElement("label", "Authorization 值");
    const input = document.createElement("input");
    input.type = "password";
    input.required = true;
    input.maxLength = 8192;
    input.autocomplete = "off";
    input.placeholder = "填写服务提供的完整 Header 值";
    label.append(input);
    const button = capabilityElement("button", "保存并重新检查", "button secondary");
    button.type = "submit";
    form.append(label, button);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      button.disabled = true;
      try {
        await jsonRequest(`/v1/user-capabilities/mcp/${encodeURIComponent(capability.id)}/authorization`, {
          method: "PUT",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ authorization: input.value, expected_revision: capability.revision }),
        });
        input.value = "";
        showMessage("#settings-mcp-message", "认证信息已保存，正在重新检查连接。");
        await refreshCapabilities();
      } catch (error) {
        showError(error);
      } finally {
        input.value = "";
        button.disabled = false;
      }
    });
    disclosure.append(form);
    if (capability.authorization_configured) {
      const removeButton = capabilityElement("button", "删除认证信息", "button secondary");
      removeButton.type = "button";
      removeButton.addEventListener("click", async () => {
        if (!window.confirm(`删除「${capability.name}」的 Authorization？该 MCP 将停用，直到重新检查并通过验证。`)) return;
        removeButton.disabled = true;
        try {
          await jsonRequest(`/v1/user-capabilities/mcp/${encodeURIComponent(capability.id)}/authorization`, {
            method: "DELETE",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ expected_revision: capability.revision }),
          });
          showMessage("#settings-mcp-message", "认证信息已删除，MCP 已停用；如服务支持免认证，可重新检查连接。");
          await refreshCapabilities();
        } catch (error) {
          showError(error);
        } finally {
          removeButton.disabled = false;
        }
      });
      disclosure.append(removeButton);
    }
    card.append(disclosure);
  }
  for (const secret of capability.environment_secrets || []) {
    const disclosure = document.createElement("details");
    disclosure.className = "portal-capability-credentials";
    disclosure.open = !secret.configured;
    disclosure.append(capabilityElement("summary", `${secret.name} · ${secret.configured ? "已配置，可更新" : "待配置"}`));
    const form = document.createElement("form");
    form.className = "stack-form";
    const label = capabilityElement("label", `${secret.name} 的密钥值`);
    const input = document.createElement("input");
    input.type = "password";
    input.required = true;
    input.maxLength = 8192;
    input.autocomplete = "off";
    input.placeholder = "仅在此处填写，不要发送到对话";
    input.disabled = capability.status === "installing";
    label.append(input);
    const button = capabilityElement("button", "保存并重新检查", "button secondary");
    button.type = "submit";
    button.disabled = capability.status === "installing";
    if (button.disabled) button.title = "正在检查 MCP，完成后即可填写密钥";
    form.append(label, button);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      button.disabled = true;
      try {
        await jsonRequest(`/v1/user-capabilities/mcp/${encodeURIComponent(capability.id)}/environment/${encodeURIComponent(secret.name)}`, {
          method: "PUT",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ value: input.value, expected_revision: capability.revision }),
        });
        showMessage("#settings-mcp-message", `${secret.name} 已安全保存，正在重新检查 MCP。`);
        await refreshCapabilities();
      } catch (error) {
        showError(error);
      } finally {
        input.value = "";
        button.disabled = false;
      }
    });
    disclosure.append(form);
    if (secret.configured) {
      const removeButton = capabilityElement("button", `删除 ${secret.name}`, "button secondary");
      removeButton.type = "button";
      removeButton.addEventListener("click", async () => {
        if (!window.confirm(`删除「${capability.name}」的 ${secret.name}？该 MCP 将停用。`)) return;
        removeButton.disabled = true;
        try {
          await jsonRequest(`/v1/user-capabilities/mcp/${encodeURIComponent(capability.id)}/environment/${encodeURIComponent(secret.name)}`, {
            method: "DELETE",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ expected_revision: capability.revision }),
          });
          showMessage("#settings-mcp-message", `${secret.name} 已删除，MCP 已停用。`);
          await refreshCapabilities();
        } catch (error) {
          showError(error);
        } finally {
          removeButton.disabled = false;
        }
      });
      disclosure.append(removeButton);
    }
    card.append(disclosure);
  }
  return card;
}

function renderInstallJob(job: PortalInstallJob): HTMLElement {
  const card = capabilityElement("article", "", "portal-capability-item");
  const heading = capabilityElement("div", "", "portal-capability-heading");
  const identity = capabilityElement("div", "", "portal-capability-identity");
  const location = new URL(job.url);
  identity.append(
    capabilityElement("strong", `${job.kind === "skill" ? "Skill" : "MCP"} · ${location.hostname}${location.pathname}`),
    capabilityElement("small", "公开 GitHub 来源 · 固定提交安装"),
  );
  const active = ["queued", "downloading", "registering", "preparing"].includes(job.status);
  heading.append(identity, capabilityElement("span", active ? "安装中" : job.status === "ready" ? "已完成" : job.status === "cancelled" ? "已取消" : "未完成", `portal-capability-status is-${active ? "installing" : job.status === "ready" ? "available" : "unavailable"}`));
  card.append(heading, capabilityElement("p", job.message));
  if (["queued", "downloading"].includes(job.status)) {
    const button = capabilityElement("button", "取消下载", "button secondary portal-capability-recheck");
    button.type = "button";
    button.addEventListener("click", async () => {
      button.disabled = true;
      try {
        await jsonRequest(`/v1/user-capabilities/install-jobs/${encodeURIComponent(job.id)}/cancel`, { method: "POST" });
        await refreshCapabilities();
      } catch (error) {
        showError(error);
        button.disabled = false;
      }
    });
    card.append(button);
  }
  return card;
}

async function refreshCapabilities(): Promise<void> {
  if (!capabilityList) return;
  const [catalog, installation] = await Promise.all([
    jsonRequest("/v1/user-capabilities") as Promise<PortalCapabilityCatalog>,
    jsonRequest("/v1/user-capabilities/install-jobs") as Promise<{ jobs: PortalInstallJob[] }>,
  ]);
  const policy = document.querySelector<HTMLElement>("#settings-capability-policy");
  if (policy) {
    const available = [catalog.policy.install_skills && "Skill", catalog.policy.install_mcp && "MCP", catalog.policy.install_tools && "Tool"].filter(Boolean);
    policy.textContent = catalog.installation_ready === false
      ? "当前可查看已登记的个人能力；新版自助安装功能尚未接入，不能在此提交安装。"
      : available.length
      ? `管理员已开放 ${available.join("、")} 安装权限；能否调用仍受使用权限和助理授权约束。`
      : "管理员尚未开放能力安装权限；已安装能力仍按使用权限和助理授权检查。";
  }
  if (mcpInstallForm) mcpInstallForm.hidden = !catalog.policy.install_mcp || catalog.installation_ready === false;
  capabilityList.replaceChildren();
  const owned = catalog.capabilities.filter((capability) => capability.owner);
  if (!owned.length) {
    capabilityList.append(capabilityElement("p", "尚未安装私有能力。"));
  } else {
    capabilityList.append(...owned.map((capability) => {
      const activeJob = installation.jobs.some((job) => job.capability_id === capability.id
        && ["queued", "downloading", "registering", "preparing"].includes(job.status));
      return renderCapability(activeJob ? { ...capability, status: "installing" } : capability);
    }));
  }
  if (installJobList) {
    installJobList.replaceChildren();
    if (installation.jobs.length) {
      installJobList.append(capabilityElement("h3", "安装任务"));
      installJobList.append(...installation.jobs.map(renderInstallJob));
    }
  }
  window.clearTimeout(capabilityPolling);
  if ((owned.some((capability) => capability.status === "installing")
    || installation.jobs.some((job) => ["queued", "downloading", "registering", "preparing"].includes(job.status)))
    && location.hash === "#capabilities") {
    capabilityPolling = window.setTimeout(() => void loadCapabilities(), 2500);
  }
}

async function loadCapabilities(): Promise<void> {
  try {
    await refreshCapabilities();
    capabilityRetryCount = 0;
    if (location.hash === "#capabilities") errorView?.classList.add("hidden");
  } catch (error) {
    if (location.hash !== "#capabilities") return;
    showError(error);
    if (location.hash === "#capabilities" && capabilityRetryCount++ < 3) {
      window.clearTimeout(capabilityPolling);
      capabilityPolling = window.setTimeout(() => void loadCapabilities(), 2500);
    }
  }
}

mcpInstallForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  errorView?.classList.add("hidden");
  const source = document.querySelector<HTMLTextAreaElement>("#settings-mcp-source");
  const button = document.querySelector<HTMLButtonElement>("#settings-mcp-submit");
  if (!source || !button) return;
  const value = source.value.trim();
  if (!value) return;
  button.disabled = true;
  try {
    const isJson = value.startsWith("{");
    const isGithub = !isJson && value.startsWith("https://github.com/");
    await jsonRequest(isJson ? "/v1/user-capabilities/mcp/json" : isGithub ? "/v1/user-capabilities/install/github" : "/v1/user-capabilities/mcp/http", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(isJson ? { content: value } : isGithub ? { kind: "mcp", url: value } : { url: value }),
    });
    source.value = "";
    showMessage("#settings-mcp-message", "MCP 已登记，正在检查来源、运行环境与工具；检查通过后才会启用。");
    await refreshCapabilities();
  } catch (error) {
    showError(error);
  } finally {
    button.disabled = false;
  }
});

if (petToggle) {
  petToggle.checked = isChatPetEnabled();
  renderPetStatus();
  petToggle.addEventListener("change", () => {
    setChatPetEnabled(petToggle.checked);
    renderPetStatus();
  });
}
if (petGallery) mountChatPetGallery(petGallery);
window.addEventListener("storage", (event) => {
  if (event.key !== "exoharness.chat-pet.enabled" || !petToggle) return;
  petToggle.checked = isChatPetEnabled();
  renderPetStatus();
});

window.ExoAuth.ready.then(async (session) => {
  const account = session.account;
  document.querySelector<HTMLElement>("#settings-username")!.textContent = account.username;
  document.querySelector<HTMLElement>("#settings-email")!.textContent = account.email;
  document.querySelector<HTMLInputElement>("#settings-display-name")!.value = account.display_name;
  const info = await jsonRequest("/v1/application-info");
  document.querySelector<HTMLElement>("#settings-version")!.textContent = `v${info.version}`;
}).catch(showError);

function showSettingsPanel(): void {
  errorView?.classList.add("hidden");
  if (errorView) errorView.textContent = "";
  capabilityRetryCount = 0;
  const requested = location.hash.slice(1);
  const section = ["password", "profile", "capabilities", "knowledge", "memory"].includes(requested) ? requested : "about";
  document.querySelectorAll<HTMLElement>("[data-settings-panel]").forEach((panel) => {
    panel.hidden = panel.dataset.settingsPanel !== section;
  });
  const title = { about: "设置", password: "修改密码", profile: "个人资料", capabilities: "我的能力", knowledge: "我的资料", memory: "我的记忆" }[section] || "设置";
  const description = {
    about: "了解当前版本与 ExoHarness。",
    password: "保护你的账号安全。",
    profile: "管理你的显示名称。",
    capabilities: "查看安装进度，管理自己的 MCP 认证信息。",
    knowledge: "助理可在对话中检索并使用相关资料。",
    memory: "管理你主动确认的长期偏好。",
  }[section] || "了解当前版本与 ExoHarness。";
  document.querySelector<HTMLElement>("#settings-title")!.textContent = title;
  document.querySelector<HTMLElement>("#settings-header-title")!.textContent = title;
  document.querySelector<HTMLElement>("#settings-description")!.textContent = description;
  document.body.classList.remove("settings-loading");
  if (section === "capabilities") void window.ExoAuth.ready.then(loadCapabilities).catch(showError);
  else window.clearTimeout(capabilityPolling);
  if (section === "knowledge") void window.ExoAuth.ready.then(() => Promise.all([loadCategories(), loadKnowledge()])).catch((error) => { if (location.hash === "#knowledge") showError(error); });
  if (section === "memory") void window.ExoAuth.ready.then(loadMemories).catch((error) => { if (location.hash === "#memory") showError(error); });
}
window.addEventListener("hashchange", showSettingsPanel);
showSettingsPanel();

document.querySelector<HTMLAnchorElement>('.portal-settings-header a[href="/chat"]')?.addEventListener("click", (event) => {
  if (window.parent === window) return;
  event.preventDefault();
  window.parent.postMessage({ type: "exoharness:close-settings" }, window.location.origin);
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && window.parent !== window) {
    window.parent.postMessage({ type: "exoharness:close-settings" }, window.location.origin);
  }
});

document.querySelector<HTMLButtonElement>("#settings-document-clear")?.addEventListener("click", clearDocumentForm);
document.querySelector<HTMLButtonElement>("#settings-memory-clear")?.addEventListener("click", clearMemoryForm);
document.querySelector<HTMLButtonElement>("#settings-category-clear")?.addEventListener("click", clearCategoryForm);
document.querySelector<HTMLButtonElement>("#settings-quick-category-toggle")?.addEventListener("click", () => {
  const form = document.querySelector<HTMLFormElement>("#settings-quick-category-form")!;
  form.hidden = !form.hidden;
  if (!form.hidden) document.querySelector<HTMLInputElement>("#settings-quick-category-name")!.focus();
});
document.querySelector<HTMLButtonElement>("#settings-quick-category-cancel")?.addEventListener("click", () => {
  const form = document.querySelector<HTMLFormElement>("#settings-quick-category-form")!;
  form.reset(); form.hidden = true;
});
document.querySelector<HTMLFormElement>("#settings-quick-category-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  try {
    const category = await jsonRequest("/v1/knowledge/categories", {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ name: document.querySelector<HTMLInputElement>("#settings-quick-category-name")!.value, parent_id: null, expected_revision: null }),
    }) as KnowledgeCategory;
    await loadCategories();
    document.querySelector<HTMLSelectElement>("#settings-document-category")!.value = category.id;
    const form = document.querySelector<HTMLFormElement>("#settings-quick-category-form")!;
    form.reset(); form.hidden = true;
    showMessage("#settings-knowledge-message", `已创建并选中分类「${category.name}」`);
  } catch (error) { showError(error); }
});

document.querySelector<HTMLFormElement>("#portal-category-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  errorView?.classList.add("hidden");
  const id = document.querySelector<HTMLInputElement>("#settings-category-id")!.value;
  const revision = document.querySelector<HTMLInputElement>("#settings-category-revision")!.value;
  const form = event.currentTarget as HTMLFormElement;
  const button = form.querySelector<HTMLButtonElement>("button[type=submit]")!;
  button.disabled = true;
  try {
    await jsonRequest(id ? `/v1/knowledge/categories/${encodeURIComponent(id)}` : "/v1/knowledge/categories", {
      method: id ? "PUT" : "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        name: document.querySelector<HTMLInputElement>("#settings-category-name")!.value,
        parent_id: document.querySelector<HTMLSelectElement>("#settings-category-parent")!.value || null,
        expected_revision: id ? Number(revision) : null,
      }),
    });
    clearCategoryForm();
    await Promise.all([loadCategories(), loadKnowledge()]);
    showMessage("#settings-knowledge-message", "分类已保存");
  } catch (error) { showError(error); } finally { button.disabled = false; }
});

document.querySelector<HTMLFormElement>("#portal-knowledge-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  errorView?.classList.add("hidden");
  const id = document.querySelector<HTMLInputElement>("#settings-document-id")!.value;
  const revision = document.querySelector<HTMLInputElement>("#settings-document-revision")!.value;
  const form = event.currentTarget as HTMLFormElement;
  const button = form.querySelector<HTMLButtonElement>("button[type=submit]")!;
  button.disabled = true;
  try {
    await jsonRequest(id ? `/v1/knowledge/documents/${encodeURIComponent(id)}` : "/v1/knowledge/documents", {
      method: id ? "PUT" : "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        title: document.querySelector<HTMLInputElement>("#settings-document-title")!.value,
        content: document.querySelector<HTMLTextAreaElement>("#settings-document-content")!.value,
        source_uri: document.querySelector<HTMLInputElement>("#settings-document-source")!.value || null,
        category_id: document.querySelector<HTMLSelectElement>("#settings-document-category")!.value || null,
        tags: document.querySelector<HTMLInputElement>("#settings-document-tags")!.value.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean),
        expected_revision: id ? Number(revision) : null,
      }),
    });
    clearDocumentForm();
    await loadKnowledge();
    showMessage("#settings-knowledge-message", "资料已保存");
  } catch (error) { showError(error); } finally { button.disabled = false; }
});

document.querySelector<HTMLInputElement>("#settings-file-import")?.addEventListener("change", async (event) => {
  const input = event.currentTarget as HTMLInputElement;
  const files = Array.from(input.files || []);
  const message = document.querySelector<HTMLElement>("#settings-file-message")!;
  if (!files.length) return;
  if (files.length > 10) { message.textContent = "每次最多导入 10 个文件。"; input.value = ""; return; }
  const categoryId = document.querySelector<HTMLSelectElement>("#settings-document-category")!.value || null;
  input.disabled = true;
  const failures: string[] = [];
  let imported = 0;
  try {
    for (const file of files) {
      try {
        const fileContent = await readKnowledgeTextFile(file);
        const result = await jsonRequest(`/v1/knowledge/documents${fileContent.file_base64 ? "/import" : ""}`, {
          method: "POST", headers: { "content-type": "application/json" },
          body: JSON.stringify({ ...fileContent, source_uri: null, category_id: categoryId, tags: [], expected_revision: null }),
        });
        for (const warning of knowledgeImportWarnings(result)) failures.push(`${file.name}：${warning}。`);
        imported += 1;
      } catch (error) { failures.push(error instanceof Error ? error.message : String(error)); }
    }
    await loadKnowledge();
    message.textContent = `${imported} 个文件已导入${failures.length ? `；${failures.join("；")}` : "。"}`;
  } finally { input.value = ""; input.disabled = false; }
});

document.querySelector<HTMLFormElement>("#portal-knowledge-search")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const output = document.querySelector<HTMLElement>("#settings-search-results")!;
  const query = document.querySelector<HTMLInputElement>("#settings-knowledge-query")!.value.trim();
  if (!query) return;
  try {
    const hits = await jsonRequest(`/v1/knowledge/search?q=${encodeURIComponent(query)}`) as KnowledgeDocument[];
    output.replaceChildren();
    if (!hits.length) output.append(capabilityElement("p", "没有找到匹配资料。"));
    hits.forEach((hit) => {
      const card = capabilityElement("article", "", "portal-knowledge-item");
      card.append(capabilityElement("strong", hit.title), capabilityElement("p", hit.excerpt));
      output.append(card);
    });
  } catch (error) { showError(error); }
});

document.querySelector<HTMLFormElement>("#portal-memory-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  errorView?.classList.add("hidden");
  const id = document.querySelector<HTMLInputElement>("#settings-memory-id")!.value;
  const revision = document.querySelector<HTMLInputElement>("#settings-memory-revision")!.value;
  const form = event.currentTarget as HTMLFormElement;
  const button = form.querySelector<HTMLButtonElement>("button[type=submit]")!;
  button.disabled = true;
  try {
    await jsonRequest(id ? `/v1/memories/${encodeURIComponent(id)}` : "/v1/memories", {
      method: id ? "PUT" : "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        content: document.querySelector<HTMLTextAreaElement>("#settings-memory-content")!.value,
        enabled: document.querySelector<HTMLInputElement>("#settings-memory-enabled")!.checked,
        expected_revision: id ? Number(revision) : null,
      }),
    });
    clearMemoryForm();
    await loadMemories();
    showMessage("#settings-memory-message", "记忆已保存");
  } catch (error) { showError(error); } finally { button.disabled = false; }
});

profileForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  errorView?.classList.add("hidden");
  const submit = document.querySelector<HTMLButtonElement>("#settings-profile-submit");
  if (!submit) return;
  submit.disabled = true;
  try {
    const payload = await jsonRequest("/v1/account/profile", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ display_name: document.querySelector<HTMLInputElement>("#settings-display-name")!.value }),
    });
    if (window.ExoAuth.session) window.ExoAuth.session.account = payload.account;
    showMessage("#settings-profile-message", "个人资料已保存");
  } catch (error) {
    showError(error);
  } finally {
    submit.disabled = false;
  }
});

passwordForm?.addEventListener("submit", async (event) => {
  event.preventDefault();
  errorView?.classList.add("hidden");
  const current = document.querySelector<HTMLInputElement>("#settings-current-password")!;
  const next = document.querySelector<HTMLInputElement>("#settings-new-password")!;
  const confirm = document.querySelector<HTMLInputElement>("#settings-confirm-password")!;
  if (next.value !== confirm.value) {
    showError(new Error("两次输入的新密码不一致"));
    confirm.focus();
    return;
  }
  const submit = document.querySelector<HTMLButtonElement>("#settings-password-submit");
  if (!submit) return;
  submit.disabled = true;
  try {
    await jsonRequest("/v1/account/password", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ current_password: current.value, new_password: next.value }),
    });
    showMessage("#settings-password-message", "密码已更新，请重新登录");
    window.setTimeout(() => window.top?.location.replace("/login"), 900);
  } catch (error) {
    showError(error);
    submit.disabled = false;
  }
});
