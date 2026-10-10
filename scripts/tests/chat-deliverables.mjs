import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';

// Interface fixture + real signed server files. No production credentials,
// generation or model calls. Keep the captured input private and outside Git.
const root=resolve(import.meta.dirname,'../../web');
assert.ok(process.env.EXOHARNESS_DELIVERY_FIXTURE,'Set EXOHARNESS_DELIVERY_FIXTURE to a private JSON with selected cards and target session.');
const live=JSON.parse(await readFile(process.env.EXOHARNESS_DELIVERY_FIXTURE,'utf8'));
const cards=Object.values(live.selected);
const profile={id:'fixture',revision:1,display_name:'测试助理',avatar_text:'测',digest:'fixture'};
const session={id:live.target.session_id,title:'已保存的成品交付验证',pinned:false,created_at:Date.now(),updated_at:Date.now(),turn_count:1,latest_run_status:'succeeded',latest_run_id:live.target.run_id,managed_coding:false,agent_profile:profile};
const turn={id:'fixture-turn',ordinal:1,prompt:'验证已完成的文件，不调用模型',attachments:[],answer:'已完成。',response:{artifacts:cards,answer:'已完成。',usage:{input_tokens:0,output_tokens:0,total_tokens:0},tool_calls:[],warnings:[]},run_id:session.latest_run_id,run_status:'succeeded',created_at:Date.now(),updated_at:Date.now()};
const requests=[];
const server=createServer(async(req,res)=>{
  try{
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
    res.setHeader('content-type','application/json');
    const result=path==='/v1/agent-profile'?profile:path==='/v1/sessions'?[session]:path===`/v1/sessions/${session.id}`?{...session,turns:[turn],notices:[],history_page:{has_more:false,before_ordinal:null,limit:20}}:path==='/v1/chat/configuration'?{model_display_name:'UI fixture'}:path==='/v1/speech-input/configuration'?{available:false}:{};
    res.end(JSON.stringify(result));
  }catch(error){res.writeHead(500).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
const origin=`http://127.0.0.1:${server.address().port}`;
let chrome;
try{
  chrome=await browser();await chrome.send('Page.navigate',{url:origin+`/chat?session=${session.id}`});
  await until(()=>chrome.evaluate('document.querySelectorAll(".chat-file-artifact").length===2'),'PPT and Word download cards');
  await until(()=>chrome.evaluate('(()=>{const v=document.querySelector(".chat-video-artifact video"),a=document.querySelector(".chat-audio-artifact audio"),i=document.querySelector(".chat-image-artifact img");return !!v&&v.videoWidth>0&&v.duration>0&&!!a&&a.duration>0&&!!i&&i.naturalWidth>0})()'),'actual public video/audio/image metadata',30000);
  const decoded=await chrome.evaluate('(()=>{const v=document.querySelector("video"),a=document.querySelector("audio");return {width:v.videoWidth,height:v.videoHeight,duration:v.duration,audioDuration:a.duration,links:[...document.querySelectorAll(".chat-file-artifact a,.video-download-link,.chat-image-artifact a")].map(a=>a.getAttribute("href"))}})()');
  assert.ok(decoded.width>0);assert.ok(decoded.height>0);assert.ok(decoded.duration>0);assert.ok(decoded.audioDuration>0);
  for(const card of cards)assert.ok(decoded.links.includes(card.download_url), `missing download ${card.filename}`);
  await chrome.send('Page.reload');
  await until(()=>chrome.evaluate('document.querySelectorAll(".chat-file-artifact").length===2 && document.querySelectorAll(".chat-audio-artifact").length===1 && document.querySelectorAll(".chat-image-artifact").length===1'),'refresh preserves cards without duplicates');
  assert.deepEqual(chrome.errors,[]);
  assert.ok(!requests.includes('/v1/chat')&&!requests.some(path=>path.endsWith('/turns')));
  console.log('PASS real bundled Chat: public server MP4 and MP3 metadata, image decode, PPT/Word downloads, refresh no duplicates; no model/generation calls.');
}catch(error){
  console.log('fixture paths',requests);
  console.log('DOM diagnostics',await chrome?.evaluate('({text:document.body.innerText.slice(-1800),cards:document.querySelectorAll(".chat-video-artifact").length,exceptions:0})'));
  throw error;
}finally{await chrome?.close();await new Promise(done=>server.close(done));}
