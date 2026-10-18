// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.
"use strict";
window.addEventListener('hashchange',()=>location.reload());
const passwordToken=new URLSearchParams(location.hash.slice(1)).get('token') || '';
const passwordForm=document.querySelector('#reset-password-form');
const passwordFields=document.querySelector('#reset-password-fields');
const newPassword=document.querySelector('#reset-password-new');
const confirmNewPassword=document.querySelector('#reset-password-confirm');
const passwordStatus=document.querySelector('#reset-password-status');
const passwordMessage=document.querySelector('#password-message');
const passwordError=document.querySelector('#password-error');
let passwordFinished=false;
async function passwordRequest(path,body){
  const response=await fetch(path,{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify(body)});
  const payload=await response.json().catch(()=>null);
  if(!response.ok)throw Object.assign(new Error(payload?.error?.message || '暂时无法处理，请稍后重试'),{code:payload?.error?.code});
  return payload;
}
function passwordFailure(error){passwordError.textContent=error.message;passwordError.classList.remove('hidden');}
async function checkPasswordLink(){
  try {
    if(!passwordToken)throw new Error('重置链接已失效或已使用，请重新申请');
    await passwordRequest('/v1/auth/password-reset/validate',{token:passwordToken});
    passwordStatus.textContent='请设置新密码。链接只能使用一次，有效期20分钟。';passwordFields.disabled=false;newPassword.focus();
  }catch(error){passwordStatus.textContent='重置链接不可用';passwordFailure(error);}
}
passwordForm.addEventListener('submit',async event=>{
  event.preventDefault();if(passwordFields.disabled || passwordFinished)return;
  passwordError.classList.add('hidden');
  if(newPassword.value!==confirmNewPassword.value){passwordFailure(new Error('两次输入的密码不一致'));return;}
  passwordFields.disabled=true;
  try {
    const result=await passwordRequest('/v1/auth/password-reset/complete',{token:passwordToken,new_password:newPassword.value});
    passwordFinished=true;newPassword.value='';confirmNewPassword.value='';passwordForm.hidden=true;passwordForm.classList.add('hidden');
    passwordStatus.textContent='密码已重置';passwordMessage.textContent=result.message;passwordMessage.classList.remove('hidden');
  }catch(error){passwordFailure(error);if(error.code!=='invalid_password_reset')passwordFields.disabled=false;}
});
void checkPasswordLink();
