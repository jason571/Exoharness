"use strict";

const $ = (selector) => document.querySelector(selector);

const DEFAULT_MCP_JSON = `{
  "mcpServers": {
    "chrome-mcp-server": {
      "type": "streamableHttp",
      "url": "http://127.0.0.1:12306/mcp"
    }
  }
}`;

let pendingSkillImport = null;

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

async function checkService() {
  try {
    await api("/health");
    $("#service-dot").classList.add("online");
    $("#service-label").textContent = "服务在线";
  } catch {
    $("#service-dot").classList.add("offline");
    $("#service-label").textContent = "服务不可用";
  }
}

function slugify(value) {
  return value.toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^[._-]+|[._-]+$/g, "") || "uploaded-tool";
}

function fileStem(fileName) {
  return fileName.replace(/\.[^.]+$/, "");
}

function isEnvironmentReference(value) {
  return /^\$\{[A-Z0-9_]+\}$/.test(value);
}

function isSensitiveHeader(name) {
  const normalized = name.toLowerCase().replaceAll("_", "-");
  return ["authorization", "proxy-authorization", "cookie", "set-cookie"].includes(normalized)
    || ["token", "secret", "api-key", "apikey", "credential", "password"]
      .some((marker) => normalized.includes(marker));
}

function inspectMcpHttpUrl(value) {
  let endpoint;
  try {
    endpoint = new URL(value);
  } catch {
    return { valid: false, insecure: false };
  }
  if (endpoint.protocol === "https:") return { valid: true, insecure: false };
  if (endpoint.protocol !== "http:") return { valid: false, insecure: false };
  const host = endpoint.hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (host === "localhost" || host === "::1" || host.startsWith("127.")) {
    return { valid: true, insecure: false };
  }
  const octets = host.split(".").map(Number);
  const privateIpv4 = octets.length === 4 && octets.every((part) => Number.isInteger(part) && part >= 0 && part <= 255)
    && (octets[0] === 10
      || (octets[0] === 172 && octets[1] >= 16 && octets[1] <= 31)
      || (octets[0] === 192 && octets[1] === 168)
      || (octets[0] === 169 && octets[1] === 254));
  const privateIpv6 = host.startsWith("fc") || host.startsWith("fd") || host.startsWith("fe80:");
  return { valid: privateIpv4 || privateIpv6, insecure: privateIpv4 || privateIpv6 };
}

function previewText(text) {
  const limit = 20000;
  return text.length > limit ? `${text.slice(0, limit)}\n\n… preview truncated …` : text;
}

function inspectMcpJson(content) {
  if (!content.trim()) {
    return { state: "waiting", title: "等待粘贴配置", detail: "粘贴后会立即检查 JSON 格式和服务数量", valid: false };
  }
  let value;
  try {
    value = JSON.parse(content);
  } catch (error) {
    return { state: "invalid", title: "JSON 格式有误", detail: error.message, valid: false };
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return { state: "invalid", title: "配置格式不正确", detail: "最外层需要是一个 JSON 对象", valid: false };
  }
  const servers = value.mcpServers ?? value.mcp_servers;
  if (servers !== undefined) {
    if (!servers || typeof servers !== "object" || Array.isArray(servers)) {
      return { state: "invalid", title: "mcpServers 格式不正确", detail: "mcpServers 需要是一个包含服务配置的对象", valid: false };
    }
    const entries = Object.entries(servers);
    if (!entries.length) {
      return { state: "invalid", title: "没有找到 MCP 服务", detail: "请在 mcpServers 中至少添加一个服务", valid: false };
    }
    let httpCount = 0;
    let stdioCount = 0;
    let headerReferenceCount = 0;
    let localSecretCount = 0;
    let insecureHttpCount = 0;
    let authorizationWithoutSchemeCount = 0;
    for (const [name, config] of entries) {
      if (!config || typeof config !== "object" || Array.isArray(config)) {
        return { state: "invalid", title: `${name} 的配置不正确`, detail: "每个 MCP 服务都需要是一个 JSON 对象", valid: false };
      }
      if (typeof config.url === "string" && config.url.trim()) {
        const endpoint = inspectMcpHttpUrl(config.url.trim());
        if (!endpoint.valid) {
          return { state: "invalid", title: `${name} 的地址不安全`, detail: "请使用 HTTPS；HTTP 仅允许 localhost 或私有局域网 IP", valid: false };
        }
        httpCount += 1;
        if (endpoint.insecure) insecureHttpCount += 1;
        if (config.headers !== undefined) {
          if (!config.headers || typeof config.headers !== "object" || Array.isArray(config.headers)) {
            return { state: "invalid", title: `${name} 的 headers 不正确`, detail: "headers 需要是 Header 名称到字符串值的对象", valid: false };
          }
          for (const [headerName, headerValue] of Object.entries(config.headers)) {
            if (typeof headerValue !== "string") {
              return { state: "invalid", title: `${name} 的 Header 不正确`, detail: `${headerName} 的值必须是字符串`, valid: false };
            }
            if (isEnvironmentReference(headerValue)) {
              headerReferenceCount += 1;
            } else if (isSensitiveHeader(headerName)) {
              localSecretCount += 1;
              if (headerName.toLowerCase().replaceAll("_", "-") === "authorization"
                && !/^[A-Za-z][A-Za-z0-9+.-]*\s+\S/.test(headerValue.trim())) {
                authorizationWithoutSchemeCount += 1;
              }
            }
          }
        }
      } else if (typeof config.command === "string" && config.command.trim()) stdioCount += 1;
      else return { state: "invalid", title: `${name} 缺少连接信息`, detail: "HTTP 服务需要 url，本地服务需要 command", valid: false };
    }
    const transports = [
      httpCount ? `${httpCount} 个 HTTP` : "",
      stdioCount ? `${stdioCount} 个本地服务` : "",
    ].filter(Boolean).join(" · ");
    const headerSummary = headerReferenceCount ? ` · ${headerReferenceCount} 个 Header 密钥引用` : "";
    const localSecretSummary = localSecretCount ? ` · ${localSecretCount} 个认证信息将保存到本机密钥保险箱` : "";
    const networkWarning = insecureHttpCount ? ` · ${insecureHttpCount} 个局域网 HTTP（未加密）` : "";
    const authorizationWarning = authorizationWithoutSchemeCount
      ? ` · ${authorizationWithoutSchemeCount} 个 Authorization 没有认证方案；若服务使用 Bearer，请粘贴完整的 Bearer <Token>`
      : "";
    return { state: "valid", title: `已识别 ${entries.length} 个 MCP 服务`, detail: `${transports}${headerSummary}${localSecretSummary}${networkWarning}${authorizationWarning}`, valid: true };
  }
  if (value.spec?.kind === "mcp_server") {
    return { state: "valid", title: "已识别 ExoHarness MCP 配置", detail: value.name || "1 个 MCP 服务", valid: true };
  }
  return { state: "invalid", title: "没有找到 MCP 配置", detail: "请粘贴包含 mcpServers 的 JSON，或 ExoHarness MCP 配置", valid: false };
}

function updateMcpJsonState() {
  const inspection = inspectMcpJson($("#mcp-json-input").value);
  const status = $("#mcp-json-status");
  status.className = `json-validation-status ${inspection.state}`;
  status.querySelector("strong").textContent = inspection.title;
  status.querySelector("small").textContent = inspection.detail;
  $("#mcp-json-submit").disabled = !inspection.valid;
  return inspection;
}

function setMcpJsonContent(content, fileName = "") {
  $("#mcp-json-input").value = content;
  $("#mcp-file-label").textContent = fileName || "已使用粘贴内容";
  updateMcpJsonState();
}

function selectImportKind(kind) {
  document.querySelectorAll("[data-import-kind]").forEach((button) => {
    const active = button.dataset.importKind === kind;
    button.classList.toggle("active", active);
    button.setAttribute("aria-selected", String(active));
  });
  document.querySelectorAll("[data-import-panel]").forEach((panel) => {
    panel.classList.toggle("hidden", panel.dataset.importPanel !== kind);
  });
}

function selectMcpMethod(method) {
  document.querySelectorAll("[data-mcp-method]").forEach((button) => {
    const active = button.dataset.mcpMethod === method;
    button.classList.toggle("active", active);
    button.setAttribute("aria-selected", String(active));
  });
  document.querySelectorAll("[data-mcp-method-panel]").forEach((panel) => {
    panel.classList.toggle("hidden", panel.dataset.mcpMethodPanel !== method);
  });
}

function fileToBase64(file) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error(`无法读取文件 ${file.name}`));
    reader.onload = () => {
      const result = String(reader.result);
      const comma = result.indexOf(",");
      if (comma < 0) reject(new Error(`无法编码文件 ${file.name}`));
      else resolve(result.slice(comma + 1));
    };
    reader.readAsDataURL(file);
  });
}

function selectedSkillFiles() {
  const folder = Array.from($("#skill-folder").files || []);
  if (folder.length) return folder;
  return Array.from($("#skill-entry-file").files || []);
}

function selectedPackage(folderSelector, zipSelector) {
  const zip = $(zipSelector).files?.[0];
  const files = Array.from($(folderSelector).files || []);
  return { zip, files };
}

async function packageContent(zip, files) {
  if (zip) return { file_name: zip.name, archive_base64: await fileToBase64(zip), files: [] };
  if (!files.length) throw new Error("请选择 ZIP 或目录");
  const root = files[0].webkitRelativePath?.split("/")[0] || files[0].name || "package";
  const uploaded = [];
  for (const file of files) uploaded.push({ path: file.webkitRelativePath || file.name, content_base64: await fileToBase64(file) });
  return { file_name: root, archive_base64: null, files: uploaded };
}

function renderSkillFiles() {
  const files = selectedSkillFiles();
  const target = $("#skill-file-list");
  if (!files.length) {
    target.textContent = "等待选择 Skill 文件";
    return;
  }
  const total = files.reduce((sum, file) => sum + file.size, 0);
  const names = files.slice(0, 8).map((file) => file.webkitRelativePath || file.name);
  target.textContent = `${files.length} 个文件 · ${total.toLocaleString()} bytes\n${names.join("\n")}${files.length > names.length ? "\n…" : ""}`;
}

function showImported(capabilities, warnings = []) {
  const panel = $("#import-result-panel");
  const list = $("#import-result-list");
  const kindLabels = { mcp_server: "MCP 服务", skill: "Skill", tool_node: "本地工具" };
  list.replaceChildren();
  for (const capability of capabilities) {
    const item = document.createElement("a");
    item.className = "import-result-item";
    item.href = `/services?capability=${encodeURIComponent(capability.id)}`;
    const title = document.createElement("strong");
    title.textContent = capability.manifest.name;
    const meta = document.createElement("span");
    meta.textContent = `${kindLabels[capability.manifest.spec.kind] || "能力"} · v${capability.manifest.version} · ${capability.enabled ? "已启用" : "未启用"}`;
    item.append(title, meta);
    list.append(item);
    if ($("#auto-prepare")?.checked) {
      void prepareImported(capability, meta);
    }
  }
  for (const warning of warnings) {
    const item = document.createElement("p");
    item.className = "import-warning";
    item.textContent = warning;
    list.append(item);
  }
  panel.classList.remove("hidden");
  panel.scrollIntoView({ behavior: "smooth", block: "start" });
}

async function prepareImported(capability, target) {
  target.textContent = "已登记，正在准备运行条件…";
  const endpoint = `/v1/capabilities/${encodeURIComponent(capability.id)}/preparation`;
  try {
    let record = await api(endpoint, { method: "POST", body: {
      expected_revision: capability.revision, actor: "web-console", confirm: true,
    } });
    const deadline = Date.now() + 35 * 60 * 1000;
    while (true) {
      if (!target.isConnected) return;
      target.textContent = record.message;
      target.dataset.preparationStatus = record.status;
      if (!["queued", "preparing"].includes(record.status)) return;
      if (Date.now() > deadline) {
        target.textContent = "准备仍在后台进行，可前往服务与能力查看";
        return;
      }
      await new Promise(resolve => setTimeout(resolve, 2000));
      record = await api(endpoint);
    }
  } catch (error) {
    target.textContent = `已登记，尚未完成准备：${error.message || String(error)}`;
    target.dataset.preparationStatus = "blocked";
  }
}

function clearSkillUpdatePreview() {
  pendingSkillImport = null;
  $("#skill-update-preview").classList.add("hidden");
  $("#skill-preview-changes").replaceChildren();
  $("#skill-submit").textContent = "检查并添加 Skill";
}

function renderSkillUpdatePreview(preview) {
  const panel = $("#skill-update-preview");
  const changes = $("#skill-preview-changes");
  const labels = { added: "新增", modified: "修改", removed: "移除" };
  panel.classList.remove("hidden");
  changes.replaceChildren();
  $("#skill-preview-title").textContent = preview.action === "unchanged" ? `${preview.name} 没有变化` : `更新 ${preview.name}`;
  $("#skill-preview-version").textContent = preview.current_revision
    ? `v${preview.current_version} → v${preview.proposed_version} · revision ${preview.current_revision}`
    : `将创建 v${preview.proposed_version}`;
  const digest = preview.proposed_digest.slice(0, 12);
  $("#skill-preview-summary").textContent = preview.action === "unchanged"
    ? `上传内容与当前不可变版本一致（${digest}…），不会创建重复版本。`
    : `发现 ${preview.file_changes.length} 项文件变化，新内容摘要 ${digest}…`;
  for (const change of preview.file_changes.slice(0, 80)) {
    const item = document.createElement("li");
    item.className = change.change;
    const kind = document.createElement("span");
    kind.textContent = labels[change.change] || "变化";
    const path = document.createElement("code");
    path.textContent = change.path;
    item.append(kind, path);
    changes.append(item);
  }
  for (const permission of preview.permission_changes.added) {
    const item = document.createElement("li");
    item.className = "added";
    const kind = document.createElement("span");
    kind.textContent = "新增权限";
    const name = document.createElement("code");
    name.textContent = permission;
    item.append(kind, name);
    changes.append(item);
  }
  for (const permission of preview.permission_changes.removed) {
    const item = document.createElement("li");
    item.className = "removed";
    const kind = document.createElement("span");
    kind.textContent = "移除权限";
    const name = document.createElement("code");
    name.textContent = permission;
    item.append(kind, name);
    changes.append(item);
  }
  if (!changes.children.length && preview.action !== "unchanged") {
    const item = document.createElement("li");
    const kind = document.createElement("span");
    kind.textContent = "配置";
    const copy = document.createElement("code");
    copy.textContent = "版本、描述或来源信息发生变化";
    item.append(kind, copy);
    changes.append(item);
  }
  panel.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

async function prepareSkillImport() {
  const zip = $("#skill-zip").files[0];
  const permissions = $("#skill-permissions").value.split(",").map((item) => item.trim()).filter(Boolean);
  const common = {
    version: $("#skill-version").value,
    permissions,
    scope: { kind: "global" },
    actor: "web-console",
  };
  if (zip) {
    const archive_base64 = await fileToBase64(zip);
    const body = {
      file_name: zip.name,
      archive_base64,
      files: [],
      ...common,
    };
    return {
      previewBody: { package_kind: "skill", ...body },
      importPath: "/v1/imports/skill/package",
      importBody: { ...body },
    };
  }
  const files = selectedSkillFiles();
  if (!files.length) throw new Error("请选择 SKILL.md、Skill 文件夹或 ZIP");
  const uploaded = [];
  for (const file of files) {
    uploaded.push({ path: file.webkitRelativePath || file.name, content_base64: await fileToBase64(file) });
  }
  const firstPath = uploaded[0].path;
  const file_name = firstPath.includes("/") ? firstPath.split("/")[0] : firstPath;
  return {
    previewBody: { package_kind: "skill", file_name, archive_base64: null, files: uploaded, ...common },
    importPath: "/v1/imports/skill",
    importBody: { files: uploaded, ...common },
  };
}

async function uploadMcp(event) {
  event.preventDefault();
  const content = $("#mcp-json-input").value.trim();
  const inspection = inspectMcpJson(content);
  if (!inspection.valid) throw new Error(inspection.detail || inspection.title);
  const file = $("#mcp-file").files[0];
  const result = await api("/v1/imports/manifest", {
    method: "POST",
    body: { file_name: file?.name || "pasted-mcp.json", content, scope: { kind: "global" }, actor: "web-console" },
  });
  showImported(result.capabilities, result.warnings);
  setMcpJsonContent("");
  $("#mcp-file").value = "";
  showToast(`已登记 ${result.capabilities.length} 个 MCP 服务，检查结果见下方`);
}

async function uploadSkill(event) {
  event.preventDefault();
  let prepared = pendingSkillImport;
  if (!prepared) {
    prepared = await prepareSkillImport();
    const preview = await api("/v1/imports/skill/preview", {
      method: "POST",
      body: prepared.previewBody,
    });
    if (preview.action === "unchanged") {
      renderSkillUpdatePreview(preview);
      $("#skill-submit").textContent = "重新检查";
      showToast("当前 Skill 已是相同内容，没有创建重复版本");
      if ($("#auto-prepare")?.checked) {
        const current = await api(`/v1/capabilities/${encodeURIComponent(preview.capability_id)}`);
        showImported([current]);
      }
      return;
    }
    if (preview.requires_confirmation) {
      prepared.importBody.expected_revision = preview.current_revision;
      pendingSkillImport = prepared;
      renderSkillUpdatePreview(preview);
      $("#skill-submit").textContent = "确认创建新版本";
      showToast("请检查变更；再次确认后才会创建新版本");
      return;
    }
  }
  const updating = Number.isInteger(prepared.importBody.expected_revision);
  let result;
  try {
    result = await api(prepared.importPath, {
      method: "POST",
      body: prepared.importBody,
    });
  } catch (error) {
    if (updating) clearSkillUpdatePreview();
    throw error;
  }
  const capabilities = result.capabilities || [result.capability];
  showImported(capabilities, result.warnings || []);
  clearSkillUpdatePreview();
  showToast(updating ? "Skill 已更新，准备结果见下方" : "Skill 已登记，准备结果见下方");
}

async function uploadMcpPackage(event) {
  event.preventDefault();
  const entry = $("#mcp-package-entry-file").files?.[0];
  const selected = selectedPackage("#mcp-package-folder", "#mcp-package-zip");
  if (entry && (selected.zip || selected.files.length)) throw new Error("启动文件、ZIP 和目录只能选择一种");
  const content = await packageContent(selected.zip, entry ? [entry] : selected.files);
  const permissions = $("#mcp-package-network").checked ? ["network.egress"] : [];
  const result = await api("/v1/imports/package", {
    method: "POST",
    body: {
      package_kind: "mcp_server",
      ...content,
      name: $("#mcp-package-name").value,
      version: $("#mcp-package-version").value,
      description: $("#mcp-package-description").value,
      namespace: $("#mcp-package-namespace").value,
      permissions,
      scope: { kind: "global" },
      actor: "web-console",
    },
  });
  showImported(result.capabilities, result.warnings);
  showToast("本地 MCP 已导入或更新，可前往验证");
}

async function uploadHttpMcp(event) {
  event.preventDefault();
  const result = await api("/v1/imports/mcp-http", {
    method: "POST",
    body: {
      name: $("#mcp-http-name").value,
      version: $("#mcp-http-version").value,
      description: $("#mcp-http-description").value,
      namespace: $("#mcp-http-namespace").value,
      url: $("#mcp-http-url").value,
      bearer_token: $("#mcp-http-secret").value || null,
      scope: { kind: "global" },
      actor: "web-console",
    },
  });
  $("#mcp-http-secret").value = "";
  showImported(result.capabilities, result.warnings);
  showToast("HTTP MCP 已导入；同名能力已更新，可前往验证");
}

async function uploadTool(event) {
  event.preventDefault();
  const file = $("#tool-file").files[0];
  const selected = selectedPackage("#tool-package-folder", "#tool-package-zip");
  const hasPackage = Boolean(selected.zip || selected.files.length);
  if (!file && !hasPackage) throw new Error("请选择工具脚本、ZIP 或目录");
  let inputSchema;
  try { inputSchema = JSON.parse($("#tool-input-schema").value); } catch { throw new Error("输入 Schema 不是有效 JSON"); }
  if (hasPackage) {
    const content = await packageContent(selected.zip, selected.files);
    const result = await api("/v1/imports/package", {
      method: "POST",
      body: {
        package_kind: "tool_node",
        ...content,
        name: $("#tool-capability-name").value,
        version: $("#tool-version").value,
        description: $("#tool-capability-description").value,
        runtime: $("#tool-package-runtime").value,
        executable: $("#tool-package-executable").value,
        entrypoint: $("#tool-package-entrypoint").value,
        tool_name: $("#tool-name").value,
        tool_description: $("#tool-description").value,
        input_schema: inputSchema,
        permissions: [],
        scope: { kind: "global" },
        actor: "web-console",
      },
    });
    showImported(result.capabilities, result.warnings);
    showToast("本地工具已导入或更新，尚未运行");
    return;
  }
  const result = await api("/v1/imports/tool", {
    method: "POST",
    body: {
      file_name: file.name,
      content_base64: await fileToBase64(file),
      name: $("#tool-capability-name").value,
      version: $("#tool-version").value,
      description: $("#tool-capability-description").value,
      tool_name: $("#tool-name").value,
      tool_description: $("#tool-description").value,
      input_schema: inputSchema,
      permissions: [],
      scope: { kind: "global" },
      actor: "web-console",
    },
  });
  showImported([result]);
  showToast("本地工具已导入或更新，尚未运行");
}

$("#mcp-file").addEventListener("change", async () => {
  const file = $("#mcp-file").files[0];
  if (file) setMcpJsonContent(await file.text(), `${file.name} · ${file.size.toLocaleString()} bytes`);
  else $("#mcp-file-label").textContent = "尚未选择文件";
});

$("#skill-entry-file").addEventListener("change", () => {
  clearSkillUpdatePreview();
  if ($("#skill-entry-file").files.length) { $("#skill-folder").value = ""; $("#skill-zip").value = ""; }
  const file = $("#skill-entry-file").files[0];
  $("#skill-entry-label").textContent = file ? `${file.name} · ${file.size.toLocaleString()} bytes` : "未选择文件";
  renderSkillFiles();
});

$("#skill-folder").addEventListener("change", () => {
  clearSkillUpdatePreview();
  if ($("#skill-folder").files.length) { $("#skill-entry-file").value = ""; $("#skill-zip").value = ""; }
  const files = Array.from($("#skill-folder").files || []);
  $("#skill-folder-label").textContent = files.length ? `${files[0].webkitRelativePath.split("/")[0]} · ${files.length} 个文件` : "未选择文件夹";
  renderSkillFiles();
});

$("#skill-zip").addEventListener("change", () => {
  clearSkillUpdatePreview();
  const file = $("#skill-zip").files[0];
  if (file) { $("#skill-entry-file").value = ""; $("#skill-folder").value = ""; }
  $("#skill-zip-label").textContent = file ? `${file.name} · ${file.size.toLocaleString()} bytes` : "未选择 ZIP";
  $("#skill-file-list").textContent = file ? `ZIP 包 · ${file.name}` : "等待选择 Skill 文件";
});

$("#skill-version").addEventListener("input", clearSkillUpdatePreview);
$("#skill-permissions").addEventListener("input", clearSkillUpdatePreview);

function renderPackageSelection(folderSelector, zipSelector, folderLabel, zipLabel, summarySelector) {
  const { zip, files } = selectedPackage(folderSelector, zipSelector);
  $(zipLabel).textContent = zip ? `${zip.name} · ${zip.size.toLocaleString()} bytes` : "未选择 ZIP";
  $(folderLabel).textContent = files.length ? `${files[0].webkitRelativePath.split("/")[0]} · ${files.length} 个文件` : "未选择目录";
  const total = files.reduce((sum, item) => sum + item.size, 0);
  $(summarySelector).textContent = zip ? `ZIP · ${zip.name}` : (files.length ? `${files.length} 个文件 · ${total.toLocaleString()} bytes` : "等待选择启动文件、ZIP 或目录");
}

$("#mcp-package-zip").addEventListener("change", () => {
  if ($("#mcp-package-zip").files.length) {
    $("#mcp-package-folder").value = "";
    $("#mcp-package-entry-file").value = "";
    $("#mcp-package-entry-file-label").textContent = "未选择文件";
  }
  renderPackageSelection("#mcp-package-folder", "#mcp-package-zip", "#mcp-package-folder-label", "#mcp-package-zip-label", "#mcp-package-files");
});
$("#mcp-package-folder").addEventListener("change", () => {
  if ($("#mcp-package-folder").files.length) {
    $("#mcp-package-zip").value = "";
    $("#mcp-package-entry-file").value = "";
    $("#mcp-package-entry-file-label").textContent = "未选择文件";
  }
  const files = Array.from($("#mcp-package-folder").files || []);
  if (files.length) {
    const root = files[0].webkitRelativePath.split("/")[0];
    const name = slugify(root);
    $("#mcp-package-name").value ||= name;
    $("#mcp-package-namespace").value ||= name;
    $("#mcp-package-description").value ||= `Imported local MCP ${root}`;
  }
  renderPackageSelection("#mcp-package-folder", "#mcp-package-zip", "#mcp-package-folder-label", "#mcp-package-zip-label", "#mcp-package-files");
});

$("#mcp-package-entry-file").addEventListener("change", () => {
  const file = $("#mcp-package-entry-file").files?.[0];
  if (file) {
    $("#mcp-package-zip").value = "";
    $("#mcp-package-folder").value = "";
    $("#mcp-package-zip-label").textContent = "未选择 ZIP";
    $("#mcp-package-folder-label").textContent = "未选择目录";
  }
  $("#mcp-package-entry-file-label").textContent = file ? `${file.name} · ${file.size.toLocaleString()} bytes` : "未选择文件";
  $("#mcp-package-files").textContent = file ? `启动文件 · ${file.name}\n运行方式将根据文件类型自动识别` : "等待选择启动文件、ZIP 或目录";
  if (file) {
    const name = slugify(fileStem(file.name));
    $("#mcp-package-name").value ||= name;
    $("#mcp-package-namespace").value ||= name;
    $("#mcp-package-description").value ||= `Imported local MCP ${file.name}`;
  }
});

$("#tool-file").addEventListener("change", async () => {
  const file = $("#tool-file").files[0];
  if (file) { $("#tool-package-zip").value = ""; $("#tool-package-folder").value = ""; }
  $("#tool-file-label").textContent = file ? `${file.name} · ${file.size.toLocaleString()} bytes` : "未选择文件";
  $("#tool-preview").textContent = file ? previewText(await file.text()) : "选择文件后显示只读预览";
  if (file) {
    const name = slugify(fileStem(file.name));
    $("#tool-capability-name").value ||= name;
    $("#tool-name").value ||= name;
    $("#tool-capability-description").value ||= `Imported tool ${file.name}`;
    $("#tool-description").value ||= `Run ${file.name}`;
  }
});

function renderToolPackageSelection() {
  const { zip, files } = selectedPackage("#tool-package-folder", "#tool-package-zip");
  $("#tool-package-zip-label").textContent = zip ? `${zip.name} · ${zip.size.toLocaleString()} bytes` : "未选择 ZIP";
  $("#tool-package-folder-label").textContent = files.length ? `${files[0].webkitRelativePath.split("/")[0]} · ${files.length} 个文件` : "未选择目录";
  if (zip || files.length) {
    $("#tool-file").value = "";
    const root = zip ? fileStem(zip.name) : files[0].webkitRelativePath.split("/")[0];
    const name = slugify(root);
    $("#tool-capability-name").value ||= name;
    $("#tool-name").value ||= name;
    $("#tool-capability-description").value ||= `Imported tool package ${root}`;
    $("#tool-description").value ||= `Run ${root}`;
    $("#tool-preview").textContent = zip ? `ZIP 包 · ${zip.name}` : `${files.length} 个目录文件`;
  }
}

$("#tool-package-zip").addEventListener("change", () => {
  if ($("#tool-package-zip").files.length) $("#tool-package-folder").value = "";
  renderToolPackageSelection();
});
$("#tool-package-folder").addEventListener("change", () => {
  if ($("#tool-package-folder").files.length) $("#tool-package-zip").value = "";
  renderToolPackageSelection();
});

function updateRuntimeDefaults(selectSelector, executableSelector) {
  const runtime = $(selectSelector).value;
  $(executableSelector).value = runtime.startsWith("python") ? "python3" : (runtime === "node" ? "node" : "sh");
}
$("#tool-package-runtime").addEventListener("change", () => updateRuntimeDefaults("#tool-package-runtime", "#tool-package-executable"));

let toastTimer = null;
function showToast(message, error = false) {
  const toast = $("#toast");
  toast.textContent = message;
  toast.classList.toggle("error", error);
  toast.classList.add("visible");
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toast.classList.remove("visible"), 4400);
}

function guarded(handler) {
  return async (event) => {
    const button = event.currentTarget.querySelector?.("button[type=submit]");
    if (button) button.disabled = true;
    try { await handler(event); } catch (error) { showToast(error.message || String(error), true); } finally {
      if (button) button.disabled = false;
      if (event.currentTarget?.id === "mcp-upload-form") updateMcpJsonState();
    }
  };
}

document.querySelectorAll("[data-import-kind]").forEach((button) => {
  button.addEventListener("click", () => selectImportKind(button.dataset.importKind));
});
document.querySelectorAll("[data-mcp-method]").forEach((button) => {
  button.addEventListener("click", () => selectMcpMethod(button.dataset.mcpMethod));
});
$("#mcp-json-input").placeholder = DEFAULT_MCP_JSON;
$("#mcp-json-input").addEventListener("input", updateMcpJsonState);
$("#mcp-example-button").addEventListener("click", () => setMcpJsonContent(DEFAULT_MCP_JSON, "示例配置"));
$("#mcp-format-button").addEventListener("click", () => {
  const content = $("#mcp-json-input").value.trim();
  if (!content) return showToast("请先粘贴 MCP JSON", true);
  try { setMcpJsonContent(`${JSON.stringify(JSON.parse(content), null, 2)}\n`, $("#mcp-file-label").textContent); }
  catch (error) { showToast(`JSON 格式有误：${error.message}`, true); }
});
$("#mcp-paste-button").addEventListener("click", async () => {
  try {
    const content = await navigator.clipboard.readText();
    if (!content.trim()) throw new Error("剪贴板中没有文本");
    setMcpJsonContent(content, "已从剪贴板粘贴");
    $("#mcp-json-input").focus();
  } catch (error) {
    showToast(`${error.message || "无法读取剪贴板"}；也可以在编辑框中按 Ctrl + V`, true);
    $("#mcp-json-input").focus();
  }
});

$("#mcp-upload-form").addEventListener("submit", guarded(uploadMcp));
$("#mcp-http-form").addEventListener("submit", guarded(uploadHttpMcp));
$("#mcp-package-form").addEventListener("submit", guarded(uploadMcpPackage));
$("#skill-upload-form").addEventListener("submit", guarded(uploadSkill));
$("#tool-upload-form").addEventListener("submit", guarded(uploadTool));
checkService();
updateMcpJsonState();
