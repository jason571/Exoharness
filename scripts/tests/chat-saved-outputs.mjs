// Actual bundled Chat and HTML against a loopback fixture. UI-only acceptance:
// no model, MCP, real account, production mutation or authentication claims.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {browser,until} from './support/browser.mjs';
const root=fileURLToPath(new URL('../../web/',import.meta.url));
let origin;
const profile={id:'fixture',revision:1,display_name:'测试助理',avatar_text:'测',digest:'fixture'};
const session={id:'fixture-session',title:'失败后保存的文件',pinned:false,created_at:Date.now(),updated_at:Date.now(),turn_count:1,latest_run_status:'failed',latest_run_id:'fixture-run',managed_coding:false,parent_id:null,project_name:null,agent_profile:profile};
const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Zl1sAAAAASUVORK5CYII=','base64');
const artifact=()=>({kind:'image',filename:'saved.png',content_type:'image/png',download_url:origin+'/fixture.png'});
const turn=()=>({id:'fixture-turn',ordinal:1,prompt:'生成海报',attachments:[],answer:null,response:{artifacts:[artifact()]},run_id:'fixture-run',run_status:'failed',run_error:'Traceback: RuntimeError: private MCP 503',created_at:Date.now(),updated_at:Date.now()});
const requests=[];
const server=createServer(async(req,res)=>{
  try {
    const path=new URL(req.url,'http://localhost').pathname;requests.push(path);
    if(path==='/chat'){
      let html=await readFile(resolve(root,'portal-chat.html'),'utf8');
      html=html.replace('<script src="/assets/portal-auth.js" defer></script>','<script>window.ExoAuth={ready:Promise.resolve(),fetch:(...args)=>fetch(...args),account:{username:"ui-fixture"}};</script>');
      res.setHeader('content-type','text/html');res.end(html);return;
    }
    if(path.startsWith('/assets/')){
      const name=path.slice(8);if(name.includes('/')||name.includes('..')){res.writeHead(404).end();return;}
      res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'application/octet-stream');res.end(await readFile(resolve(root,name)));return;
    }
    if(path==='/fixture.png'){res.setHeader('content-type','image/png');res.end(png);return;}
    res.setHeader('content-type','application/json');
    const result=path==='/v1/agent-profile'?profile:path==='/v1/sessions'?[session]:path==='/v1/sessions/fixture-session'?{...session,turns:[turn()],notices:[],history_page:{has_more:false,before_ordinal:null,limit:20}}:path==='/v1/chat/configuration'?{model_display_name:'UI fixture'}:path==='/v1/speech-input/configuration'?{available:false}:{};
    res.end(JSON.stringify(result));
  } catch(error){res.writeHead(500).end();}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));origin=`http://127.0.0.1:${server.address().port}`;
let chrome;
try {
  chrome=await browser();await chrome.send('Page.navigate',{url:origin+'/chat?session=fixture-session'});
  await until(()=>chrome.evaluate("document.querySelectorAll('.chat-saved-outputs').length===1"),'saved failed output');
  assert.equal(await chrome.evaluate("document.querySelector('.chat-saved-outputs').open"),false);
  assert.ok(!await chrome.evaluate('/Traceback|RuntimeError|MCP 503|本次运行失败/.test(document.body.innerText)'));
  assert.ok(await chrome.evaluate("document.querySelector('.chat-saved-outputs summary').textContent.includes('非最终成品')"));
  assert.equal(await chrome.evaluate("document.querySelector('[data-run-id=fixture-run]').dataset.runStatus"),'failed');
  await chrome.evaluate("document.querySelector('.chat-saved-outputs summary').click()");
  await until(()=>chrome.evaluate("Array.from(document.querySelectorAll('.chat-saved-outputs img')).some(image=>image.naturalWidth===1)"),'saved image decodes');
  assert.ok(await chrome.evaluate("Array.from(document.querySelectorAll('.chat-saved-outputs a')).some(a=>a.href.includes('/fixture.png'))"));
  const before=await chrome.evaluate('performance.timeOrigin');
  await chrome.send('Page.reload');
  await until(()=>chrome.evaluate(`performance.timeOrigin!==${before} && document.readyState==='complete'`),'new document after reload');
  await until(()=>chrome.evaluate("document.querySelectorAll('.chat-saved-outputs').length===1"),'refresh preserves one saved output');
  assert.equal(await chrome.evaluate("document.querySelector('.chat-saved-outputs').open"),false);
  assert.deepEqual(chrome.errors,[]);
  assert.ok(!requests.includes('/v1/chat')&&!requests.some(path=>path.endsWith('/turns')));
  console.log('PASS actual Chat browser: failed history keeps a collapsed saved-image card, image decodes, download link retained, refresh has no duplicates; no model or MCP calls.');
} finally {await chrome?.close();await new Promise(resolve=>server.close(resolve));}
