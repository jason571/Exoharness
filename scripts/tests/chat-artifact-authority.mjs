// Actual bundled Chat with sample metadata. No model/MCP calls or real credentials.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';
const root=resolve(import.meta.dirname,'../../web');
const profile={id:'fixture',revision:1,display_name:'Fixture',avatar_text:'F',digest:'fixture'};
const session={id:'fixture-session',title:'Declared video only',pinned:false,created_at:1,updated_at:1,turn_count:2,latest_run_status:'succeeded',latest_run_id:'fixture-run',managed_coding:false,agent_profile:profile};
let origin;
const requests=[];
const server=createServer(async(req,res)=>{
  const path=new URL(req.url,'http://localhost').pathname;requests.push(req.url);
  try{
    if(path==='/chat'){
      let html=await readFile(resolve(root,'portal-chat.html'),'utf8');
      html=html.replace('<script src="/assets/portal-auth.js" defer></script>','<script>window.ExoAuth={ready:Promise.resolve(),fetch:(...args)=>fetch(...args),account:{username:"fixture"}};</script>');
      res.setHeader('content-type','text/html');res.end(html);return;
    }
    if(path.startsWith('/assets/')){
      const name=path.slice(8);if(!/^[a-zA-Z0-9_.-]+$/.test(name))throw new Error('Invalid asset');
      res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'application/octet-stream');res.end(await readFile(resolve(root,name)));return;
    }
    if(path.endsWith('.mp4')){res.writeHead(404).end();return;}
    const card={kind:'video',filename:'final.mp4',content_type:'video/mp4',url:origin+'/final.mp4?signature=fixture',download_url:origin+'/final.mp4?signature=fixture'};
    const answers=[`Completed.\n\nVideo: [final.mp4](${origin}/final.mp4)`, `An ordinary reference: [other video](${origin}/reference.mp4)`];
    const turns=answers.map((answer,index)=>({id:'turn-'+index,ordinal:index+1,prompt:'Sample',attachments:[],answer,response:{answer,artifacts:index===0?[card]:[],usage:{total_tokens:0},tool_calls:[],warnings:[]},run_id:'run-'+index,run_status:'succeeded',created_at:1,updated_at:1}));
    const routes={'/v1/agent-profile':profile,'/v1/sessions':[session],'/v1/sessions/fixture-session':{...session,turns,notices:[],history_page:{has_more:false,before_ordinal:null,limit:20}},'/v1/chat/configuration':{model_display_name:'Fixture'},'/v1/speech-input/configuration':{available:false}};
    res.setHeader('content-type','application/json');res.end(JSON.stringify(routes[path]??{}));
  }catch{res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));origin=`http://127.0.0.1:${server.address().port}`;
let chrome;
try{
  chrome=await browser();
  for(let iteration=0;iteration<2;iteration++){
    if(iteration===0)await chrome.send('Page.navigate',{url:origin+'/chat?session=fixture-session'});
    else await chrome.send('Page.reload');
    await until(()=>chrome.evaluate('document.querySelectorAll(".chat-message.assistant").length===2'),'persisted answers');
    const state=await chrome.evaluate('({videos:document.querySelectorAll("video").length,src:document.querySelector("video")?.src,download:document.querySelector(".chat-video-artifact .video-download-link")?.href,links:[...document.querySelectorAll(".rich-content a")].map(a=>a.href)})');
    assert.equal(state.videos,1);assert.equal(state.src,origin+'/final.mp4?signature=fixture');assert.equal(state.download,state.src);
    assert.ok(!state.links.includes(origin+'/final.mp4'));assert.ok(state.links.includes(origin+'/reference.mp4'));
  }
  assert.ok(!requests.includes('/final.mp4'));assert.ok(!requests.includes('/reference.mp4'));
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS history and reload: only declared video card, exact signed URL, redundant unsigned link hidden; unrelated prose link remains ordinary. No model or MCP calls.');
}finally{await chrome?.close();await new Promise(done=>server.close(done));}
