// Browser contract for CSP captured from real Rust media API responses.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {browser,until} from './support/browser.mjs';

const prefix=process.argv[2];
assert.ok(prefix,'Pass EXOHARNESS_PREVIEW_CSP_FIXTURE prefix from the API tests');
const policies={};
for(const kind of ['single','project'])policies[kind]=JSON.parse(await readFile(`${prefix}-${kind}.json`,'utf8')).csp;
let forbidden=0;const landings=[];
const external=createServer((req,res)=>{
 const url=new URL(req.url,'http://localhost');
 if(url.pathname==='/forbidden')forbidden++;
 if(url.pathname==='/state')landings.push({storage:url.searchParams.get('storage'),opener:url.searchParams.get('opener')});
 res.setHeader('Content-Type','text/html');
 res.end(`<h1>Link destination</h1><script>let allowed=false;try{localStorage.setItem('test','ok');allowed=true}catch{}fetch('/state?storage='+allowed+'&opener='+(window.opener===null))</script>`);
});
await new Promise(done=>external.listen(0,'127.0.0.1',done));
const destination=`http://127.0.0.1:${external.address().port}`;
const source=createServer((req,res)=>{
 const kind=req.url.slice(1);if(!policies[kind]){res.writeHead(404).end();return;}
 res.setHeader('Content-Type','text/html');res.setHeader('Content-Security-Policy',policies[kind]);
 res.end(`<!doctype html><button id="run">Test action</button><a id="link" href="${destination}/landing" target="_blank" rel="noopener noreferrer">Open link</a><script>
 window.previewState={script:true,cookiesDenied:false,storageDenied:false,clicks:0};
 try{document.cookie}catch{previewState.cookiesDenied=true}
 try{localStorage.getItem('test')}catch{previewState.storageDenied=true}
 document.querySelector('#run').onclick=()=>previewState.clicks++;
 fetch('${destination}/forbidden').catch(()=>{});
 </script>`);
});
await new Promise(done=>source.listen(0,'127.0.0.1',done));
let chrome;
try {
 chrome=await browser();
 for(const kind of ['single','project']) {
  const url=`http://127.0.0.1:${source.address().port}/${kind}`;
  await chrome.send('Page.navigate',{url});
  await until(()=>chrome.evaluate(`Boolean(window.previewState)`),'sandboxed preview ready');
  assert.deepEqual(await chrome.evaluate('window.previewState'),{script:true,cookiesDenied:true,storageDenied:true,clicks:0});
  await chrome.evaluate(`document.querySelector('#run').click()`);
  assert.equal(await chrome.evaluate('previewState.clicks'),1);
  const before=await chrome.send('Target.getTargets');
  const expected=landings.length+1;
  const clicked=await chrome.send('Runtime.evaluate',{expression:`document.querySelector('#link').click()`,userGesture:true});
  assert.ok(!clicked.exceptionDetails);
  await until(()=>landings.length===expected,'user link opened normal destination');
  assert.deepEqual(landings.at(-1),{storage:'true',opener:'true'});
  const after=await chrome.send('Target.getTargets');
  const created=after.targetInfos.find(t=>t.type==='page'&&!before.targetInfos.some(p=>p.targetId===t.targetId));
  assert.ok(created,'a separate destination tab exists');
  await chrome.send('Target.closeTarget',{targetId:created.targetId});
  assert.equal(await chrome.evaluate('location.href'),url);
 }
 assert.equal(forbidden,0,'preview network access did not expand');
 assert.deepEqual(chrome.errors,[]);
 console.log('PASS API preview CSP: external link opens, destination has normal origin and no opener; preview cookies/storage stay inaccessible, cross-origin fetch stays blocked, scripts and controls still work.');
} finally {
 await chrome?.close();
 source.closeAllConnections();external.closeAllConnections();
 await Promise.all([new Promise(done=>source.close(done)),new Promise(done=>external.close(done))]);
}
