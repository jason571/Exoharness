// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

interface Session {
  id: string; client_id: string; user_id: string; user_label: string;
  external_user_id?: string;
  device_id: string | null; assistant_id: string; title: string;
  turn_count: number; updated_at: number;
}
interface Page { sessions: Session[]; total: number; page: number; page_size: number; has_more: boolean; }
interface FileResult { filename?: string; download_url?: string; url?: string; }
interface Turn {
  id: string; ordinal: number; prompt: string; answer?: string | null;
  attachments?: { name?: string; download_url?: string; url?: string }[];
  run_id?: string | null; run_status?: string | null;
  response?: { answer?: string; artifacts?: FileResult[]; public_notice?: { message?: string } } | null;
}
interface Detail { id: string; title: string; external: Session; turns: Turn[]; history_page: { has_more: boolean; before_ordinal: number | null }; }
interface Application { client_id: string; name: string; require_device_id?: boolean; }
type Api = <T>(path: string, method?: string, body?: unknown) => Promise<T>;
const el = <T extends HTMLElement = HTMLElement>(id: string): T => document.getElementById(id) as T;
function text<K extends keyof HTMLElementTagNameMap>(tag: K, value: string, data = false) {
  const node = document.createElement(tag); node.textContent = value;
  if (data) node.dataset.i18n = "off";
  return node;
}
function field(value: string, extra?: string) {
  const cell = text("td", ""); cell.append(text("span", value, true));
  if (extra) cell.append(text("small", extra, true));
  return cell;
}
function fileLink(name: string, value: string | undefined): HTMLAnchorElement | null {
  if (!value) return null;
  try {
    const parsed = new URL(value);
    if (!["http:", "https:"].includes(parsed.protocol) || parsed.username || parsed.password) return null;
    const link = text("a", name, true); link.href = value;
    link.target = "_blank"; link.rel = "noopener noreferrer"; return link;
  } catch { return null; }
}

function mountExternalConversations(api: Api, client: string) {
  let page = 1, hasMore = false, total = 0, listRevision = 0, detailRevision = 0;
  let active: Session | null = null, before: number | null = null, older = false;
  let loading = false, detailLoading = false;
  let applications: Application[] = [];
  const missingDevice = (client: string) => applications.find(app => app.client_id === client)?.require_device_id ? "设备关联待补全" : "未要求设备";
  const app = el<HTMLInputElement>("external-conversation-app");
  app.value = client;
  const dialog = el<HTMLDialogElement>("external-conversation-dialog");
  function pagination() {
    el<HTMLButtonElement>("external-conversation-prev").disabled = loading || page <= 1;
    el<HTMLButtonElement>("external-conversation-next").disabled = loading || !hasMore;
    el("external-conversation-total").textContent = String(total);
    el("external-conversation-page").textContent = String(page);
    el<HTMLButtonElement>("external-conversation-search").disabled = loading;
  }
  function detailButtons() {
    el<HTMLButtonElement>("external-conversation-older").disabled = detailLoading || !older;
    el<HTMLButtonElement>("external-conversation-latest").disabled = detailLoading;
  }
  async function load(next: number) {
    const revision = ++listRevision; loading = true; pagination();
    el("external-conversation-rows").replaceChildren();
    el("external-conversation-note").textContent = "正在读取外部对话";
    const params = new URLSearchParams({ page: String(next), limit: "10" });
    for (const [key, id] of [["q", "external-conversation-query"], ["user", "external-conversation-user"], ["device", "external-conversation-device"]]) {
      const value = el<HTMLInputElement>(id).value.trim(); if (value) params.set(key, value);
    }
    params.set("client_id", client);
    try {
      const result = await api<Page>(`/v1/management/external-conversations?${params}`);
      if (revision !== listRevision) return;
      page = result.page; total = result.total; hasMore = result.has_more;
      const body = el<HTMLTableSectionElement>("external-conversation-rows"); body.replaceChildren();
      for (const session of result.sessions.filter(session => session.client_id === client)) {
        const row = document.createElement("tr");
        const title = field(session.title, session.id);
        const open = text("button", "查看对话"); open.className = "button secondary";
        open.type = "button"; open.onclick = () => { active = session; before = null; void show(); };
        title.append(open);
        const device = session.device_id ? field(session.device_id) : text("td", missingDevice(session.client_id));
        row.append(title, field(session.client_id, session.assistant_id), field(session.user_label || session.user_id, session.external_user_id || session.user_id), device,
          field(String(session.turn_count)), field(new Date(session.updated_at).toLocaleString()));
        body.append(row);
      }
      el("external-conversation-note").textContent = result.sessions.length ? "每页最多显示 10 条对话" : "没有符合条件的外部对话";
    } catch (error) {
      if (revision === listRevision) el("external-conversation-note").textContent = error instanceof Error ? error.message : "读取外部对话失败";
    } finally { if (revision === listRevision) { loading = false; pagination(); } }
  }
  async function show() {
    if (!active) return;
    const session = active, revision = ++detailRevision;
    detailLoading = true; older = false; detailButtons();
    el("external-conversation-content").replaceChildren();
    el("external-conversation-detail-info").replaceChildren();
    el("external-conversation-detail-note").textContent = "正在读取对话内容";
    el("external-conversation-detail-title").textContent = session.title;
    if (!dialog.open) dialog.showModal();
    const params = new URLSearchParams({ turn_limit: "10" });
    if (before !== null) params.set("before_ordinal", String(before));
    try {
      const detail = await api<Detail>(`/v1/management/external-applications/${encodeURIComponent(session.client_id)}/sessions/${encodeURIComponent(session.id)}?${params}`);
      if (revision !== detailRevision || !dialog.open) return;
      const info = el("external-conversation-detail-info"); info.replaceChildren();
      const fields: [string, string | null][] = [["应用 ID", detail.external.client_id], ["用户", detail.external.user_label], ["用户 ID", detail.external.user_id], ["外部用户 ID", detail.external.external_user_id || null], ["设备 ID", detail.external.device_id], ["会话 ID", detail.id], ["对话轮数", String(detail.external.turn_count)]];
      for (const [label, value] of fields) {
        const item = text("div", ""); item.append(text("strong", label));
        item.append(value ? text("span", value, true) : text("span", label === "设备 ID" ? missingDevice(session.client_id) : "未提供")); info.append(item);
      }
      const content = el("external-conversation-content"); content.replaceChildren();
      for (const turn of detail.turns) {
        const article = text("article", ""); article.className = "external-conversation-turn";
        const heading = text("header", ""); heading.append(text("strong", "轮次"), text("span", String(turn.ordinal), true));
        if (turn.run_status) heading.append(text("span", turn.run_status, true));
        if (turn.run_id) heading.append(text("small", turn.run_id, true)); article.append(heading);
        article.append(text("h3", "用户")); const prompt = text("div", turn.prompt, true); prompt.className = "external-conversation-prose"; article.append(prompt);
        for (const attachment of turn.attachments || []) {
          const link = fileLink(attachment.name || "attachment", attachment.download_url || attachment.url); if (link) article.append(link);
        }
        article.append(text("h3", "助理"));
        const value = turn.answer || turn.response?.answer || "";
        const answer = text("div", value, true); answer.className = "external-conversation-prose";
        const direct = /^https?:\/\/\S+$/.test(value.trim()) ? fileLink(value.trim(), value.trim()) : null;
        if (direct) answer.replaceChildren(direct);
        article.append(answer);
        const notice = turn.response?.public_notice?.message;
        if (notice) article.append(text("p", notice));
        const files = text("div", ""); files.className = "external-conversation-files";
        for (const artifact of turn.response?.artifacts || []) {
          const link = fileLink(artifact.filename || "file", artifact.download_url || artifact.url); if (link) files.append(link);
        }
        article.append(files); content.append(article);
      }
      older = detail.history_page.has_more; before = detail.history_page.before_ordinal;
      el("external-conversation-detail-note").textContent = detail.turns.length ? "每页最多显示 10 轮消息" : "暂无对话内容";
    } catch (error) {
      if (revision === detailRevision) el("external-conversation-detail-note").textContent = error instanceof Error ? error.message : "读取对话内容失败";
    } finally { if (revision === detailRevision) { detailLoading = false; detailButtons(); } }
  }
  el<HTMLFormElement>("external-conversation-filter").onsubmit = event => { event.preventDefault(); void load(1); };
  el("external-conversation-prev").onclick = () => { void load(Math.max(1, page - 1)); };
  el("external-conversation-next").onclick = () => { void load(page + 1); };
  el("external-conversation-older").onclick = () => { void show(); };
  el("external-conversation-latest").onclick = () => { before = null; void show(); };
  el("external-conversation-close").onclick = () => dialog.close();
  dialog.addEventListener("close", () => { ++detailRevision; active = null; detailLoading = false; el("external-conversation-content").replaceChildren(); });
  pagination(); detailButtons();
  return {
    applications(values: Application[]) {
      applications = values.filter(item => item.client_id === client);
      if (!applications.length) { el("external-conversation-note").textContent = "找不到此应用"; return; }
      el("external-conversation-application-name").textContent = `${applications[0].name} · ${client}`;
      void load(1);
    },
  };
}

async function api<T>(path: string): Promise<T> {
  const auth = (window as unknown as {ExoAuth:{fetch:typeof fetch}}).ExoAuth;
  const response = await auth.fetch(path);
  const value = await response.json();
  if (!response.ok) throw Error(value?.error?.message || value?.message || String(response.status));
  return value as T;
}
const client = new URLSearchParams(location.search).get("client_id");
if (!client || !/^[A-Za-z0-9_.-]{1,128}$/.test(client)) {
  el("external-conversation-note").textContent = "请从外部应用列表选择应用";
} else {
  const conversations = mountExternalConversations(api, client);
  void api<{applications:Application[]}>("/v1/management/external-applications")
    .then(result => conversations.applications(result.applications))
    .catch(() => {el("external-conversation-note").textContent="读取外部对话失败";});
}

export {};
