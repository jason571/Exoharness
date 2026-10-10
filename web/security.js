"use strict";

const toast = document.querySelector("#toast");
function showToast(message, failed = false) {
  toast.textContent = message;
  toast.classList.toggle("error", failed);
  toast.classList.add("visible");
  setTimeout(() => toast.classList.remove("visible"), 3500);
}

const passwordUpdateForm = document.querySelector("#password-update-form");
const passwordRole = document.querySelector("#password-role");
const passwordRoleDescription = document.querySelector("#password-role-description");
const passwordUpdateButton = document.querySelector("#password-update-button");

function updatePasswordRoleCopy() {
  const isAdmin = passwordRole.value === "admin";
  passwordRoleDescription.textContent = isAdmin
    ? "管理员拥有全部配置与管理权限。"
    : "只读用户可查看获授权的管理页面，不能修改管理配置。";
  passwordUpdateButton.textContent = isAdmin ? "更新管理员密码" : "更新只读用户密码";
}

passwordRole.addEventListener("change", updatePasswordRoleCopy);
passwordUpdateForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const data = new FormData(passwordUpdateForm);
  if (data.get("new_password") !== data.get("confirmation")) {
    showToast("两次输入的新密码不一致", true);
    return;
  }
  const role = String(data.get("role"));
  passwordUpdateButton.disabled = true;
  try {
    const response = await window.ExoAuth.fetch(`/v1/auth/passwords/${role}`, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ current_password: data.get("current_password"), new_password: data.get("new_password") }),
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error?.message || "密码更新失败");
    passwordUpdateForm.reset();
    updatePasswordRoleCopy();
    if (role === "admin") {
      window.location.replace("/exoharness-login");
    } else {
      showToast("只读用户密码已更新，已有对话会话已撤销");
    }
  } catch (error) {
    showToast(error.message, true);
  } finally {
    passwordUpdateButton.disabled = false;
  }
});

updatePasswordRoleCopy();

const registrationEmailForm = document.querySelector("#registration-email-form");
const registrationEmailSubject = document.querySelector("#registration-email-subject");
const registrationEmailBody = document.querySelector("#registration-email-body");
const registrationMailerStatus = document.querySelector("#registration-mailer-status");
const registrationEmailUpdated = document.querySelector("#registration-email-updated");
const registrationTemplatePlaceholders = document.querySelector("#registration-template-placeholders");
let registrationEmailRevision = 0;
let registrationTemplateTarget = registrationEmailBody;

function displayUpdatedAt(timestamp) {
  if (!timestamp) return "当前使用默认内容";
  return `上次保存：${new Date(timestamp * 1000).toLocaleString("zh-CN", { hour12: false })}`;
}

function renderRegistrationPlaceholders(placeholders) {
  registrationTemplatePlaceholders.replaceChildren();
  placeholders.forEach(({ value, label }) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "registration-template-token";
    button.textContent = `${label} ${value}`;
    button.addEventListener("click", () => {
      const target = registrationTemplateTarget || registrationEmailBody;
      const start = target.selectionStart ?? target.value.length;
      const end = target.selectionEnd ?? target.value.length;
      target.setRangeText(value, start, end, "end");
      target.focus();
    });
    registrationTemplatePlaceholders.append(button);
  });
}

async function loadRegistrationEmailTemplate() {
  const saveButton = registrationEmailForm.querySelector('button[type="submit"]');
  saveButton.disabled = true;
  try {
    const response = await window.ExoAuth.fetch("/v1/auth/registration-email");
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error?.message || "注册邮件配置读取失败");
    registrationEmailSubject.value = payload.subject;
    registrationEmailBody.value = payload.body;
    registrationEmailRevision = payload.revision;
    registrationEmailUpdated.textContent = displayUpdatedAt(payload.updated_at);
    registrationMailerStatus.textContent = payload.mailer_configured ? "邮件服务已连接" : "邮件服务未配置";
    registrationMailerStatus.classList.toggle("is-ready", Boolean(payload.mailer_configured));
    registrationMailerStatus.classList.toggle("is-warning", !payload.mailer_configured);
    renderRegistrationPlaceholders(payload.placeholders || []);
  } catch (error) {
    registrationMailerStatus.textContent = "读取失败";
    registrationMailerStatus.classList.add("is-warning");
    showToast(error.message, true);
  } finally {
    saveButton.disabled = false;
  }
}

[registrationEmailSubject, registrationEmailBody].forEach((field) => {
  field.addEventListener("focus", () => { registrationTemplateTarget = field; });
});

registrationEmailForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const button = registrationEmailForm.querySelector('button[type="submit"]');
  button.disabled = true;
  try {
    const response = await window.ExoAuth.fetch("/v1/auth/registration-email", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        subject: registrationEmailSubject.value,
        body: registrationEmailBody.value,
        revision: registrationEmailRevision,
      }),
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error?.message || "注册邮件配置保存失败");
    registrationEmailSubject.value = payload.subject;
    registrationEmailBody.value = payload.body;
    registrationEmailRevision = payload.revision;
    registrationEmailUpdated.textContent = displayUpdatedAt(payload.updated_at);
    showToast("注册邮件内容已保存");
  } catch (error) {
    showToast(error.message, true);
    if (String(error.message).includes("revision")) await loadRegistrationEmailTemplate();
  } finally {
    button.disabled = false;
  }
});

loadRegistrationEmailTemplate();

const schedulerForm = document.querySelector("#scheduler-form");
const schedulerUserLimit = document.querySelector("#scheduler-user-limit");
const schedulerGlobalLimit = document.querySelector("#scheduler-global-limit");
const schedulerState = document.querySelector("#scheduler-state");
const schedulerUpdated = document.querySelector("#scheduler-updated");
const schedulerSave = document.querySelector("#scheduler-save");
const addResourcePoolButton = document.querySelector("#add-resource-pool");
const resourcePoolList = document.querySelector("#resource-pool-list");
const mcpResourceList = document.querySelector("#mcp-resource-list");
let schedulerRevision = 0;
let schedulerLoaded = false;
let schedulerMcps = [];
let schedulerAssignments = {};

function schedulerUpdatedAt(timestamp) {
  if (!timestamp) return "使用默认配置";
  return `上次保存：${new Date(timestamp).toLocaleString("zh-CN", { hour12: false })}`;
}

function currentResourcePools() {
  return [...resourcePoolList.querySelectorAll(".resource-pool-row")].map((row) => ({
    id: row.querySelector('[data-field="id"]').value.trim(),
    name: row.querySelector('[data-field="name"]').value.trim(),
    max_concurrency: Number(row.querySelector('[data-field="limit"]').value),
  }));
}

function renderMcpAssignments() {
  const pools = currentResourcePools().filter((pool) => pool.id);
  mcpResourceList.replaceChildren();
  if (!schedulerMcps.length) {
    const empty = document.createElement("div");
    empty.className = "scheduler-empty";
    empty.textContent = "当前没有已登记的 MCP。导入 MCP 后可在这里声明共享资源。";
    mcpResourceList.append(empty);
    return;
  }
  schedulerMcps.forEach((mcp) => {
    const row = document.createElement("label");
    row.className = "mcp-resource-row";
    const identity = document.createElement("span");
    identity.className = "mcp-resource-identity";
    const name = document.createElement("strong");
    name.textContent = mcp.name;
    const detail = document.createElement("small");
    detail.textContent = `${mcp.enabled ? "已启用" : "未启用"} · 独立并发 ${mcp.capability_max_concurrency}`;
    identity.append(name, detail);
    const select = document.createElement("select");
    select.dataset.capabilityId = mcp.id;
    const standalone = document.createElement("option");
    standalone.value = "";
    standalone.textContent = "使用 MCP 自身限制";
    select.append(standalone);
    pools.forEach((pool) => {
      const option = document.createElement("option");
      option.value = pool.id;
      option.textContent = `${pool.name || pool.id} · 并发 ${pool.max_concurrency || 1}`;
      select.append(option);
    });
    select.value = schedulerAssignments[mcp.id] || "";
    select.addEventListener("change", () => {
      if (select.value) schedulerAssignments[mcp.id] = select.value;
      else delete schedulerAssignments[mcp.id];
    });
    row.append(identity, select);
    mcpResourceList.append(row);
  });
}

function addResourcePoolRow(pool = { id: "", name: "", max_concurrency: 1 }) {
  const row = document.createElement("div");
  row.className = "resource-pool-row";
  const idLabel = document.createElement("label");
  idLabel.innerHTML = '<span>资源池 ID</span><input data-field="id" maxlength="64" placeholder="gpu-main" required>';
  idLabel.querySelector("input").value = pool.id;
  const nameLabel = document.createElement("label");
  nameLabel.innerHTML = '<span>显示名称</span><input data-field="name" maxlength="100" placeholder="主 GPU 资源池" required>';
  nameLabel.querySelector("input").value = pool.name;
  const limitLabel = document.createElement("label");
  limitLabel.innerHTML = '<span>池并发</span><input data-field="limit" type="number" min="1" max="1024" required>';
  limitLabel.querySelector("input").value = String(pool.max_concurrency || 1);
  const remove = document.createElement("button");
  remove.type = "button";
  remove.className = "resource-pool-remove";
  remove.setAttribute("aria-label", "删除资源池");
  remove.textContent = "删除";
  remove.addEventListener("click", () => {
    const removedId = idLabel.querySelector("input").value.trim();
    row.remove();
    Object.entries(schedulerAssignments).forEach(([capabilityId, poolId]) => {
      if (poolId === removedId) delete schedulerAssignments[capabilityId];
    });
    renderMcpAssignments();
  });
  [idLabel, nameLabel, limitLabel].forEach((label) => {
    label.querySelector("input").addEventListener("change", renderMcpAssignments);
  });
  row.append(idLabel, nameLabel, limitLabel, remove);
  resourcePoolList.append(row);
}

function renderSchedulerConfiguration(payload) {
  schedulerLoaded = true;
  schedulerRevision = payload.revision;
  schedulerMcps = payload.mcp_capabilities || [];
  schedulerAssignments = { ...(payload.mcp_resource_assignments || {}) };
  schedulerUserLimit.value = String(payload.max_active_runs_per_user);
  schedulerGlobalLimit.value = String(payload.max_global_agent_runs);
  resourcePoolList.replaceChildren();
  (payload.resource_pools || []).forEach(addResourcePoolRow);
  renderMcpAssignments();
  schedulerUpdated.textContent = schedulerUpdatedAt(payload.updated_at);
  schedulerState.textContent = "配置已生效";
  schedulerState.classList.add("is-ready");
  schedulerState.classList.remove("is-warning");
}

async function loadSchedulerConfiguration() {
  schedulerSave.disabled = true;
  try {
    const response = await window.ExoAuth.fetch("/v1/scheduler-configuration");
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error?.message || "运行调度配置读取失败");
    renderSchedulerConfiguration(payload);
  } catch (error) {
    schedulerLoaded = false;
    schedulerState.textContent = "读取失败";
    schedulerState.classList.remove("is-ready");
    schedulerState.classList.add("is-warning");
    showToast(error.message, true);
  } finally {
    schedulerSave.disabled = !schedulerLoaded;
  }
}

addResourcePoolButton.addEventListener("click", () => {
  addResourcePoolRow();
  resourcePoolList.lastElementChild?.querySelector("input")?.focus();
});

schedulerForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!schedulerLoaded) return;
  const resourcePools = currentResourcePools();
  const validPoolIds = new Set(resourcePools.map((pool) => pool.id));
  const assignments = {};
  mcpResourceList.querySelectorAll("select[data-capability-id]").forEach((select) => {
    if (select.value && validPoolIds.has(select.value)) assignments[select.dataset.capabilityId] = select.value;
  });
  schedulerSave.disabled = true;
  try {
    const response = await window.ExoAuth.fetch("/v1/scheduler-configuration", {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        revision: schedulerRevision,
        max_active_runs_per_user: Number(schedulerUserLimit.value),
        max_global_agent_runs: Number(schedulerGlobalLimit.value),
        resource_pools: resourcePools,
        mcp_resource_assignments: assignments,
      }),
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw Object.assign(new Error(payload?.error?.message || "运行调度配置保存失败"), { code: payload?.error?.code });
    renderSchedulerConfiguration(payload);
    showToast("运行调度配置已保存并立即生效");
  } catch (error) {
    showToast(error.message, true);
    if (error.code === "state_conflict" || String(error.message).includes("revision")) await loadSchedulerConfiguration();
  } finally {
    schedulerSave.disabled = !schedulerLoaded;
  }
});

loadSchedulerConfiguration();

const artifactForm = document.querySelector("#artifact-configuration-form");
const artifactMode = document.querySelector("#artifact-access-mode");
const artifactDays = document.querySelector("#artifact-access-days");
const artifactSave = document.querySelector("#artifact-configuration-save");
const artifactState = document.querySelector("#artifact-configuration-state");
let artifactRevision = null;
function updateArtifactMode() {
  const limited = artifactMode.value === "limited";
  document.querySelector("#artifact-access-days-field").hidden = !limited;
  artifactDays.disabled = !limited;
  artifactDays.required = limited;
}
function renderArtifactConfiguration(configuration) {
  artifactRevision = configuration.revision;
  artifactMode.value = configuration.access_ttl_seconds ? "limited" : "permanent";
  artifactDays.value = String(Math.ceil(configuration.access_ttl_seconds / 86400) || 30);
  updateArtifactMode();
  artifactState.textContent = configuration.access_ttl_seconds ? `当前有效期：${artifactDays.value} 天` : "当前：永久有效";
}
async function loadArtifactConfiguration() {
  try {
    const response = await window.ExoAuth.fetch("/v1/artifact-configuration");
    const payload = await response.json();
    if (!response.ok) throw new Error(payload?.error?.message || "文件配置读取失败");
    renderArtifactConfiguration(payload);
    artifactSave.disabled = false;
  } catch (error) { artifactState.textContent = error.message; }
}
artifactMode.addEventListener("change", updateArtifactMode);
artifactForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (artifactRevision == null) return;
  artifactSave.disabled = true;
  try {
    const response = await window.ExoAuth.fetch("/v1/artifact-configuration", {
      method: "PUT", headers: { "content-type": "application/json" },
      body: JSON.stringify({ revision: artifactRevision, access_ttl_seconds: artifactMode.value === "permanent" ? 0 : Number(artifactDays.value) * 86400 }),
    });
    const payload = await response.json();
    if (!response.ok) {
      if (response.status === 409) await loadArtifactConfiguration();
      throw new Error(payload?.error?.message || "文件配置保存失败");
    }
    renderArtifactConfiguration(payload);
    showToast("文件访问配置已生效");
  } catch (error) { showToast(error.message, true); }
  finally { artifactSave.disabled = false; }
});
loadArtifactConfiguration();
