// Original assistant page -> persistent CRUD and avatars. No provider or tool runs.
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
const id=`browser-assistant-${Date.now()}`;
const png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aot0AAAAASUVORK5CYII=';
let chrome;
try {
  chrome=await browser();const {send,evaluate}=chrome;
  await send('Page.navigate',{url:'http://127.0.0.1:9797/exoharness-login'});
  await until(()=>evaluate("document.readyState==='complete'&&!!document.querySelector('#login-password')"),'assistant test login');
  await evaluate(`document.querySelector('#login-password').value=${JSON.stringify(admin)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>evaluate("location.pathname==='/overview'&&document.body.dataset.authRole==='admin'"),'administrator session');
  await send('Page.navigate',{url:'http://127.0.0.1:9797/agents'});
  await until(()=>evaluate("document.readyState==='complete'&&document.body.dataset.authRole==='admin'&&!!document.querySelector('#agent-profile-list [data-profile-id]')"),'original assistant page loaded');
  await until(()=>evaluate("document.querySelector('#agent-id').readOnly"),'initial assistant detail loaded');
  await evaluate(`(()=>{const fetch=window.ExoAuth.fetch.bind(window.ExoAuth);window.assistantTestResponses=[];window.ExoAuth.fetch=async(path,options)=>{const response=await fetch(path,options);if(path.startsWith('/v1/agent')){window.assistantTestResponses.push({method:options?.method||'GET',path,status:response.status});window.assistantTestResponses=window.assistantTestResponses.slice(-20)}return response}})()`);
  const count=await evaluate("document.querySelectorAll('#agent-profile-list [data-profile-id]').length");
  await evaluate(`document.querySelector('#new-agent').click();document.querySelector('#agent-id').value=${JSON.stringify(id)};document.querySelector('#agent-display-name').value='Browser assistant fixture';document.querySelector('#agent-description').value='Disabled browser fixture';document.querySelector('#agent-system-prompt').value='Fixture instructions only.';document.querySelector('#agent-enabled').checked=false;document.querySelector('#agent-published').checked=false;document.querySelector('#agent-profile-form').requestSubmit()`);
  const read=()=>evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/agent-profiles/'+encodeURIComponent(${JSON.stringify(id)}));if(response.status===404)return null;if(!response.ok)throw new Error('assistant fixture read failed');return response.json()})()`);
  await until(async()=>!!await read(),'original create action');
  await until(()=>evaluate(`!!document.querySelector('[data-profile-id="${id}"]')`),'created record rendered');
  await until(()=>evaluate("!document.querySelector('#save-agent-profile').disabled"),'create handler completed');
  let value=await read();assert.equal(value.id,id);assert.equal(value.active_revision,1);assert.equal(value.enabled,false);assert.equal(value.is_default,false);
  assert.equal(await evaluate("document.querySelectorAll('#agent-profile-list [data-profile-id]').length"),count+1);
  // Select current record, then exercise the actual file input and save handler.
  await evaluate(`document.querySelector('#new-agent').click();document.querySelector('[data-profile-id="${id}"]').click()`);
  await until(()=>evaluate(`document.querySelector('#agent-id').value===${JSON.stringify(id)}&&document.querySelector('#agent-id').readOnly`),'selected existing identity');
  await evaluate(`const bytes=Uint8Array.from(atob(${JSON.stringify(png)}),c=>c.charCodeAt(0));const transfer=new DataTransfer();transfer.items.add(new File([bytes],'avatar.png',{type:'image/png'}));const input=document.querySelector('#agent-avatar-file');input.files=transfer.files;input.dispatchEvent(new Event('change',{bubbles:true}));document.querySelector('#agent-display-name').value='Updated fixture';document.querySelector('#agent-profile-form').requestSubmit()`);
  await until(async()=>{const value=await read();return value?.active_revision===2&&!!value.avatar},'avatar updates current assistant');
  value=await read();assert.equal(value.id,id);assert.equal(value.display_name,'Updated fixture');const avatar=value.avatar;
  assert.equal(await evaluate("document.querySelectorAll('#agent-profile-list [data-profile-id]').length"),count+1);
  await send('Page.reload');
  await until(()=>evaluate(`document.readyState==='complete'&&!!document.querySelector('[data-profile-id="${id}"] img')`),'saved avatar survives page refresh');
  await until(()=>evaluate("document.querySelector('#agent-id').readOnly"),'refreshed initial detail loaded');
  await evaluate(`(()=>{const fetch=window.ExoAuth.fetch.bind(window.ExoAuth);window.assistantTestResponses=[];window.ExoAuth.fetch=async(path,options)=>{const response=await fetch(path,options);if(path.startsWith('/v1/agent')){window.assistantTestResponses.push({method:options?.method||'GET',path,status:response.status});window.assistantTestResponses=window.assistantTestResponses.slice(-20)}return response}})()`);
  await until(()=>evaluate(`(()=>{const image=document.querySelector('[data-profile-id="${id}"] img');return !!image&&image.complete&&image.naturalWidth>0})()`),'saved image is actually decoded');
  value=await read();assert.deepEqual(value.avatar,avatar);assert.equal(value.active_revision,2);
  assert.equal(await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/agent-assets/'+${JSON.stringify(avatar.asset_id)});return response.ok&&response.headers.get('content-type')==='image/png'&&(await response.arrayBuffer()).byteLength>0})()`),true);
  await evaluate(`document.querySelector('[data-profile-id="${id}"]').click()`);
  await until(()=>evaluate(`document.querySelector('#agent-id').value===${JSON.stringify(id)}`),'selected for delete');
  await evaluate("window.confirm=()=>true;document.querySelector('#agent-profile-actions-panel [data-action=delete]').click()");
  await until(async()=>await read()===null,'original delete action');
  await until(()=>evaluate(`!document.querySelector('[data-profile-id="${id}"]')`),'deleted record removed from list');
  assert.equal(await evaluate("document.querySelectorAll('#agent-profile-list [data-profile-id]').length"),count);
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS original assistant page: create disabled fixture, update same ID, avatar upload and reload, delete; existing assistants unchanged. No capability executed.');
} catch(error) {
  if(chrome)console.log(await chrome.evaluate(`({fixturePresent:!!document.querySelector('[data-profile-id="${id}"]'),listCount:document.querySelectorAll('#agent-profile-list [data-profile-id]').length,toast:document.querySelector('#toast')?.textContent,responses:window.assistantTestResponses})`));
  throw error;
} finally {
  if(chrome) {
    await chrome.evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/agent-profiles/'+encodeURIComponent(${JSON.stringify(id)}));if(response.ok){const removed=await window.ExoAuth.fetch('/v1/agent-profiles/'+encodeURIComponent(${JSON.stringify(id)}),{method:'DELETE',headers:{'content-type':'application/json'},body:JSON.stringify({confirm:true})});if(!removed.ok)throw new Error('assistant fixture cleanup failed')}})()`);
    await chrome.close();
  }
}
