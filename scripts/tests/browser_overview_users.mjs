// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';

const root=resolve(import.meta.dirname,'../../web');
const owner='user_f4aa8823baa3553f3e7408a1b49d11fb';
const escaped='user_escape_fixture';
let nickname='在水一方 · flyang';
const queries=[];
function overview(query) {
  const name=query.get('user')||'';
  queries.push(name);
  const empty=name==='missing';
  return {generated_at:Date.now(),counts:{succeeded:empty?0:2},runs:empty?0:2,tokens:100,
    assistants:[],models:[],groups:{users:empty?[]:[{id:owner,runs:1,tokens:50,succeeded:1,attention:0,completed_duration_ms:1000},{id:escaped,runs:1,tokens:50,succeeded:1,attention:0,completed_duration_ms:1000}],assistants:[]},
    user_labels:{[owner]:nickname,[escaped]:'<img src=x onerror="window.__injected=true"> · safe'},
    activity:empty?[]:[{id:'run-fixture',session_id:'session-fixture',title:'Username regression',status:'succeeded',user_id:owner,created_at:Date.now(),turn_ordinal:1,attempt:1}],
    scheduler:{ready:true,active:0,global_limit:16,per_user_limit:2,pools:[]}};
}
const server=createServer(async(req,res)=>{
  try {
    const url=new URL(req.url,'http://localhost');
    if(url.pathname==='/v1/auth/session'){res.setHeader('content-type','application/json');res.end(JSON.stringify({role:'admin',authentication_enabled:true}));}
    else if(url.pathname==='/v1/management/overview'){res.setHeader('content-type','application/json');res.end(JSON.stringify(overview(url.searchParams)));}
    else if(url.pathname==='/v1/assistant-capabilities'){res.setHeader('content-type','application/json');res.end('[]');}
    else {
      const name=url.pathname==='/overview'?'overview.html':url.pathname.startsWith('/assets/')?url.pathname.slice(8):null;
      if(!name)throw new Error('unknown route');
      const path=resolve(root,name);assert.ok(path.startsWith(root+'/'));
      res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html;charset=utf-8');
      res.end(await readFile(path));
    }
  }catch {res.statusCode=404;res.end();}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const chrome=await browser();
try {
  await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/overview`});
  await until(()=>chrome.evaluate('document.querySelector("#overview-activity")?.innerText.includes("flyang")'),'human account label');
  assert.ok((await chrome.evaluate('document.querySelector("#overview-usage").innerText')).includes('在水一方 · flyang'));
  assert.ok(!(await chrome.evaluate('document.body.innerText')).includes(owner),'opaque owner ID must not replace known label');
  assert.equal(await chrome.evaluate('!!document.querySelector("#overview-usage img") || !!window.__injected'),false,'labels must be text, not executable markup');
  nickname='新的昵称 · flyang';
  await chrome.evaluate('document.querySelector("#overview-refresh").click()');
  await until(()=>chrome.evaluate('document.querySelector("#overview-activity").innerText.includes("新的昵称")'),'refreshed label');
  await chrome.evaluate('const input=document.querySelector("#overview-user");input.value="flyang";input.dispatchEvent(new Event("input",{bubbles:true}));');
  await until(async()=>queries.includes('flyang')&&await chrome.evaluate('document.querySelector("#overview-status").innerText.includes("flyang")'),'username filter');
  await chrome.evaluate('document.querySelector("#overview-user").value="missing";document.querySelector("#overview-filters").requestSubmit();');
  await until(()=>chrome.evaluate('document.querySelector("#overview-status").innerText.includes("暂无执行记录")'),'empty filtered result');
  await chrome.evaluate('document.querySelector("#overview-user").value="";document.querySelector("#overview-filters").requestSubmit();');
  await until(()=>chrome.evaluate('document.querySelector("#overview-activity").innerText.includes("flyang")'),'cleared filter');
  for(const width of [1440,390]) {
    await chrome.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:width<600});
    assert.ok(await chrome.evaluate('document.querySelector("#overview-activity").innerText.includes("flyang")'));
  }
  assert.equal(chrome.errors.length,0);
  console.log('PASS: overview activity and usage account labels, text-only rendering, rename refresh, username filter, empty/cleared filters, desktop/mobile and no runtime errors.');
}finally {await chrome.close();await new Promise(resolve=>server.close(resolve));}
