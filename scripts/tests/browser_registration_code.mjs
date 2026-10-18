// Actual registration assets; isolated HTTP responses, no email delivery.
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';
const root=resolve(import.meta.dirname,'../../web');
let mode='registered',requests=0;
const server=createServer(async(req,res)=>{
  const path=new URL(req.url,'http://localhost').pathname;
  try {
    if(path==='/v1/auth/registration-code' && req.method==='POST') {
      let body='';for await(const chunk of req)body+=chunk;
      assert.equal(JSON.parse(body).email,'sample@example.com');requests++;
      res.setHeader('content-type','application/json');
      if(mode==='registered') {res.writeHead(409).end(JSON.stringify({error:{code:'email_already_registered',message:'该邮箱已注册，请直接登录'}}));return;}
      if(mode==='failure') {res.writeHead(503).end(JSON.stringify({error:{code:'email_delivery_failed',message:'验证码邮件未能发送，请稍后重试'}}));return;}
      res.writeHead(202).end(JSON.stringify({message:'验证码已发送，请查收邮箱',expires_in:600,resend_after:2}));return;
    }
    const name=path==='/register'?'portal-register.html':path.startsWith('/assets/')?path.slice(8):null;
    assert.ok(name && /^[\w.-]+$/.test(name));
    res.setHeader('content-type',name.endsWith('.js')?'text/javascript':name.endsWith('.css')?'text/css':'text/html;charset=utf-8');
    res.end(await readFile(resolve(root,name)));
  }catch {res.writeHead(404).end();}
});
await new Promise(done=>server.listen(0,'127.0.0.1',done));
let chrome;
try {
  chrome=await browser();
  for(const language of ['zh-CN','en']) {
    mode='registered';
    await chrome.send('Page.addScriptToEvaluateOnNewDocument',{source:`localStorage.setItem('exoharness.ui-language',${JSON.stringify(language)});`});
    await chrome.send('Page.navigate',{url:`http://127.0.0.1:${server.address().port}/register?language=${language}`});
    await until(()=>chrome.evaluate('document.readyState==="complete" && !!document.querySelector("#portal-send-code")'),'registration page');
    const before=requests;
    await chrome.evaluate('document.querySelector("#portal-send-code").click()');
    assert.equal(requests,before,'empty email never submits');
    await chrome.evaluate('document.querySelector("#portal-register-email").value="sample@example.com";document.querySelector("#portal-send-code").click()');
    const message=language==='en'?'This email is already registered. Please sign in.':'该邮箱已注册，请直接登录';
    await until(()=>chrome.evaluate(`document.querySelector('#portal-register-error').textContent===${JSON.stringify(message)} && !document.querySelector('#portal-register-error').classList.contains('hidden')`),'explicit existing-account message');
    assert.equal(await chrome.evaluate('document.querySelector("#portal-send-code").disabled'),false);
    assert.equal(await chrome.evaluate('document.querySelector("#portal-register-message").classList.contains("hidden")'),true);
    assert.equal(await chrome.evaluate('new URL(document.querySelector(".portal-access-switch a").href).pathname'),'/login');
    assert.equal(requests,before+1,'one click makes one request');
    mode='success';
    await chrome.evaluate('document.querySelector("#portal-send-code").click()');
    const success=language==='en'?'Verification code sent. Please check your email.':'验证码已发送，请查收邮箱';
    await until(()=>chrome.evaluate(`document.querySelector('#portal-register-message').textContent===${JSON.stringify(success)} && !document.querySelector('#portal-register-message').classList.contains('hidden')`),'new email success message');
    assert.equal(await chrome.evaluate('document.querySelector("#portal-send-code").disabled'),true);
    assert.equal(await chrome.evaluate('document.querySelector("#portal-register-error").classList.contains("hidden")'),true);
    await until(()=>chrome.evaluate('!document.querySelector("#portal-send-code").disabled'),'resend countdown finishes');
    mode='failure';await chrome.evaluate('document.querySelector("#portal-send-code").click()');
    await until(()=>chrome.evaluate('!document.querySelector("#portal-register-error").classList.contains("hidden")'),'delivery failure visible');
    assert.equal(await chrome.evaluate('document.querySelector("#portal-send-code").disabled'),false);
    assert.equal(await chrome.evaluate('document.querySelector("#portal-register-message").classList.contains("hidden")'),true);
    assert.equal(requests,before+3);
    const previousLoad=await chrome.evaluate('performance.timeOrigin');
    await chrome.send('Page.reload');
    await until(()=>chrome.evaluate(`performance.timeOrigin!==${previousLoad} && document.readyState==="complete" && !!document.querySelector("#portal-send-code") && !document.querySelector("#portal-send-code").disabled && document.querySelector("#portal-register-error").classList.contains("hidden")`),'refresh resets transient state');
    for(const width of [1440,390]) {
      await chrome.send('Emulation.setDeviceMetricsOverride',{width,height:900,deviceScaleFactor:1,mobile:width<600});
      assert.equal(await chrome.evaluate('!!document.querySelector("#portal-send-code")'),true);
    }
  }
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS actual registration UI: registered email explicit, no false success/countdown, sign-in link, normal send/countdown, delivery failure/retry, invalid input, Chinese/English, refresh and desktop/mobile.');
}finally {await chrome?.close();await new Promise(done=>server.close(done));}
