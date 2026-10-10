// Real original page -> deployed API -> protocol-only mock. No user code runs.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {browser,until} from './support/browser.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const data=execFileSync('python3',[resolve(root,'scripts/local-service.py'),'data-path'],{encoding:'utf8'}).trim();
const credentials=JSON.parse(await readFile(resolve(data,'credentials.json'),'utf8'));
const admin=credentials.find(record=>record.id==='admin').token;
const name=`browser-mcp-${Date.now()}`,requests=[];
const server=createServer(async(req,res)=>{
  if(req.method==='DELETE'){res.writeHead(200);res.end();return;}
  if(req.method!=='POST'){res.writeHead(405);res.end();return;}
  let body='';for await(const chunk of req)body+=chunk;
  const input=JSON.parse(body);requests.push(input.method);
  assert.equal(req.headers.authorization,'Token browser-fixture');
  if(input.id===undefined){res.writeHead(202);res.end();return;}
  let result;
  if(input.method==='initialize')result={protocolVersion:'2025-06-18',capabilities:{tools:{}},serverInfo:{name:'browser-fixture',version:'1'}};
  else if(input.method==='tools/list')result={tools:[{name:'echo',description:'Mock echo',inputSchema:{type:'object',properties:{text:{type:'string'}},required:['text']}}]};
  else throw new Error('Validation must not call business tools');
  res.writeHead(200,{'content-type':'application/json','mcp-session-id':'browser-fixture'});
  res.end(JSON.stringify({jsonrpc:'2.0',id:input.id,result}));
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const url=`http://127.0.0.1:${server.address().port}/mcp`;
let chrome,capability;
try {
  chrome=await browser();const {send,evaluate}=chrome;
  await send('Page.navigate',{url:'http://127.0.0.1:9797/exoharness-login'});
  await until(()=>evaluate("document.readyState==='complete'&&!!document.querySelector('#login-password')"),'administrator login');
  await evaluate(`document.querySelector('#login-password').value=${JSON.stringify(admin)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>evaluate("location.pathname==='/overview'&&document.body.dataset.authRole==='admin'"),'administrator session');
  await send('Page.navigate',{url:'http://127.0.0.1:9797/imports'});
  await until(()=>evaluate("document.readyState==='complete'&&document.body.dataset.authRole==='admin'&&!!document.querySelector('#mcp-json-input')"),'original import page');
  const content=JSON.stringify({mcpServers:{[name]:{type:'streamableHttp',url,headers:{Authorization:'Token browser-fixture'}}}});
  await evaluate(`document.querySelector('#auto-prepare').checked=false;const input=document.querySelector('#mcp-json-input');input.value=${JSON.stringify(content)};input.dispatchEvent(new Event('input',{bubbles:true}));document.querySelector('#mcp-upload-form').requestSubmit()`);
  await until(()=>evaluate(`document.querySelector('#import-result-list').textContent.includes(${JSON.stringify(name)})`),'original pasted JSON import');
  capability=await evaluate(`(async()=>{const records=await(await window.ExoAuth.fetch('/v1/capabilities')).json();return records.find(record=>record.manifest.name===${JSON.stringify(name)})})()`);
  assert.ok(capability?.id);assert.ok(!JSON.stringify(capability).includes('browser-fixture'));assert.deepEqual(requests,[]);
  const path='/v1/capabilities/'+encodeURIComponent(capability.id);
  const result=await evaluate(`(async()=>{
    const call=async(path,method,body)=>{const response=await window.ExoAuth.fetch(path,{method,headers:{'content-type':'application/json'},body:JSON.stringify(body)});if(!response.ok)throw new Error('MCP contract failed');return response.json()};
    await call(${JSON.stringify(path+'/permissions/network.egress')},'PUT',{decision:'allow',expected_revision:${capability.revision}});
    const prepared=await call(${JSON.stringify(path+'/preparation')},'POST',{confirm:true,expected_revision:${capability.revision}});
    const tools=await(await window.ExoAuth.fetch(${JSON.stringify(path+'/tools')})).json();
    const secrets=await(await window.ExoAuth.fetch(${JSON.stringify(path+'/secrets')})).json();return {prepared,tools,secrets};
  })()`);
  assert.equal(result.prepared.status,'ready');assert.equal(result.tools[0].inputSchema.required[0],'text');assert.equal(result.secrets[0].configured,true);
  assert.deepEqual(requests,['initialize','notifications/initialized','tools/list']);
  await send('Page.reload');await until(()=>evaluate("document.readyState==='complete'&&document.body.dataset.authRole==='admin'"),'authenticated refresh');
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS real original MCP page: JSON import, secret stripping, explicit permissions, initialize/list only, persisted Schema, refresh. No UI edits, host scripts or business tools.');
} finally {
  if(chrome&&capability){
    const path='/v1/capabilities/'+encodeURIComponent(capability.id);
    await chrome.evaluate(`(async()=>{const current=await(await window.ExoAuth.fetch(${JSON.stringify(path)})).json();const response=await window.ExoAuth.fetch(${JSON.stringify(path)},{method:'DELETE',headers:{'content-type':'application/json'},body:JSON.stringify({expected_revision:current.revision})});if(!response.ok)throw new Error('Fixture cleanup failed')})()`);
  }
  if(chrome)await chrome.close();
  await new Promise(resolve=>server.close(resolve));
}
