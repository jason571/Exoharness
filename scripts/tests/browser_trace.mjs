// Read-only real-browser acceptance of an existing failed run, no model calls.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {browser,until} from './support/browser.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const data=execFileSync('python3',[resolve(root,'scripts/local-service.py'),'data-path'],{encoding:'utf8'}).trim();
const admin=JSON.parse(await readFile(resolve(data,'credentials.json'),'utf8')).find(item=>item.id==='admin').token;
const session=process.argv[2],run=process.argv[3];
assert.ok(session&&run,'provide an existing session and run');
const chrome=await browser();
try {
  await chrome.send('Page.navigate',{url:'http://127.0.0.1:9797/exoharness-login'});
  await until(()=>chrome.evaluate("document.readyState==='complete'&&!!document.querySelector('#login-password')"),'login page');
  await chrome.evaluate(`document.querySelector('#login-password').value=${JSON.stringify(admin)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>chrome.evaluate("location.pathname==='/overview'&&document.body.dataset.authRole==='admin'"),'administrator session');
  await chrome.send('Page.navigate',{url:`http://127.0.0.1:9797/trace?session=${encodeURIComponent(session)}&run=${encodeURIComponent(run)}`});
  await until(()=>chrome.evaluate(`document.querySelector('#trace-run-id')?.textContent===${JSON.stringify(run)}&&!document.querySelector('#trace-content')?.hidden`),'trace detail');
  assert.equal(await chrome.evaluate("document.querySelector('#trace-status').textContent"),'失败');
  await chrome.evaluate("document.querySelector('.trace-flow-node[data-run-outcome]').click()");
  await until(()=>chrome.evaluate("document.querySelector('#trace-detail-title')?.textContent==='运行失败'"),'failed node detail');
  assert.ok(await chrome.evaluate("document.querySelector('#trace-result').textContent.includes('ModuleNotFoundError')"),'saved failure diagnostic');
  await chrome.evaluate("document.querySelector('.trace-flow-node[data-model-call-id][data-focus=call]').click()");
  await until(()=>chrome.evaluate("document.querySelector('#trace-request-summary')?.textContent.includes('逻辑输入可核验')"),'model input evidence');
  assert.ok(await chrome.evaluate("document.querySelector('#trace-request-part-select')?.options.length>1 || document.querySelector('#trace-request-part')?.options.length>1"),'request evidence parts');
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS real 9797 Trace browser: failed user run, failure diagnostics, model input evidence; no generation or replay.');
} catch(error) {
  console.log('trace browser diagnostic',await chrome.evaluate("({path:location.pathname,label:document.querySelector('#service-label')?.textContent,run:document.querySelector('#trace-run-id')?.textContent,resources:performance.getEntriesByType('resource').filter(x=>x.name.includes('/v1/')).map(x=>({url:x.name,duration:x.duration})),state:typeof state==='undefined'?null:{loading:state.loading,sessions:state.sessions.length,selected:state.selectedSessionId,run:state.selectedRunId}})"),chrome.errors);
  throw error;
} finally {await chrome.close();}
