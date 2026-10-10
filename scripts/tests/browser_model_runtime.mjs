// Original test button -> deployed Core model runtime -> local protocol server.
// No external provider/model, real credentials, sandbox code or paid request.
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
const name=`browser-runtime-${Date.now()}`,provider=`browser-runtime-provider-${Date.now()}`;
let calls=0,chrome,baseline;const protocolErrors=[];
const server=createServer(async(request,response)=>{
  try {
  if(request.url!=='/v1/chat/completions'||request.method!=='POST') {response.writeHead(404);return response.end();}
  const chunks=[];for await(const chunk of request)chunks.push(chunk);
  const value=JSON.parse(Buffer.concat(chunks).toString('utf8'));
  assert.equal(value.model,'browser-fixture');assert.equal(value.stream,false);
  assert.equal(value.max_tokens,128);assert.equal(value.messages.length,1);assert.equal(value.tools,undefined);
  assert.equal(value.top_k,40);
  assert.equal(request.headers.authorization,'Bearer browser-runtime-fixture-key');calls++;
  response.writeHead(200,{'content-type':'application/json'});
  response.end(JSON.stringify({id:'local-browser-response',choices:[{message:{role:'assistant',content:'OK'},finish_reason:'stop'}],usage:{prompt_tokens:5,completion_tokens:1}}));
  } catch {
    protocolErrors.push('local diagnostic request did not match the protocol fixture');
    response.writeHead(500,{'content-type':'application/json'});response.end('{}');
  }
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}/v1`;
try {
  chrome=await browser();const {send,evaluate}=chrome;
  await send('Page.navigate',{url:'http://127.0.0.1:9797/exoharness-login'});
  await until(()=>evaluate("document.readyState==='complete'&&!!document.querySelector('#login-password')"),'runtime test login');
  await evaluate(`document.querySelector('#login-password').value=${JSON.stringify(admin)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>evaluate("location.pathname==='/overview'&&document.body.dataset.authRole==='admin'"),'administrator session');
  baseline=await evaluate("(async()=>{const response=await window.ExoAuth.fetch('/v1/model-catalog');if(!response.ok)throw new Error('baseline read failed');return response.json()})()");
  assert.ok(baseline.default_model,'isolated catalog has a default model to restore');
  await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/model-configuration',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({api_key:'browser-runtime-fixture-key',provider:{name:${JSON.stringify(provider)},api:'openai_compatible',model_type:'llm',base_url:${JSON.stringify(base)},enabled:true},model:{name:${JSON.stringify(name)},provider:${JSON.stringify(provider)},model_id:'browser-fixture',display_name:'Local runtime fixture',model_type:'llm',enabled:true,max_tokens:128,top_k:40,thinking:'none'}})});if(!response.ok)throw new Error('local fixture configuration failed')})()`);
  await send('Page.navigate',{url:'http://127.0.0.1:9797/models'});
  await until(()=>evaluate(`document.readyState==='complete'&&!!document.querySelector('.model-item[data-name="${name}"] [data-action=model-test]')`),'fixture test action loaded');
  await evaluate(`document.querySelector('.model-item[data-name="${name}"] [data-action=model-test]').click()`);
  await until(()=>evaluate("document.querySelector('#toast').textContent.includes('测试通过：Local runtime fixture')||document.querySelector('#toast').textContent.includes('测试通过：browser-runtime-')"),'original model test reports actual success');
  assert.equal(calls,1);assert.deepEqual(protocolErrors,[]);assert.deepEqual(chrome.errors,[]);
  console.log('PASS original model test action -> deployed typed Core runtime -> actual local HTTP completion; exactly one request, no external provider or paid generation.');
} finally {
  try {
    if(chrome&&baseline) {
      await chrome.evaluate(`(async()=>{const call=async(path,method,body)=>{const response=await window.ExoAuth.fetch(path,{method,headers:{'content-type':'application/json'},body:JSON.stringify(body)});if(!response.ok)throw new Error('runtime fixture cleanup failed')};const catalog=await(await window.ExoAuth.fetch('/v1/model-catalog')).json();if(catalog.models.some(item=>item.name===${JSON.stringify(name)})){await call('/v1/models/'+encodeURIComponent(${JSON.stringify(baseline.default_model)})+'/default','PUT',{});await call('/v1/models/'+encodeURIComponent(${JSON.stringify(name)}),'DELETE',{confirm:true})}if(catalog.providers.some(item=>item.name===${JSON.stringify(provider)}))await call('/v1/model-providers/'+encodeURIComponent(${JSON.stringify(provider)}),'DELETE',{confirm:true})})()`);
      const after=await chrome.evaluate("(async()=> (await window.ExoAuth.fetch('/v1/model-catalog')).json())()");
      assert.deepEqual(after,baseline,'all imported models and defaults restored');
    }
  } finally {await chrome?.close();await new Promise(resolve=>server.close(resolve));}
}
