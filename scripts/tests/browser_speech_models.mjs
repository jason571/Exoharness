// Original admin test buttons, real local HTTP protocols; no external speech API.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {browser,until} from './support/browser.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const data=execFileSync('python3',[resolve(root,'scripts/local-service.py'),'data-path'],{encoding:'utf8'}).trim();
const admin=JSON.parse(await readFile(resolve(data,'credentials.json'),'utf8')).find(item=>item.id==='admin').token;
let chrome,baseline;const fixtures=[],calls={asr:0,tts:0},errors=[];
const server=createServer(async(req,res)=>{
  try {
    const chunks=[];for await(const chunk of req)chunks.push(chunk);const bytes=Buffer.concat(chunks);
    assert.equal(req.method,'POST');
    if(req.url==='/v1/audio/transcriptions') {
      assert.ok(req.headers['content-type'].startsWith('multipart/form-data; boundary='));
      assert.equal(req.headers.authorization,'Bearer local-speech-fixture-key');
      assert.ok(bytes.includes(Buffer.from('RIFF')));assert.ok(bytes.length>32044);calls.asr++;
      res.writeHead(200,{'content-type':'application/json'});res.end('{"text":""}');
    } else if(req.url==='/synthesize') {
      assert.equal(req.headers['x-api-key'],'local-speech-fixture-key');
      assert.equal(req.headers['x-api-resource-id'],'speech-fixture-resource');
      assert.ok(req.headers['x-api-request-id']);const value=JSON.parse(bytes);
      assert.equal(value.req_params.speaker,'configured-fixture-voice');assert.equal(value.max_tokens,undefined);calls.tts++;
      res.writeHead(200,{'content-type':'text/event-stream'});
      res.end('data: {"code":0,"data":"SUQz"}\n\ndata: {"code":20000000}\n\n');
    } else {throw new Error('Unexpected fixture route')}
  } catch {errors.push('speech fixture protocol mismatch');res.writeHead(500,{'content-type':'application/json'});res.end('{}')}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const origin=`http://127.0.0.1:${server.address().port}`;
async function post(path,method,body) {
  return chrome.evaluate(`(async()=>{const response=await window.ExoAuth.fetch(${JSON.stringify(path)},{method:${JSON.stringify(method)},headers:{'content-type':'application/json'},body:JSON.stringify(${JSON.stringify(body)})});if(!response.ok)throw new Error('speech fixture configuration failed');return response.status===204?null:response.json()})()`);
}
try {
  chrome=await browser();
  await chrome.send('Page.navigate',{url:'http://127.0.0.1:9797/exoharness-login'});
  await until(()=>chrome.evaluate("document.readyState==='complete'&&!!document.querySelector('#login-password')"),'speech model login');
  await chrome.evaluate(`document.querySelector('#login-password').value=${JSON.stringify(admin)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>chrome.evaluate("location.pathname==='/overview'&&document.body.dataset.authRole==='admin'"),'speech admin session');
  baseline=await chrome.evaluate("(async()=> (await window.ExoAuth.fetch('/v1/model-catalog')).json())()");
  for(const kind of ['asr','tts']) {
    const name=`speech-${kind}-${Date.now()}`,provider=`provider-${name}`;fixtures.push({name,provider,kind});
    const model={name,provider,model_id:'speech-fixture-resource',display_name:'Local speech fixture',model_type:kind,enabled:true,max_tokens:1};
    if(kind==='tts')model.tts={profile_id:'local-profile',speaker:'configured-fixture-voice'};
    await post('/v1/model-configuration','POST',{api_key:'local-speech-fixture-key',provider:{name:provider,model_type:kind,api:kind==='tts'?'volcengine_tts_sse':'openai_transcription',base_url:origin+(kind==='tts'?'/synthesize':'/v1'),enabled:true},model});
    await chrome.send('Page.navigate',{url:'http://127.0.0.1:9797/models'});
    await until(()=>chrome.evaluate(`document.readyState==='complete'&&!!document.querySelector('.model-item[data-name="${name}"] [data-action=model-test]')`),'original speech test button');
    await chrome.evaluate(`document.querySelector('.model-item[data-name="${name}"] [data-action=model-test]').click()`);
    await until(()=>chrome.evaluate(`document.querySelector('#toast').textContent.includes(${JSON.stringify(kind==='tts'?'TTS 测试通过':'ASR 转录接口通过')})`),'original speech test success');
    assert.equal(calls[kind],1);
  }
  assert.deepEqual(errors,[]);assert.deepEqual(chrome.errors,[]);
  console.log('PASS original TTS and ASR model buttons -> typed diagnostics -> real local SSE audio/multipart WAV; max_tokens=1 does not block speech; each exactly one request.');
} finally {
  try {
    if(chrome&&baseline) {
      for(const fixture of fixtures.reverse()) {
        const current=await chrome.evaluate("(async()=> (await window.ExoAuth.fetch('/v1/model-catalog')).json())()");
        const previous=baseline.models.find(model=>model.model_type===fixture.kind&&model.enabled);
        if(current.models.some(model=>model.name===fixture.name)) {
          if(previous)await post('/v1/models/'+encodeURIComponent(previous.name)+'/default','PUT',{});
          await post('/v1/models/'+encodeURIComponent(fixture.name),'DELETE',{confirm:true});
        }
        if(current.providers.some(provider=>provider.name===fixture.provider))await post('/v1/model-providers/'+encodeURIComponent(fixture.provider),'DELETE',{confirm:true});
      }
      const after=await chrome.evaluate("(async()=> (await window.ExoAuth.fetch('/v1/model-catalog')).json())()");
      assert.deepEqual(after,baseline,'all original enabled models/defaults/provider projections restored');
    }
  } finally {await chrome?.close();await new Promise(resolve=>server.close(resolve));}
}
