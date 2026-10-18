// Copyright (C) 2026 Jason Yang | SPDX-License-Identifier: AGPL-3.0-only
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';
const root=resolve(import.meta.dirname,'../../web');
const apps=[];const keys=new Map();const writes=[];const conversationReads=[];let profiles=[{id:'coder',display_name:'Coder',enabled:true,is_default:true,access:{published:true}}];
const conversations=Array.from({length:23},(_,i)=>({id:'session-'+i,client_id:'app_a',assistant_id:'carenest',user_id:'harness-user',external_user_id:'local-user',user_label:'flyang',device_id:i===22?'watch-b':'watch-a',title:i===0?'<img src=x onerror=window.__injected=true>':'Conversation '+i,turn_count:12,updated_at:1000+i}));
const server=createServer(async(req,res)=>{try{
  const url=new URL(req.url,'http://localhost');let raw='';for await(const chunk of req)raw+=chunk;const body=raw?JSON.parse(raw):null;
  const json=value=>{res.setHeader('content-type','application/json');res.end(JSON.stringify(value));};
  if(req.method!=='GET'){writes.push({path:url.pathname,method:req.method,csrf:req.headers['x-exoharness-csrf']});assert.equal(req.headers['x-exoharness-csrf'],'fixture-csrf');}
  if(url.pathname==='/v1/auth/session')return json({role:'admin',authentication_enabled:true,csrf_token:'fixture-csrf'});
  if(url.pathname==='/v1/agent-profiles'){if(req.method==='POST'){profiles.push({...body,is_default:false});res.statusCode=201;return json(body);}return json(profiles);}
  if(url.pathname==='/v1/management/external-applications'){
    if(req.method==='POST'){const app={...body,revision:1};apps.push(app);const secret='fixture-application-key-'+app.client_id+'-'.repeat(32);keys.set(app.client_id,secret);res.statusCode=201;return json({application:app,client_secret:secret});}
    return json({applications:apps});
  }
  if(url.pathname==='/v1/management/external-conversations'){
    assert.ok(Number(url.searchParams.get('limit'))<=10);assert.ok(url.searchParams.get('client_id'));conversationReads.push(url.searchParams.get('client_id'));let rows=conversations.filter(s=>apps.some(a=>a.client_id===s.client_id));
    const app=url.searchParams.get('client_id'),device=url.searchParams.get('device'),user=url.searchParams.get('user'),q=url.searchParams.get('q');
    if(app)rows=rows.filter(s=>s.client_id===app);if(device)rows=rows.filter(s=>s.device_id.includes(device));if(user)rows=rows.filter(s=>[s.user_label,s.user_id,s.external_user_id].some(v=>v.includes(user)));if(q)rows=rows.filter(s=>s.title.includes(q)||s.id.includes(q));
    const page=Number(url.searchParams.get('page')||1),offset=(page-1)*10;return json({sessions:rows.slice(offset,offset+10),page,page_size:10,total:rows.length,has_more:offset+10<rows.length});
  }
  const history=url.pathname.match(/^\/v1\/management\/external-applications\/([^/]+)\/sessions\/(.+)$/);
  if(history){assert.ok(Number(url.searchParams.get('turn_limit'))<=10);const item=conversations.find(s=>s.client_id===history[1]&&s.id===history[2]);assert.ok(item);let turns=Array.from({length:12},(_,i)=>({id:'turn-'+i,ordinal:i+1,prompt:i===2?'<img src=x onerror=window.__injected=true>':'Private prompt '+(i+1),answer:'Reply '+(i+1),run_status:'succeeded',response:{artifacts:i===11?[{filename:'safe.docx',download_url:'https://media.example/result?signature=fixture'},{filename:'unsafe',download_url:'javascript:alert(1)'}]:[]}}));const before=Number(url.searchParams.get('before_ordinal')||13);turns=turns.filter(t=>t.ordinal<before);const more=turns.length>10;turns=turns.slice(-10);return json({id:item.id,title:item.title,external:item,turns,history_page:{has_more:more,before_ordinal:turns[0]?.ordinal||null}});}
  const match=url.pathname.match(/^\/v1\/management\/external-applications\/([^/]+)(\/key|\/grants)?$/);
  if(match){const app=apps.find(a=>a.client_id===match[1]);assert.ok(app);if(match[2]==='/key'){assert.equal(body.confirm,true);return json({client_id:app.client_id,client_secret:keys.get(app.client_id)});}if(match[2]==='/grants')return json({grants:[]});if(req.method==='DELETE'){if(body.expected_revision!==app.revision){res.statusCode=409;return json({message:'Application revision changed'});}apps.splice(apps.indexOf(app),1);keys.delete(app.client_id);res.statusCode=204;return res.end();}if(req.method==='PUT'){assert.equal(body.expected_revision,app.revision);Object.assign(app,body.application,{revision:app.revision+1});let secret='';if(body.rotate_secret){secret='rotated-key-'+app.client_id+'-'.repeat(32);keys.set(app.client_id,secret);}return json({application:app,client_secret:secret});}}
  const name=url.pathname==='/external-applications'?'external-applications.html':url.pathname==='/external-conversations'?'external-conversations.html':url.pathname.startsWith('/assets/')?url.pathname.slice(8):null;if(!name)throw Error('unknown route');const file=resolve(root,name);assert.ok(file.startsWith(root+'/'));res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':name.endsWith('.json')?'application/json':'text/html;charset=utf-8');res.end(await readFile(file));
}catch(error){res.statusCode=500;res.end(JSON.stringify({message:error.message}));}});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const chrome=await browser();
try{
  await chrome.send('Page.addScriptToEvaluateOnNewDocument',{source:"localStorage.setItem('exoharness.management.theme','light');localStorage.setItem('exoharness.ui-language','zh-CN');window.confirm=()=>true;"});
  await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/external-applications`});
  await until(()=>chrome.evaluate('document.querySelector("#external-assistant")?.options.length===1 && !document.querySelector("#external-save").disabled'),'application form');
  assert.ok(await chrome.evaluate("Array.from(document.querySelectorAll('.main-nav a')).some(a=>a.getAttribute('href')==='/external-applications' && a.getAttribute('aria-current')==='page')"));
  await chrome.evaluate('document.querySelector("#external-template").click()');
  await until(()=>chrome.evaluate('document.querySelector("#external-assistant").value==="carenest" && !document.querySelector("#external-save").disabled'),'CareNest assistant template');
  await chrome.evaluate('document.querySelector("#external-client").value="app_a";document.querySelector("#external-name").value="<img src=x onerror=window.__injected=true>";document.querySelector("#external-form").requestSubmit();');
  await until(()=>chrome.evaluate('document.querySelector("#external-key-dialog").open && !document.querySelector("#external-save").disabled'),'generated key');
  const keyA=await chrome.evaluate('document.querySelector("#external-issued-secret").value');assert.equal(keyA,keys.get('app_a'));
  assert.equal(await chrome.evaluate('getComputedStyle(document.querySelector("#external-key-dialog")).backgroundColor'),'rgb(255, 255, 255)');
  assert.equal(await chrome.evaluate('!!window.__injected || !!document.querySelector("#external-list img")'),false);
  await chrome.evaluate('document.querySelector("#external-key-close").click()');assert.equal(await chrome.evaluate('document.querySelector("#external-issued-secret").value'),'');
  await chrome.send('Page.reload');await until(()=>chrome.evaluate('document.querySelector("#external-list")?.textContent.includes("app_a") && !document.querySelector("#external-save").disabled'),'saved application after reload');
  assert.equal(await chrome.evaluate('document.querySelector("#external-issued-secret").value'),'');
  profiles.push({id:'xiaojia',display_name:'小嘉',enabled:true,is_default:false,access:{published:true}},{id:'xiaoan',display_name:'小安',enabled:true,is_default:false,access:{published:false}});
  await chrome.evaluate('document.querySelector("#external-refresh").click()');
  await until(()=>chrome.evaluate('document.querySelector("#external-assistant").options.length===4 && !document.querySelector("#external-save").disabled'),'all four enabled assistants, including unpublished XiaoAn');
  assert.ok(await chrome.evaluate('Array.from(document.querySelector("#external-assistant").options).some(o=>o.value==="xiaoan" && o.textContent==="小安")'));
  const originalSecret=keys.get('app_a');
  await chrome.evaluate('document.querySelector("#external-list [data-action=edit]").click()');
  await until(()=>chrome.evaluate('document.querySelector("#external-form-title").textContent==="编辑应用" && !document.querySelector("#external-save").disabled'),'explicit edit action');
  assert.equal(await chrome.evaluate('document.querySelector("#external-client").disabled'),true);
  const editSave=chrome.evaluate('document.querySelector("#external-assistant").value="xiaoan";document.querySelector("#external-access-ttl").value="600";document.querySelector("#external-form").requestSubmit()');
  await chrome.send('Page.handleJavaScriptDialog',{accept:true}).catch(()=>{});await editSave;
  await until(()=>chrome.evaluate('document.querySelector("#external-message").textContent==="应用已保存" && !document.querySelector("#external-save").disabled'),'edited application saved');
  assert.equal(apps[0].assistant_id,'xiaoan');assert.equal(apps[0].access_ttl_seconds,600);assert.equal(apps[0].require_device_id,true);assert.equal(keys.get('app_a'),originalSecret);
  assert.ok((await chrome.evaluate('document.querySelector("#external-list").textContent')).includes('小安'));
  await chrome.evaluate('Array.from(document.querySelectorAll("#external-list button")).find(b=>b.textContent.includes("查看 Key")).click()');
  await until(()=>chrome.evaluate('document.querySelector("#external-key-dialog").open'),'administrator key reveal');assert.equal(await chrome.evaluate('document.querySelector("#external-issued-secret").value'),keyA);
  await chrome.evaluate('document.querySelector("#external-key-close").click();document.querySelector("#external-new").click()');
  await until(()=>chrome.evaluate('!document.querySelector("#external-client").disabled && !document.querySelector("#external-save").disabled'),'new second application');
  await chrome.evaluate('document.querySelector("#external-client").value="app_b";document.querySelector("#external-name").value="Second app";document.querySelector("#external-form").requestSubmit();');
  await until(()=>chrome.evaluate('document.querySelector("#external-key-dialog").open && !document.querySelector("#external-save").disabled'),'second independent key');
  const keyB=keys.get('app_b');assert.notEqual(keyB,keyA);await chrome.evaluate('document.querySelector("#external-key-close").click()');
  // Dismiss the translated native confirmation through CDP.
  const rotate=chrome.evaluate('document.querySelector("#external-rotate").click()');await chrome.send('Page.handleJavaScriptDialog',{accept:true}).catch(()=>{});await rotate;
  await until(()=>chrome.evaluate('document.querySelector("#external-key-dialog").open'),'key rotation');assert.notEqual(keys.get('app_b'),keyB);assert.equal(keys.get('app_a'),keyA);
  await chrome.evaluate('document.querySelector("#external-key-close").click()');
  // Ordinary edits must preserve transport policy and legacy callbacks.
  const appB=apps.find(a=>a.client_id==='app_b');appB.allow_backend_identity=false;appB.redirect_uris=['https://legacy.example/callback'];
  await chrome.evaluate('document.querySelector("#external-refresh").click()');
  await until(()=>chrome.evaluate('!document.querySelector("#external-save").disabled'),'refresh legacy backend policy');
  const policySave=chrome.evaluate('document.querySelector("#external-name").value="Legacy second app";document.querySelector("#external-form").requestSubmit()');
  await chrome.send('Page.handleJavaScriptDialog',{accept:true}).catch(()=>{});await policySave;
  await until(()=>chrome.evaluate('document.querySelector("#external-message").textContent==="应用已保存" && !document.querySelector("#external-save").disabled'),'preserved legacy backend policy');
  assert.equal(appB.allow_backend_identity,false);assert.deepEqual(appB.redirect_uris,['https://legacy.example/callback']);
  profiles.find(p=>p.id==='carenest').enabled=false;
  const roleWrites=writes.filter(w=>w.path==='/v1/agent-profiles').length;
  await chrome.evaluate('document.querySelector("#external-template").click()');
  await until(()=>chrome.evaluate('document.querySelector("#external-message").textContent.includes("尚未启用")'),'existing inactive assistant is preserved');
  assert.equal(writes.filter(w=>w.path==='/v1/agent-profiles').length,roleWrites);
  assert.equal(profiles.find(p=>p.id==='carenest').enabled,false);
  assert.equal(await chrome.evaluate('!!document.querySelector("#external-conversations") || !!document.querySelector("#external-backend-identity") || !!document.querySelector("#external-redirects") || !!document.querySelector("#external-require-device")'),false,'application management excludes history and transport/policy switches');
  assert.equal(apps.find(a=>a.client_id==='app_a').allow_backend_identity,true);
  assert.equal(apps.find(a=>a.client_id==='app_a').require_device_id,true);
  assert.deepEqual(apps.find(a=>a.client_id==='app_a').redirect_uris,[]);
  await chrome.evaluate('document.querySelector("#external-list a[href*=app_a]").click()');
  await until(()=>chrome.evaluate('document.querySelectorAll("#external-conversation-rows tr").length===10 && !document.querySelector("#external-conversation-search").disabled'),'first conversation page');
  assert.equal(await chrome.evaluate('document.querySelector("#external-conversation-total").textContent'),'23');
  await chrome.evaluate('document.querySelector("#external-conversation-next").click()');await until(()=>chrome.evaluate('document.querySelector("#external-conversation-page").textContent==="2" && !document.querySelector("#external-conversation-search").disabled'),'second conversation page');
  await chrome.evaluate('document.querySelector("#external-conversation-next").click()');await until(()=>chrome.evaluate('document.querySelectorAll("#external-conversation-rows tr").length===3'),'last conversation page');
  assert.ok(await chrome.evaluate('document.querySelector("#external-conversation-next").disabled'));
  await chrome.evaluate('document.querySelector("#external-conversation-device").value="watch-b";document.querySelector("#external-conversation-filter").requestSubmit()');await until(()=>chrome.evaluate('document.querySelectorAll("#external-conversation-rows tr").length===1 && !document.querySelector("#external-conversation-search").disabled'),'device search');
  assert.ok((await chrome.evaluate('document.querySelector("#external-conversation-rows").innerText')).includes('local-user'));
  await chrome.evaluate('document.querySelector("#external-conversation-rows button").click()');await until(()=>chrome.evaluate('document.querySelectorAll(".external-conversation-turn").length===10 && !document.querySelector("#external-conversation-latest").disabled'),'ten turns in conversation dialog');
  assert.equal(await chrome.evaluate('getComputedStyle(document.querySelector("#external-conversation-dialog")).backgroundColor'),'rgb(255, 255, 255)');
  assert.equal(await chrome.evaluate('!!window.__injected || !!document.querySelector("#external-conversation-content img")'),false);
  assert.equal(await chrome.evaluate('document.querySelectorAll("#external-conversation-content a").length'),1);
  await chrome.evaluate('document.querySelector("#external-conversation-older").click()');await until(()=>chrome.evaluate('document.querySelectorAll(".external-conversation-turn").length===2'),'older conversation page');
  assert.ok(await chrome.evaluate('document.querySelector("#external-conversation-older").disabled'));
  await chrome.evaluate('document.querySelector("#external-conversation-close").click();document.querySelector("#external-conversation-device").value="";document.querySelector("#external-conversation-user").value="nonexistent";document.querySelector("#external-conversation-filter").requestSubmit()');await until(()=>chrome.evaluate('document.querySelectorAll("#external-conversation-rows tr").length===0 && !document.querySelector("#external-conversation-search").disabled'),'empty user search');
  await chrome.send('Runtime.evaluate',{expression:"window.dispatchEvent(new StorageEvent('storage',{key:'exoharness.ui-language',newValue:'en'}));"});
  await until(()=>chrome.evaluate('document.querySelector("h1").textContent==="External conversations"'),'English interface');
  assert.ok((await chrome.evaluate('document.body.innerText')).includes('Back to applications'));
  for(const width of [1440,390]){await chrome.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:width<600});assert.ok(await chrome.evaluate('document.documentElement.scrollWidth<=innerWidth+1'),'responsive width');}
  assert.ok(await chrome.evaluate('!JSON.stringify(localStorage).includes("fixture-application-key") && !JSON.stringify(localStorage).includes("rotated-key")'));
  assert.ok(conversationReads.every(client=>client==='app_a'));
  assert.equal(await chrome.evaluate('document.querySelector("#external-conversation-app").tagName'),'INPUT','no all-application selector');
  await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/external-conversations?client_id=app_b`});
  await until(()=>chrome.evaluate('document.querySelector("#external-conversation-application-name")?.textContent.includes("app_b") && !document.querySelector("#external-conversation-search").disabled'),'second application history page');
  assert.equal(await chrome.evaluate('document.querySelectorAll("#external-conversation-rows tr").length'),0,'no first application conversations on second application page');
  assert.equal(conversationReads.at(-1),'app_b');
  await chrome.send('Page.reload');
  await until(()=>chrome.evaluate('document.querySelector("#external-conversation-application-name")?.textContent.includes("app_b")'),'refresh keeps application scope');
  await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/external-conversations`});
  await until(()=>chrome.evaluate('document.querySelector("#external-conversation-note")?.textContent.includes("选择应用")'),'missing application does not load all conversations');
  assert.equal(conversationReads.at(-1),'app_b');
  await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/external-applications`});
  await until(()=>chrome.evaluate('document.querySelectorAll("#external-list [data-action=edit]").length===2 && !document.querySelector("#external-save").disabled'),'two application edit/delete actions');
  await chrome.evaluate('document.querySelectorAll("#external-list [data-action=edit]")[1].click()');
  await until(()=>chrome.evaluate('document.querySelector("#external-client").value==="app_b" && !document.querySelector("#external-save").disabled'),'select second application for deletion');
  // A concurrent edit must stop deletion, without discarding the selected form.
  apps.find(a=>a.client_id==='app_b').revision++;
  const staleDelete=chrome.evaluate('document.querySelectorAll("#external-list [data-action=delete]")[1].click()');
  await chrome.send('Page.handleJavaScriptDialog',{accept:true}).catch(()=>{});await staleDelete;
  await until(()=>chrome.evaluate('document.querySelector("#external-message").textContent==="Application revision changed" && !document.querySelector("#external-save").disabled'),'stale revision delete rejected');
  assert.equal(apps.length,2);
  await chrome.evaluate('document.querySelector("#external-refresh").click()');
  await until(()=>chrome.evaluate('!document.querySelector("#external-save").disabled'),'refresh delete revision');
  const deleteWrites=writes.filter(w=>w.method==='DELETE').length;
  await chrome.evaluate('window.confirm=()=>false;document.querySelectorAll("#external-list [data-action=delete]")[1].click()');
  await until(()=>chrome.evaluate('!document.querySelector("#external-save").disabled'),'cancel application deletion');
  assert.equal(writes.filter(w=>w.method==='DELETE').length,deleteWrites);assert.equal(apps.length,2);
  await chrome.evaluate('window.confirm=()=>true;document.querySelectorAll("#external-list [data-action=delete]")[1].click()');
  await until(()=>chrome.evaluate('document.querySelector("#external-message").textContent==="应用已删除" && !document.querySelector("#external-save").disabled'),'application deleted');
  assert.equal(apps.length,1);assert.equal(apps[0].client_id,'app_a');assert.equal(keys.get('app_a'),keyA);
  assert.equal(await chrome.evaluate('document.querySelector("#external-client").value'),'');
  assert.equal(await chrome.evaluate('document.querySelector("#external-issued-secret").value'),'');
  await chrome.send('Page.reload');
  await until(()=>chrome.evaluate('document.querySelectorAll("#external-list [data-action=delete]").length===1 && !document.querySelector("#external-save").disabled'),'deletion survives reload');
  assert.equal(await chrome.evaluate('document.querySelector("#external-list").textContent.includes("app_b")'),false);
  await chrome.send('Runtime.evaluate',{expression:"window.dispatchEvent(new StorageEvent('storage',{key:'exoharness.ui-language',newValue:'en'}));"});
  await until(()=>chrome.evaluate('document.querySelector("#external-list [data-action=edit]").textContent==="Edit"'),'English edit/delete actions');
  assert.equal(await chrome.evaluate('document.querySelector("#external-list [data-action=delete]").textContent'),'Delete');
  assert.ok((await chrome.evaluate('document.querySelector("#external-list").textContent')).includes('小安'));
  assert.ok(writes.length>=4);assert.equal(chrome.errors.length,0);
  console.log('PASS: external conversation search, users/devices, ten-row pagination, bounded turn history, file links, safe content, application menu, CareNest role template, four assistants including unpublished profiles, assistant display names, explicit editing without key rotation, deletion/cancel/stale revision/refresh, key generation/storage/reveal/rotation, two independent applications, CSRF, XSS-safe names, light theme, bilingual desktop/mobile and no credential browser persistence.');
}finally{await chrome.close();await new Promise(resolve=>server.close(resolve));}
