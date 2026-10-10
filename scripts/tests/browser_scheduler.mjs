import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { browser, until } from './support/browser.mjs';
const root=resolve(import.meta.dirname,'../..');
let failing=false, conflict=false, writes=[];
let configuration={revision:3,max_active_runs_per_user:2,max_global_agent_runs:4,resource_pools:[{id:'gpu',name:'GPU',max_concurrency:1}],mcp_resource_assignments:{a:'gpu'},updated_at:Date.now(),mcp_capabilities:[{id:'a',name:'Alpha',enabled:true,capability_max_concurrency:1},{id:'b',name:'Beta',enabled:true,capability_max_concurrency:2}]};
const server=createServer(async(req,res)=>{
  res.setHeader('content-type','application/json');
  if(req.url==='/v1/auth/session')return res.end(JSON.stringify({role:'admin',csrf_token:'test',authentication_enabled:true}));
  if(req.url==='/v1/scheduler-configuration'){
    if(req.method==='GET'){if(failing){res.statusCode=503;return res.end(JSON.stringify({error:{message:'配置暂时不可用'}}));}return res.end(JSON.stringify(configuration));}
    let body='';for await(const part of req)body+=part;const saved=JSON.parse(body);writes.push(saved);
    if(conflict){configuration={...configuration,revision:5,max_global_agent_runs:6};res.statusCode=409;return res.end(JSON.stringify({error:{code:'state_conflict',message:'配置已变化'}}));}
    configuration={...configuration,...saved,revision:saved.revision+1,updated_at:Date.now()};return res.end(JSON.stringify(configuration));
  }
  if(req.url==='/security'){res.setHeader('content-type','text/html');return res.end(await readFile(resolve(root,'web/security.html')));}
  if(req.url?.startsWith('/assets/')&&/^[-a-z.]+$/.test(req.url.slice(8))){try{res.setHeader('content-type',req.url.endsWith('.css')?'text/css':'text/javascript');return res.end(await readFile(resolve(root,'web',req.url.slice(8))));}catch{}}
  res.statusCode=404;res.end(JSON.stringify({error:{message:'not part of scheduler fixture'}}));
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));const base=`http://127.0.0.1:${server.address().port}`;const chrome=await browser();
async function open(){await chrome.send('Page.navigate',{url:`${base}/security`});await until(()=>chrome.evaluate('document.readyState==="complete"'),'loaded security page');}
try{
  await open();await until(()=>chrome.evaluate('document.querySelector("#scheduler-state").classList.contains("is-ready")'),'real loaded state');
  assert.deepEqual(await chrome.evaluate('[document.querySelector("#scheduler-user-limit").value,document.querySelector("#scheduler-global-limit").value]'),['2','4']);
  await chrome.evaluate('document.querySelector("#scheduler-global-limit").value="1000"');
  for (const width of [1280, 390]) {
    await chrome.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:width<600});
    const size=await chrome.evaluate('(()=>{const input=document.querySelector("#scheduler-global-limit");return {width:input.getBoundingClientRect().width,value:input.value,overflow:document.documentElement.scrollWidth>innerWidth}})()');
    assert.ok(size.width>=160, `global limit input too narrow: ${size.width}`);
    assert.equal(size.value,'1000');assert.equal(size.overflow,false);
  }
  await chrome.send('Emulation.clearDeviceMetricsOverride');
  assert.equal(await chrome.evaluate('document.querySelectorAll(".mcp-resource-row").length'),2);
  assert.equal(await chrome.evaluate('document.querySelector("select[data-capability-id=a]").value'),'gpu');
  await chrome.evaluate('document.querySelector("#scheduler-user-limit").value="3";document.querySelector("#scheduler-global-limit").value="8";document.querySelector("#scheduler-form").requestSubmit()');
  await until(()=>Promise.resolve(writes.length===1),'save submitted');
  await until(()=>chrome.evaluate('!document.querySelector("#scheduler-save").disabled'),'save complete');
  assert.equal(writes[0].revision,3);assert.equal(writes[0].max_active_runs_per_user,3);assert.equal(writes[0].max_global_agent_runs,8);assert.deepEqual(writes[0].mcp_resource_assignments,{a:'gpu'});
  conflict=true;await chrome.evaluate('document.querySelector("#scheduler-form").requestSubmit()');
  await until(()=>chrome.evaluate('document.querySelector("#scheduler-global-limit").value==="6" && !document.querySelector("#scheduler-save").disabled'),'conflict refresh');
  failing=true;await open();await until(()=>chrome.evaluate('document.querySelector("#scheduler-state").classList.contains("is-warning")'),'load failed');
  assert.equal(await chrome.evaluate('document.querySelector("#scheduler-save").disabled'),true);
  assert.equal(await chrome.evaluate('document.querySelector("#scheduler-user-limit").value'),'');
  const count=writes.length;await chrome.evaluate('document.querySelector("#scheduler-form").dispatchEvent(new Event("submit",{cancelable:true}))');
  await new Promise(done=>setTimeout(done,100));assert.equal(writes.length,count,'failed read must not save fake values');
  assert.deepEqual(chrome.errors,[]);console.log('PASS scheduler page: loaded values, resource assignments, save revision, conflict refresh, failed load prevents writes');
}finally{await chrome.close();await new Promise(done=>server.close(done));}
