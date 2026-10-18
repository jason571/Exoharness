// Isolated real Chat browser. No production account, model or generation calls.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';
const root=resolve(import.meta.dirname,'../../web');
const profile={id:'fixture',revision:1,display_name:'Fixture',avatar_text:'F',digest:'fixture'};
const session={id:'notice-session',title:'Notice fixture',pinned:false,created_at:1,updated_at:1,turn_count:1,latest_run_status:'running',latest_run_id:'notice-run',managed_coding:false,agent_profile:profile};
let code='task_processing_limited';
let persisted=false;
const partial={answer:'39÷3=13，13×24=312。配音已保留，动画暂时不可用。',partial_result:true,model:{name:'Fixture',provider:'fixture',model_id:'fixture'},usage:{total_tokens:88},warnings:[],artifacts:[],tool_calls:[]};
const timers=new Set();
const server=createServer(async(req,res)=>{
  const path=new URL(req.url,'http://localhost').pathname;
  try {
    if(path==='/chat') {
      const html=(await readFile(resolve(root,'portal-chat.html'),'utf8')).replace('<script src="/assets/portal-auth.js" defer></script>','<script>window.ExoAuth={ready:Promise.resolve(),fetch:(...args)=>fetch(...args),account:{username:"fixture"}};</script>');
      res.setHeader('content-type','text/html');res.end(html);return;
    }
    if(path.startsWith('/assets/')) {
      const name=path.slice(8);assert.match(name,/^[\w.-]+$/);
      res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'application/octet-stream');
      res.end(await readFile(resolve(root,name)));return;
    }
    if(path==='/v1/runs/notice-run/events') {
      res.setHeader('content-type','text/event-stream');
      const send=value=>res.write('event: chat\ndata: '+JSON.stringify(value)+'\n\n');
      send({type:'assistant_start'});
      send({type:'upstream_error',message:'Traceback: private MCP 503'});
      send({type:'tool_progress',tool_call_id:'fixture-call',progress:4,total:8,message:'RuntimeError: private'});
      const timer=setTimeout(()=>{
        send({type:'failed',message:'当前步骤在执行前被拒绝；请查看追踪',public_notice:{code,message:'private diagnostic'},artifacts:[],response:partial});
        res.end();timers.delete(timer);
      },1000);
      timers.add(timer);res.on('close',()=>{clearTimeout(timer);timers.delete(timer);});return;
    }
    const turn={id:'turn',ordinal:1,prompt:'Create something',attachments:[],answer:persisted?partial.answer:null,response:persisted?partial:null,run_id:'notice-run',run_status:persisted?'failed':'running',created_at:1,updated_at:1};
    const routes={'/v1/agent-profile':profile,'/v1/sessions':[session],'/v1/sessions/notice-session':{...session,turns:[turn],notices:[],history_page:{has_more:false}},'/v1/chat/configuration':{model_display_name:'Fixture'},'/v1/speech-input/configuration':{available:false}};
    res.setHeader('content-type','application/json');res.end(JSON.stringify(routes[path]??{}));
  } catch {res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let chrome;
try {
  chrome=await browser();
  for(const language of ['zh-CN','en']) {
    persisted=false;
    await chrome.send('Page.addScriptToEvaluateOnNewDocument',{source:`localStorage.setItem('exoharness.ui-language',${JSON.stringify(language)});`});
    await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/chat?session=notice-session&case=${language}`});
    await until(()=>chrome.evaluate('!!document.body && document.body.innerText.includes("13×24=312")'),'streamed partial answer is retained');
    assert.ok(!await chrome.evaluate('/Traceback|RuntimeError|private|MCP 503|查看追踪/.test(document.body.innerText)'));
    assert.equal(await chrome.evaluate('document.querySelectorAll(".chat-message.assistant").length'),1,'partial answer has one assistant card');
    persisted=true;
    const before=await chrome.evaluate('performance.timeOrigin');
    await chrome.send('Page.reload');
    await until(()=>chrome.evaluate(`performance.timeOrigin!==${before} && document.readyState==='complete' && document.body.innerText.includes("13×24=312")`),'partial answer survives refresh');
    assert.equal(await chrome.evaluate('document.querySelectorAll(".chat-message.assistant").length'),1);
    for(const width of [1440,390]) {
      await chrome.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:width<600});
      assert.ok(await chrome.evaluate('document.body.innerText.includes("13×24=312")'));
    }
  }
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS partial answer streaming, retained failed status, refresh, no duplicate cards or raw diagnostics, Chinese/English and desktop/mobile; no model or MCP calls.');
} finally {await chrome?.close();for(const timer of timers)clearTimeout(timer);server.closeAllConnections();await new Promise(done=>server.close(done));}
