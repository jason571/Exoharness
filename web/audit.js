"use strict";

const $ = (selector) => document.querySelector(selector);

const state = {
  audit: [],
  capabilities: [],
  filter: "all",
  search: "",
};

const ACTIONS = {
  "capability.registered": { label: "添加能力", group: "change" },
  "capability.updated": { label: "更新能力", group: "change" },
  "capability.rolled_back": { label: "恢复旧版本", group: "change" },
  "capability.enabled": { label: "启用能力", group: "change" },
  "capability.disabled": { label: "停用能力", group: "change" },
  "capability.removed": { label: "移除能力", group: "change" },
  "permission.decided": { label: "更新权限", group: "permission" },
  "health.updated": { label: "完成验证", group: "validation" },
  "capability.invocation.started": { label: "开始调用", group: "invocation" },
  "capability.invocation.completed": { label: "调用完成", group: "invocation" },
};

const GROUP_LABELS = {
  change: "能力变更",
  validation: "验证",
  permission: "权限",
  invocation: "调用",
  other: "其他",
};

async function api(path) {
  const response = await window.ExoAuth.fetch(path);
  const text = await response.text();
  let payload = null;
  if (text) {
    try { payload = JSON.parse(text); } catch { payload = text; }
  }
  if (!response.ok) throw new Error(payload?.error?.message || `${response.status} ${response.statusText}`);
  return payload;
}

function setService(online, label, failed = false) {
  $("#service-dot").classList.toggle("online", online);
  $("#service-dot").classList.toggle("offline", failed);
  $("#service-label").textContent = label;
}

function showToast(message, error = false) {
  const toast = $("#toast");
  toast.textContent = message;
  toast.classList.toggle("error", error);
  toast.classList.add("visible");
  window.setTimeout(() => toast.classList.remove("visible"), 4200);
}

function actionInfo(event) {
  const info = ACTIONS[event.action] || { label: "系统操作", group: "other" };
  if (event.action === "capability.invocation.completed" && event.details?.status === "failed") {
    return { ...info, label: "调用失败" };
  }
  return info;
}

function invocationTool(event) {
  return typeof event.details?.tool === "string" ? event.details.tool : "";
}

function capabilityName(event) {
  const capability = state.capabilities.find((item) => item.id === event.capability_id);
  if (capability) return capability.manifest.name;
  return event.capability_id ? "已移除的能力" : "系统";
}

function actorLabel(actor) {
  if (actor === "web-console") return "控制台";
  if (actor === "system") return "系统";
  if (actor?.startsWith("codex-")) return "自动任务";
  return actor || "未知来源";
}

function formatTime(timestamp) {
  const milliseconds = timestamp > 10_000_000_000 ? timestamp : timestamp * 1000;
  return new Date(milliseconds).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function visibleEvents() {
  const query = state.search.trim().toLowerCase();
  return state.audit.filter((event) => {
    const info = actionInfo(event);
    if (state.filter !== "all" && info.group !== state.filter) return false;
    if (!query) return true;
    return [info.label, GROUP_LABELS[info.group], capabilityName(event), invocationTool(event), actorLabel(event.actor)]
      .some((value) => value.toLowerCase().includes(query));
  });
}

function renderAudit() {
  const list = $("#audit-record-list");
  const events = visibleEvents();
  list.replaceChildren();
  $("#audit-result-count").textContent = state.filter === "all" && !state.search.trim()
    ? (state.audit.length < 200 ? `共 ${state.audit.length} 条记录` : "显示最近 200 条记录")
    : `显示 ${events.length} 条 · 共 ${state.audit.length} 条`;
  if (!events.length) {
    const empty = document.createElement("div");
    empty.className = "audit-empty-state";
    const icon = document.createElement("span");
    icon.textContent = "⌕";
    const title = document.createElement("strong");
    title.textContent = state.audit.length ? "没有符合条件的记录" : "暂无操作记录";
    const copy = document.createElement("p");
    copy.textContent = state.audit.length ? "可以调整搜索内容或记录类型。" : "添加、验证或启停能力后，记录会显示在这里。";
    empty.append(icon, title, copy);
    list.append(empty);
    return;
  }
  for (const event of events) {
    const info = actionInfo(event);
    const item = document.createElement("article");
    item.className = `audit-record ${info.group}`;
    const marker = document.createElement("span");
    marker.className = "audit-record-marker";
    marker.setAttribute("aria-hidden", "true");
    const copy = document.createElement("div");
    copy.className = "audit-record-copy";
    const heading = document.createElement("div");
    const title = document.createElement("strong");
    title.textContent = info.label;
    const group = document.createElement("span");
    group.textContent = GROUP_LABELS[info.group];
    heading.append(title, group);
    const name = document.createElement("p");
    const tool = invocationTool(event);
    name.textContent = tool ? `${capabilityName(event)} · ${tool}` : capabilityName(event);
    const meta = document.createElement("small");
    const duration = Number(event.details?.duration_ms);
    const durationLabel = event.action === "capability.invocation.completed" && Number.isFinite(duration)
      ? ` · ${duration} ms`
      : "";
    meta.textContent = `${formatTime(event.created_at)} · ${actorLabel(event.actor)}${durationLabel}`;
    copy.append(heading, name, meta);
    item.append(marker, copy);
    list.append(item);
  }
}

async function loadAudit() {
  const button = $("#audit-refresh");
  button.disabled = true;
  setService(false, "正在同步操作记录");
  try {
    const [audit, capabilities] = await Promise.all([api("/v1/audit"), api("/v1/capabilities")]);
    state.audit = audit;
    state.capabilities = capabilities;
    renderAudit();
    setService(true, "服务在线");
  } catch (error) {
    setService(false, "服务不可用", true);
    showToast(error.message || String(error), true);
  } finally {
    button.disabled = false;
  }
}

$("#audit-search").addEventListener("input", (event) => {
  state.search = event.currentTarget.value;
  renderAudit();
});
document.querySelectorAll("[data-audit-filter]").forEach((button) => {
  button.addEventListener("click", () => {
    state.filter = button.dataset.auditFilter;
    document.querySelectorAll("[data-audit-filter]").forEach((item) => item.classList.toggle("active", item === button));
    renderAudit();
  });
});
$("#audit-refresh").addEventListener("click", loadAudit);

loadAudit();
