// Real browser decoding through the running 9790 service. Locally synthesized
// codec fixtures, not an external model request or a media quality evaluation.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {browser,until} from './support/browser.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..');
const data=execFileSync('python3',[resolve(root,'scripts/local-service.py'),'data-path'],{encoding:'utf8'}).trim();
const admin=JSON.parse(await readFile(resolve(data,'credentials.json'),'utf8')).find(item=>item.id==='admin').token;
const directory=await mkdtemp(resolve(tmpdir(),'exoharness-codec-test-'));
async function api(path,method='GET',body) {
  const response=await fetch('http://127.0.0.1:9797'+path,{method,headers:{Authorization:'Bearer '+admin,'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});
  assert.ok(response.status===200||response.status===201,'local authenticated fixture request');return response.json();
}
let chrome,runPath;
try {
  execFileSync('ffmpeg',['-loglevel','error','-f','lavfi','-i','color=c=blue:s=160x90:r=12','-f','lavfi','-i','sine=frequency=440:sample_rate=44100','-t','1','-c:v','libx264','-pix_fmt','yuv420p','-c:a','aac','-movflags','+faststart',resolve(directory,'clip.mp4')]);
  execFileSync('ffmpeg',['-loglevel','error','-i',resolve(directory,'clip.mp4'),'-vn',resolve(directory,'sound.mp3')]);
  execFileSync('ffmpeg',['-loglevel','error','-i',resolve(directory,'clip.mp4'),'-frames:v','1',resolve(directory,'cover.png')]);
  const session=await api('/v1/local/sessions','POST',{});
  const base='/v1/local/sessions/'+session.session_id;
  const created=await api(base+'/runs','POST',{request_id:'codec-'+Date.now(),expected_session_version:session.version,input:'Local browser codec delivery acceptance; no model generation'});
  runPath=base+'/runs/'+created.run.id;
  const urls={};
  for(const [kind,name] of [['video','clip.mp4'],['audio','sound.mp3'],['image','cover.png']]) {
    const published=await api(runPath+'/artifacts','POST',{filename:name,content_base64:(await readFile(resolve(directory,name))).toString('base64')});
    urls[kind]=published.url;assert.ok(published.url.startsWith('http://127.0.0.1:9790/'));
    const response=await fetch(published.download_url);assert.equal(response.status,200);assert.ok(response.headers.get('content-disposition').startsWith('attachment'));
    assert.deepEqual(Buffer.from(await response.arrayBuffer()),await readFile(resolve(directory,name)));
  }
  chrome=await browser();
  await chrome.send('Page.navigate',{url:'http://127.0.0.1:9798'});
  await until(()=>chrome.evaluate("document.readyState==='complete'"),'local browser document');
  const decoded=await chrome.evaluate(`(async()=>{
    const sources=${JSON.stringify(urls)};const results={};
    for(const kind of ['video','audio','image']) {
      const element=document.createElement(kind==='image'?'img':kind);
      element.crossOrigin='anonymous';document.body.append(element);
      results[kind]=await new Promise((resolve,reject)=>{
        const timer=setTimeout(()=>reject(new Error('media decode timeout')),12000);
        element.onerror=()=>{clearTimeout(timer);reject(new Error('media decode failed'))};
        const finish=()=>{clearTimeout(timer);resolve(kind==='image'?{width:element.naturalWidth,height:element.naturalHeight}:{duration:element.duration,width:element.videoWidth||0,height:element.videoHeight||0})};
        if(kind==='image')element.onload=finish;else element.onloadeddata=finish;
        element.src=sources[kind];if(kind!=='image'){element.preload='auto';element.load()}
      });element.remove();
    }return results;
  })()`);
  assert.equal(decoded.video.width,160);assert.equal(decoded.video.height,90);
  assert.ok(decoded.video.duration>=0.9&&decoded.video.duration<2);
  assert.ok(decoded.audio.duration>=0.9&&decoded.audio.duration<2);
  assert.equal(decoded.image.width,160);assert.equal(decoded.image.height,90);
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS 9790: real Chrome decodes MP4+AAC, MP3 and PNG; downloaded bytes exactly match; all use one permanent signed file endpoint.');
} finally {
  await chrome?.close();
  try {if(runPath){const current=await api(runPath);await api(runPath,'PATCH',{expected_version:current.run.version,action:'cancel'})}}
  finally {await rm(directory,{recursive:true,force:true});}
}
