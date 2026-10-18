// Actual bundled Chat + SSE events captured from the Rust API integration test.
// No generation or model calls; progress must be visible in the real DOM.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile,writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';
const root=resolve(import.meta.dirname,'../../web');
const fixturePath=process.env.EXOHARNESS_PROGRESS_SSE_FIXTURE || process.argv[2];
let fixture=fixturePath
  ? JSON.parse(await readFile(fixturePath,'utf8')) : {progress:[
    {type:'tool_progress',tool_call_id:'owned-call',progress:84.25,total:100,message:'[sampling] ComfyUI 节点 10 采样进度 7/8'},
    {type:'tool_progress',tool_call_id:'owned-call',progress:88,message:'正在编码视频，已运行 125 秒'},
    {type:'tool_progress',tool_call_id:'owned-call',progress:90,total:100,message:'正在处理...'},
  ]};
if(fixture.source?.startsWith('production retained')) {
  const actual=fixture.progress.find(event=>event.progress===84.25 && event.total===100);
  assert.ok(actual,'production capture contains the known real sampling progress');
  // The first browser assertion uses a real production SSE event. The later
  // cases deliberately add controlled elapsed/diagnostic/terminal edge cases.
  fixture={progress:[actual,
    {type:'tool_progress',tool_call_id:actual.tool_call_id,progress:88,message:'正在编码视频，已运行 125 秒'},
    {type:'tool_progress',tool_call_id:actual.tool_call_id,progress:90,total:100,message:'正在处理...'},
  ]};
}
const profile={id:'fixture',revision:1,display_name:'Fixture',avatar_text:'F',digest:'fixture'};
const session={id:'progress-session',title:'Progress regression',pinned:false,created_at:1,updated_at:1,
  turn_count:1,latest_run_status:'running',latest_run_id:'progress-run',managed_coding:false,agent_profile:profile};
const turn={id:'progress-turn',ordinal:1,prompt:'Progress replay',attachments:[],answer:null,response:null,
  run_id:'progress-run',run_status:'running',created_at:1,updated_at:1};
const streams=new Set();let latest=null,completed=false,connections=0,sequence=0;
const send=(res,value)=>res.write(`id: ${++sequence}\nevent: chat\ndata: ${JSON.stringify(value)}\n\n`);
const emit=value=>{latest=value;for(const res of streams)send(res,value);};
const server=createServer(async(req,res)=>{
  const path=new URL(req.url,'http://localhost').pathname;
  try {
    if(path==='/chat') {
      let html=await readFile(resolve(root,'portal-chat.html'),'utf8');
      html=html.replace('<script src="/assets/portal-auth.js" defer></script>',
        '<script>window.ExoAuth={ready:Promise.resolve(),fetch:(...args)=>fetch(...args),account:{username:"fixture"}};</script>');
      res.setHeader('content-type','text/html');res.end(html);return;
    }
    if(path.startsWith('/assets/')) {
      const name=path.slice(8);assert.match(name,/^[\w.-]+$/);
      res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'application/octet-stream');
      res.end(await readFile(resolve(root,name)));return;
    }
    if(path==='/v1/runs/progress-run/events') {
      res.setHeader('content-type','text/event-stream');res.setHeader('cache-control','no-cache');res.flushHeaders();
      streams.add(res);connections++;send(res,{type:'assistant_start'});
      if(latest)send(res,latest);
      res.on('close',()=>streams.delete(res));return;
    }
    const status=completed?'succeeded':'running';
    const saved=completed?{...turn,run_status:status,answer:'已完成。',response:{answer:'已完成。',artifacts:[],tool_calls:[],warnings:[],usage:{total_tokens:0}}}:turn;
    const routes={'/v1/agent-profile':profile,'/v1/sessions':[{...session,latest_run_status:status}],
      '/v1/sessions/progress-session':{...session,latest_run_status:status,turns:[saved],notices:[],history_page:{has_more:false}},
      '/v1/chat/configuration':{model_display_name:'Fixture'},'/v1/speech-input/configuration':{available:false}};
    res.setHeader('content-type','application/json');res.end(JSON.stringify(routes[path]??{}));
  }catch {res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let chrome;
const dom=()=>chrome.evaluate(`(()=>{const track=document.querySelector('.tool-progress-track');return {
  text:document.querySelector('.tool-progress-value')?.textContent,
  detail:document.querySelector('.tool-progress-message')?.textContent,
  now:track?.getAttribute('aria-valuenow'),aria:track?.getAttribute('aria-valuetext'),
  indefinite:track?.classList.contains('is-indeterminate')}})()`);
try {
  chrome=await browser();
  await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/chat?session=progress-session`});
  await until(()=>streams.size>0,'running persisted conversation SSE');
  emit(fixture.progress[0]);
  await until(async()=> (await dom()).detail===fixture.progress[0].message,'actual normal service message visible');
  let value=await dom();assert.equal(value.now,'84');assert.equal(value.indefinite,false);
  assert.ok(value.text.includes('84%'));assert.equal(value.aria,fixture.progress[0].message);
  assert.ok(!value.detail.includes('已完成内容会保留'));
  if(process.argv[3]) {
    const screenshot=await chrome.send('Page.captureScreenshot',{format:'png'});
    await writeFile(process.argv[3],Buffer.from(screenshot.data,'base64'));
  }
  emit(fixture.progress[1]);
  await until(async()=> (await dom()).now==='88','progress update with previous total retained');
  value=await dom();assert.equal(value.detail,fixture.progress[1].message);assert.ok(value.text.includes('2分5秒'));
  emit(fixture.progress[2]);
  await until(async()=> (await dom()).now==='90','safe diagnostic fallback preserves percentage');
  assert.equal((await dom()).detail,'正在处理...');
  emit({type:'tool_progress',tool_call_id:'legacy-call',progress:4,total:8,message:'RuntimeError: private'});
  await until(async()=> (await dom()).now==='50','older service diagnostic still hidden in frontend');
  assert.equal((await dom()).detail,'正在处理...');
  assert.equal(await chrome.evaluate('/RuntimeError|private/.test(document.body.innerText)'),false);
  emit({type:'tool_progress',tool_call_id:'second-call',progress:0,message:'Preparing references'});
  await until(async()=> (await dom()).detail==='Preparing references','new task with unknown total');
  assert.equal((await dom()).indefinite,true);assert.equal((await dom()).now,null);
  emit({type:'tool_progress',tool_call_id:'second-call',progress:2,total:4,message:'视频生成中，已运行 30 秒'});
  await until(async()=> (await dom()).now==='50','new task total becomes available');
  emit({type:'tool_progress',tool_call_id:'second-call',progress:1,total:4,message:'继续生成中'});
  await until(async()=> (await dom()).detail==='继续生成中','monotonic progress within one task');
  assert.equal((await dom()).now,'50');
  // Restore the latest authoritative value before replaying on reload.
  emit({type:'tool_progress',tool_call_id:'second-call',progress:2,total:4,message:'视频生成中，已运行 30 秒'});
  const previous=connections;
  await chrome.send('Page.reload');
  await until(async()=>connections>previous && (await dom()).now==='50','refresh reattaches and shows saved real progress');
  for(const width of [1440,390]) {
    await chrome.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:width<600});
    assert.equal((await dom()).detail,'视频生成中，已运行 30 秒');
  }
  emit({type:'tool_progress',tool_call_id:'second-call',progress:4,total:4,message:'生成完成'});
  await until(async()=> (await dom()).now==='100','terminal progress reaches 100 percent');
  completed=true;emit({type:'complete',response:{answer:'已完成。',artifacts:[],tool_calls:[],warnings:[],usage:{total_tokens:0}}});
  await until(()=>chrome.evaluate('[...document.querySelectorAll(".chat-message.assistant .rich-content")].some(item=>item.textContent.includes("已完成。"))'),'complete replaces generation progress');
  assert.equal(await chrome.evaluate('[...document.querySelectorAll(".tool-progress-message")].some(item=>item.checkVisibility())'),false);
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS actual bundled Chat: API SSE progress message, percentage, elapsed time, updates, diagnostics, unknown total, task switch, monotonicity, refresh, desktop/mobile and completion.');
}finally {for(const stream of streams)stream.end();await chrome?.close();await new Promise(done=>server.close(done));}
