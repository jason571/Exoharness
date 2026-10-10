// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Reproducible screenshots of the actual UI with public sample data.
// Requires Node 22+ and /usr/bin/google-chrome. No model/MCP calls or credentials.
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './tests/support/browser.mjs';

const root=new URL('../',import.meta.url).pathname;
const output=resolve(root,'docs/assets');
const epoch=Date.UTC(2026,9,10,8);
const profile={id:'demo',revision:1,display_name:'ExoHarness Assistant',avatar_text:'EH',digest:'demo'};
const session={id:'demo-session',title:'From a request to a result',pinned:true,created_at:epoch,updated_at:epoch,turn_count:1,latest_run_status:'succeeded',latest_run_id:'demo-run',managed_coding:false,parent_id:null,project_name:null,agent_profile:profile};
const answer='## From a request to a result\n\n1. **Discover** the capabilities allowed for this user and assistant.\n2. **Execute** tools through MCP or an isolated workspace.\n3. **Keep** the conversation, progress, and invocation records.\n4. **Deliver** files with previews and download links.\n\nThis UI preview uses sample content. Configure models and capabilities to run your own tasks.';
const report='# ExoHarness sample report\n\nThis is static demonstration content, not a model-generated result.\n';
const workflow='<svg xmlns="http://www.w3.org/2000/svg" width="900" height="120" viewBox="0 0 900 120"><rect width="900" height="120" rx="18" fill="#141823"/><g font-family="sans-serif" font-size="22" text-anchor="middle" fill="#e6ebfa"><text x="110" y="68">Request</text><text x="335" y="68">Agent</text><text x="560" y="68">Tools</text><text x="785" y="68">Files</text></g><g stroke="#7d9bfa" stroke-width="3" fill="none"><path d="M182 60h74m-10-7 10 7-10 7M407 60h74m-10-7 10 7-10 7M632 60h74m-10-7 10 7-10 7"/></g></svg>';
const overview={counts:{succeeded:1},runs:1,tokens:0,generated_at:epoch,assistants:[{id:'demo',name:profile.display_name,enabled:true,skill_ids:[],access:{skills:{mode:'all'},mcp:{mode:'all'},tools:{mode:'all'},models:{mode:'all'}}}],activity:[{id:'demo-run',session_id:session.id,title:session.title,status:'succeeded',user_id:'demo-user',created_at:epoch,turn_ordinal:1,attempt:1}],user_labels:{'demo-user':'Demo user'},groups:{users:[{id:'demo-user',runs:1,succeeded:1,attention:0,tokens:0,completed_duration_ms:0}],assistants:[{id:'demo',runs:1,succeeded:1,attention:0,tokens:0,completed_duration_ms:0}]},models:[],scheduler:{active:0,global_limit:16,per_user_limit:2,pools:[],ready:true}};
const capabilities=[['skill','Document workflow'],['mcp_server','Example MCP service'],['tool','File delivery']].map(([kind,name],i)=>({id:`demo-${i}`,manifest:{name,spec:{kind}},availability:{available:true},dependencies:[]}));
let origin;
const server=createServer(async(req,res)=>{
  const path=new URL(req.url,'http://localhost').pathname;
  try {
    if(path==='/chat'||path==='/overview'){
      let html=await readFile(resolve(root,'web',path==='/chat'?'portal-chat.html':'overview.html'),'utf8');
      html=html.replace('<script src="/assets/portal-auth.js" defer></script>','<script>window.ExoAuth={ready:Promise.resolve(),fetch:(...args)=>fetch(...args),account:{username:"demo-user"}};</script>');
      res.setHeader('content-type','text/html');res.end(html);return;
    }
    if(path.startsWith('/assets/')){
      const name=path.slice(8);if(!/^[a-zA-Z0-9_.-]+$/.test(name)){res.writeHead(404).end();return;}
      res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'application/octet-stream');
      res.end(await readFile(resolve(root,'web',name)));return;
    }
    if(path==='/demo-workflow.svg'||path==='/demo-report.md'){
      res.setHeader('content-type',path.endsWith('.svg')?'image/svg+xml':'text/plain');res.end(path.endsWith('.svg')?workflow:report);return;
    }
    const artifacts=[{kind:'image',filename:'workflow.svg',content_type:'image/svg+xml',url:origin+'/demo-workflow.svg',download_url:origin+'/demo-workflow.svg'},{kind:'file',filename:'sample-report.md',content_type:'text/markdown',url:origin+'/demo-report.md',download_url:origin+'/demo-report.md'}];
    const turn={id:'demo-turn',ordinal:1,prompt:'Explain how an AI agent turns a request into a downloadable result.',attachments:[],answer,response:{answer,artifacts,model:{name:'sample',provider:'sample',model_id:'sample',api:'sample'},usage:{total_tokens:0},tool_calls:[],warnings:[]},run_id:'demo-run',run_status:'succeeded',run_error:null,created_at:epoch,updated_at:epoch};
    const routes={'/v1/agent-profile':profile,'/v1/sessions':[session],'/v1/sessions/demo-session':{...session,turns:[turn],notices:[],history_page:{has_more:false,before_ordinal:null,limit:20}},'/v1/chat/configuration':{model_display_name:'UI preview'},'/v1/speech-input/configuration':{available:false},'/v1/auth/session':{id:'demo-admin',username:'Demo administrator',role:'admin'},'/v1/management/overview':overview,'/v1/assistant-capabilities':capabilities};
    res.setHeader('content-type','application/json');res.end(JSON.stringify(routes[path]??{}));
  }catch{res.writeHead(404).end();}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
origin=`http://127.0.0.1:${server.address().port}`;
let chrome;
try{
  await mkdir(output,{recursive:true});chrome=await browser();
  await chrome.send('Emulation.setDeviceMetricsOverride',{width:1440,height:1000,deviceScaleFactor:1,mobile:false});
  await chrome.send('Page.addScriptToEvaluateOnNewDocument',{source:'localStorage.setItem("exoharness.ui-language","en");'});
  for(const [path,name,ready] of [['/chat?session=demo-session','demo-chat.png','document.querySelectorAll(".chat-file-artifact").length===1'],['/overview','demo-management.png','document.querySelectorAll("#overview-metrics .metric-tile").length===5']]){
    await chrome.send('Page.navigate',{url:origin+path});await until(()=>chrome.evaluate(ready),'demo page content');
    await chrome.evaluate('document.fonts.ready');
    await chrome.evaluate(`(()=>{const label=document.createElement('div');label.textContent='UI PREVIEW · SAMPLE DATA · NO MODEL / MCP CALLS';label.style.cssText='position:fixed;bottom:8px;right:16px;z-index:9999;font:11px sans-serif;letter-spacing:1px;background:#151923;color:#ccd6ef;padding:7px 12px;border:1px solid #526180;border-radius:8px';document.body.append(label);const messages=document.querySelector('#chat-messages');if(messages)messages.scrollTop=0;})()`);
    await until(()=>chrome.evaluate('Array.from(document.images).every(i=>i.complete)'),'demo images');
    const shot=await chrome.send('Page.captureScreenshot',{format:'png'});await writeFile(resolve(output,name),Buffer.from(shot.data,'base64'));
    console.log(`Created docs/assets/${name}`);
  }
  if(chrome.errors.length)throw new Error('Browser reported a runtime exception');
}finally{await chrome?.close();await new Promise(resolve=>server.close(resolve));}
