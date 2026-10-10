const $ = (selector) => document.querySelector(selector);
const state = { sessions: [], runs: [], runPageOffset: 0, trace: null, selectedSessionId: null, selectedRunId: null, selectedInvocationId: null, selectedModelCallId: null, selectedRunDetail: false, selectedFocus: null, returnFocus: null, search: "", loading: false, sessionLoadVersion: 0, traceLoadVersion: 0 };
const TRACE_EVENT_PAGE_SIZE = 256;
const timelineState = { mode: "readable", runId: null, renderKey: null, expanded: new Set(), rawOpen: new Set(), pageAfter: 0, previousPages: [], pending: null };

async function api(path) {
  const response = await fetch(path, { headers: { accept: "application/json" } });
  if (response.status === 401 || response.status === 403) {
    location.assign(`/login?next=${encodeURIComponent(location.pathname)}`);
    throw new Error("需要管理员登录");
  }
  if (!response.ok) {
    const payload = await response.json().catch(() => ({}));
    const error = new Error(payload.message || `请求失败（${response.status}）`);
    error.status = response.status;
    throw error;
  }
  return response.json();
}

function setService(ok, text) {
  $("#service-dot").classList.toggle("online", ok);
  $("#service-dot").classList.toggle("offline", !ok);
  $("#service-label").textContent = text;
}

function formatTime(value, compact = false) {
  if (!value) return "—";
  const options = compact
    ? { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false }
    : { year: "numeric", month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false };
  return new Date(value).toLocaleString("zh-CN", options);
}

function duration(run) {
  const start = run.started_at || run.created_at;
  const end = run.finished_at || Date.now();
  if (!start) return "—";
  const milliseconds = Math.max(0, end - start);
  if (milliseconds < 1000) return `${milliseconds} ms`;
  const seconds = Math.round(milliseconds / 1000);
  if (seconds < 60) return `${seconds} 秒`;
  return `${Math.floor(seconds / 60)} 分 ${seconds % 60} 秒`;
}

function statusLabel(status) {
  return ({ pending: "等待", queued: "排队中", running: "运行中", succeeded: "成功", failed: "失败", cancelled: "已取消", recoverable: "可恢复", waiting_authorization: "等待授权", submitted: "已提交" })[status] || status || "未知";
}

function formatTokens(value) {
  const number = Number(value || 0);
  if (!Number.isFinite(number)) return "—";
  return new Intl.NumberFormat("zh-CN").format(number);
}

function formatCost(value) {
  const number = Number(value || 0);
  if (!Number.isFinite(number) || number <= 0) return "未提供";
  return `$${number.toFixed(number < 0.01 ? 5 : 3)}`;
}

function modelKindLabel(kind) {
  return ({ agent: "Agent 推理", capability_routing: "能力匹配", context_compaction: "上下文压缩", loop_review: "循环检查" })[kind] || kind || "模型调用";
}

function revealMobileDetail(selector) {
  if (window.matchMedia("(max-width: 720px)").matches) {
    $(selector)?.scrollIntoView({ block: "start", behavior: "instant" });
  }
}

function renderRuns() {
  const needle = state.search.trim().toLowerCase();
  const sessions = state.sessions.filter((session) => !needle || [session.id, session.title, session.status, session.latest_prompt].join(" ").toLowerCase().includes(needle));
  $("#trace-run-count").textContent = sessions.length;
  const list = $("#trace-run-list");
  list.replaceChildren();
  if (!sessions.length) {
    const empty = document.createElement("p");
    empty.className = "list-message";
    empty.textContent = "没有符合条件的会话记录";
    list.append(empty);
    return;
  }

  sessions.forEach((session) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "trace-run-item";
    button.classList.toggle("active", session.id === state.selectedSessionId);
    button.dataset.sessionId = session.id;
    button.setAttribute("aria-current", session.id === state.selectedSessionId ? "true" : "false");
    button.innerHTML = `
      <span class="trace-run-item-head"><strong></strong><i></i></span>
      <span class="trace-run-prompt"></span>
      <span class="trace-run-item-meta"><span></span><time></time></span>`;
    button.querySelector("strong").textContent = session.title || "未命名会话";
    button.querySelector("i").textContent = statusLabel(session.status);
    button.querySelector("i").dataset.status = session.status;
    button.querySelector(".trace-run-prompt").textContent = session.latest_prompt || "无输入文本";
    button.querySelector(".trace-run-item-meta span").textContent = `${session.turn_count} 轮消息 · ${formatTokens(session.total_tokens)} Token`;
    button.querySelector("time").textContent = formatTime(session.updated_at, true);
    button.addEventListener("click", () => selectSession(session.id, false, null, true));
    list.append(button);
  });
}

function renderConversation(revealSelected = false) {
  const session = state.sessions.find((item) => item.id === state.selectedSessionId);
  if (!session) return;
  $("#trace-conversation-title").textContent = session.title || "未命名会话";
  $("#trace-conversation-id").textContent = session.id;
  $("#trace-conversation-turns").textContent = `${session.turn_count} 轮消息 · ${session.run_count} 次执行`;
  $("#trace-conversation-tokens").textContent = formatTokens(session.total_tokens);
  $("#trace-conversation-calls").textContent = `${session.model_call_count} 次模型请求 · ${session.invocation_count} 次能力调用`;
  const list = $("#trace-turn-list");
  const scrollTop = list.scrollTop;
  const selectionChanged = list.dataset.selectedRunId !== (state.selectedRunId || "");
  list.dataset.selectedRunId = state.selectedRunId || "";
  list.replaceChildren();
  [...state.runs].reverse().forEach((run) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "trace-turn-item";
    button.classList.toggle("active", run.id === state.selectedRunId);
    button.dataset.runId = run.id;
    button.setAttribute("aria-pressed", String(run.id === state.selectedRunId));
    button.innerHTML = '<span class="trace-turn-heading"><strong></strong><small></small></span><span class="trace-turn-prompt"></span><span class="trace-turn-meta"></span>';
    button.querySelector("strong").textContent = `第 ${run.turn_ordinal} 轮${run.attempt > 1 ? ` · 重试 ${run.attempt - 1}` : ""}`;
    button.querySelector("small").textContent = statusLabel(run.status);
    button.querySelector("small").dataset.status = run.status;
    button.querySelector(".trace-turn-prompt").textContent = run.prompt_preview || "无输入文本";
    button.querySelector(".trace-turn-meta").textContent = `${formatTime(run.created_at, true)} · ${formatTokens(run.total_tokens)} Token`;
    button.addEventListener("click", () => selectRun(run.id, false, true));
    list.append(button);
  });
  list.scrollTop = scrollTop;
  const active = list.querySelector(".active");
  if ((selectionChanged || revealSelected) && active) {
    const itemBounds = active.getBoundingClientRect();
    const listBounds = list.getBoundingClientRect();
    if (itemBounds.bottom > listBounds.bottom) list.scrollTop += itemBounds.bottom - listBounds.bottom;
    else if (itemBounds.top < listBounds.top) list.scrollTop -= listBounds.top - itemBounds.top;
  }
  $("#trace-load-older").hidden = state.runPageOffset >= session.run_count;
}

async function selectSession(id, keepRun = false, requestedRun = null, revealOnMobile = false) {
  const version = ++state.sessionLoadVersion;
  const sameSession = state.selectedSessionId === id;
  state.selectedSessionId = id;
  if (!sameSession) {
    closeDetailDrawer(false);
    state.runs = [];
    state.runPageOffset = 0;
    state.selectedRunId = null;
    state.trace = null;
    $("#trace-content").hidden = true;
    $("#trace-empty").hidden = false;
  }
  renderRuns();
  try {
    const runs = await api(`/v1/trace/runs?session_id=${encodeURIComponent(id)}&limit=100`);
    if (version !== state.sessionLoadVersion) return;
    // Keep older pages already loaded for this session, without duplication.
    const older = sameSession ? state.runs.filter((run) => !runs.some((item) => item.id === run.id)) : [];
    state.runs = [...runs, ...older];
    state.runPageOffset = Math.max(state.runPageOffset, runs.length);
    const selected = requestedRun || (keepRun && state.selectedRunId
      ? state.selectedRunId : state.runs[0]?.id);
    renderConversation();
    if (selected) {
      await selectRun(selected, keepRun);
      if (revealOnMobile && state.selectedSessionId === id && state.trace?.run.id === selected) {
        revealMobileDetail(".trace-conversation-panel");
      }
    }
    else {
      $("#trace-content").hidden = true;
      $("#trace-empty").hidden = false;
    }
  } catch (error) { setService(false, error.message); }
}

async function loadOlderRuns() {
  const id = state.selectedSessionId;
  const button = $("#trace-load-older");
  button.disabled = true;
  try {
    const runs = await api(`/v1/trace/runs?session_id=${encodeURIComponent(id)}&limit=100&offset=${state.runPageOffset}`);
    if (id !== state.selectedSessionId) return;
    state.runs.push(...runs.filter((run) => !state.runs.some((item) => item.id === run.id)));
    state.runPageOffset += runs.length;
    state.runs.sort((a,b)=>b.created_at-a.created_at||b.attempt-a.attempt);
    renderConversation();
  } catch (error) { setService(false, error.message); }
  finally { button.disabled = false; }
}

function invocationKind(invocation) {
  if (invocation.capability_id.startsWith("skill:")) return "skill";
  if (invocation.capability_id.startsWith("mcp_server:")) return "mcp";
  return "tool";
}

function flowNode(kind, eyebrow, title, subtitle, invocation, focus = "call") {
  const node = document.createElement("button");
  node.type = "button";
  node.className = `trace-flow-node ${kind}`;
  node.innerHTML = `<span class="trace-flow-icon"></span><span class="trace-flow-copy"><small></small><strong></strong><em></em></span>`;
  node.querySelector(".trace-flow-icon").textContent = ({ user: "U", agent: "AI", model: "LLM", skill: "S", mcp: "M", tool: "T", result: "R", error: "!" })[kind] || "·";
  node.querySelector("small").textContent = eyebrow;
  node.querySelector("strong").textContent = title;
  node.querySelector("em").textContent = subtitle;
  if (invocation) {
    node.dataset.invocationId = invocation.id;
    node.dataset.focus = focus;
    node.setAttribute("aria-haspopup", "dialog");
    node.setAttribute("aria-expanded", "false");
    node.addEventListener("click", () => showInvocation(invocation, node, focus));
  }
  else node.disabled = true;
  return node;
}

function modelFlowNode(kind, eyebrow, title, subtitle, call, focus) {
  const node = flowNode(kind, eyebrow, title, subtitle);
  node.disabled = false;
  node.dataset.modelCallId = call.id;
  node.dataset.focus = focus;
  node.setAttribute("aria-haspopup", "dialog");
  node.setAttribute("aria-expanded", "false");
  node.addEventListener("click", () => showModelCall(call, node, focus));
  return node;
}

function flowEdge(label = "") {
  const edge = document.createElement("span");
  edge.className = "trace-flow-edge";
  edge.innerHTML = `<span></span><small></small>`;
  edge.querySelector("small").textContent = label;
  return edge;
}

function flowRow(index, nodes, edgeLabels = []) {
  const row = document.createElement("div");
  row.className = "trace-flow-row";
  const sequence = document.createElement("span");
  sequence.className = "trace-flow-sequence";
  sequence.textContent = index;
  const track = document.createElement("div");
  track.className = "trace-flow-track";
  nodes.forEach((node, nodeIndex) => {
    if (nodeIndex) track.append(flowEdge(edgeLabels[nodeIndex - 1] || ""));
    track.append(node);
  });
  row.append(sequence, track);
  return row;
}

function renderFlow(trace) {
  const flow = $("#trace-flow");
  flow.replaceChildren();
  flow.append(flowRow("START", [
    flowNode("user", "输入", "用户请求", `Turn ${trace.turn_ordinal}`),
    flowNode("agent", "运行时", "Agent", "解析与规划"),
  ], ["提交"]));

  let selectedNode = null;
  let selectedItem = null;
  let selectedType = null;
  const steps = [
    ...(trace.model_calls || []).map((call) => ({ type: "model", time: call.started_at, item: call })),
    ...trace.invocations.map((invocation) => ({ type: "invocation", time: invocation.created_at, item: invocation })),
  ].sort((left, right) => left.time - right.time || left.type.localeCompare(right.type));

  steps.forEach((step, index) => {
    if (step.type === "model") {
      const call = step.item;
      const resultKind = call.status === "failed" || call.status === "cancelled" ? "error" : "result";
      const inputNode = modelFlowNode(
        "model",
        "MODEL",
        modelKindLabel(call.call_kind),
        `${call.provider} · ${call.model}`,
        call,
        "call",
      );
      const resultNode = modelFlowNode(
        resultKind,
        "RESPONSE",
        call.status === "succeeded" ? call.stop_reason : statusLabel(call.status),
        call.usage_reported ? `${formatTokens(call.usage.total_tokens)} Token · ${duration({ created_at: call.started_at, started_at: call.started_at, finished_at: call.completed_at })}` : duration({ created_at: call.started_at, started_at: call.started_at, finished_at: call.completed_at }),
        call,
        "result",
      );
      if (state.selectedModelCallId === call.id) {
        selectedNode = state.selectedFocus === "result" ? resultNode : inputNode;
        selectedItem = call;
        selectedType = "model";
      }
      flow.append(flowRow(String(index + 1).padStart(2, "0"), [
        flowNode("agent", "运行时", modelKindLabel(call.call_kind), `Attempt ${call.attempt + 1}`),
        inputNode,
        resultNode,
      ], ["请求", "响应"]));
      return;
    }

    const invocation = step.item;
    const kind = invocationKind(invocation);
    const rejectedBeforeExecution = invocation.result?.details?._meta?.exoharness?.execution_error?.submitted === false;
    const outcomeKind = invocation.outcome_status === "failed" ? "error" : "result";
    const outcomeTitle = rejectedBeforeExecution ? "参数校验失败 · 未执行" : invocation.outcome_status === "failed" ? "调用失败" : invocation.completed_at ? "已返回" : "等待返回";
    const callNode = flowNode(kind, kind.toUpperCase(), invocation.tool_name, rejectedBeforeExecution ? "调用前校验" : statusLabel(invocation.status), invocation, "call");
    const resultNode = flowNode(outcomeKind, "RESULT", outcomeTitle, invocation.completed_at ? duration({ created_at: invocation.created_at, started_at: invocation.submitted_at, finished_at: invocation.completed_at }) : statusLabel(invocation.status), invocation, "result");
    if (state.selectedInvocationId === invocation.id) {
      selectedNode = state.selectedFocus === "result" ? resultNode : callNode;
      selectedItem = invocation;
      selectedType = "invocation";
    }
    flow.append(flowRow(String(index + 1).padStart(2, "0"), [
      flowNode("agent", "Agent", "发起能力调用", `Occurrence ${invocation.occurrence}`),
      callNode,
      resultNode,
    ], ["调用", "返回"]));
  });

  const finalKind = trace.run.status === "succeeded" ? "result" : trace.run.status === "running" ? "agent" : "error";
  const finalNode = flowNode(finalKind, "RUN", statusLabel(trace.run.status), trace.run.error || duration(trace.run));
  finalNode.disabled = false;
  finalNode.dataset.runOutcome = trace.run.id;
  finalNode.setAttribute("aria-haspopup", "dialog");
  finalNode.setAttribute("aria-expanded", String(state.selectedRunDetail));
  finalNode.addEventListener("click", () => showRunOutcome(trace, finalNode));
  flow.append(flowRow("END", [
    flowNode("agent", "Agent", "处理调用结果", trace.invocations.length ? "生成最终响应" : "直接响应"),
    finalNode,
  ], ["完成"]));
  if (state.selectedRunDetail) showRunOutcome(trace, finalNode, state.selectedFocus);
  else if (selectedNode && selectedType === "model") showModelCall(selectedItem, selectedNode, state.selectedFocus);
  else if (selectedNode && selectedType === "invocation") showInvocation(selectedItem, selectedNode, state.selectedFocus);
  else if (state.selectedInvocationId || state.selectedModelCallId) closeDetailDrawer(false);
}

function renderModelUsage(trace) {
  const calls = trace.model_calls || [];
  const container = $("#trace-model-usage");
  container.replaceChildren();
  if (!calls.length) {
    container.innerHTML = '<p class="trace-usage-empty">旧运行或尚未发起模型请求，暂无逐次 Token 数据</p>';
    $("#trace-usage-meta").textContent = "0 次模型请求";
    return;
  }
  const total = calls.reduce((sum, call) => sum + Number(call.usage?.total_tokens || 0), 0);
  const largest = Math.max(...calls.map((call) => Number(call.usage?.total_tokens || 0)), 1);
  const retries = calls.filter((call) => Number(call.attempt || 0) > 0).length;
  $("#trace-usage-meta").textContent = `${calls.length} 次请求 · ${retries} 次重试 · ${formatTokens(total)} Token`;
  calls.forEach((call) => {
    const usage = call.usage || {};
    const tokens = Number(usage.total_tokens || 0);
    const contextEstimate = Number(call.context?.request_estimated_tokens || 0);
    const button = document.createElement("button");
    button.type = "button";
    button.className = "trace-usage-row";
    button.classList.toggle("selected", state.selectedModelCallId === call.id);
    button.innerHTML = `
      <span class="trace-usage-copy"><strong></strong><small></small></span>
      <span class="trace-usage-bar"><i></i></span>
      <span class="trace-usage-value"><span>实际 Token</span><strong>${call.usage_reported ? formatTokens(tokens) : "未提供"}</strong></span>
      <span class="trace-usage-value"><span>上下文估算</span><strong>${formatTokens(contextEstimate)}</strong></span>
      <span class="trace-usage-value"><span>耗时</span><strong>${duration({ created_at: call.started_at, started_at: call.started_at, finished_at: call.completed_at })}</strong></span>`;
    button.querySelector(".trace-usage-copy strong").textContent = `${modelKindLabel(call.call_kind)} · #${call.sequence}`;
    button.querySelector(".trace-usage-copy small").textContent = `${call.model} · Attempt ${Number(call.attempt || 0) + 1}`;
    button.querySelector(".trace-usage-bar i").style.setProperty("--usage-width", `${Math.max(tokens > 0 ? 3 : 0, Math.round(tokens / largest * 100))}%`);
    button.addEventListener("click", () => {
      const node = [...document.querySelectorAll(".trace-flow-node[data-model-call-id]")]
        .find((item) => item.dataset.modelCallId === call.id && item.dataset.focus === "call");
      showModelCall(call, node || button, "call");
    });
    container.append(button);
  });
}

function timelinePayload(event) {
  const payload = event.payload || {};
  // Native lifecycle envelopes and legacy chat payloads are different shapes.
  // Keep the original event untouched for the raw-event view.
  return payload.type && payload.data && typeof payload.data === "object" && !Array.isArray(payload.data)
    ? payload.data : payload;
}

function timelineType(event) {
  return event.event_type?.replace(/^pro_/, "") || event.payload?.type || "unknown";
}

function timelineInvocation(event, invocations = []) {
  const payload = timelinePayload(event);
  const id = payload.invocation_id || payload.tool_call_id;
  return id ? invocations.find((invocation) => invocation.id === id) : null;
}

function eventSummary(event, invocations = []) {
  const payload = timelinePayload(event);
  const type = timelineType(event);
  const invocation = timelineInvocation(event, invocations);
  const tool = invocation?.tool_name || payload.tool || payload.name;
  if (["invocation_prepared", "invocation_submitted", "invocation_resolved", "tool_call", "tool_result"].includes(type)) {
    const phase = ({ invocation_prepared: "准备调用（尚未提交）", invocation_submitted: "已提交调用", invocation_resolved: "调用已结算", tool_call: "调用能力", tool_result: payload.is_error ? "能力返回错误" : "能力返回" })[type];
    return `${phase} · ${tool || payload.invocation_id || payload.tool_call_id || "未知能力"}`;
  }
  if (type === "run_transition") return `${statusLabel(payload.from)} → ${statusLabel(payload.to)}`;
  if (event.event_type === "connection_resolution") {
    const bindings = Array.isArray(payload.bindings) ? payload.bindings : [];
    const targets = new Set(bindings.flatMap((binding) => Array.isArray(binding.targets) ? binding.targets.map((target) => target.capability_id).filter(Boolean) : []));
    return targets.size ? `${bindings.length} 个依赖连接 · ${targets.size} 个已授权服务` : "未匹配依赖地址，保留服务自身配置";
  }
  if (event.event_type === "text_delta" && typeof payload.delta === "string") return payload.delta;
  const summary = payload.message || payload.stage || payload.tool || payload.name || payload.status || "事件已记录";
  return typeof summary === "string" ? summary : JSON.stringify(summary);
}

// Display projection only: never rewrite the journal or join across event boundaries.
function timelineEntries(events, mode = "readable", invocations = []) {
  const entries = [];
  for (const event of events) {
    const isText = event.event_type === "text_delta" && typeof event.payload?.delta === "string";
    // Include all stream metadata so different messages/requests cannot be joined.
    const identity = isText ? JSON.stringify(Object.keys(event.payload).filter((key) => key !== "delta").sort().map((key) => [key, event.payload[key]])) : null;
    const previous = entries.at(-1);
    if (mode === "readable" && isText && previous?.identity === identity
        && Number.isSafeInteger(event.sequence) && Number.isSafeInteger(previous.last.sequence)
        && previous.last.sequence + 1 === event.sequence) {
      previous.events.push(event);
      previous.parts.push(event.payload.delta);
      previous.last = event;
    } else {
      entries.push({ first: event, last: event, events: [event], identity, parts: [eventSummary(event, invocations)] });
    }
  }
  return entries.map((entry) => ({ ...entry, text: entry.parts.join("") }));
}

function timelineTime(value) {
  if (!value) return "—";
  return `${formatTime(value, true)}.${String(new Date(value).getMilliseconds()).padStart(3, "0")}`;
}

function renderTimelinePaging(trace) {
  const last = trace.events?.at(-1)?.sequence || 0;
  const more = last > 0 && Number(trace.run.last_sequence || 0) > last;
  for (const [id, visible] of [["prev", timelineState.previousPages.length > 0], ["next", more], ["latest", more]]) {
    const button = $(`#trace-event-${id}`);
    button.hidden = !visible;
    button.disabled = Boolean(timelineState.pending);
  }
}

async function loadTimelinePage(direction) {
  const trace = state.trace;
  if (!trace || trace.run.id !== state.selectedRunId || timelineState.pending) return;
  const previousAfter = timelineState.pageAfter;
  const previousPages = [...timelineState.previousPages];
  const after = direction === "prev" ? timelineState.previousPages.pop()
    : direction === "latest" ? Math.max(0, Number(trace.run.last_sequence || 0) - TRACE_EVENT_PAGE_SIZE)
    : trace.events.at(-1)?.sequence;
  if (after === undefined || after === previousAfter) return;
  if (direction !== "prev") timelineState.previousPages.push(previousAfter);
  timelineState.pageAfter = after;
  const pending = { runId: trace.run.id, after };
  timelineState.pending = pending;
  renderTimelinePaging(trace);
  const result = await selectRun(trace.run.id, true);
  if (timelineState.pending !== pending) return;
  if (result === false) {
    timelineState.pageAfter = previousAfter;
    timelineState.previousPages = previousPages;
  }
  timelineState.pending = null;
  if (state.trace) renderTimelinePaging(state.trace);
}

function renderTimeline(trace) {
  const timeline = $("#trace-timeline");
  const events = trace.events || [];
  const changedRun = timelineState.runId !== trace.run.id;
  if (changedRun) {
    timelineState.runId = trace.run.id;
    timelineState.expanded.clear();
    timelineState.rawOpen.clear();
  }
  renderTimelinePaging(trace);
  const renderKey = JSON.stringify([trace.run.id, timelineState.mode, events[0]?.sequence, events.length, events.at(-1)?.sequence, trace.run.last_sequence]);
  // Persisted events are immutable. Do not disturb selection/focus every refresh.
  if (timelineState.renderKey === renderKey) return;
  timelineState.renderKey = renderKey;
  const changedWindow = timeline.dataset.eventAfter !== String(timelineState.pageAfter);
  timeline.dataset.eventAfter = String(timelineState.pageAfter);
  const scrollTop = changedRun || changedWindow ? 0 : timeline.scrollTop;
  const focusedKey = timeline.contains(document.activeElement) ? document.activeElement.dataset.timelineControl : null;
  const entries = timelineEntries(events, timelineState.mode, trace.invocations);
  const incomplete = timelineState.pageAfter > 0 || Number(trace.run.last_sequence || 0) > Number(events.at(-1)?.sequence || 0);
  $("#trace-timeline-meta").textContent = `${entries.length} 条展示 · ${events.length} 条原始事件${incomplete ? ` · 当前事件 #${events[0]?.sequence || "—"}–#${events.at(-1)?.sequence || "—"} / 最新 #${trace.run.last_sequence}` : ""}`;
  $("#trace-timeline-warning").hidden = !incomplete;
  document.querySelectorAll("[data-timeline-mode]").forEach((button) => {
    button.setAttribute("aria-pressed", String(button.dataset.timelineMode === timelineState.mode));
  });
  timeline.replaceChildren();
  if (!events.length) {
    timeline.innerHTML = '<p class="list-message">该 Run 没有持久化事件</p>';
    return;
  }
  const labels = { text_delta: "助手输出", assistant_start: "助手开始响应", turn_end: "本轮响应结束", tool_call: "调用能力", tool_result: "能力返回", tool_progress: "执行进度", turn_created: "接收请求", run_created: "创建运行", run_transition: "运行状态", invocation_prepared: "准备调用", invocation_submitted: "提交调用", invocation_resolved: "调用结算", recovery_updated: "恢复状态", run_status: "运行状态", status: "状态更新", phase: "执行阶段", resource_queue: "执行资源", capability_routing: "能力选择", error: "执行错误", connection_resolution: "依赖连接" };
  const fragment = document.createDocumentFragment();
  entries.forEach((entry) => {
    const event = entry.first;
    const key = `${timelineState.mode}:${event.sequence}`;
    const item = document.createElement("article");
    item.className = "trace-step";
    item.classList.toggle("trace-step-error", /error|fail/.test(event.event_type) || event.payload?.status === "failed" || event.payload?.is_error === true);
    item.dataset.eventSequence = event.sequence;
    item.innerHTML = `<span class="trace-step-marker"></span><div class="trace-step-copy"><span class="trace-step-heading"><strong></strong><code></code></span><div class="trace-step-meta"><time></time><small></small></div><p class="trace-step-text"></p></div>`;
    const copy = item.querySelector(".trace-step-copy");
    item.querySelector("strong").textContent = timelineState.mode === "raw" ? event.event_type : (labels[timelineType(event)] || event.event_type);
    const paragraph = item.querySelector("p");
    // Raw model output and service messages are evidence, not interface labels.
    const payload = timelinePayload(event);
    if (timelineType(event) === "text_delta" || timelineType(event) === "tool_progress" || payload.message) paragraph.dataset.i18n = "off";
    paragraph.textContent = entry.text.trim() ? entry.text : "（空白片段）";
    const timestamp = item.querySelector("time");
    timestamp.textContent = timelineTime(event.created_at) + (event.created_at !== entry.last.created_at ? ` – ${timelineTime(entry.last.created_at)}` : "");
    if (event.created_at) timestamp.dateTime = new Date(event.created_at).toISOString();
    timestamp.title = `${formatTime(event.created_at)} → ${formatTime(entry.last.created_at)}`;
    item.querySelector("code").textContent = entry.events.length > 1 ? `#${event.sequence}–${entry.last.sequence}` : `#${event.sequence}`;
    item.querySelector("small").textContent = entry.events.length > 1 ? `${entry.events.length} 个文字片段已合并` : "";
    if (entry.text.length > 240 || entry.text.split("\n").length > 4) {
      const expand = document.createElement("button");
      expand.type = "button";
      expand.className = "trace-text-expand";
      expand.dataset.timelineControl = `${key}:text`;
      paragraph.id = `trace-event-text-${event.sequence}`;
      expand.setAttribute("aria-controls", paragraph.id);
      const update = () => {
        const open = timelineState.expanded.has(key);
        paragraph.classList.toggle("collapsed", !open);
        expand.setAttribute("aria-expanded", String(open));
        expand.textContent = open ? "收起全文" : "展开全文";
      };
      expand.addEventListener("click", () => {
        if (timelineState.expanded.has(key)) timelineState.expanded.delete(key);
        else timelineState.expanded.add(key);
        update();
      });
      update();
      copy.append(expand);
    }
    const invocation = timelineInvocation(event, trace.invocations);
    if (invocation) {
      const link = document.createElement("button");
      const focus = ["invocation_resolved", "tool_result"].includes(timelineType(event)) ? "result" : "call";
      link.type = "button";
      link.className = "trace-text-expand";
      link.textContent = focus === "result" ? "查看返回结果" : "查看调用参数";
      link.dataset.invocationId = invocation.id;
      link.dataset.focus = focus;
      link.dataset.timelineControl = `${key}:invocation`;
      link.setAttribute("aria-haspopup", "dialog");
      link.setAttribute("aria-expanded", "false");
      link.addEventListener("click", () => showInvocation(invocation, link, focus));
      copy.append(link);
    }
    const details = document.createElement("details");
    details.className = "trace-event-data";
    const summary = document.createElement("summary");
    summary.textContent = entry.events.length > 1 ? `查看 ${entry.events.length} 条原始事件` : "查看原始事件";
    summary.dataset.timelineControl = `${key}:raw`;
    details.append(summary);
    const populate = () => {
      if (details.querySelector("pre")) return;
      const pre = document.createElement("pre");
      pre.textContent = JSON.stringify(entry.events.length === 1 ? event : entry.events, null, 2);
      details.append(pre);
    };
    if (timelineState.rawOpen.has(key)) { populate(); details.open = true; }
    details.addEventListener("toggle", () => {
      if (!details.isConnected) return;
      if (details.open) { timelineState.rawOpen.add(key); populate(); }
      else timelineState.rawOpen.delete(key);
    });
    copy.append(details);
    fragment.append(item);
  });
  timeline.append(fragment);
  if (focusedKey && !changedRun) {
    [...timeline.querySelectorAll("[data-timeline-control]")].find((control) => control.dataset.timelineControl === focusedKey)?.focus({ preventScroll: true });
  }
  timeline.scrollTop = scrollTop;
}

function setDetailTab(focus) {
  const selected = focus === "result" ? "result" : "arguments";
  const nodeFocus = selected === "result" ? "result" : "call";
  state.selectedFocus = nodeFocus;
  document.querySelectorAll(".trace-flow-node[data-invocation-id]").forEach((node) => {
    const active = node.dataset.invocationId === state.selectedInvocationId && node.dataset.focus === nodeFocus;
    node.classList.toggle("selected", active);
    node.setAttribute("aria-expanded", String(active));
  });
  document.querySelectorAll(".trace-flow-node[data-model-call-id]").forEach((node) => {
    const active = node.dataset.modelCallId === state.selectedModelCallId && node.dataset.focus === nodeFocus;
    node.classList.toggle("selected", active);
    node.setAttribute("aria-expanded", String(active));
  });
  document.querySelectorAll("[data-trace-tab]").forEach((button) => {
    const active = button.dataset.traceTab === selected;
    button.classList.toggle("active", active);
    button.setAttribute("aria-selected", String(active));
    button.tabIndex = active ? 0 : -1;
  });
  $("#trace-arguments-panel").hidden = selected !== "arguments";
  $("#trace-result-panel").hidden = selected !== "result";
}

function openDetailDrawer(trigger) {
  const drawer = $("#trace-detail-drawer");
  const alreadyOpen = drawer.classList.contains("open");
  state.returnFocus = trigger;
  $("#trace-detail-backdrop").hidden = false;
  drawer.removeAttribute("inert");
  drawer.setAttribute("aria-hidden", "false");
  drawer.classList.add("open");
  document.body.classList.add("trace-drawer-open");
  if (!alreadyOpen) setTimeout(() => $("#trace-detail-close").focus({ preventScroll: true }), 0);
}

function closeDetailDrawer(restoreFocus = true) {
  const drawer = $("#trace-detail-drawer");
  if (!drawer.classList.contains("open")) return;
  drawer.classList.remove("open");
  drawer.setAttribute("aria-hidden", "true");
  drawer.setAttribute("inert", "");
  $("#trace-detail-backdrop").hidden = true;
  document.body.classList.remove("trace-drawer-open");
  document.querySelectorAll(".trace-flow-node.selected, .trace-step [data-invocation-id].selected").forEach((item) => {
    item.classList.remove("selected");
    item.setAttribute("aria-expanded", "false");
  });
  state.selectedInvocationId = null;
  state.selectedModelCallId = null;
  state.selectedRunDetail = false;
  state.selectedFocus = null;
  if (restoreFocus && state.returnFocus?.isConnected) state.returnFocus.focus({ preventScroll: true });
  state.returnFocus = null;
}

function showInvocation(invocation, node, focus) {
  $("#trace-request-evidence").hidden = true;
  state.selectedRunDetail = false;
  state.selectedInvocationId = invocation.id;
  state.selectedModelCallId = null;
  document.querySelectorAll(".trace-flow-node.selected, .trace-step [data-invocation-id].selected").forEach((item) => {
    item.classList.remove("selected");
    item.setAttribute("aria-expanded", "false");
  });
  node.classList.add("selected");
  node.setAttribute("aria-expanded", "true");
  const kind = invocationKind(invocation).toUpperCase();
  $("#trace-detail-kind").textContent = kind;
  $("#trace-detail-kind").dataset.kind = kind.toLowerCase();
  $("#trace-detail-title").textContent = invocation.tool_name;
  $("#trace-detail-capability").textContent = invocation.capability_id;
  $("#trace-arguments-tab").textContent = "调用参数";
  $("#trace-result-tab").textContent = "返回结果";
  $("#trace-arguments-heading").textContent = "调用参数";
  $("#trace-result-heading").textContent = "返回结果";
  $("#trace-facts").innerHTML = `
    <div><dt>类型</dt><dd>${kind}</dd></div>
    <div><dt>能力</dt><dd></dd></div>
    <div><dt>Revision</dt><dd>${invocation.capability_revision}</dd></div>
    <div><dt>Invocation ID</dt><dd></dd></div>
    <div><dt>状态</dt><dd>${statusLabel(invocation.status)}</dd></div>
    <div><dt>参数摘要</dt><dd></dd></div>`;
  const values = $("#trace-facts").querySelectorAll("dd");
  values[1].textContent = invocation.capability_id;
  values[3].textContent = invocation.id;
  values[5].textContent = invocation.arguments_digest;
  $("#trace-arguments").textContent = JSON.stringify(invocation.arguments, null, 2);
  $("#trace-result").textContent = invocation.result == null ? "尚未返回" : JSON.stringify(invocation.result, null, 2);
  const index = state.trace?.invocations.findIndex((item) => item.id === invocation.id) ?? -1;
  const total = state.trace?.invocations.length || 0;
  $("#trace-detail-position").textContent = index >= 0 ? `调用 ${index + 1} / ${total}` : "调用详情";
  $("#trace-detail-prev").disabled = index <= 0;
  $("#trace-detail-next").disabled = index < 0 || index >= total - 1;
  setDetailTab(focus);
  openDetailDrawer(node);
}

const requestEvidenceCache = new Map();
let requestEvidenceRun = null;
let requestEvidenceSelection = null;

async function loadRequestEvidence(call) {
  const runId = state.selectedRunId;
  const key = `${runId}/${call.logical_call_id}/${call.attempt || 0}`;
  const current = () => state.selectedRunId === runId && state.selectedModelCallId === call.id;
  const panel = $("#trace-request-evidence");
  const select = $("#trace-request-part");
  if (requestEvidenceRun !== runId) {
    requestEvidenceRun = runId;
    requestEvidenceCache.clear();
  }
  if (requestEvidenceSelection?.key !== key) requestEvidenceSelection = { key, part: "" };
  panel.hidden = false;
  select.hidden = true;
  $("#trace-request-summary").textContent = "正在读取请求清单…";
  const path = `/v1/trace/runs/${encodeURIComponent(runId)}/requests/${encodeURIComponent(call.logical_call_id)}/${call.attempt || 0}`;
  try {
    if (!requestEvidenceCache.has(key)) requestEvidenceCache.set(key, api(path));
    const evidence = await requestEvidenceCache.get(key);
    if (!current()) return;
    const delta = evidence.changes;
    $("#trace-arguments-heading").textContent = "模型输入证据";
    $("#trace-request-summary").textContent = `相比上次请求：新增 ${delta.added.length} 项，移除 ${delta.removed.length} 项，复用 ${delta.retained.length} 项。${evidence.exact_projection ? "逻辑输入可核验。" : "含脱敏或省略内容。"} 复用指日志存储去重，不代表模型没有接收或消耗 Token。`;
    select.replaceChildren();
    select.add(new Option("请求清单与变化", ""));
    const transformations = evidence.transformations || [];
    if (transformations.length) {
      const causes = {
        media_reference_projection: "媒体转为引用", deduplicate_skill_resources: "技能资源去重",
        retire_protocol_rejected_arguments: "移除未执行的错误参数", local_repair_projection: "局部修复上下文",
        resolved_tool_checkpoint_projection: "保留阶段结果引用", retire_unselected_skill_instructions: "移出未选用的技能",
        retire_superseded_checkpoints: "移出已替代的检查点", select_tool_working_set: "选择本步工具",
        context_compaction: "整理历史上下文", workflow_working_set_view: "当前工作流上下文",
        local_argument_repair_view: "当前参数修复上下文", durable_result_reference: "大结果保存为引用",
        restore_context_checkpoint: "恢复保存的上下文", compaction_request_selection: "选取待整理内容",
        reachable_schema_definitions: "移除 Schema 未引用的定义（保留约束）",
        loop_review_request_selection: "选取进展检查内容", unsubmitted_workflow_response: "移出未提交的回复",
        unexecuted_response_correction: "纠正未实际执行的回复",
      };
      $("#trace-request-summary").textContent += ` 已记录 ${transformations.length} 个上下文变换，可核验变换前后的来源。`;
      for (const change of transformations) {
        for (const part of change.before || []) {
          if (!(change.after || []).some(after => after.kind === part.kind && after.sha256 === part.sha256)) {
            select.add(new Option(`变换 ${change.sequence} · ${causes[change.reason] || change.reason} · 原 ${part.kind} ${part.index + 1}`, part.sha256));
          }
        }
      }
    }
    for (const part of evidence.manifest.parts) {
      const label = ({ configuration: "模型配置", system: "系统说明", message: "消息", tool: "工具 Schema" })[part.kind] || part.kind;
      select.add(new Option(`${label} ${part.index + 1} · ${formatTokens(part.bytes)} 字节${part.redacted ? " · 已脱敏" : ""}`, part.sha256));
    }
    select.hidden = false;
    select.value = requestEvidenceSelection.part;
    let selectionVersion = 0;
    select.onchange = async () => {
      const part = select.value;
      const version = ++selectionVersion;
      requestEvidenceSelection = { key, part };
      if (!part) {
        $("#trace-arguments").textContent = JSON.stringify({ ...evidence, context_metrics: call.context }, null, 2);
        return;
      }
      $("#trace-arguments").textContent = "正在读取所选内容…";
      try {
        const content = await api(`${path}?part=${encodeURIComponent(part)}`);
        if (current() && version === selectionVersion && select.value === part) {
          $("#trace-arguments").textContent = JSON.stringify(content, null, 2);
        }
      } catch (error) {
        if (current() && version === selectionVersion) $("#trace-arguments").textContent = error.message;
      }
    };
    await select.onchange();
  } catch (error) {
    if (!current()) return;
    $("#trace-request-summary").textContent = error.status === 404
      ? "这次历史请求未保存输入清单，仍可查看原有用量和结构指标。"
      : `请求证据读取失败：${error.message}`;
    requestEvidenceCache.delete(key);
  }
}

function showModelCall(call, node, focus) {
  state.selectedRunDetail = false;
  state.selectedModelCallId = call.id;
  state.selectedInvocationId = null;
  document.querySelectorAll(".trace-flow-node.selected, .trace-step [data-invocation-id].selected").forEach((item) => {
    item.classList.remove("selected");
    item.setAttribute("aria-expanded", "false");
  });
  if (node?.classList.contains("trace-flow-node")) {
    node.classList.add("selected");
    node.setAttribute("aria-expanded", "true");
  }
  $("#trace-detail-kind").textContent = "MODEL";
  $("#trace-detail-kind").dataset.kind = "model";
  $("#trace-detail-title").textContent = modelKindLabel(call.call_kind);
  $("#trace-detail-capability").textContent = `${call.provider} · ${call.model}`;
  $("#trace-arguments-tab").textContent = "输入结构";
  $("#trace-result-tab").textContent = "输出摘要";
  $("#trace-arguments-heading").textContent = "模型输入结构（不保存原文）";
  $("#trace-result-heading").textContent = "模型输出摘要（不重复保存原文）";
  $("#trace-facts").innerHTML = `
    <div><dt>调用类型</dt><dd></dd></div>
    <div><dt>模型</dt><dd></dd></div>
    <div><dt>Provider</dt><dd></dd></div>
    <div><dt>Attempt</dt><dd>${Number(call.attempt || 0) + 1}</dd></div>
    <div><dt>状态 / 停止原因</dt><dd></dd></div>
    <div><dt>实际 Token</dt><dd></dd></div>
    <div><dt>首事件时延</dt><dd></dd></div>
    <div><dt>总耗时</dt><dd></dd></div>
    <div><dt>输入 Token（不含缓存）</dt><dd></dd></div>
    <div><dt>输出 Token</dt><dd></dd></div>
    <div><dt>缓存 Token</dt><dd></dd></div>
    <div><dt>工具 Schema（估算）</dt><dd></dd></div>
    <div><dt>系统提示（估算）</dt><dd></dd></div>
    <div><dt>用户消息（估算）</dt><dd></dd></div>
    <div><dt>历史助理消息（估算）</dt><dd></dd></div>
    <div><dt>工具结果（估算）</dt><dd></dd></div>
    <div><dt>摘要与执行引用（估算）</dt><dd></dd></div>
    <div><dt>Skill 说明 / 引用（摘要内估算）</dt><dd></dd></div>
    <div><dt>Checkpoint（摘要内估算）</dt><dd></dd></div>
    <div><dt>局部修复（摘要内估算）</dt><dd></dd></div>
    <div><dt>大结果引用（摘要内估算）</dt><dd></dd></div>
    <div><dt>工具参数（助理消息内估算）</dt><dd></dd></div>
    <div><dt>Schema 投影减少（估算）</dt><dd></dd></div>`;
  const values = $("#trace-facts").querySelectorAll("dd");
  values[0].textContent = modelKindLabel(call.call_kind);
  values[1].textContent = call.model;
  values[2].textContent = call.provider;
  values[4].textContent = `${statusLabel(call.status)} / ${call.stop_reason}`;
  values[5].textContent = call.usage_reported ? formatTokens(call.usage?.total_tokens) : "Provider 未提供";
  values[6].textContent = call.time_to_first_event_ms == null ? "未提供" : `${formatTokens(call.time_to_first_event_ms)} ms`;
  values[7].textContent = `${formatTokens(call.duration_ms)} ms`;
  const reportedField = name => call.usage_reported && call.usage?.reported?.[name] !== false;
  values[8].textContent = reportedField("input") ? formatTokens(call.usage?.input) : "未提供";
  values[9].textContent = reportedField("output") ? formatTokens(call.usage?.output) : "未提供";
  values[10].textContent = `读取 ${reportedField("cache_read") ? formatTokens(call.usage?.cache_read) : "未提供"} · 写入 ${reportedField("cache_write") ? formatTokens(call.usage?.cache_write) : "未提供"}`;
  const estimate = key => Number.isFinite(call.context?.[key]) ? formatTokens(call.context[key]) : "未记录";
  values[11].textContent = estimate("tool_schema_tokens");
  ["system_prompt_tokens", "user_tokens", "assistant_tokens", "tool_result_tokens", "compaction_tokens"]
    .forEach((key, index) => { values[12 + index].textContent = estimate(key); });
  const pairedEstimate = (a, b) => `${estimate(a)} / ${estimate(b)}`;
  values[17].textContent = pairedEstimate("active_skill_tokens", "skill_reference_tokens");
  values[18].textContent = pairedEstimate("checkpoint_tokens", "checkpoint_receipt_tokens");
  values[19].textContent = estimate("repair_context_tokens");
  values[20].textContent = estimate("result_reference_tokens");
  values[21].textContent = estimate("tool_argument_tokens");
  values[22].textContent = `${estimate("tool_schema_projection_saved_tokens")}（不等于实际计费节省）`;
  $("#trace-arguments").textContent = JSON.stringify({
    raw_content_captured: false,
    note: "此处是上下文结构估算，不是 Provider 计费。Skill、Checkpoint、修复和引用是摘要内子项，工具参数是助理消息内子项，不要重复相加。完整逻辑输入见请求证据（敏感信息脱敏）。",
    context: call.context,
  }, null, 2);
  loadRequestEvidence(call);
  $("#trace-result").textContent = JSON.stringify({
    usage_reported: call.usage_reported,
    usage: call.usage,
    response_id: call.response_id,
    stop_reason: call.stop_reason,
    error: call.error,
    output: call.output,
  }, null, 2);
  const calls = state.trace?.model_calls || [];
  const index = calls.findIndex((item) => item.id === call.id);
  $("#trace-detail-position").textContent = index >= 0 ? `模型请求 ${index + 1} / ${calls.length}` : "模型调用详情";
  $("#trace-detail-prev").disabled = index <= 0;
  $("#trace-detail-next").disabled = index < 0 || index >= calls.length - 1;
  setDetailTab(focus);
  openDetailDrawer(node);
}

function showRunOutcome(trace, node, focus = "result") {
  $("#trace-request-evidence").hidden = true;
  state.selectedInvocationId = null;
  state.selectedModelCallId = null;
  state.selectedRunDetail = true;
  document.querySelectorAll(".trace-flow-node.selected, .trace-step [data-invocation-id].selected").forEach((item) => {
    item.classList.remove("selected");
    item.setAttribute("aria-expanded", "false");
  });
  node.classList.add("selected");
  node.setAttribute("aria-expanded", "true");
  $("#trace-detail-kind").textContent = "RUN";
  $("#trace-detail-kind").dataset.kind = trace.run.status === "succeeded" ? "result" : "error";
  $("#trace-detail-title").textContent = `运行${statusLabel(trace.run.status)}`;
  $("#trace-detail-capability").textContent = trace.run.id;
  $("#trace-arguments-tab").textContent = "运行信息";
  $("#trace-result-tab").textContent = "结束详情";
  $("#trace-arguments-heading").textContent = "运行状态与预算";
  $("#trace-result-heading").textContent = "结束原因与关联错误";
  $("#trace-facts").replaceChildren();
  for (const [label, value] of [["状态", statusLabel(trace.run.status)], ["耗时", duration(trace.run)], ["实际 Token", formatTokens(trace.run.budget?.usage?.total_tokens)], ["能力调用", trace.invocations.length]]) {
    const item = document.createElement("div");
    const title = document.createElement("dt");
    const content = document.createElement("dd");
    title.textContent = label;
    content.textContent = String(value);
    item.append(title, content);
    $("#trace-facts").append(item);
  }
  $("#trace-arguments").textContent = JSON.stringify(trace.run, null, 2);
  $("#trace-result").textContent = JSON.stringify({
    status: trace.run.status,
    error: trace.run.error || null,
    budget_stop_reason: trace.run.budget?.stop_reason || null,
    // Historical errors are context, not a fabricated cause of a successful Run.
    last_failed_invocation: [...trace.invocations].reverse().find((item) => item.outcome_status === "failed") || null,
    last_failed_model_call: [...(trace.model_calls || [])].reverse().find((item) => item.error) || null,
    terminal_events: (trace.events || []).filter((event) => /error|fail|cancel|recover|complete|run_status/.test(event.event_type)),
  }, null, 2);
  $("#trace-detail-position").textContent = "运行结束详情";
  $("#trace-detail-prev").disabled = true;
  $("#trace-detail-next").disabled = true;
  setDetailTab(focus);
  openDetailDrawer(node);
}

function navigateDetail(offset) {
  if (state.selectedRunDetail) return;
  const focus = state.selectedFocus || "call";
  if (state.selectedModelCallId) {
    const calls = state.trace?.model_calls || [];
    const index = calls.findIndex((item) => item.id === state.selectedModelCallId);
    const call = calls[index + offset];
    if (!call) return;
    const node = [...document.querySelectorAll(".trace-flow-node[data-model-call-id]")]
      .find((item) => item.dataset.modelCallId === call.id && item.dataset.focus === focus);
    showModelCall(call, node, focus);
    return;
  }
  const invocations = state.trace?.invocations || [];
  const index = invocations.findIndex((item) => item.id === state.selectedInvocationId);
  const invocation = invocations[index + offset];
  if (!invocation) return;
  const node = [...document.querySelectorAll(".trace-flow-node[data-invocation-id]")]
    .find((item) => item.dataset.invocationId === invocation.id && item.dataset.focus === focus);
  if (node) showInvocation(invocation, node, focus);
}

function renderOverview(trace) {
  const usage = trace.run.budget?.usage || {};
  const calls = trace.model_calls || [];
  const modelUsage = calls.reduce((totals, call) => {
    const current = call.usage || {};
    totals.input += Number(current.input || 0);
    totals.output += Number(current.output || 0);
    totals.cache += Number(current.cache_read || 0) + Number(current.cache_write || 0);
    totals.total += Number(current.total_tokens || 0);
    totals.cost += Number(current.cost?.total || 0);
    return totals;
  }, { input: 0, output: 0, cache: 0, total: 0, cost: 0 });
  $("#trace-session-title").textContent = `第 ${trace.turn_ordinal} 轮 · 本轮执行${trace.run.attempt > 1 ? ` · 重试 ${trace.run.attempt - 1}` : ""}`;
  $("#trace-run-id").textContent = trace.run.id;
  $("#trace-status").textContent = statusLabel(trace.run.status);
  $("#trace-status").dataset.status = trace.run.status;
  $("#trace-created-at").textContent = formatTime(trace.run.created_at);
  $("#trace-prompt").textContent = trace.prompt || "无输入文本";
  $("#trace-duration").textContent = duration(trace.run);
  $("#trace-model-calls").textContent = calls.length;
  $("#trace-total-tokens").textContent = formatTokens(usage.total_tokens ?? modelUsage.total);
  $("#trace-cost").textContent = usage.cost_microusd ? formatCost(Number(usage.cost_microusd) / 1_000_000) : formatCost(modelUsage.cost);
  $("#trace-input-tokens").textContent = formatTokens(usage.input_tokens ?? modelUsage.input);
  $("#trace-output-tokens").textContent = formatTokens(usage.output_tokens ?? modelUsage.output);
  $("#trace-cache-tokens").textContent = formatTokens((usage.cache_read_tokens ?? 0) + (usage.cache_write_tokens ?? 0) || modelUsage.cache);
  $("#trace-invocations").textContent = trace.invocations.length;
  $("#trace-flow-meta").textContent = `${calls.length} 次模型请求 · ${trace.invocations.length} 次能力调用 · ${statusLabel(trace.run.status)}`;
  const ledger = trace.usage_ledger;
  if (ledger) {
    const notes = [];
    if (ledger.usage_unreported_attempts) notes.push(`${ledger.usage_unreported_attempts} 次请求未提供用量`);
    if (ledger.usage_incomplete_attempts) notes.push(`${ledger.usage_incomplete_attempts} 次请求仅提供部分用量`);
    if (ledger.unfinished_request_receipts) notes.push(`${ledger.unfinished_request_receipts} 次请求尚无结束记录`);
    if (ledger.legacy_baseline) notes.push(`含历史累计 ${formatTokens(ledger.legacy_baseline_tokens)} Token`);
    if (ledger.physical_attempts && !ledger.materialized_totals_match) notes.push("用量对账异常");
    if (notes.length) $("#trace-flow-meta").textContent += ` · ${notes.join(" · ")}`;
  }
}

async function selectRun(id, quiet = false, revealOnMobile = false) {
  if (state.selectedRunId !== id) {
    timelineState.pageAfter = 0;
    timelineState.previousPages = [];
    timelineState.pending = null;
    closeDetailDrawer(false);
    state.selectedInvocationId = null;
    state.selectedModelCallId = null;
    state.selectedFocus = null;
  }
  state.selectedRunId = id;
  const version = ++state.traceLoadVersion;
  const eventAfter = timelineState.pageAfter;
  renderRuns();
  renderConversation();
  if (!quiet) setService(true, "正在读取运行轨迹");
  try {
    const trace = await api(`/v1/trace/runs/${encodeURIComponent(id)}?event_after=${eventAfter}&event_limit=${TRACE_EVENT_PAGE_SIZE}`);
    if (state.selectedRunId !== id || version !== state.traceLoadVersion) return;
    if (trace.run.session_id !== state.selectedSessionId) throw new Error("指定执行不属于当前会话，请检查 Trace 链接");
    // A deep-linked attempt can be outside the first 100 records. Keep it
    // selected on refresh without loading unrelated attempts first.
    if (!state.runs.some(run=>run.id===id)) {
      state.runs.push({...trace.run,turn_ordinal:trace.turn_ordinal,prompt_preview:trace.prompt,
        total_tokens:trace.run.budget?.usage?.total_tokens||0});
      state.runs.sort((a,b)=>b.created_at-a.created_at||b.attempt-a.attempt);
    }
    state.trace = trace;
    $("#trace-empty").hidden = true;
    $("#trace-content").hidden = false;
    renderConversation(!quiet);
    renderOverview(trace);
    renderModelUsage(trace);
    renderFlow(trace);
    renderTimeline(trace);
    setService(true, trace.run.status === "running" ? "运行中 · 自动刷新" : "运行轨迹已同步");
    if (revealOnMobile) revealMobileDetail(".trace-overview-panel");
    return true;
  } catch (error) {
    if (state.selectedRunId !== id || version !== state.traceLoadVersion) return;
    setService(false, error.message);
    return false;
  }
}

async function loadRuns(keepSelection = true) {
  if (state.loading) return;
  state.loading = true;
  const button = $("#trace-refresh");
  button.disabled = true;
  setService(false, "正在同步会话记录");
  try {
    state.sessions = await api("/v1/trace/sessions?limit=200");
    const requested = !state.selectedSessionId ? new URLSearchParams(location.search) : null;
    const preservedSession = state.selectedSessionId || requested?.get("session");
    if (preservedSession && !state.sessions.some(session => session.id === preservedSession)) {
      const matching = await api(`/v1/trace/sessions?session_id=${encodeURIComponent(preservedSession)}&limit=1`);
      state.sessions.push(...matching);
    }
    const id = keepSelection && state.sessions.some((session) => session.id === state.selectedSessionId)
      ? state.selectedSessionId : requested?.get("session") || state.sessions[0]?.id;
    renderRuns();
    if (id) {
      const requestedRun = requested?.get("run");
      await selectSession(id, keepSelection, requestedRun);
      if (requestedRun && state.trace?.run.id===requestedRun) {
        const detail=$(".trace-overview-panel");
        detail.tabIndex=-1;
        detail.focus({preventScroll:true});
        detail.scrollIntoView({block:"start",behavior:"instant"});
      }
    }
    else {
      $("#trace-empty").hidden = false;
      $("#trace-content").hidden = true;
      setService(true, "暂无会话记录");
    }
  } catch (error) {
    setService(false, error.message);
  } finally {
    button.disabled = false;
    state.loading = false;
  }
}

async function copyData(targetId, button) {
  try {
    await navigator.clipboard.writeText($(`#${targetId}`).textContent);
    const previous = button.textContent;
    button.textContent = "已复制";
    setTimeout(() => { button.textContent = previous; }, 1200);
  } catch {
    button.textContent = "复制失败";
  }
}

$("#trace-search").addEventListener("input", (event) => { state.search = event.target.value; renderRuns(); });
$("#trace-refresh").addEventListener("click", () => loadRuns(true));
$("#trace-load-older").addEventListener("click", loadOlderRuns);
document.querySelectorAll("[data-timeline-mode]").forEach((button) => button.addEventListener("click", () => {
  timelineState.mode = button.dataset.timelineMode;
  if (state.trace) renderTimeline(state.trace);
}));
for (const direction of ["prev", "next", "latest"]) {
  $(`#trace-event-${direction}`).addEventListener("click", () => loadTimelinePage(direction));
}
document.querySelectorAll("[data-copy-target]").forEach((button) => button.addEventListener("click", () => copyData(button.dataset.copyTarget, button)));
document.querySelectorAll("[data-trace-tab]").forEach((button) => button.addEventListener("click", () => setDetailTab(button.dataset.traceTab)));
$("#trace-detail-close").addEventListener("click", () => closeDetailDrawer());
$("#trace-detail-backdrop").addEventListener("click", () => closeDetailDrawer());
$("#trace-detail-prev").addEventListener("click", () => navigateDetail(-1));
$("#trace-detail-next").addEventListener("click", () => navigateDetail(1));
document.addEventListener("keydown", (event) => {
  const drawer = $("#trace-detail-drawer");
  if (!drawer.classList.contains("open")) return;
  if (event.key === "Escape") {
    event.preventDefault();
    closeDetailDrawer();
    return;
  }
  if (event.key !== "Tab") return;
  const focusable = [...drawer.querySelectorAll("button:not(:disabled), [href], input, select, textarea, [tabindex]:not([tabindex='-1'])")]
    .filter((item) => !item.hidden);
  if (!focusable.length) return;
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
});
setInterval(() => {
  if (document.visibilityState === "visible") loadRuns(true);
}, 8000);
loadRuns(false);
