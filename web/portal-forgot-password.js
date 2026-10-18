// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.
"use strict";
const resetForm=document.querySelector('#forgot-password-form');
const resetEmail=document.querySelector('#forgot-password-email');
const resetButton=document.querySelector('#forgot-password-submit');
const resetMessage=document.querySelector('#password-message');
const resetError=document.querySelector('#password-error');
let resetTimer=null;
resetForm.addEventListener('submit',async event=>{
  event.preventDefault();if(!resetEmail.reportValidity() || resetButton.disabled)return;
  resetButton.disabled=true;resetMessage.classList.add('hidden');resetError.classList.add('hidden');
  try {
    const response=await fetch('/v1/auth/password-reset',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({email:resetEmail.value})});
    const payload=await response.json().catch(()=>null);
    if(!response.ok)throw new Error(payload?.error?.message || '重置邮件未能发送，请稍后重试');
    resetMessage.textContent=payload.message;resetMessage.classList.remove('hidden');
    let remaining=Number(payload.resend_after)||60;
    resetButton.textContent=`${remaining} 秒后重发`;
    resetTimer=window.setInterval(()=>{
      remaining--;
      if(remaining<=0){clearInterval(resetTimer);resetTimer=null;resetButton.disabled=false;resetButton.textContent='重新发送';}
      else resetButton.textContent=`${remaining} 秒后重发`;
    },1000);
  }catch(error){resetButton.disabled=false;resetError.textContent=error.message;resetError.classList.remove('hidden');}
});
