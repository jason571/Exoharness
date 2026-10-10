// Registered browser login, not an administrator cookie or frontend mock.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {browser,until} from './support/browser.mjs';
const user=process.env.EXOHARNESS_BROWSER_USER,password=process.env.EXOHARNESS_BROWSER_PASSWORD;
assert.ok(user&&password,'run via live_portal_assistant.py');
const chrome=await browser();
try {
  const {send,evaluate}=chrome;
  await send('Browser.grantPermissions',{origin:'http://127.0.0.1:9798',permissions:['audioCapture']});
  await send('Page.navigate',{url:'http://127.0.0.1:9798/login'});
  await until(()=>evaluate("document.readyState==='complete'&&!!document.querySelector('#portal-login-password')"),'registered login ready');
  await evaluate(`document.querySelector('#portal-login-identifier').value=${JSON.stringify(user)};document.querySelector('#portal-login-password').value=${JSON.stringify(password)};document.querySelector('#portal-login-form').requestSubmit()`);
  await until(()=>evaluate("location.pathname==='/chat'&&!!window.ExoAuth"),'registered portal login');
  const profile=await evaluate("(async()=>{const response=await window.ExoAuth.fetch('/v1/agent-profile');if(!response.ok)throw new Error('profile unavailable');return response.json()})()");
  assert.equal(profile.id,'xiaolan');assert.ok(profile.display_name);
  assert.equal('system_prompt' in profile,false);assert.equal('access' in profile,false);
  await until(()=>evaluate(`document.querySelector('#conversation-title')?.textContent===${JSON.stringify(profile.display_name)}`),'assigned assistant title');
  if(profile.avatar?.asset_id)await until(()=>evaluate("document.querySelector('#conversation-avatar img')?.naturalWidth>0"),'assigned avatar');
  if (!process.argv.includes('--settings-only')) {
  try {
    await until(()=>evaluate("document.querySelector('#chat-microphone')?.dataset.availability==='available'"),'real ASR availability',15000);
  } catch(error) {
    console.log(await evaluate("(async()=>({microphone:{state:document.querySelector('#chat-microphone')?.dataset.availability,title:document.querySelector('#chat-microphone')?.title},secure:window.isSecureContext,recorder:typeof MediaRecorder,media:!!navigator.mediaDevices,permission:(await navigator.permissions.query({name:'microphone'})).state,configuration:await (await window.ExoAuth.fetch('/v1/speech-input/configuration')).json()}))()"));
    throw error;
  }
  // Real configured ASR through the portal auth+CSRF wrapper. No LLM call.
  const audio=await readFile('/tmp/exoharness-final-quality/lesson-short-speech.wav');
  const result=await evaluate(`(async()=>{const bytes=Uint8Array.from(atob(${JSON.stringify(audio.toString('base64'))}),c=>c.charCodeAt(0));const response=await window.ExoAuth.fetch('/v1/speech-input',{method:'POST',headers:{'content-type':'audio/wav'},body:bytes});return {status:response.status,body:await response.json()}})()`);
  assert.equal(result.status,200);assert.ok(result.body.text.trim());
  }
  await send('Page.reload');
  await until(()=>evaluate(`document.querySelector('#conversation-title')?.textContent===${JSON.stringify(profile.display_name)}`),'assistant survives refresh');
  await send('Page.navigate',{url:'http://127.0.0.1:9798/settings#password'});
  await until(()=>evaluate("document.querySelector('#settings-version')?.textContent?.startsWith('v') && document.querySelector('#settings-username')?.textContent?.startsWith('portal_browser_')"),'settings actual version and account');
  assert.equal(await evaluate("document.querySelector('#settings-error')?.classList.contains('hidden')"),true);
  // Original forms, real authenticated API, and durable refresh. No LLM call.
  await send('Page.navigate',{url:'http://127.0.0.1:9798/settings#capabilities'});
  await until(()=>evaluate("document.querySelector('#settings-capability-list')?.textContent?.includes('尚未安装私有能力')"),'personal catalog genuine empty state');
  assert.equal(await evaluate("document.querySelector('#settings-error')?.classList.contains('hidden')"),true);
  const catalog=await evaluate("(async()=>{const r=await window.ExoAuth.fetch('/v1/user-capabilities');return {status:r.status,body:await r.json()}})()");
  assert.equal(catalog.status,200);assert.deepEqual(catalog.body.capabilities,[]);
  assert.equal(catalog.body.installation_ready,false);
  assert.equal(await evaluate("document.querySelector('#portal-mcp-install-form').hidden"),true);
  // Error state belongs to its panel, not to all subsequent panels.
  await evaluate("document.querySelector('#settings-error').textContent='前一面板错误';document.querySelector('#settings-error').classList.remove('hidden');location.hash='#memory'");
  await until(()=>evaluate("document.querySelector('#settings-memory-list')?.textContent?.includes('还没有记忆')"),'same-document panel switch');
  assert.equal(await evaluate("document.querySelector('#settings-error')?.classList.contains('hidden')"),true);
  await send('Page.navigate',{url:'http://127.0.0.1:9798/settings#knowledge'});
  await until(()=>evaluate("document.querySelector('#settings-document-list')?.textContent?.includes('还没有资料')"),'personal knowledge empty list');
  await evaluate("document.querySelector('#settings-document-title').value='Browser owned knowledge';document.querySelector('#settings-document-content').value='Personal data survives refresh';document.querySelector('#portal-knowledge-form').requestSubmit()");
  await until(()=>evaluate("document.querySelector('#settings-document-list')?.textContent?.includes('Browser owned knowledge')"),'original knowledge save');
  await send('Page.reload');
  await until(()=>evaluate("document.querySelector('#settings-document-list')?.textContent?.includes('Browser owned knowledge')"),'knowledge persisted refresh');
  assert.equal(await evaluate("document.querySelector('#settings-error')?.classList.contains('hidden')"),true);
  await send('Page.navigate',{url:'http://127.0.0.1:9798/settings#memory'});
  await until(()=>evaluate("document.querySelector('#settings-memory-list')?.textContent?.includes('还没有记忆')"),'personal memory empty list');
  await evaluate("document.querySelector('#settings-memory-content').value='Browser owned preference';document.querySelector('#settings-memory-enabled').checked=true;document.querySelector('#portal-memory-form').requestSubmit()");
  await until(()=>evaluate("document.querySelector('#settings-memory-list')?.textContent?.includes('Browser owned preference')"),'original memory save');
  await send('Page.reload');
  await until(()=>evaluate("document.querySelector('#settings-memory-list')?.textContent?.includes('Browser owned preference')"),'memory persisted refresh');
  assert.equal(await evaluate("document.querySelector('#settings-error')?.classList.contains('hidden')"),true);
  assert.deepEqual(chrome.errors,[]);
  console.log(process.argv.includes('--settings-only')
    ? 'PASS registered portal original settings: personal capability catalog, memory, knowledge, cross-panel error isolation, save and refresh; ASR explicitly outside this run.'
    : 'PASS registered portal: identity/avatar, real ASR, capability/knowledge/memory forms and persistent refresh. Chat execution remains unimplemented.');
} finally {await chrome.close();}
