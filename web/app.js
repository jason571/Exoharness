"use strict";

const state = {
  capabilities: [],
  selectedId: new URLSearchParams(window.location.search).get("capability"),
  permissions: [],
  secrets: [],
  tools: [],
  skillInstructions: "",
  runtime: null,
  filter: "all",
  search: "",
};

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => Array.from(document.querySelectorAll(selector));

async function api(path, options = {}) {
  const init = { ...options, headers: { ...(options.headers || {}) } };
  if (init.body && typeof init.body !== "string") {
    init.headers["content-type"] = "application/json";
    init.body = JSON.stringify(init.body);
  }
  const response = await window.ExoAuth.fetch(path, init);
  const text = await response.text();
  let payload = null;
  if (text) {
    try { payload = JSON.parse(text); } catch { payload = text; }
  }
  if (!response.ok) throw new Error(payload?.error?.message || `${response.status} ${response.statusText}`);
  return payload;
}

const capabilityKind = (capability) => capability.manifest.spec.kind;
const kindLabel = (kind) => ({ skill: "Skill", mcp_server: "MCP 服务", tool_node: "本地工具" }[kind] || "能力");
const kindIcon = (kind) => ({ skill: "SK", mcp_server: "MC", tool_node: "TL" }[kind] || "CP");
const selectedCapability = () => state.capabilities.find((item) => item.id === state.selectedId) || null;

function scopeLabel(scope) {
  if (scope.kind === "global") return "全部项目";
  if (scope.kind === "project") return `项目：${scope.project || "未指定"}`;
  if (scope.kind === "session") return `当前会话`;
  return "自定义范围";
}

const availabilityLabel = (status) => ({
  available: "可用",
  instructions_only: "说明已登记",
  execution_ready: "执行已就绪",
  degraded: "部分可用",
  pending_permission: "待授权",
  permission_denied: "权限已拒绝",
  runtime_missing: "环境未准备",
  unverified: "尚未验证",
  unavailable: "不可用",
  disabled: "未启用",
}[status] || "状态未知");

function availabilityReason(availability) {
  if (!availability) return "尚未取得可用性状态";
  if (availability.message) return availability.message;
  return ({
    available: "已启用，权限完整，最近验证通过",
    degraded: "最近验证显示部分能力可用",
    pending_permission: "需要先明确允许运行权限",
    permission_denied: "运行权限已被拒绝",
    runtime_missing: "需要先准备能力专用的 Conda 环境",
    unverified: "权限已满足，但尚未通过服务验证",
    unavailable: "最近一次服务验证未通过",
    disabled: "当前未启用，不会加入 Agent 运行",
  })[availability.status] || "尚未取得可用性状态";
}

async function refreshCapabilityOverview() {
  state.capabilities = await api("/v1/capabilities/overview");
  renderMetrics();
  renderCapabilities();
  renderSelectedAvailability();
}

function renderSelectedAvailability(fallbackHealth = "unknown") {
  const capability = selectedCapability();
  if (!capability) return;
  const detail = $("#detail-health");
  detail.textContent = availabilityLabel(capability.availability?.status);
  detail.dataset.health = capability.availability?.health || fallbackHealth;
  detail.title = availabilityReason(capability.availability);
}

async function loadAll({ preserveSelection = true } = {}) {
  setService(false, "正在同步 Registry");
  try {
    state.capabilities = await api("/v1/capabilities/overview");
    if (!preserveSelection || !state.capabilities.some((item) => item.id === state.selectedId)) {
      state.selectedId = state.capabilities[0]?.id || null;
    }
    renderMetrics();
    renderCapabilities();
    await renderDetail();
    setService(true, "服务在线");
  } catch (error) {
    setService(false, "服务不可用", true);
    showToast(error.message, true);
  }
}

function setService(online, label, failed = false) {
  $("#service-dot").classList.toggle("online", online);
  $("#service-dot").classList.toggle("offline", failed);
  $("#service-label").textContent = label;
}

function renderMetrics() {
  $("#metric-total").textContent = state.capabilities.length;
  $("#metric-enabled").textContent = state.capabilities.filter((item) => item.enabled).length;
  $("#metric-available").textContent = state.capabilities.filter((item) => item.availability?.available).length;
}

function renderCapabilities() {
  const list = $("#capability-list");
  list.replaceChildren();
  const query = state.search.trim().toLowerCase();
  const visible = state.capabilities.filter((item) => {
    if (state.filter !== "all" && capabilityKind(item) !== state.filter) return false;
    if (!query) return true;
    return [
      item.manifest.name,
      item.manifest.description,
      kindLabel(capabilityKind(item)),
      availabilityLabel(item.availability?.status),
    ]
      .some((value) => String(value || "").toLowerCase().includes(query));
  });
  $("#capability-count").textContent = query || state.filter !== "all"
    ? `显示 ${visible.length} 项 · 共 ${state.capabilities.length} 项`
    : `共 ${state.capabilities.length} 项`;
  if (!visible.length) {
    const message = document.createElement("p");
    message.className = "list-message";
    message.textContent = state.capabilities.length ? "此类型暂时没有能力" : "尚无能力，请前往技能导入添加";
    list.append(message);
    return;
  }
  for (const capability of visible) {
    const kind = capabilityKind(capability);
    const button = document.createElement("button");
    button.type = "button";
    button.className = `capability-card${capability.id === state.selectedId ? " selected" : ""}`;
    button.addEventListener("click", async () => {
      state.selectedId = capability.id;
      state.tools = [];
      state.secrets = [];
      state.skillInstructions = "";
      state.runtime = null;
      window.history.replaceState(null, "", `/services?capability=${encodeURIComponent(capability.id)}`);
      renderCapabilities();
      await renderDetail();
    });
    const icon = document.createElement("span");
    icon.className = "kind-icon";
    icon.textContent = kindIcon(kind);
    const copy = document.createElement("span");
    const name = document.createElement("strong");
    name.textContent = capability.manifest.name;
    const meta = document.createElement("small");
    meta.textContent = `${kindLabel(kind)} · v${capability.manifest.version} · 第 ${capability.revision} 版`;
    copy.append(name, meta);
    const status = document.createElement("span");
    status.className = `capability-card-status${capability.enabled ? " enabled" : ""}`;
    const statusDot = document.createElement("span");
    statusDot.className = `card-state${capability.enabled ? " enabled" : ""}`;
    const statusText = document.createElement("small");
    statusText.textContent = capability.enabled ? "已启用" : "未启用";
    status.append(statusDot, statusText);
    const availability = document.createElement("span");
    availability.className = `capability-availability ${capability.availability?.status || "unknown"}`;
    availability.textContent = availabilityLabel(capability.availability?.status);
    availability.title = availabilityReason(capability.availability);
    availability.setAttribute("aria-label", `可用性：${availability.textContent}。${availability.title}`);
    button.append(icon, copy, status, availability);
    list.append(button);
  }
}

async function renderDetail() {
  const capability = selectedCapability();
  $("#empty-detail").classList.toggle("hidden", Boolean(capability));
  $("#capability-detail").classList.toggle("hidden", !capability);
  if (!capability) return;
  const kind = capabilityKind(capability);
  $("#detail-kind").textContent = kindLabel(kind);
  $("#detail-name").textContent = capability.manifest.name;
  $("#detail-description").textContent = capability.manifest.description;
  $("#detail-version").textContent = capability.manifest.version;
  $("#detail-revision").textContent = `第 ${capability.revision} 版`;
  $("#detail-scope").textContent = scopeLabel(capability.manifest.scope);
  $("#detail-id").textContent = capability.id;
  $("#detail-manifest").textContent = "正在读取已脱敏的配置详情…";
  const enabled = $("#detail-enabled");
  enabled.textContent = capability.enabled ? "已启用" : "未启用";
  enabled.classList.toggle("enabled", capability.enabled);
  $("#toggle-enabled").textContent = capability.enabled ? "停用服务" : "启用服务";
  $("#discover-tools").disabled = kind === "skill" || !capability.enabled;
  $("#discover-tools").classList.toggle("hidden", kind === "skill");
  $("#test-section").classList.toggle("hidden", kind === "skill");
  $("#skill-instructions-section").classList.toggle("hidden", kind !== "skill");
  $("#delete-capability-help").textContent = kind === "skill"
    ? "永久删除这项 Skill 的登记、全部历史版本和服务器包文件。此操作无法恢复，操作记录仍会保留。"
    : "从服务列表移除这项登记，并删除它保存在本机的认证信息。操作记录仍会保留。";
  $("#delete-capability").textContent = kind === "skill" ? "彻底删除 Skill" : "删除登记";
  if (kind === "skill") $("#skill-instructions").textContent = "正在读取技能说明…";
  const [permissions, health, secrets, skillInstructions, runtime] = await Promise.all([
    api(`/v1/capabilities/${encodeURIComponent(capability.id)}/permissions`),
    api(`/v1/capabilities/${encodeURIComponent(capability.id)}/health`),
    api(`/v1/capabilities/${encodeURIComponent(capability.id)}/secrets`),
    kind === "skill"
      ? api(`/v1/capabilities/${encodeURIComponent(capability.id)}/skill-instructions`)
      : Promise.resolve(null),
    api(`/v1/capabilities/${encodeURIComponent(capability.id)}/runtime`),
  ]);
  if (state.selectedId !== capability.id) return;
  state.permissions = permissions;
  state.secrets = secrets;
  state.skillInstructions = skillInstructions?.content || "";
  state.runtime = runtime;
  if (kind === "skill") {
    $("#skill-instructions").textContent = state.skillInstructions || "此 Skill 没有说明内容。";
  }
  renderSelectedAvailability(health.status);
  $("#detail-manifest").textContent = JSON.stringify(redactedManifest(capability.manifest, secrets), null, 2);
  renderSecrets();
  renderPythonRuntime();
  renderPermissions();
}

function renderPythonRuntime() {
  const runtime = state.runtime;
  const section = $("#python-runtime-section");
  section.classList.toggle("hidden", !runtime?.applicable);
  if (!runtime?.applicable) return;
  const labels = {
    ready: "已准备",
    blocked: "待启用或授权",
    missing: "未准备",
    needs_binding: "待绑定",
    conda_unavailable: "Conda 不可用",
  };
  $("#python-runtime-status").textContent = runtime.ready ? "依赖已核验" : labels[runtime.status] || "待核验";
  $("#python-runtime-status").dataset.status = runtime.status;
  $("#python-runtime-message").textContent = runtime.message;
  const missing = runtime.evidence?.missing || [];
  if (missing.length) $("#python-runtime-message").textContent += `；缺少：${missing.join("、")}`;
  $("#python-runtime-path").textContent = runtime.evidence?.image_id || "尚未核验";
  const button = $("#prepare-python-runtime");
  button.disabled = false;
  button.textContent = runtime.ready ? "重新检查沙盒依赖" : "启用并检查沙盒依赖";
}

function redactedManifest(manifest, secrets) {
  const redacted = JSON.parse(JSON.stringify(manifest));
  const statusByName = new Map(secrets.map((secret) => [secret.name, secret]));
  for (const reference of redacted.secret_refs || []) {
    const status = statusByName.get(reference.name);
    reference.key = status?.configured ? "[已配置，不显示]" : "[未配置]";
  }
  return redacted;
}

function secretUsageLabel(capability, secretName) {
  const transport = capability.manifest.spec?.config?.transport || {};
  if (transport.auth_secret === secretName) return "Bearer Token";
  for (const [header, binding] of Object.entries(transport.headers || {})) {
    if (binding?.reference === secretName) return header;
  }
  return secretName;
}

function isAuthorizationSecret(capability, secretName) {
  const transport = capability?.manifest?.spec?.config?.transport || {};
  return Object.entries(transport.headers || {}).some(([header, binding]) =>
    header.toLowerCase() === "authorization" && binding?.reference === secretName);
}

function renderSecrets() {
  const capability = selectedCapability();
  const section = $("#secret-section");
  section.classList.toggle("hidden", !state.secrets.length);
  if (!capability || !state.secrets.length) return;
  const configured = state.secrets.filter((secret) => secret.configured).length;
  $("#secret-summary").textContent = `${configured}/${state.secrets.length} 已配置`;
  const list = $("#secret-list");
  list.replaceChildren();
  for (const secret of state.secrets) {
    const row = document.createElement("div");
    row.className = "secret-row";

    const heading = document.createElement("div");
    heading.className = "secret-row-heading";
    const copy = document.createElement("div");
    const name = document.createElement("strong");
    name.textContent = secretUsageLabel(capability, secret.name);
    const provider = document.createElement("small");
    const authorization = isAuthorizationSecret(capability, secret.name);
    provider.textContent = secret.mutable
      ? (authorization ? "保存完整 Header 值，例如 Bearer <Token> · 本机密钥保险箱" : "本机密钥保险箱")
      : "由环境变量提供";
    copy.append(name, provider);
    const status = document.createElement("span");
    status.className = `secret-status${secret.configured ? " configured" : ""}`;
    status.textContent = secret.configured ? "已配置" : "未配置";
    heading.append(copy, status);
    row.append(heading);

    if (secret.mutable) {
      const controls = document.createElement("div");
      controls.className = "secret-controls";
      const input = document.createElement("input");
      input.type = "password";
      input.autocomplete = "new-password";
      input.placeholder = authorization
        ? (secret.configured ? "输入完整 Authorization 值进行轮换" : "例如 Bearer <Token>")
        : (secret.configured ? "输入新值进行轮换" : "输入认证信息");
      input.setAttribute("aria-label", `${name.textContent} 新值`);
      const save = document.createElement("button");
      save.type = "button";
      save.className = "button secondary compact-button";
      save.textContent = secret.configured ? "更新" : "保存";
      save.addEventListener("click", guarded(async () => {
        const value = input.value;
        if (!value) throw new Error("请输入新的认证信息");
        save.disabled = true;
        try {
          const updated = await api(`/v1/capabilities/${encodeURIComponent(capability.id)}/secrets/${encodeURIComponent(secret.name)}`, {
            method: "PUT",
            body: { value, actor: "web-console" },
          });
          input.value = "";
          replaceSecretStatus(updated);
          showToast(secret.configured ? "认证信息已轮换" : "认证信息已保存");
        } finally {
          save.disabled = false;
        }
      }));
      const remove = document.createElement("button");
      remove.type = "button";
      remove.className = "button ghost compact-button danger-button";
      remove.textContent = "删除";
      remove.disabled = !secret.configured;
      remove.addEventListener("click", guarded(async () => {
        if (!window.confirm(`删除 ${name.textContent}？删除后，此服务在重新配置前可能无法通过验证。`)) return;
        const updated = await api(`/v1/capabilities/${encodeURIComponent(capability.id)}/secrets/${encodeURIComponent(secret.name)}`, {
          method: "DELETE",
          body: { actor: "web-console" },
        });
        replaceSecretStatus(updated);
        showToast("认证信息已删除");
      }));
      controls.append(input, save, remove);
      row.append(controls);
    }
    list.append(row);
  }
}

function replaceSecretStatus(updated) {
  const index = state.secrets.findIndex((secret) => secret.name === updated.name);
  if (index >= 0) state.secrets[index] = updated;
  const capability = selectedCapability();
  if (capability) {
    $("#detail-manifest").textContent = JSON.stringify(redactedManifest(capability.manifest, state.secrets), null, 2);
  }
  renderSecrets();
}

function renderPermissions() {
  const capability = selectedCapability();
  const list = $("#permission-list");
  list.replaceChildren();
  if (!state.permissions.length) {
    const empty = document.createElement("span");
    empty.className = "permission allow";
    empty.textContent = "无额外权限声明";
    list.append(empty);
  } else {
    for (const permission of state.permissions) {
      const item = document.createElement("span");
      item.className = `permission ${permission.decision}`;
      const decision = permission.decision === "allow" ? "已允许" : permission.decision === "deny" ? "已拒绝" : "待授权";
      item.textContent = `${permissionDescription(permission.permission)} · ${decision}`;
      item.title = permission.permission;
      list.append(item);
    }
  }
  const allowed = state.permissions.filter((item) => item.decision === "allow").length;
  const pending = state.permissions.filter((item) => item.decision !== "allow");
  const runtimeBlocked = Boolean(state.runtime?.applicable && !state.runtime.ready);
  $("#permission-summary").textContent = state.permissions.length ? `${allowed}/${state.permissions.length} 已授权` : "无需额外权限";
  $("#allow-permissions").disabled = !pending.length;
  $("#allow-permissions").classList.toggle("hidden", !pending.length);
  $("#validate-capability").disabled = runtimeBlocked;
  $("#validate-capability").textContent = runtimeBlocked ? "请先准备运行环境" : pending.length ? "授权并验证" : "验证可用性";
  // Enabling instructions is independent of granting sandbox execution.
  // Otherwise a disabled Skill can never reach its preparation prerequisite.
  $("#toggle-enabled").disabled = false;
  $("#permission-help").textContent = pending.length
    ? `验证前需要明确允许：${pending.map((item) => permissionDescription(item.permission)).join("、")}`
    : state.permissions.length ? "这项能力所需的权限已经授权。" : "这项能力不需要额外权限，可以直接验证。";
}

async function preparePythonRuntime() {
  if (!state.runtime?.applicable) return;
  // Reuse the single preparation path instead of an unimplemented Conda API.
  $("#prepare-capability").click();
}

function permissionDescription(permission) {
  return ({
    "process.spawn": "启动本地程序",
    "network.egress": "连接远程 MCP 服务",
  })[permission] || permission;
}

async function grantPermissions(capability, pending) {
  for (const permission of pending) {
    const record = await api(`/v1/capabilities/${encodeURIComponent(capability.id)}/permissions/${encodeURIComponent(permission.permission)}`, {
      method: "PUT",
      body: { decision: "allow", actor: "web-console" },
    });
    const index = state.permissions.findIndex((item) => item.permission === record.permission);
    if (index >= 0) state.permissions[index] = record;
  }
  renderPermissions();
}

async function validateSelectedCapability() {
  const capability = selectedCapability();
  if (!capability) return;
  const pending = state.permissions.filter((item) => item.decision !== "allow");
  const confirmation = pending.length
    ? `验证此服务需要允许以下权限：\n\n${pending.map((item) => `• ${permissionDescription(item.permission)}`).join("\n")}\n\n是否授权并继续验证？验证会启动本地进程或连接远程服务，但不会调用具体工具。`
    : "执行服务验证？MCP 验证会启动本地进程或连接远程服务，但不会调用具体工具。";
  if (!window.confirm(confirmation)) return;
  const button = $("#validate-capability");
  button.disabled = true;
  button.textContent = pending.length ? "授权中…" : "验证中…";
  try {
    if (pending.length) {
      await grantPermissions(capability, pending);
      button.textContent = "验证中…";
    }
    const result = await api(`/v1/capabilities/${encodeURIComponent(capability.id)}/validate`, {
      method: "POST",
      body: { actor: "web-console", confirm: true },
    });
    state.tools = result.tools || [];
    renderValidation(result);
    await refreshCapabilityOverview();
    const message = result.status === "passed" ? "服务验证通过" : result.status === "blocked" ? "等待权限授权" : "服务验证未通过";
    showToast(message, result.status === "failed");
  } finally {
    button.disabled = false;
    renderPermissions();
  }
}

function renderValidation(result) {
  $("#validation-summary").textContent = result.status === "passed" ? "验证通过" : result.status === "blocked" ? "等待授权" : "验证未通过";
  const list = $("#validation-checks");
  list.replaceChildren();
  for (const check of result.checks) {
    const item = document.createElement("div");
    item.className = `validation-check ${check.status}`;
    const title = document.createElement("strong");
    title.textContent = check.name === "permission" ? "权限授权" : check.name;
    const message = document.createElement("span");
    message.textContent = check.message;
    item.append(title, message);
    list.append(item);
  }
}

async function toggleSelectedCapability() {
  const capability = selectedCapability();
  if (!capability) return;
  await api(`/v1/capabilities/${encodeURIComponent(capability.id)}/enabled`, {
    method: "PUT",
    body: { enabled: !capability.enabled, expected_revision: capability.revision, actor: "web-console" },
  });
  showToast(capability.enabled ? "服务已停用" : "服务已启用");
  await loadAll();
}

async function deleteSelectedCapability() {
  const capability = selectedCapability();
  if (!capability) return;
  const capabilityType = capabilityKind(capability);
  const kind = kindLabel(capabilityType);
  const deletesSkillFiles = capabilityType === "skill";
  const secretNote = state.secrets.some((secret) => secret.provider === "local")
    ? "\n它保存在本机的认证信息也会一并删除。"
    : "";
  const deletionNote = deletesSkillFiles
    ? "\n该 Skill 的所有历史版本和服务器包文件夹都会永久删除，无法恢复。"
    : "";
  const confirmed = window.confirm(
    `删除 ${kind}“${capability.manifest.name}”？${deletionNote}${secretNote}\n\n操作记录会保留；删除后可以重新导入同名能力。`,
  );
  if (!confirmed) return;
  const button = $("#delete-capability");
  button.disabled = true;
  button.textContent = "正在删除…";
  try {
    await api(`/v1/capabilities/${encodeURIComponent(capability.id)}`, {
      method: "DELETE",
      body: { expected_revision: capability.revision, actor: "web-console" },
    });
    state.selectedId = null;
    state.permissions = [];
    state.secrets = [];
    state.tools = [];
    state.runtime = null;
    window.history.replaceState(null, "", "/services");
    await loadAll({ preserveSelection: false });
    showToast(deletesSkillFiles ? `${kind}登记和包文件已彻底删除` : `${kind}登记已删除，可以重新导入`);
  } finally {
    button.disabled = false;
    button.textContent = "删除登记";
  }
}

async function allowPermissions() {
  const capability = selectedCapability();
  if (!capability) return;
  const pending = state.permissions.filter((item) => item.decision !== "allow");
  if (!pending.length) return;
  const detail = pending.map((item) => `• ${permissionDescription(item.permission)}`).join("\n");
  if (!window.confirm(`允许以下运行权限？\n\n${detail}\n\n请先在“技术详情”中确认命令、地址和权限范围。`)) return;
  await grantPermissions(capability, pending);
  await refreshCapabilityOverview();
  showToast("运行权限已允许，可以开始验证");
}

async function discoverTools() {
  const capability = selectedCapability();
  if (!capability) return;
  const button = $("#discover-tools");
  button.disabled = true;
  button.textContent = "正在连接…";
  try {
    state.tools = await api(`/v1/capabilities/${encodeURIComponent(capability.id)}/tools`);
    showToast(`发现 ${state.tools.length} 个工具`);
  } finally {
    button.disabled = false;
    button.textContent = "刷新工具列表";
  }
}

async function executeTest(event) {
  event.preventDefault();
  const capability = selectedCapability();
  if (!capability) return;
  const prompt = $("#test-prompt").value.trim();
  if (!prompt) throw new Error("请输入测试消息");
  const result = $("#test-result");
  result.textContent = "正在发现当前服务的工具…";
  try {
    state.tools = await api(`/v1/capabilities/${encodeURIComponent(capability.id)}/tools`);
    if (!state.tools.length) throw new Error("当前服务没有发现可调用工具");
    result.textContent = `已发现 ${state.tools.length} 个工具，正在由当前模型选择并调用…`;
    const payload = await api("/v1/chat", {
      method: "POST",
      body: {
        model: null,
        prompt,
        history: [],
        attachments: [],
        capability_ids: [capability.id],
        actor: "web-console",
      },
    });
    const model = `${payload.model.name} · ${payload.model.provider}/${payload.model.model_id}`;
    if (!payload.tool_calls.length) {
      result.textContent = `测试未通过：模型没有调用当前服务的工具。\n\n模型回复：${payload.answer || "（无文本）"}\n\n模型：${model}\n已发现工具：${state.tools.map((tool) => tool.name).join(", ")}`;
      showToast("测试未通过：模型没有调用工具，请把测试意图描述得更明确", true);
      return;
    }
    result.textContent = `${payload.answer || "（模型未返回文本）"}\n\n模型：${model}\n实际调用：${payload.tool_calls.join(", ")}`;
    showToast("工具调用测试通过");
  } catch (error) {
    result.textContent = `测试失败\n\n${error.message || String(error)}`;
    throw error;
  }
}

let toastTimer = null;
function showToast(message, error = false) {
  const toast = $("#toast");
  toast.textContent = message;
  toast.classList.toggle("error", error);
  toast.classList.add("visible");
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toast.classList.remove("visible"), 4200);
}

function guarded(handler) {
  return async (event) => {
    try { await handler(event); } catch (error) { showToast(error.message || String(error), true); }
  };
}

$("#refresh-button").addEventListener("click", guarded(() => loadAll()));
$("#prepare-capability").addEventListener("click", guarded(async () => {
  const capability = state.capabilities.find(item => item.id === state.selectedId);
  if (!capability) return;
  if (!window.confirm("启用能力并检查声明的运行依赖？仅使用已授权权限，不执行上传脚本或业务操作；缺少依赖时会明确提示。")) return;
  const button = $("#prepare-capability");
  button.disabled = true;
  try {
    const endpoint = `/v1/capabilities/${encodeURIComponent(capability.id)}/preparation`;
    let record = await api(endpoint, {method: "POST", body: {expected_revision: capability.revision, actor: "web-console", confirm: true}});
    showToast(record.message);
    const deadline = Date.now() + 35 * 60 * 1000;
    while (["queued", "preparing"].includes(record.status) && Date.now() < deadline) {
      button.textContent = "正在准备…";
      await new Promise(resolve => setTimeout(resolve, 2000));
      record = await api(endpoint);
      if (state.selectedId !== capability.id) return;
    }
    showToast(record.message, record.status === "blocked");
    await loadAll();
  } finally {
    button.disabled = false;
    button.textContent = "自动准备并启用";
  }
}));
$("#validate-capability").addEventListener("click", guarded(validateSelectedCapability));
$("#toggle-enabled").addEventListener("click", guarded(toggleSelectedCapability));
$("#allow-permissions").addEventListener("click", guarded(allowPermissions));
$("#prepare-python-runtime").addEventListener("click", guarded(preparePythonRuntime));
$("#delete-capability").addEventListener("click", guarded(deleteSelectedCapability));
$("#discover-tools").addEventListener("click", guarded(discoverTools));
$("#copy-skill-instructions").addEventListener("click", guarded(async () => {
  if (!state.skillInstructions) throw new Error("当前没有可复制的技能说明");
  await navigator.clipboard.writeText(state.skillInstructions);
  const button = $("#copy-skill-instructions");
  button.textContent = "已复制";
  window.setTimeout(() => { button.textContent = "复制说明"; }, 1200);
}));
$("#test-form").addEventListener("submit", guarded(executeTest));
$("#capability-search").addEventListener("input", (event) => {
  state.search = event.currentTarget.value;
  renderCapabilities();
});
for (const filter of $$(".filter")) {
  filter.addEventListener("click", () => {
    state.filter = filter.dataset.filter;
    for (const item of $$(".filter")) item.classList.toggle("active", item === filter);
    renderCapabilities();
  });
}

loadAll({ preserveSelection: true });
