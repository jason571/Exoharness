// Actual Chat bundle with an existing published production MP3. Pass a card
// JSON path as argv[2]; this test never invokes a model or generation tool.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';
const root=new URL('../../web',import.meta.url).pathname;
if(!process.argv[2])throw new Error('Pass an existing published audio card JSON file. No generation is performed.');
const card=JSON.parse(await readFile(process.argv[2],'utf8'));
assert.ok(card.url.startsWith('https://media.clhh.tech/artifacts/'));
const profile={id:'fixture',revision:1,display_name:'测试助理',avatar_text:'测',digest:'fixture'};
const session={id:'fixture-session',title:'已生成的歌曲',pinned:false,created_at:Date.now(),updated_at:Date.now(),turn_count:1,latest_run_status:'succeeded',latest_run_id:'fixture-run',managed_coding:false,parent_id:null,project_name:null,agent_profile:profile};
const turn={id:'fixture-turn',ordinal:1,prompt:'生成歌曲',attachments:[],answer:'歌曲已生成。',response:{answer:'歌曲已生成。',artifacts:[card],model:{name:'fixture',provider:'fixture',model_id:'fixture',api:'fixture'},usage:{total_tokens:0},tool_calls:[],warnings:[]},run_id:'fixture-run',run_status:'succeeded',run_error:null,created_at:Date.now(),updated_at:Date.now()};
const server=createServer(async(req,res)=>{
    const path=new URL(req.url,'http://localhost').pathname;
    try {
        if(path==='/chat') {
            let html=await readFile(resolve(root,'portal-chat.html'),'utf8');
            html=html.replace('<script src="/assets/portal-auth.js" defer></script>','<script>window.ExoAuth={ready:Promise.resolve(),fetch:(...args)=>fetch(...args),account:{username:"ui-fixture"}};</script>');
            res.setHeader('content-type','text/html');res.end(html);return;
        }
        if(path.startsWith('/assets/')) {
            const name=path.slice(8);if(name.includes('/')||name.includes('..')){res.writeHead(404).end();return;}
            res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'application/octet-stream');res.end(await readFile(resolve(root,name)));return;
        }
        res.setHeader('content-type','application/json');
        const result=path==='/v1/agent-profile'?profile:path==='/v1/sessions'?[session]:path==='/v1/sessions/fixture-session'?{...session,turns:[turn],notices:[],history_page:{has_more:false,before_ordinal:null,limit:20}}:path==='/v1/chat/configuration'?{model_display_name:'UI fixture'}:path==='/v1/speech-input/configuration'?{available:false}:{};
        res.end(JSON.stringify(result));
    }catch{res.writeHead(500).end();}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let chrome;
try {
    chrome=await browser();
    await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/chat?session=fixture-session`});
    await until(()=>chrome.evaluate("document.querySelector('audio')?.duration>0"),'real public MP3 metadata',30000);
    const state=await chrome.evaluate("(()=>{const a=document.querySelector('audio');return {duration:a.duration,src:a.src,error:a.error?.code,download:document.querySelector('.chat-audio-artifact .video-download-link')?.href}})()");
    assert.ok(Number.isFinite(state.duration)&&state.duration>120);
    assert.equal(state.src,card.url);assert.equal(state.download,card.download_url);assert.equal(state.error,undefined);
    const play=await chrome.send('Runtime.evaluate',{expression:"document.querySelector('audio').muted=true;document.querySelector('audio').play()",awaitPromise:true,returnByValue:true,userGesture:true});
    assert.equal(play.exceptionDetails,undefined,'user-initiated playback');
    await until(()=>chrome.evaluate("document.querySelector('audio').currentTime>0.1"),'audio playback advances',15000);
    await chrome.evaluate("document.querySelector('audio').currentTime=120");
    await until(()=>chrome.evaluate("document.querySelector('audio').currentTime>=120 && !document.querySelector('audio').seeking"),'range seek',15000);
    await chrome.evaluate("document.querySelector('audio').pause()");
    assert.deepEqual(chrome.errors,[]);
    console.log(JSON.stringify({passed:true,duration_seconds:state.duration,host:'media.clhh.tech',playback_advances:true,seek_seconds:120,source:'real existing song',generated_again:false}));
} catch(error) {
    console.log(JSON.stringify({errors:chrome?.errors,state:await chrome?.evaluate("(()=>{const a=document.querySelector('audio');return {present:!!a,duration:a?.duration,ready:a?.readyState,media_error:a?.error?.code,cards:document.querySelectorAll('.chat-audio-artifact').length}})()") }));
    throw error;
} finally {await chrome?.close();await new Promise(resolve=>server.close(resolve));}
