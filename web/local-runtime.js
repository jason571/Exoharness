"use strict";
let token = "", identity = null, sessions = [], selected = null, lastRequest = null;
const node = (id) => document.getElementById(id);
const message = (value) => { node("message").textContent = value; };
async function api(path, options = {}) {
  const response = await fetch(path, { ...options, headers: { "Authorization": `Bearer ${token}`, ...(options.body ? { "Content-Type": "application/json" } : {}), ...options.headers } });
  const body = await response.json();
  if (!response.ok) throw new Error(body.error?.message || `HTTP ${response.status}`);
  return body;
}
function scopePath(path) {
  const query = new URLSearchParams({ owner: selected.owner });
  return `${path}?${query}`;
}
function safeButton(text, action, className = "secondary") {
  const button = document.createElement("button"); button.type = "button"; button.className = className; button.textContent = text;
  button.onclick = () => action().catch((error) => message(error.message)); return button;
}
async function refresh() {
  sessions = (await api("/v1/local/sessions")).sessions;
  node("sessions").replaceChildren();
  for (const session of sessions) {
    const button = safeButton(`${session.title}\n${session.owner} · ${session.id.slice(-12)}`, async () => { selected = session; lastRequest = null; await showRuns(); await refresh(); }, "session");
    button.classList.toggle("selected", session.id === selected?.id); node("sessions").append(button);
  }
  if (!sessions.length) node("sessions").textContent = "还没有验证会话";
  if (selected) { selected = sessions.find((session) => session.id === selected.id) || null; if (selected) await showRuns(); }
}
async function showRuns() {
  node("detail-title").textContent = `${selected.owner} · ${selected.id.slice(-12)}`;
  node("run-form").hidden = selected.owner !== identity.id;
  const runs = (await api(scopePath(`/v1/local/sessions/${selected.id}/runs`))).runs;
  node("runs").replaceChildren();
  for (const run of runs) {
    const card = document.createElement("div"); card.className = "run";
    const title = document.createElement("strong"); title.textContent = `${run.state} · 版本 ${run.version}`;
    const id = document.createElement("small"); id.textContent = run.id;
    const explanation = document.createElement("small"); explanation.textContent = "仅状态机检查记录，未提交模型或外部工具";
    const buttons = document.createElement("div"); buttons.className = "row";
    buttons.append(safeButton("查看事件", async () => { const result = await api(scopePath(`/v1/local/sessions/${selected.id}/runs/${run.id}/events`)); node("events").textContent = result.events.map((event) => JSON.stringify(event, null, 2)).join("\n\n"); }));
    const actions = run.state === "queued" ? [["模拟开始", "start"], ["取消记录", "cancel"]] : run.state === "running" ? [["暂停记录", "pause"], ["取消记录", "cancel"]] : run.state === "paused" ? [["继续原 Run", "resume"], ["取消记录", "cancel"]] : [];
    if (selected.owner === identity.id) for (const [label, action] of actions) buttons.append(safeButton(label, async () => {
      await api(`/v1/local/sessions/${selected.id}/runs/${run.id}`, { method: "PATCH", body: JSON.stringify({ expected_version: run.version, action }) }); await refresh(); message("状态与事件已提交 PostgreSQL；没有执行外部任务");
    }));
    card.append(title, id, explanation, buttons); node("runs").append(card);
  }
  if (!runs.length) node("runs").textContent = "尚无检查记录";
}
node("login-form").onsubmit = async (event) => {
  event.preventDefault(); token = node("token").value;
  try { identity = (await api("/v1/local/status")).principal; node("token").value = ""; node("login").hidden = true; node("workspace").hidden = false; node("identity").textContent = `${identity.id} · ${identity.role}`; await refresh(); message(""); } catch (error) { token = ""; message(error.message); }
};
node("new-session").onclick = () => (async () => { const result = await api("/v1/local/sessions", { method: "POST" }); await refresh(); selected = sessions.find((session) => session.id === result.session_id); await refresh(); message("验证会话已保存"); })().catch((error) => message(error.message));
node("refresh").onclick = () => refresh().catch((error) => message(error.message));
node("logout").onclick = () => { token = ""; identity = null; selected = null; lastRequest = null; node("workspace").hidden = true; node("login").hidden = false; node("sessions").replaceChildren(); node("runs").replaceChildren(); node("events").textContent = "尚未选择 Run"; message(""); };
node("run-form").onsubmit = async (event) => {
  event.preventDefault(); const input = node("input").value;
  if (!lastRequest || lastRequest.input !== input || lastRequest.session !== selected.id) lastRequest = { input, session: selected.id, request_id: crypto.randomUUID(), expected_session_version: selected.version };
  try { await api(`/v1/local/sessions/${selected.id}/runs`, { method: "POST", body: JSON.stringify({ input, request_id: lastRequest.request_id, expected_session_version: lastRequest.expected_session_version }) }); lastRequest = null; node("input").value = ""; await refresh(); message("检查记录已保存；未提交生成任务"); } catch (error) { message(error.message); }
};
fetch("/health").then((response) => { if (!response.ok) throw new Error(); node("health").textContent = "数据库与 HTTP 就绪"; }).catch(() => { node("health").textContent = "服务暂不可用"; });
