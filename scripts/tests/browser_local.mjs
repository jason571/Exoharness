// Explicit real-Chrome acceptance. Credentials stay in memory, never in URLs.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { readFile, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root=resolve(dirname(fileURLToPath(import.meta.url)),"../..");
const data=execFileSync("python3",[resolve(root,"scripts/local-service.py"),"data-path"],{encoding:"utf8"}).trim();
const credentials=JSON.parse(await readFile(resolve(data,"credentials.json"),"utf8"));
const key=credentials.find(record=>record.id==="tester2").token;
const admin=credentials.find(record=>record.id==="admin").token;
const profile=await mkdtemp(resolve(tmpdir(),"exoharness-browser-test-"));
const chrome=spawn("/usr/bin/google-chrome",["--headless=new","--no-sandbox","--disable-gpu","--disable-background-networking","--no-first-run","--no-default-browser-check","--remote-debugging-port=0",`--user-data-dir=${profile}`,"about:blank"],{stdio:"ignore"});
const pause=(ms)=>new Promise(resolve=>setTimeout(resolve,ms));
let socket;
async function until(check,label) {
  const deadline=Date.now()+8000;
  while(Date.now()<deadline){if(await check())return;await pause(80);}
  throw new Error(`browser timeout: ${label}`);
}
try {
  let port;
  await until(async()=>{try{port=Number((await readFile(resolve(profile,"DevToolsActivePort"),"utf8")).split("\n")[0]);return true;}catch{return false;}},"Chrome start");
  const target=await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`,{method:"PUT"})).json();
  socket=new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=()=>reject(new Error("CDP connection failed"));});
  let sequence=0;const pending=new Map();const errors=[];
  socket.onmessage=event=>{
    const result=JSON.parse(event.data);
    if(result.method==="Runtime.exceptionThrown")errors.push("browser runtime exception");
    if(result.id&&pending.has(result.id)){const {resolve,reject}=pending.get(result.id);pending.delete(result.id);if(result.error)reject(new Error("CDP operation failed"));else resolve(result.result);}
  };
  function send(method,params={}){return new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));});}
  async function evaluate(expression){const result=await send("Runtime.evaluate",{expression,returnByValue:true,awaitPromise:true});if(result.exceptionDetails)throw new Error("browser evaluation failed");return result.result.value;}
  await send("Page.enable");await send("Runtime.enable");
  await send("Page.navigate",{url:"http://127.0.0.1:9798/local-validation"});
  await until(()=>evaluate("!!document.querySelector('#login-form') && document.querySelector('#health').textContent.includes('就绪')"),"page ready");
  assert.equal(await evaluate("document.title.includes('ExoHarness') && !document.title.includes('Pro')"),true);
  await evaluate(`document.querySelector('#token').value=${JSON.stringify(key)};document.querySelector('#login-form').requestSubmit();`);
  await until(()=>evaluate("!document.querySelector('#workspace').hidden && document.querySelector('#identity').textContent.includes('tester2')"),"authentication");
  const count=await evaluate("document.querySelectorAll('.session').length");
  await evaluate("document.querySelector('#new-session').click()");
  await until(()=>evaluate(`document.querySelectorAll('.session').length>${count} && !document.querySelector('#run-form').hidden`),"new session");
  await evaluate("document.querySelector('#input').value='Browser state-machine evidence; no generation';document.querySelector('#run-form').requestSubmit()");
  await until(()=>evaluate("document.querySelectorAll('.run').length===1"),"persisted run");
  for(const [label,state] of [["模拟开始","running"],["暂停记录","paused"],["继续原 Run","running"],["取消记录","cancelled"]]) {
    await evaluate(`Array.from(document.querySelectorAll('.run button')).find(button=>button.textContent===${JSON.stringify(label)}).click()`);
    await until(()=>evaluate(`document.querySelector('.run strong').textContent.includes(${JSON.stringify(state)})`),label);
  }
  await evaluate("Array.from(document.querySelectorAll('.run button')).find(button=>button.textContent==='查看事件').click()");
  await until(()=>evaluate("document.querySelector('#events').textContent.includes('run_transition')"),"event details");
  assert.equal(await evaluate("document.querySelector('#token').value"),"");
  assert.equal(await evaluate("localStorage.length"),0);
  await send("Emulation.setDeviceMetricsOverride",{width:390,height:844,deviceScaleFactor:1,mobile:true});
  await pause(150);
  assert.equal(await evaluate("document.documentElement.scrollWidth<=window.innerWidth"),true);
  const screenshot=await send("Page.captureScreenshot",{format:"png"});
  await writeFile("/tmp/exoharness-local-mobile.png",Buffer.from(screenshot.data,"base64"));
  await evaluate("document.querySelector('#logout').click()");
  assert.equal(await evaluate("document.querySelector('#workspace').hidden && !document.querySelector('#login').hidden"),true);
  assert.deepEqual(errors,[]);
  console.log("PASS real Chrome: product name, login, session/run lifecycle, events, mobile layout, logout; no stored key.");
  // Test the unchanged 8787/8788 pages through the compatibility login API.
  // Business routes beyond login are not yet an accepted migration.
  await send("Emulation.clearDeviceMetricsOverride");
  await send("Page.navigate",{url:"http://127.0.0.1:9797/exoharness-login"});
  await until(()=>evaluate("document.readyState==='complete' && !!document.querySelector('#login-password')"),"original admin login page and deferred scripts");
  await evaluate(`document.querySelector('#login-password').value=${JSON.stringify(admin)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>evaluate("location.pathname==='/overview' && document.body.dataset.authRole==='admin'"),"original administrator login API");
  await until(()=>evaluate("document.querySelectorAll('#overview-metrics .metric-tile').length===5 && document.querySelector('#overview-error').hidden"),"real overview API");
  await evaluate("document.querySelector('#overview-user').value='no-matching-overview-test-user';document.querySelector('#overview-filters').requestSubmit()");
  await until(()=>evaluate("document.querySelector('#overview-status').textContent.includes('暂无执行记录') && document.querySelector('#overview-error').hidden"),"empty overview is not an error");
  await evaluate("window.__originalOverviewFetch=window.ExoAuth.fetch;window.ExoAuth.fetch=async function(path,options){if(path.startsWith('/v1/management/overview'))return new Response('{}',{status:404});return window.__originalOverviewFetch.call(this,path,options)};document.querySelector('#overview-filters').requestSubmit()");
  await until(()=>evaluate("!document.querySelector('#overview-error').hidden && document.querySelector('#overview-error').textContent.includes('暂未开放')"),"missing endpoint has a readable status");
  assert.equal(await evaluate("/404|读取失败/.test(document.querySelector('#overview-error').textContent)"),false);
  await evaluate("window.ExoAuth.fetch=window.__originalOverviewFetch;delete window.__originalOverviewFetch;document.querySelector('#overview-user').value='';document.querySelector('#overview-filters').requestSubmit()");
  await until(()=>evaluate("document.querySelector('#overview-error').hidden"),"overview recovers after a transient failure");
  console.log("PASS overview: real data, empty filter, readable missing-endpoint status, recovery.");
  await send("Page.navigate",{url:"http://127.0.0.1:9797/users"});
  await until(()=>evaluate("document.readyState==='complete' && !!window.ExoAuth && document.body.dataset.authRole==='admin'"),"user management original page");
  assert.equal(await evaluate("(async()=>{const response=await window.ExoAuth.fetch('/v1/management/users');const body=await response.json();return response.status===200 && Array.isArray(body.users) && Array.isArray(body.assistants) && Number.isInteger(body.total)})()"),true);
  await send("Page.navigate",{url:"http://127.0.0.1:9797/overview"});
  await until(()=>evaluate("document.readyState==='complete' && document.body.dataset.authRole==='admin'"),"overview return");
  const originalOverview=await readFile(resolve(root,"web/overview.html"),"utf8");
  assert.equal(await evaluate(`(async()=> (await (await fetch('/overview')).text())===${JSON.stringify(originalOverview)})()`),true);
  const managementScreenshot=await send("Page.captureScreenshot",{format:"png"});
  await writeFile("/tmp/exoharness-management-reused.png",Buffer.from(managementScreenshot.data,"base64"));
  let installedConfiguration;
  try {
    const pointer=JSON.parse(await readFile(resolve(data,"configuration/current.json"),"utf8"));
    const imported=resolve(data,"configuration",pointer.generation);
    installedConfiguration={
      catalog:JSON.parse(await readFile(resolve(imported,"models.json"),"utf8")),
      profiles:JSON.parse(await readFile(resolve(imported,"agent-profiles.json"),"utf8")),
    };
  } catch(error) {
    if(error.code!=="ENOENT")throw error;
  }
  if(installedConfiguration) {
    await send("Page.navigate",{url:"http://127.0.0.1:9797/models"});
    await until(()=>evaluate(`document.querySelectorAll('.model-item').length===${installedConfiguration.catalog.models.length}`),"imported model list");
    assert.equal(await evaluate(`document.querySelector('#model-count').textContent.includes(${JSON.stringify(String(installedConfiguration.catalog.models.length))})`),true);
    await send("Page.navigate",{url:"http://127.0.0.1:9797/agents"});
    await until(()=>evaluate(`document.querySelectorAll('#agent-profile-list .agent-profile-item').length===${installedConfiguration.profiles.profiles.length}`),"imported assistant list");
    await until(()=>evaluate("Array.from(document.querySelectorAll('#agent-profile-list img')).every(image=>image.complete && image.naturalWidth>0)"),"imported avatars");
    console.log("PASS original model/assistant pages display the imported catalogs and avatars.");
  }
  await send("Page.navigate",{url:"http://127.0.0.1:9798/login"});
  await until(()=>evaluate("document.readyState==='complete' && !!document.querySelector('#portal-login-identifier')"),"original portal login page and deferred scripts");
  await evaluate(`document.querySelector('#portal-login-identifier').value='tester2';document.querySelector('#portal-login-password').value=${JSON.stringify(key)};document.querySelector('#portal-login-form').requestSubmit()`);
  await until(()=>evaluate("!document.querySelector('#portal-login-error').classList.contains('hidden') && document.querySelector('#portal-login-error').textContent.includes('账号或密码不正确')"),"diagnostic key is not a registered account password");
  assert.equal(await evaluate("location.pathname==='/login'"),true);
  console.log("PASS management pages and persistent login; portal correctly rejects diagnostic-key login. Verified registration/account lifecycle separately tested through real PostgreSQL/API with a recording mailer; live SMTP browser signup is not yet accepted.");
} finally {
  socket?.close();chrome.kill("SIGTERM");
  await new Promise(resolve=>{if(chrome.exitCode!==null)resolve();else chrome.once("exit",resolve);});
  await rm(profile,{recursive:true,force:true});
}
