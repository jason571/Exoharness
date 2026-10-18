// Actual password-recovery assets; isolated API responses, no real email or account changes.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';

const root=resolve(import.meta.dirname,'../../web');
const resetToken='test-reset-token-not-a-production-credential';
const calls=[];
let emailMode='success',linkMode='valid',completeMode='success';
const invalid={error:{code:'invalid_password_reset',message:'重置链接已失效或已使用，请重新申请'}};
const server=createServer(async(req,res)=>{
  const path=new URL(req.url,'http://localhost').pathname;
  if(path.startsWith('/v1/auth/password-reset') && req.method==='POST') {
    let body='';for await(const chunk of req)body+=chunk;
    const input=JSON.parse(body);calls.push({path,input});
    res.setHeader('content-type','application/json');
    let status=200,payload;
    if(path==='/v1/auth/password-reset') {
      if(emailMode==='unknown'){status=404;payload={error:{code:'email_not_registered',message:'该邮箱尚未注册，请先注册'}};}
      else if(emailMode==='failure'){status=503;payload={error:{code:'email_delivery_failed',message:'重置邮件未能发送，请稍后重试'}};}
      else {status=202;payload={message:'重置链接已发送，请在20分钟内打开邮件设置新密码',expires_in:1200,resend_after:1};}
    } else if(path.endsWith('/validate')) {
      if(linkMode!=='valid'||input.token!==resetToken){status=400;payload=invalid;}
      else payload={valid:true,expires_in:1200};
    } else if(completeMode==='expired') {status=400;payload=invalid;}
    else if(completeMode==='failure'){status=503;payload={error:{code:'temporary',message:'暂时无法处理，请稍后重试'}};}
    else {payload={message:'密码已重置，请使用新密码登录'};linkMode='used';}
    res.writeHead(status).end(JSON.stringify(payload));return;
  }
  try {
    const pages={'/login':'portal-login.html','/forgot-password':'portal-forgot-password.html','/reset-password':'portal-reset-password.html'};
    const name=pages[path] || (path.startsWith('/assets/')?path.slice(8):null);
    if(!name||!/^[\w.-]+$/.test(name))throw new Error('not found');
    res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html;charset=utf-8');
    res.end(await readFile(resolve(root,name)));
  }catch {res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let chrome;
try {
  chrome=await browser();
  async function navigate(path) {
    const previous=await chrome.evaluate('performance.timeOrigin');
    await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}${path}`});
    await until(()=>chrome.evaluate(`performance.timeOrigin!==${previous} && document.readyState==='complete'`),'page loaded');
  }
  async function visibleError(text) {
    await until(()=>chrome.evaluate(`!document.querySelector('#password-error').classList.contains('hidden') && document.querySelector('#password-error').textContent===${JSON.stringify(text)}`),'visible error');
  }
  async function fill(password,confirmation=password) {
    await chrome.evaluate(`document.querySelector('#reset-password-new').value=${JSON.stringify(password)};document.querySelector('#reset-password-confirm').value=${JSON.stringify(confirmation)};document.querySelector('#reset-password-submit').click()`);
  }
  for(const language of ['zh-CN','en']) {
    const en=language==='en';
    await chrome.send('Page.addScriptToEvaluateOnNewDocument',{source:`localStorage.setItem('exoharness.ui-language',${JSON.stringify(language)});localStorage.setItem('exoharness.portal-theme','light');`});
    await navigate('/login');
    assert.equal(await chrome.evaluate('new URL(document.querySelector("#portal-forgot-password-link").href).pathname'),'/forgot-password');
    await navigate('/forgot-password');
    assert.equal(await chrome.evaluate('document.querySelector("p").textContent.includes("20")'),true);
    const start=calls.length;
    await chrome.evaluate('document.querySelector("#forgot-password-submit").click()');
    assert.equal(calls.length,start,'invalid email does not submit');
    emailMode='unknown';
    await chrome.evaluate('document.querySelector("#forgot-password-email").value="sample@example.com";document.querySelector("#forgot-password-submit").click()');
    await visibleError(en?'This email is not registered. Please register first.':'该邮箱尚未注册，请先注册');
    assert.equal(await chrome.evaluate('document.querySelector("#forgot-password-submit").disabled'),false);
    emailMode='failure';
    await chrome.evaluate('document.querySelector("#forgot-password-submit").click()');
    await visibleError(en?'The reset email could not be sent. Please try again later.':'重置邮件未能发送，请稍后重试');
    assert.equal(await chrome.evaluate('document.querySelector("#forgot-password-submit").disabled'),false);
    emailMode='success';
    await chrome.evaluate('document.querySelector("#forgot-password-submit").click()');
    await until(()=>chrome.evaluate('!document.querySelector("#password-message").classList.contains("hidden")'),'reset link sent');
    assert.equal(await chrome.evaluate('document.querySelector("#forgot-password-submit").disabled'),true);
    await until(()=>chrome.evaluate(`document.querySelector('#forgot-password-submit').textContent===${JSON.stringify(en?'Resend in 1 sec':'1 秒后重发')}`),'localized resend countdown');
    assert.equal(await chrome.evaluate('document.querySelector("#password-message").textContent.includes("20")'),true);
    await until(()=>chrome.evaluate('!document.querySelector("#forgot-password-submit").disabled'),'resend countdown');
    assert.equal(await chrome.evaluate('document.querySelector("#forgot-password-submit").textContent'),en?'Send again':'重新发送');
    assert.equal(calls.length,start+3);
    assert.deepEqual(calls.at(-1).input,{email:'sample@example.com'});
    const beforeMissing=calls.length;
    await navigate('/reset-password');
    const invalidMessage=en?'The reset link has expired or was already used. Please request another link.':'重置链接已失效或已使用，请重新申请';
    await visibleError(invalidMessage);
    assert.equal(await chrome.evaluate('document.querySelector("#reset-password-fields").disabled'),true);
    assert.equal(calls.length,beforeMissing,'missing fragment makes no validation request');
    linkMode='expired';
    await navigate(`/reset-password#token=${resetToken}`);
    await visibleError(invalidMessage);
    assert.equal(await chrome.evaluate('document.querySelector("#reset-password-fields").disabled'),true);
    linkMode='valid';completeMode='failure';
    // Change the query so this is a full navigation rather than only a fragment change.
    await navigate(`/reset-password?attempt=valid#token=${resetToken}`);
    await until(()=>chrome.evaluate('!document.querySelector("#reset-password-fields").disabled'),'valid link enables form');
    const beforeMismatch=calls.length;
    await fill('new-test-password','different-password');
    await visibleError(en?'Passwords do not match':'两次输入的密码不一致');
    assert.equal(calls.length,beforeMismatch,'password mismatch never submits');
    await fill('new-test-password');
    await until(()=>chrome.evaluate('!document.querySelector("#reset-password-fields").disabled && !document.querySelector("#password-error").classList.contains("hidden")'),'temporary completion failure can retry');
    completeMode='success';
    await fill('new-test-password');
    await until(()=>chrome.evaluate('!document.querySelector("#password-message").classList.contains("hidden")'),'successful password reset');
    assert.deepEqual(calls.at(-1).input,{token:resetToken,new_password:'new-test-password'});
    assert.equal(await chrome.evaluate('getComputedStyle(document.querySelector("#reset-password-form")).display'),'none');
    assert.equal(await chrome.evaluate('document.querySelector("#reset-password-new").value'), '');
    assert.equal(await chrome.evaluate('new URL(document.querySelector(".portal-access-switch a").href).pathname'),'/login');
    const previous=await chrome.evaluate('performance.timeOrigin');
    await chrome.send('Page.reload');
    await until(()=>chrome.evaluate(`performance.timeOrigin!==${previous} && document.readyState==='complete'`),'reload used link');
    await visibleError(invalidMessage);
    assert.equal(await chrome.evaluate('document.querySelector("#reset-password-fields").disabled'),true);
    linkMode='valid';completeMode='expired';
    await navigate(`/reset-password?attempt=expires#token=${resetToken}`);
    await until(()=>chrome.evaluate('!document.querySelector("#reset-password-fields").disabled'),'second valid link');
    await fill('new-test-password');
    await visibleError(invalidMessage);
    assert.equal(await chrome.evaluate('document.querySelector("#reset-password-fields").disabled'),true);
    for(const width of [1440,390]) {
      await chrome.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:width<600});
      assert.equal(await chrome.evaluate('document.documentElement.scrollWidth<=innerWidth'),true,'reset form has no horizontal overflow');
    }
  }
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS password recovery UI: login entry, email validation/delivery/retry/countdown, 20-minute notice, missing/expired/used links, mismatch, recoverable failure, successful reset, refresh, Chinese/English and desktop/mobile.');
}finally {await chrome?.close();await new Promise(done=>server.close(done));}
