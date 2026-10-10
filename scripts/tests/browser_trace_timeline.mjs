// Browser regression against isolated metadata fixtures. No model/MCP calls.
// EXOHARNESS_TEST_CHROME=/path/to/chrome node scripts/test-trace-ui.mjs
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { spawn } from "node:child_process";
import { readFile, writeFile, mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const web = process.env.EXOHARNESS_TRACE_TEST_WEB || fileURLToPath(new URL("../../web/", import.meta.url));
const output = await mkdtemp(join(tmpdir(), "exoharness-trace-ui-"));
let excludeRequested = false;
let longTimeline = false;
let slowNextDetail = false;
const eventPageRequests = [];
const detailRequests = [];
const pageOffsets = [];
const epoch = Date.now() - 10000;
const streamText = " The audio URL origin is not allowed.\n<fixture> 不应当作 HTML。\n" + "这是完整的连续输出，展开后应保留所有内容。".repeat(20);
const streamParts = [" ", "The", " audio", " URL", " origin", " is", " not", streamText.slice(" The audio URL origin is not".length)];
const timelineEvents = [
  { event_type: "assistant_start", payload: {} },
  ...streamParts.map((delta) => ({ event_type: "text_delta", payload: { type: "text_delta", delta } })),
  { event_type: "tool_call", payload: { tool: "example.read", arguments: { id: "fixture" } } },
  { event_type: "text_delta", payload: { delta: "另一次响应" } },
  { event_type: "error", payload: { message: "fixture error <unsafe>" } },
  { event_type: "text_delta", payload: { delta: "错误后输出" } },
].map((event, index) => ({ ...event, sequence: index + 1, created_at: epoch + index * 50 }));
const longTimelineEvents = Array.from({length:6001}, (_, index) => ({
  sequence:index+1,event_type:index===6000?'error':'status',created_at:epoch+index,
  payload:{message:index===6000?'Final failure <unsafe>':`Event ${index+1}`},
}));
const prompts = ["生成一个美女跳舞的视频", "9比16", "30s", "超写实美女"];
const runs = prompts.map((prompt, index) => ({
  id: `run-${index + 1}`, session_id: "session-one", session_title: prompts[0], turn_ordinal: index + 1,
  attempt: 1, prompt_preview: prompt, status: index === 2 ? "failed" : index === 3 ? "recoverable" : "succeeded", total_tokens: (index + 1) * 100,
  error: index === 2 ? "Input validation rejected: storyboard type mismatch" : index === 3 ? "Run budget exceeded: token limit <fixture>" : null,
  model_call_count: 1, invocation_count: 0, created_at: epoch + index * 1000,
  started_at: epoch + index * 1000, finished_at: epoch + index * 1000 + 200,
})).reverse();
const sessions = [{
  id: "session-one", title: prompts[0], turn_count: 4, run_count: 4, model_call_count: 4,
  invocation_count: 0, total_tokens: 1000, latest_run_id: "run-4", latest_prompt: prompts[3],
  status: "succeeded", created_at: epoch, updated_at: epoch + 3000,
}, { id: "session-two", title: prompts[0], turn_count: 1, run_count: 1, model_call_count: 0,
  invocation_count: 0, total_tokens: 70, latest_run_id: "other-run", latest_prompt: "另一个用户的独立会话",
  status: "succeeded", created_at: epoch - 1000, updated_at: epoch - 1000 }];
const otherRun = { ...runs[0], id: "other-run", session_id: "session-two", turn_ordinal: 1, prompt_preview: sessions[1].latest_prompt, total_tokens: 70 };
const modelCalls = ["new", "legacy"].map((name, i) => ({id:`model-${name}`,logical_call_id:name,sequence:i+1,attempt:0,
  call_kind:"agent",model:"fixture",provider:"fixture",status:"succeeded",stop_reason:"stop",started_at:epoch+i,
  duration_ms:10,usage_reported:true,usage:{total_tokens:50,input:40,output:10,reported:{input:true,output:true,total_tokens:true,cache_read:false,cache_write:false}},context:{},output:{}}));
modelCalls[0].context = {request_estimated_tokens:1000, system_prompt_tokens:80,user_tokens:120,
  assistant_tokens:200,tool_argument_tokens:100,tool_result_tokens:150,compaction_tokens:250,
  active_skill_tokens:60,skill_reference_tokens:10,checkpoint_tokens:50,checkpoint_receipt_tokens:5,
  repair_context_tokens:40,result_reference_tokens:20,tool_schema_tokens:200,
  tool_schema_original_tokens:1200,tool_schema_projection_saved_tokens:1000};
const requestEvidence = {manifest:{boundary:"agent_provider_input_v1",parts:[{kind:"message",index:0,sha256:"fixture-part",bytes:48,redacted:false}]},
  changes:{added:[{}],removed:[],retained:[{},{}]},exact_projection:true,
  transformations:[{sequence:1,reason:"durable_result_reference",before:[{kind:"message",index:0,sha256:"original-part",bytes:40000}],after:[]}]};
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, "http://localhost");
    let value;
    if (url.pathname === "/v1/auth/session") value = { role: "admin", authentication_enabled: true };
    else if (url.pathname === "/v1/trace/sessions") value = url.searchParams.has("session_id")
      ? sessions.filter(item => item.id === url.searchParams.get("session_id")) : excludeRequested ? [sessions[1]] : sessions;
    else if (url.pathname === "/v1/trace/runs") {
      const items = url.searchParams.get("session_id") === "session-one" ? runs : [otherRun];
      const offset = Number(url.searchParams.get("offset") || 0);
      pageOffsets.push(offset);
      value = excludeRequested && url.searchParams.get('session_id')==='session-one'
        ? (offset===0?items.filter(item=>item.id!=='run-3'):offset===3?items.filter(item=>item.id==='run-3'):[])
        : items.slice(offset, offset + Number(url.searchParams.get("limit") || 100));
    } else if (url.pathname === "/v1/trace/runs/run-1/requests/new/0") {
      value = url.searchParams.has("part") ? {role:"user",text:"Exact input <unsafe>"} : requestEvidence;
    } else if (url.pathname.includes("/requests/")) {
      response.writeHead(404); response.end(); return;
    } else if (url.pathname.startsWith("/v1/trace/runs/")) {
      detailRequests.push(url.pathname.split('/').at(-1));
      const run = [...runs, otherRun].find((item) => item.id === url.pathname.split("/").at(-1));
      assert(run);
      const allEvents = run.id==='run-3' ? (longTimeline ? longTimelineEvents : timelineEvents) : [];
      const after = Number(url.searchParams.get('event_after') || 0);
      const limit = Math.min(Number(url.searchParams.get('event_limit') || 5000), 512);
      eventPageRequests.push({run:run.id,after,limit});
      value = { run: { ...run, last_sequence:allEvents.at(-1)?.sequence || 0, budget: { usage: { total_tokens: run.total_tokens } } },
        usage_ledger:{version:1,physical_attempts:2,usage_unreported_attempts:1,usage_incomplete_attempts:1,materialized_totals_match:true},
        session_title: run.session_title, turn_ordinal: run.turn_ordinal, prompt: run.prompt_preview,
        events: allEvents.filter(event=>event.sequence>after).slice(0,limit), invocations: run.id === "run-3" ? [{
          id:"rejected-call", capability_id:"skill:global:_:document-writer", capability_revision:1,
          tool_name:"skill.document-writer.read", arguments:{resources:[17]}, arguments_digest:"sha256:fixture",
          result:{isError:true, details:{_meta:{exoharness:{execution_error:{phase:"input_validation",submitted:false,retry_scope:"arguments"}}}}},
          status:"completed",outcome_status:"failed",occurrence:1,created_at:epoch,submitted_at:null,completed_at:epoch+10,
        }] : [], model_calls: run.id === "run-1" ? modelCalls : [] };
      if (slowNextDetail) {
        slowNextDetail = false;
        value.run.budget.usage.total_tokens = 999;
        await new Promise(resolve=>setTimeout(resolve,350));
      }
    }
    if (value) {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify(value));
    } else {
      const asset = url.pathname === "/trace" ? "trace.html" : url.pathname.replace("/assets/", "");
      assert(["trace.html", "trace.js", "auth.js", "styles.css"].includes(asset));
      response.writeHead(200, { "content-type": asset.endsWith("css") ? "text/css" : asset.endsWith("js") ? "application/javascript" : "text/html" });
      response.end(await readFile(join(web, asset)));
    }
  } catch { response.writeHead(404); response.end(); }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const browser = spawn(process.env.EXOHARNESS_TEST_CHROME || "/opt/google/chrome/chrome", [
  "--headless=new", "--no-sandbox", "--disable-dev-shm-usage", "--disable-gpu",
  "--remote-debugging-port=0", `--user-data-dir=${join(output, "profile")}`, "about:blank",
], { stdio: ["ignore", "ignore", "pipe"] });
let socket;
try {
  const endpoint = await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("Chrome did not start")), 15000);
    browser.on("error", reject);
    browser.stderr.on("data", (data) => {
      const match = data.toString().match(/DevTools listening on (ws:\/\/\S+)/);
      if (match) { clearTimeout(timeout); resolve(new URL(match[1])); }
    });
  });
  const pages = await fetch(`http://127.0.0.1:${endpoint.port}/json/list`).then((response) => response.json());
  socket = new WebSocket(pages.find((page) => page.type === "page").webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let sequence = 0;
  const pending = new Map();
  const errors = [];
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.method === "Runtime.exceptionThrown") errors.push(message.params.exceptionDetails.text);
    if (!message.id) return;
    const callback = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) callback.reject(new Error(JSON.stringify(message.error)));
    else callback.resolve(message.result);
  };
  const cdp = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++sequence; pending.set(id, { resolve, reject }); socket.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => {
    const value = await cdp("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (value.exceptionDetails) throw new Error(value.exceptionDetails.text);
    return value.result.value;
  };
  const waitFor = async (expression) => {
    const deadline = Date.now() + 10000;
    while (Date.now() < deadline) {
      if (await evaluate(expression)) return;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw new Error(`UI condition not reached: ${expression}`);
  };
  await cdp("Runtime.enable");
  await cdp("Emulation.setDeviceMetricsOverride", { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
  await cdp("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/trace` });
  await waitFor("document.querySelectorAll('.trace-turn-item').length === 4 && !document.querySelector('#trace-content').hidden");
  assert.equal(await evaluate("document.querySelectorAll('.trace-run-item').length"), 2, "four messages are one session, another user's same title stays separate");
  assert.equal(await evaluate("document.querySelector('#trace-conversation-turns').textContent"), "4 轮消息 · 4 次执行");
  assert.equal(await evaluate("document.querySelector('#trace-conversation-tokens').textContent"), "1,000");
  await evaluate("document.querySelector('[data-run-id=\"run-3\"]').click()");
  await waitFor("document.querySelector('#trace-prompt').textContent === '30s'");
  await waitFor("document.querySelector('[data-invocation-id=\"rejected-call\"]') && document.querySelector('#service-label').textContent === '运行轨迹已同步'");
  assert.equal(await evaluate("document.querySelector('#trace-total-tokens').textContent"), "300");
  assert.equal(await evaluate("document.querySelector('#trace-conversation-tokens').textContent"), "1,000");
  // Stream chunks preserve exact whitespace and stop at every journal boundary.
  assert.equal(await evaluate("document.querySelectorAll('#trace-timeline .trace-step').length"), 6);
  assert.equal(await evaluate("document.querySelector('[data-event-sequence=\"2\"] .trace-step-text').textContent"), streamText);
  assert.equal(await evaluate("document.querySelector('[data-event-sequence=\"2\"] code').textContent"), "#2–9");
  assert(await evaluate("document.querySelector('#trace-timeline-meta').textContent.includes('6 条展示 · 13 条原始事件')"));
  assert(await evaluate("document.querySelector('[data-event-sequence=\"2\"] time').textContent.includes(' – ')"));
  assert.equal(await evaluate("document.querySelectorAll('#trace-timeline fixture, #trace-timeline unsafe').length"), 0);
  assert(await evaluate("document.querySelector('[data-event-sequence=\"12\"]').classList.contains('trace-step-error')"));
  assert(await evaluate("document.querySelector('[data-event-sequence=\"2\"] .trace-step-text').classList.contains('collapsed')"));
  await evaluate("document.querySelector('[data-event-sequence=\"2\"] .trace-text-expand').click(); document.querySelector('[data-event-sequence=\"2\"] summary').click()");
  await waitFor("document.querySelector('[data-event-sequence=\"2\"] details pre')");
  const rawEvents = JSON.parse(await evaluate("document.querySelector('[data-event-sequence=\"2\"] details pre').textContent"));
  assert.deepEqual(rawEvents, timelineEvents.slice(1, 9), "raw event sequence, timestamps, payload and whitespace remain intact");
  assert(await evaluate("(()=>{const node=document.querySelector('[data-event-sequence=\"2\"]');renderTimeline(state.trace);return node===document.querySelector('[data-event-sequence=\"2\"]')})()"), "unchanged refresh keeps DOM, selection and open details");
  await evaluate("document.querySelector('#trace-timeline').scrollTop=120;document.querySelector('[data-event-sequence=\"2\"] .trace-text-expand').focus({preventScroll:true});renderTimeline({...state.trace,events:[...state.trace.events,{sequence:14,event_type:'status',payload:{status:'ok'},created_at:Date.now()}]})");
  assert(await evaluate("document.querySelector('[data-event-sequence=\"2\"] details').open"), "new events preserve open raw details");
  assert(await evaluate("document.querySelector('[data-event-sequence=\"2\"] .trace-text-expand').getAttribute('aria-expanded')==='true'"), "new events preserve expanded text");
  assert.equal(await evaluate("document.querySelector('#trace-timeline').scrollTop"), 120);
  assert(await evaluate("document.activeElement.dataset.timelineControl === 'readable:2:text'"), "new events preserve keyboard focus");
  await evaluate("document.querySelector('[data-timeline-mode=\"raw\"]').click()");
  assert.equal(await evaluate("document.querySelectorAll('#trace-timeline .trace-step').length"), 13);
  assert.equal(await evaluate("document.querySelector('[data-event-sequence=\"3\"] strong').textContent"), "text_delta");
  await evaluate("document.querySelector('[data-timeline-mode=\"readable\"]').click()");
  assert.equal(await evaluate("document.querySelectorAll('#trace-timeline .trace-step').length"), 6);
  const grouping = await evaluate(`(() => {
    const delta = (sequence, text, extra = {}) => ({ sequence, event_type: 'text_delta', payload: { delta: text, ...extra } });
    const project = events => timelineEntries(events).map(entry => entry.text);
    return {
      whitespace: project([delta(1, ''), delta(2, ' '), delta(3, '\\n中'), delta(4, '文')]),
      gap: project([delta(1, 'a'), delta(3, 'b')]),
      duplicate: project([delta(1, 'a'), delta(1, 'b')]),
      metadata: project([delta(1, 'a', {message_id:'one'}), delta(2, 'b', {message_id:'two'})]),
      boundaries: project([delta(1, 'a'), {sequence:2,event_type:'turn_end'}, delta(3,'b'), {sequence:4,event_type:'assistant_start'}, delta(5,'c')]),
      malformed: project([delta(1, 'a'), delta(2, {unexpected:true}), delta(3,'b')]),
      raw: timelineEntries([delta(1,'a'),delta(2,'b')], 'raw').length,
      empty: project([]),
      large: timelineEntries(Array.from({length:5000},(_,i)=>delta(i+1,'x'))).map(entry=>[entry.events.length,entry.text.length])
    };
  })()`);
  assert.deepEqual(grouping.whitespace, [" \n中文"]);
  assert.deepEqual(grouping.gap, ["a", "b"]);
  assert.deepEqual(grouping.duplicate, ["a", "b"]);
  assert.deepEqual(grouping.metadata, ["a", "b"]);
  assert.equal(grouping.boundaries.length, 5);
  assert.equal(grouping.malformed.length, 3);
  assert.equal(grouping.raw, 2);
  assert.deepEqual(grouping.empty, []);
  // Native lifecycle records must name their committed invocation, not show
  // a generic placeholder. Original envelopes and ordering remain untouched.
  const lifecycle = await evaluate(`(() => {
    const invocations = [{id:'native-id',capability_id:'mcp_server:global:_:fixture',tool_name:'render_video',arguments:{duration:15},result:{status:'completed'},status:'completed'}];
    const events = [
      {sequence:1,event_type:'pro_invocation_prepared',payload:{type:'invocation_prepared',data:{invocation_id:'native-id'}}},
      {sequence:2,event_type:'pro_invocation_submitted',payload:{type:'invocation_submitted',data:{invocation_id:'native-id',attempt:1}}},
      {sequence:3,event_type:'tool_call',payload:{tool_call_id:'native-id',tool:'render_video'}},
      {sequence:4,event_type:'pro_invocation_resolved',payload:{type:'invocation_resolved',data:{invocation_id:'native-id',state:'completed'}}},
      {sequence:5,event_type:'tool_result',payload:{tool_call_id:'native-id',is_error:false}},
      {sequence:6,event_type:'pro_invocation_submitted',payload:{type:'invocation_submitted',data:{invocation_id:'unmatched'}}},
    ];
    const before=JSON.stringify(events);
    renderTimeline({...state.trace,events,invocations});
    const links=[...document.querySelectorAll('#trace-timeline [data-invocation-id]')];
    const summaries=[...document.querySelectorAll('#trace-timeline .trace-step-text')].map(node=>node.textContent);
    const labels=[...document.querySelectorAll('#trace-timeline .trace-step-heading strong')].map(node=>node.textContent);
    links[0].click();
    const argumentsText=document.querySelector('#trace-arguments').textContent;
    document.querySelector('#trace-detail-close').click();
    links.at(-1).click();
    const resultText=document.querySelector('#trace-result').textContent;
    document.querySelector('#trace-detail-close').click();
    const reset=links.every(link=>link.getAttribute('aria-expanded')==='false');
    timelineState.renderKey=null;renderTimeline(state.trace);
    return {summaries,labels,links:links.length,argumentsText,resultText,reset,unchanged:before===JSON.stringify(events)};
  })()`);
  assert.equal(lifecycle.links,5);
  assert(lifecycle.summaries.slice(0,5).every(text=>text.includes('render_video')));
  assert(lifecycle.summaries[0].includes('尚未提交'));
  assert(lifecycle.summaries[5].includes('unmatched'));
  assert.deepEqual(lifecycle.labels,['准备调用','提交调用','调用能力','调用结算','能力返回','提交调用']);
  assert.deepEqual(JSON.parse(lifecycle.argumentsText),{duration:15});
  assert.deepEqual(JSON.parse(lifecycle.resultText),{status:'completed'});
  assert(lifecycle.reset && lifecycle.unchanged);
  assert.deepEqual(grouping.large, [[5000, 5000]]);
  assert.equal(await evaluate("eventSummary({event_type:'connection_resolution',payload:{bindings:[{targets:[{capability_id:'one'},{capability_id:'one'}]},{targets:[{capability_id:'two'}]}]}})"), "2 个依赖连接 · 2 个已授权服务");
  assert.equal(await evaluate("eventSummary({event_type:'connection_resolution',payload:{bindings:[{targets:[]}]}})"), "未匹配依赖地址，保留服务自身配置");
  await evaluate("renderTimeline({...state.trace,run:{...state.trace.run,last_sequence:6000}})");
  assert(await evaluate("!document.querySelector('#trace-timeline-warning').hidden"), "server event limit must not silently imply a complete timeline");
  await evaluate("renderTimeline(state.trace)");
  assert(await evaluate("document.querySelector('.trace-flow').textContent.includes('参数校验失败 · 未执行')"));
  await evaluate("[...document.querySelectorAll('.trace-flow-node')].find(node=>node.textContent.includes('参数校验失败 · 未执行')).click()");
  await waitFor("document.querySelector('#trace-result').textContent.includes('input_validation')");
  assert(await evaluate("document.querySelector('#trace-arguments').textContent.includes('17')"));
  await evaluate("document.querySelector('#trace-detail-close').click()");
  await evaluate("document.querySelector('[data-run-outcome=\"run-3\"]').click()");
  await waitFor("document.querySelector('#trace-detail-drawer').classList.contains('open') && document.querySelector('#trace-result').textContent.includes('storyboard type mismatch')");
  assert(await evaluate("document.querySelector('#trace-result').textContent.includes('input_validation')"), "final failure includes recorded invocation evidence");
  await evaluate("document.querySelector('#trace-arguments-tab').click()");
  assert(await evaluate("document.querySelector('#trace-arguments').textContent.includes('run-3')"));
  await cdp("Input.dispatchKeyEvent", { type: "keyDown", key: "Escape", code: "Escape" });
  await waitFor("!document.querySelector('#trace-detail-drawer').classList.contains('open')");
  assert(await evaluate("document.activeElement.dataset.runOutcome === 'run-3'"), "closing final node restores keyboard focus");
  await writeFile(join(output, "desktop.png"), Buffer.from((await cdp("Page.captureScreenshot")).data, "base64"));
  await evaluate("document.querySelector('.trace-timeline-panel').scrollIntoView({block:'start'});document.querySelector('#trace-timeline').scrollTop=0;document.querySelector('[data-event-sequence=\"2\"] .trace-text-expand').click();document.querySelector('[data-event-sequence=\"2\"] summary').click()");
  await writeFile(join(output, "timeline-desktop.png"), Buffer.from((await cdp("Page.captureScreenshot")).data, "base64"));
  assert.deepEqual(await evaluate("[...document.querySelectorAll('.main-nav > a')].filter(link => link.href.endsWith('/services') || link.href.endsWith('/imports')).map(link => link.textContent.trim())"), ["技能中心", "技能导入"]);
  assert(await evaluate("[...document.querySelectorAll('.main-nav > a')].every(link => link.querySelector('.main-nav-icon svg'))"), "every admin navigation item has an icon");
  await cdp("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
  await waitFor("document.querySelector('.main-nav').getAttribute('aria-hidden') === 'true'");
  await new Promise((resolve) => setTimeout(resolve, 300));
  const widths = await evaluate("({viewport:innerWidth, root:document.documentElement.scrollWidth})");
  assert(widths.root <= widths.viewport, `mobile page must not overflow horizontally: ${JSON.stringify(widths)}`);
  await evaluate("document.querySelector('.trace-timeline-panel').scrollIntoView({block:'start'});window.scrollBy(0,-80)");
  await writeFile(join(output, "timeline-mobile.png"), Buffer.from((await cdp("Page.captureScreenshot")).data, "base64"));
  assert(await evaluate("(()=>{const flow=document.querySelector('.trace-flow');return flow.scrollWidth<=flow.clientWidth+1})()"), "mobile call path fits the viewport without sideways scrolling");
  await evaluate("document.querySelector('.mobile-nav-toggle').click()");
  await new Promise((resolve) => setTimeout(resolve, 300));
  assert(await evaluate("(()=>{const nav=document.querySelector('.main-nav');const rect=nav.getBoundingClientRect();return nav.getAttribute('aria-hidden')==='false' && rect.left>=0 && rect.right<=innerWidth})()"), "mobile navigation remains usable");
  await evaluate("document.querySelector('.mobile-nav-close').click()");
  await new Promise((resolve) => setTimeout(resolve, 300));
  // The same Run can already display this prompt. Await the selection request
  // before checking scroll position; a late mobile reveal is not test readiness.
  await evaluate("selectRun('run-3',false,true)");
  await evaluate("document.querySelector('.trace-flow-panel').scrollIntoView({block:'start',behavior:'instant'})");
  await waitFor("(()=>{const top=document.querySelector('.trace-flow-panel').getBoundingClientRect().top;return top>=-1&&top<innerHeight/2})()");
  const flowView = await evaluate("(()=>{const r=document.querySelector('.trace-flow-panel').getBoundingClientRect();return {top:r.top,height:r.height,innerHeight,scrollY,scrollHeight:document.documentElement.scrollHeight}})()");
  assert(flowView.top>=-1&&flowView.top<flowView.innerHeight/2, `mobile call path can be brought into view: ${JSON.stringify(flowView)}`);
  await writeFile(join(output, "flow-mobile.png"), Buffer.from((await cdp("Page.captureScreenshot")).data, "base64"));
  await evaluate("document.querySelector('[data-invocation-id=\"rejected-call\"]').click()");
  await waitFor("document.querySelector('#trace-detail-drawer').classList.contains('open')");
  assert(await evaluate("(()=>{const r=document.querySelector('#trace-detail-drawer').getBoundingClientRect();return r.left>=0&&r.right<=innerWidth+1&&r.height<=innerHeight})()"), "mobile invocation details fit the viewport");
  await evaluate("document.querySelector('#trace-detail-close').click()");
  await evaluate("document.querySelector('[data-run-id=\"run-4\"]').click()");
  await waitFor("document.querySelector('#trace-prompt').textContent === '超写实美女'");
  assert(await evaluate("(()=>{const top=document.querySelector('.trace-overview-panel').getBoundingClientRect().top;return top>=0&&top<innerHeight/2})()"), "selecting a round reveals its Trace details on mobile");
  await evaluate("document.querySelector('[data-run-outcome=\"run-4\"]').click()");
  await waitFor("document.querySelector('#trace-result').textContent.includes('Run budget exceeded')");
  assert(await evaluate("document.querySelector('#trace-result').textContent.includes('<fixture>')"), "error text is not parsed as HTML");
  await evaluate("document.querySelector('#trace-detail-close').click()");
  assert(await evaluate("(()=>{const list=document.querySelector('#trace-turn-list').getBoundingClientRect();const active=document.querySelector('.trace-turn-item.active').getBoundingClientRect();return active.top>=list.top-1 && active.bottom<=list.bottom+1})()"), "selected round is visible in the mobile scroll panel");
  await evaluate("(()=>{const panel=document.querySelector('.trace-conversation-panel');window.scrollTo(0,panel.getBoundingClientRect().top+scrollY-80)})()");
  await writeFile(join(output, "mobile.png"), Buffer.from((await cdp("Page.captureScreenshot")).data, "base64"));
  await evaluate("document.querySelector('[data-run-id=\"run-1\"]').click()");
  await waitFor("document.querySelector('[data-model-call-id=\"model-new\"]')");
  await evaluate("document.querySelector('[data-model-call-id=\"model-new\"]').click()");
  await waitFor("document.querySelector('#trace-request-summary').textContent.includes('新增 1 项')");
  assert(await evaluate("document.querySelector('#trace-request-summary').textContent.includes('不代表模型没有接收')"));
  assert(await evaluate("document.querySelector('#trace-request-summary').textContent.includes('已记录 1 个上下文变换')"));
  assert(await evaluate("document.querySelector('#trace-request-part option[value=\"original-part\"]').textContent.includes('大结果保存为引用')"));
  assert(await evaluate("document.querySelector('#trace-flow-meta').textContent.includes('1 次请求未提供用量')"));
  assert(await evaluate("document.querySelector('#trace-flow-meta').textContent.includes('1 次请求仅提供部分用量')"));
  assert(await evaluate("document.querySelector('#trace-facts').textContent.includes('读取 未提供 · 写入 未提供')"), "unknown cache usage is not displayed as zero");
  const attribution = await evaluate("Object.fromEntries([...document.querySelectorAll('#trace-facts > div')].map(row=>[row.querySelector('dt').textContent,row.querySelector('dd').textContent]))");
  assert.equal(attribution['实际 Token'], '50', 'estimates never replace reported usage');
  assert.equal(attribution['用户消息（估算）'], '120');
  assert.equal(attribution['Skill 说明 / 引用（摘要内估算）'], '60 / 10');
  assert.equal(attribution['局部修复（摘要内估算）'], '40');
  assert.equal(attribution['工具参数（助理消息内估算）'], '100');
  assert.equal(attribution['Schema 投影减少（估算）'], '1,000（不等于实际计费节省）');
  await evaluate("(()=>{const select=document.querySelector('#trace-request-part');select.value='original-part';select.dispatchEvent(new Event('change'))})()");
  await waitFor("document.querySelector('#trace-arguments').textContent.includes('Exact input <unsafe>')");
  await evaluate("const select=document.querySelector('#trace-request-part');select.value='fixture-part';select.dispatchEvent(new Event('change'))");
  await waitFor("document.querySelector('#trace-arguments').textContent.includes('Exact input <unsafe>')");
  assert.equal(await evaluate("document.querySelectorAll('#trace-arguments unsafe').length"),0);
  await evaluate("document.querySelector('#trace-request-evidence').scrollIntoView({block:'start'})");
  await writeFile(join(output, "model-request-mobile.png"), Buffer.from((await cdp("Page.captureScreenshot")).data, "base64"));
  await evaluate("document.querySelector('#trace-detail-close').click();document.querySelector('[data-model-call-id=\"model-legacy\"]').click()");
  await waitFor("document.querySelector('#trace-request-summary').textContent.includes('历史请求未保存')");
  assert(await evaluate("[...document.querySelectorAll('#trace-facts > div')].find(row=>row.querySelector('dt').textContent==='局部修复（摘要内估算）').querySelector('dd').textContent==='未记录'"), 'old observations are unknown, not zero');
  await evaluate("document.querySelector('#trace-detail-close').click()");
  await evaluate("document.querySelector('[data-session-id=\"session-two\"]').click()");
  await waitFor("document.querySelector('#trace-prompt').textContent === '另一个用户的独立会话'");
  assert(await evaluate("(()=>{const top=document.querySelector('.trace-conversation-panel').getBoundingClientRect().top;return top>=0&&top<innerHeight/2})()"), "selecting a session reveals its rounds on mobile");
  assert.equal(await evaluate("document.querySelectorAll('.trace-turn-item').length"), 1);
  assert.equal(await evaluate("document.querySelector('#trace-conversation-tokens').textContent"), "70");
  assert.equal(await evaluate("document.querySelectorAll('#trace-timeline .trace-step').length"), 0, "switching runs removes previous timeline");
  excludeRequested=true;detailRequests.length=0;
  await cdp("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/trace?session=session-one&run=run-3` });
  await waitFor("document.querySelector('#trace-prompt')?.textContent === '30s' && document.querySelector('#trace-conversation-id')?.textContent === 'session-one'");
  assert.equal(await evaluate("document.querySelector('#trace-total-tokens').textContent"), "300", "overview deep link selects the exact failed Run");
  assert.deepEqual(detailRequests,['run-3'],'deep link must not load the newest unrelated Run first');
  assert.equal(await evaluate("document.querySelector('.trace-turn-item.active').dataset.runId"),'run-3');
  assert.equal(await evaluate("(()=>{const r=document.querySelector('.trace-overview-panel').getBoundingClientRect();return r.top>=0&&r.top<innerHeight/2})()"),true,'mobile direct link reveals the selected task without scrolling past the session list');
  assert.equal(await evaluate("document.querySelector('#trace-load-older').hidden"),false,'a directly pinned Run does not count as a loaded page');
  await evaluate("document.querySelector('#trace-load-older').click()");
  await waitFor("document.querySelector('#trace-load-older').hidden");
  assert.equal(pageOffsets.at(-1),3,'pagination must not skip records after pinning an older Run');
  assert.equal(await evaluate("document.querySelectorAll('.trace-turn-item').length"),4,'loading the pinned Run again does not duplicate it');
  await evaluate("document.querySelector('#trace-refresh').click()");
  await waitFor("!document.querySelector('#trace-refresh').disabled");
  assert.equal(await evaluate("document.querySelector('#trace-run-id').textContent"),'run-3');
  assert(detailRequests.every(id=>id==='run-3'),'refresh preserves an older linked Run');
  await cdp("Emulation.setDeviceMetricsOverride", { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false });
  await waitFor("document.querySelector('.main-nav').getAttribute('aria-hidden') === 'true'");
  await evaluate("document.querySelector('.mobile-nav-toggle').click()");
  assert(await evaluate("document.querySelector('.main-nav').getAttribute('aria-hidden') === 'false'"), "tablet navigation opens without squeezing the header");
  await evaluate("document.querySelector('.mobile-nav-close').click()");
  // A long journal is read in bounded windows, including events beyond the old
  // 5,000-event ceiling. No hidden accumulation or all-event DOM reconstruction.
  longTimeline=true;
  await evaluate("selectRun('run-3',true)");
  assert.equal(await evaluate("state.trace.events.length"),256);
  assert.equal(await evaluate("document.querySelectorAll('#trace-timeline .trace-step').length"),256);
  assert.equal(await evaluate("document.querySelector('#trace-event-next').hidden"),false);
  await evaluate("loadTimelinePage('next')");
  assert.equal(await evaluate("state.trace.events[0].sequence"),257);
  assert.equal(await evaluate("state.trace.events.at(-1).sequence"),512);
  await evaluate("loadTimelinePage('prev')");
  assert.equal(await evaluate("state.trace.events[0].sequence"),1);
  await evaluate("loadTimelinePage('latest')");
  assert.equal(await evaluate("state.trace.events[0].sequence"),5746);
  assert.equal(await evaluate("state.trace.events.at(-1).sequence"),6001);
  assert(await evaluate("document.querySelector('[data-event-sequence=\"6001\"]').textContent.includes('Final failure <unsafe>')"));
  assert.equal(await evaluate("document.querySelector('#trace-event-next').hidden"),true);
  assert.equal(await evaluate("document.querySelectorAll('#trace-timeline unsafe').length"),0);
  await evaluate("selectRun('run-3',true)");
  assert.equal(eventPageRequests.at(-1).after,5745,'quiet refresh preserves the selected window');
  assert(eventPageRequests.every(request=>request.limit===256),'every UI request is bounded');
  assert.equal(await evaluate("(()=>{const node=document.querySelector('[data-event-sequence=\"6001\"]');renderTimeline(state.trace);return node===document.querySelector('[data-event-sequence=\"6001\"]')})()"),true);
  // Late responses must not overwrite a newer response to the same Run.
  slowNextDetail=true;
  await evaluate("window.heldTrace=selectRun('run-3',true); 'started'");
  while(slowNextDetail) await new Promise(resolve=>setTimeout(resolve,10));
  await evaluate("selectRun('run-3',true)");
  await evaluate("window.heldTrace");
  assert.equal(await evaluate("document.querySelector('#trace-total-tokens').textContent"),'300');
  // Late responses must also stay out of a subsequently selected Run.
  slowNextDetail=true;
  await evaluate("window.heldTrace=selectRun('run-3',true); 'started'");
  while(slowNextDetail) await new Promise(resolve=>setTimeout(resolve,10));
  await evaluate("selectRun('run-1',true)");
  await evaluate("window.heldTrace");
  assert.equal(await evaluate("state.trace.run.id"),'run-1');
  assert.equal(await evaluate("timelineState.pageAfter"),0);
  assert.equal(await evaluate("document.querySelector('#trace-event-prev').hidden"),true);
  assert.deepEqual(errors, []);
  console.log(`Trace stream aggregation, raw events, boundaries, refresh state, session grouping, totals, user isolation and mobile layout passed. Screenshots: ${output}`);
} finally {
  socket?.close(); browser.kill("SIGTERM"); server.close();
}
