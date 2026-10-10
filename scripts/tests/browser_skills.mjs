import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFile,mkdtemp,mkdir,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {browser,until} from './support/browser.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const data=execFileSync('python3',[resolve(root,'scripts/local-service.py'),'data-path'],{encoding:'utf8'}).trim();
const credentials=JSON.parse(await readFile(resolve(data,'credentials.json'),'utf8'));
const admin=credentials.find(record=>record.id==='admin').token;
const fixture=await mkdtemp(resolve(tmpdir(),'exoharness-directory-fixture-'));
const name=`browser-fixture-${Date.now()}`;
const directory=resolve(fixture,name);await mkdir(directory);await mkdir(resolve(directory,'references'));
const content=`---\nname: ${name}\ndescription: Browser directory import fixture\n---\n完整说明 中文。\n`;
await writeFile(resolve(directory,'SKILL.md'),content);await writeFile(resolve(directory,'references/notes.md'),'Reference data, not executable.');
await writeFile(resolve(directory,'main.py'),"raise RuntimeError('preparation must not execute this uploaded script')\n");
const invalidDirectory=resolve(fixture,'missing-entry');await mkdir(invalidDirectory);await writeFile(resolve(invalidDirectory,'notes.md'),'No Skill entry here.');
const chrome=await browser();const {send,evaluate}=chrome;let capability;
try {
  await send('Page.navigate',{url:'http://127.0.0.1:9797/exoharness-login'});
  await until(()=>evaluate("document.readyState==='complete'&&!!document.querySelector('#login-password')"),'original administrator login');
  await evaluate(`document.querySelector('#login-password').value=${JSON.stringify(admin)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>evaluate("location.pathname==='/overview'&&document.body.dataset.authRole==='admin'"),'administrator session');
  await send('Page.navigate',{url:'http://127.0.0.1:9797/imports'});
  await until(()=>evaluate("document.readyState==='complete'&&document.body.dataset.authRole==='admin'&&!!document.querySelector('#skill-folder')"),'Skill import page');
  await evaluate(`document.querySelector('[data-import-kind=skill]').click();document.querySelector('#auto-prepare').checked=${process.argv.includes('--auto-prepare')}`);
  await evaluate("window.__skillRequests=[];const original=window.ExoAuth.fetch.bind(window.ExoAuth);window.ExoAuth.fetch=(path,...args)=>{if(typeof path==='string'&&path.startsWith('/v1/imports/skill'))window.__skillRequests.push(path);return original(path,...args)}");
  const {root:documentRoot}=await send('DOM.getDocument');
  const {nodeId}=await send('DOM.querySelector',{nodeId:documentRoot.nodeId,selector:'#skill-folder'});
  await send('DOM.setFileInputFiles',{nodeId,files:[directory]});
  await until(()=>evaluate("document.querySelector('#skill-folder').files.length===3"),'actual directory selection');
  await until(()=>evaluate(`document.querySelector('#skill-folder-label').textContent.includes(${JSON.stringify(name)})&&document.querySelector('#skill-file-list').textContent.includes('3 个文件')`),'selected directory label and file list');
  assert.deepEqual(await evaluate('window.__skillRequests'),[],'selecting a folder alone must not upload or register');
  await evaluate("document.querySelector('#skill-upload-form').requestSubmit()");
  await until(()=>evaluate(`document.querySelector('#import-result-list').textContent.includes(${JSON.stringify(name)})`),'directory preview and import');
  assert.deepEqual(await evaluate('window.__skillRequests'),['/v1/imports/skill/preview','/v1/imports/skill'],'submit sends actual preview and import exactly once');
  capability=await evaluate(`(async()=>{const records=await (await window.ExoAuth.fetch('/v1/capabilities')).json();return records.find(record=>record.manifest.name===${JSON.stringify(name)})})()`);
  assert.ok(capability?.id);
  const read=await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/capabilities/'+${JSON.stringify(capability.id)}+'/skill-instructions');return await response.json()})()`);
  assert.equal(read.content,content);
  if(process.argv.includes('--auto-prepare')) {
    await until(()=>evaluate("['ready','blocked'].includes(document.querySelector('#import-result-list [data-preparation-status]')?.dataset.preparationStatus)"),'default auto preparation result',30000);
    assert.equal(await evaluate("document.querySelector('#import-result-list [data-preparation-status]').dataset.preparationStatus"),'ready');
    capability=await evaluate(`(async()=>await (await window.ExoAuth.fetch('/v1/capabilities/'+${JSON.stringify(capability.id)})).json())()`);
    assert.equal(capability.enabled,true);
  }
  await send('Page.navigate',{url:'http://127.0.0.1:9797/services?capability='+encodeURIComponent(capability.id)});
  await until(()=>evaluate(`document.querySelector('#detail-name')?.textContent===${JSON.stringify(name)}&&document.querySelector('#python-runtime-section')&&!document.querySelector('#python-runtime-section').classList.contains('hidden')`),'script Skill detail loaded');
  assert.equal(await evaluate("document.querySelector('#toggle-enabled').disabled"),false,'missing dependency evidence must not prevent enabling Skill instructions');
  assert.equal(await evaluate("document.querySelector('#prepare-python-runtime').disabled"),false,'sandbox check cannot depend on obsolete Conda availability');
  await send('Page.navigate',{url:'http://127.0.0.1:9797/imports'});
  await until(()=>evaluate("document.readyState==='complete'&&!!document.querySelector('#skill-folder')&&document.body.dataset.authRole==='admin'"),'return to import page before refresh');
  await send('Page.reload');
  await until(()=>evaluate("document.readyState==='complete'&&document.body.dataset.authRole==='admin'&&!!document.querySelector('#skill-folder')"),'authenticated refresh');
  await evaluate("document.querySelector('[data-import-kind=skill]').click();window.__skillRequests=[];const original=window.ExoAuth.fetch.bind(window.ExoAuth);window.ExoAuth.fetch=(path,...args)=>{if(typeof path==='string'&&path.startsWith('/v1/imports/skill'))window.__skillRequests.push(path);return original(path,...args)}");
  const {root:refreshedRoot}=await send('DOM.getDocument');
  const {nodeId:refreshedInput}=await send('DOM.querySelector',{nodeId:refreshedRoot.nodeId,selector:'#skill-folder'});
  await send('DOM.setFileInputFiles',{nodeId:refreshedInput,files:[invalidDirectory]});
  await until(()=>evaluate("document.querySelector('#skill-folder').files.length===1"),'invalid directory selected, not silently ignored');
  await evaluate("document.querySelector('#skill-upload-form').requestSubmit()");
  await until(()=>evaluate("document.querySelector('#toast').textContent.includes('SKILL.md')"),'missing root entry reports a visible error');
  assert.deepEqual(await evaluate('window.__skillRequests'),['/v1/imports/skill/preview'],'rejected preview must not upload/register a partial package');
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS real Chrome original import page: real directory selection, preview/import, original instructions, refresh; no page edits.');
} catch(error) {
  console.error('Browser Skill regression failed:',error);
  throw error;
} finally {
  if(capability) {
    const path='http://127.0.0.1:9797/v1/capabilities/'+encodeURIComponent(capability.id);
    const headers={Authorization:'Bearer '+admin,'content-type':'application/json'};
    const current=await(await fetch(path,{headers})).json();
    assert.equal((await fetch(path,{method:'DELETE',headers,body:JSON.stringify({expected_revision:current.revision})})).ok,true,'fixture cleanup');
  }
  await chrome.close();await rm(fixture,{recursive:true,force:true});
}
