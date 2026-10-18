// Test-only Chrome driver. Owns the browser, profile and pending CDP requests.
import {spawn} from 'node:child_process';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
export async function until(check,label,timeout=12000) {
  const end=Date.now()+timeout;
  while(Date.now()<end) {if(await check())return;await pause(80);}
  throw new Error(`browser timeout: ${label}`);
}
export async function browser() {
  const profile=await mkdtemp(resolve(tmpdir(),'exoharness-skill-browser-'));
  const chrome=spawn('/usr/bin/google-chrome',['--headless=new','--no-sandbox','--disable-gpu','--disable-background-networking','--no-first-run','--no-default-browser-check','--remote-debugging-port=0',`--user-data-dir=${profile}`,'about:blank'],{stdio:'ignore'});
  let socket;
  async function close() {
    socket?.close();chrome.kill('SIGTERM');
    await new Promise(resolve=>{if(chrome.exitCode!==null)return resolve();chrome.once('exit',resolve);setTimeout(resolve,3000).unref();});
    if(chrome.exitCode===null) {chrome.kill('SIGKILL');await new Promise(resolve=>chrome.once('exit',resolve));}
    await rm(profile,{recursive:true,force:true,maxRetries:5,retryDelay:100});
  }
  try {
    let port;
    await until(async()=>{try{port=Number((await readFile(resolve(profile,'DevToolsActivePort'),'utf8')).split('\n')[0]);return true;}catch{return false;}},'Chrome startup');
    const target=await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`,{method:'PUT'})).json();
    socket=new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((resolve,reject)=>{socket.onopen=resolve;socket.onerror=()=>reject(new Error('CDP connection failed'));});
    let sequence=0;const pending=new Map(),errors=[];
    socket.onmessage=event=>{
      const result=JSON.parse(event.data);
      if(result.method==='Runtime.exceptionThrown')errors.push('browser runtime exception');
      if(result.id&&pending.has(result.id)){const {resolve,reject}=pending.get(result.id);pending.delete(result.id);if(result.error)reject(new Error('CDP operation failed'));else resolve(result.result);}
    };
    const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++sequence;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));});
    const evaluate=async expression=>{const result=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(result.exceptionDetails)throw new Error('browser evaluation failed');return result.result.value;};
    await send('Page.enable');await send('Runtime.enable');
    return {send,evaluate,errors,close};
  } catch(error) {await close();throw error;}
}
