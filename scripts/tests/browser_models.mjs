// Original model page -> durable management APIs. No provider request or charge.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {browser,until} from './support/browser.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const data=execFileSync('python3',[resolve(root,'scripts/local-service.py'),'data-path'],{encoding:'utf8'}).trim();
const credentials=JSON.parse(await readFile(resolve(data,'credentials.json'),'utf8'));
const admin=credentials.find(item=>item.id==='admin').token;
const name=`browser-model-${Date.now()}`,provider=`browser-provider-${Date.now()}`;
let chrome;
try {
  chrome=await browser();const {send,evaluate}=chrome;
  await send('Page.navigate',{url:'http://127.0.0.1:9797/exoharness-login'});
  await until(()=>evaluate("document.readyState==='complete'&&!!document.querySelector('#login-password')"),'model test login');
  await evaluate(`document.querySelector('#login-password').value=${JSON.stringify(admin)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>evaluate("location.pathname==='/overview'&&document.body.dataset.authRole==='admin'"),'administrator session');
  await send('Page.navigate',{url:'http://127.0.0.1:9797/models'});
  await until(()=>evaluate("document.readyState==='complete'&&document.body.dataset.authRole==='admin'&&document.querySelector('#model-list').textContent.includes('LLM')"),'original models loaded');
  await evaluate(`document.querySelector('#new-model').click();
    document.querySelector('#model-name').value=${JSON.stringify(name)};
    document.querySelector('#model-display-name').value='Mock disabled model';
    document.querySelector('#provider-name').value=${JSON.stringify(provider)};
    document.querySelector('#provider-base-url').value='https://example.test/v1';
    document.querySelector('#provider-api-key').value='browser-private-fixture-key';
    document.querySelector('#model-enabled').checked=false;
    document.querySelector('#model-config-form').requestSubmit()`);
  await until(()=>evaluate(`!!document.querySelector('.model-item[data-name="${name}"]')`),'original save button persists model');
  const read=()=>evaluate(`(async()=>{const catalog=await(await window.ExoAuth.fetch('/v1/model-catalog')).json();const model=catalog.models.find(item=>item.name===${JSON.stringify(name)});const provider=catalog.providers.find(item=>item.name===${JSON.stringify(provider)});return {model,provider,catalogText:JSON.stringify(catalog)}})()`);
  let value=await read();assert.equal(value.model.enabled,false);assert.equal(value.provider.api_key_configured,true);assert.ok(!value.catalogText.includes('browser-private-fixture-key'));
  await evaluate(`document.querySelector('.model-item[data-name="${name}"]').click();document.querySelector('#model-temperature').value='0.5';document.querySelector('#provider-api-key').value='';document.querySelector('#model-config-form').requestSubmit()`);
  await until(async()=> (await read()).model.temperature===0.5,'editing current model preserves stored key');
  await send('Page.reload');
  await until(()=>evaluate(`document.readyState==='complete'&&!!document.querySelector('.model-item[data-name="${name}"]')`),'durable model refresh');
  value=await read();assert.equal(value.provider.api_key_configured,true);assert.equal(value.model.temperature,0.5);
  await evaluate(`window.confirm=()=>true;document.querySelector('.model-item[data-name="${name}"] [data-action="model-delete"]').click()`);
  await until(()=>evaluate(`!document.querySelector('.model-item[data-name="${name}"]')`),'original delete action');
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS original model page: save disabled fixture, no key echo, edit existing configuration, refresh, delete. No enabled production models changed and no provider called.');
} finally {
  if(chrome) {
    await chrome.evaluate(`(async()=>{const catalog=await(await window.ExoAuth.fetch('/v1/model-catalog')).json();
      const call=async(path)=>{const response=await window.ExoAuth.fetch(path,{method:'DELETE',headers:{'content-type':'application/json'},body:JSON.stringify({confirm:true})});if(!response.ok)throw new Error('model fixture cleanup failed')};
      if(catalog.models.some(item=>item.name===${JSON.stringify(name)}))await call('/v1/models/'+encodeURIComponent(${JSON.stringify(name)}));
      if(catalog.providers.some(item=>item.name===${JSON.stringify(provider)}))await call('/v1/model-providers/'+encodeURIComponent(${JSON.stringify(provider)}));
    })()`);
    await chrome.close();
  }
}
