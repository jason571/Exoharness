// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.
interface Application {client_id:string;name:string;assistant_id:string;redirect_uris:string[];scopes:string[];enabled:boolean;access_ttl_seconds:number;grant_ttl_seconds:number;revision:number;require_device_id?:boolean;allow_backend_identity?:boolean;}
interface Assistant {id:string;display_name:string;enabled:boolean;is_default:boolean;access?:{published?:boolean};}
interface Grant {id:string;user_id:string;username:string;display_name:string;active:boolean;expires_at:number;}
interface Issued {application:Application;client_secret:string;}
const $=<T extends HTMLElement=HTMLElement>(id:string):T=>{const el=document.getElementById(id);if(!el)throw Error(`Missing ${id}`);return el as T;};
const input=(id:string)=>$<HTMLInputElement>(id);
let profiles:Assistant[]=[];let applications:Application[]=[];let selected:Application|null=null;let busy=false;let requiredDevice=false;
const scopeLabels:Record<string,string>={"identity:read":"读取账号身份","sessions:read":"读取应用会话","sessions:write":"创建会话与发送消息","runs:read":"读取执行进度","runs:cancel":"取消应用任务"};
async function api<T>(path:string,method="GET",body?:unknown):Promise<T>{const auth=(window as unknown as {ExoAuth:{fetch:typeof fetch}}).ExoAuth;const response=await auth.fetch(path,{method,headers:body===undefined?undefined:{"Content-Type":"application/json"},body:body===undefined?undefined:JSON.stringify(body)});const text=await response.text();const data=text?JSON.parse(text):null;if(!response.ok)throw Error(data?.error?.message||data?.message||String(response.status));return data as T;}
function message(value:string){$("external-message").textContent=value;}
function node<K extends keyof HTMLElementTagNameMap>(tag:K,text:string):HTMLElementTagNameMap[K]{const el=document.createElement(tag);el.textContent=text;return el;}
async function guarded(work:()=>Promise<void>){if(busy)return;busy=true;for(const el of document.querySelectorAll<HTMLButtonElement>("#external-form button,#external-new,#external-refresh,#external-list button,#external-grants button"))el.disabled=true;try{message("");await work();}catch(error){message(error instanceof Error?error.message:String(error));}finally{busy=false;for(const el of document.querySelectorAll<HTMLButtonElement>("#external-form button,#external-new,#external-refresh,#external-list button,#external-grants button"))el.disabled=false;}}
function clearSecret(){input("external-issued-secret").value="";input("external-issued-client").value="";}
function showKey(issued:Issued){if(!issued.client_secret)return;clearSecret();input("external-issued-client").value=issued.application.client_id;input("external-issued-secret").value=issued.client_secret;$<HTMLDialogElement>("external-key-dialog").showModal();}
async function assistants(){profiles=await api<Assistant[]>("/v1/agent-profiles");const select=$<HTMLSelectElement>("external-assistant");const old=select.value;select.replaceChildren();for(const p of profiles.filter(p=>p.enabled)){const option=node("option",p.display_name);option.value=p.id;option.dataset.i18n="off";select.append(option);}if([...select.options].some(o=>o.value===old))select.value=old;}
function fill(app:Application|null){selected=app;$("external-save").textContent=app?"保存应用":"生成 Key 并保存应用";$("external-form-title").textContent=app?"编辑应用":"新增应用";input("external-client").value=app?.client_id||"";input("external-client").disabled=!!app;input("external-name").value=app?.name||"";input("external-access-ttl").value=String(app?.access_ttl_seconds||300);input("external-grant-ttl").value=String(app?.grant_ttl_seconds||2592000);input("external-enabled").checked=app?.enabled??true;requiredDevice=app?.require_device_id??false;const select=$<HTMLSelectElement>("external-assistant");if(app){if(![...select.options].some(o=>o.value===app.assistant_id)){const option=node("option",profiles.find(p=>p.id===app.assistant_id)?.display_name||app.assistant_id);option.value=app.assistant_id;option.dataset.i18n="off";select.append(option);}select.value=app.assistant_id;}for(const checkbox of document.querySelectorAll<HTMLInputElement>("#external-scopes input"))checkbox.checked=app?app.scopes.includes(checkbox.value):true;$("external-rotate").hidden=!app;$("external-grants").replaceChildren();$("external-grants-note").textContent=app?"正在读取用户授权":"选择应用后查看用户授权。";}
async function grants(app:Application){const data=await api<{grants:Grant[]}>(`/v1/management/external-applications/${encodeURIComponent(app.client_id)}/grants`);if(selected?.client_id!==app.client_id)return;const list=$("external-grants");list.replaceChildren();$("external-grants-note").textContent=data.grants.length?"仅显示最近 200 条用户授权。":"暂无用户授权";for(const grant of data.grants){const row=node("div","");row.className="external-app-row";const info=node("div","");const name=node("strong",`${grant.display_name} · ${grant.username}`);name.dataset.i18n="off";info.append(name,node("small",grant.active?"授权有效":"授权已失效"));const expires=node("small",new Date(grant.expires_at*1000).toLocaleString());expires.dataset.i18n="off";info.append(expires);row.append(info);if(grant.active){const revoke=node("button","撤销授权");revoke.className="button secondary";revoke.onclick=()=>void guarded(async()=>{if(!confirm("撤销此用户对该应用的授权？"))return;await api(`/v1/management/external-applications/${encodeURIComponent(app.client_id)}/grants/${encodeURIComponent(grant.id)}`,"DELETE");await grants(app);});row.append(revoke);}list.append(row);}}
function render(){
 const list=$("external-list");list.replaceChildren();
 if(!applications.length){list.append(node("p","尚未登记外部应用"));return;}
 for(const app of applications){
  const row=node("div","");row.className="external-app-row";
  const info=node("div","");const name=node("strong",app.name);name.dataset.i18n="off";
  const id=node("small",app.client_id);id.dataset.i18n="off";
  const role=node("small","");const roleName=node("span",profiles.find(p=>p.id===app.assistant_id)?.display_name||app.assistant_id);roleName.dataset.i18n="off";
  role.append(node("span","绑定助理"),document.createTextNode("："),roleName);
  info.append(name,id,role,node("small",app.enabled?"已启用":"已停用"));
  const edit=node("button","编辑");edit.className="button secondary";edit.dataset.action="edit";
  edit.onclick=()=>void guarded(async()=>{await assistants();fill(app);await grants(app);$("external-form-title").scrollIntoView({block:"nearest"});input("external-name").focus();});
  const view=node("button","查看 Key");view.className="button secondary";
  view.onclick=()=>void guarded(async()=>{if(!confirm("确认查看此应用的 Key？"))return;const result=await api<{client_id:string;client_secret:string}>(`/v1/management/external-applications/${encodeURIComponent(app.client_id)}/key`,"POST",{confirm:true});showKey({application:app,client_secret:result.client_secret});});
  const remove=node("button","删除");remove.className="button danger";remove.dataset.action="delete";
  remove.onclick=()=>void guarded(async()=>{
   if(!confirm("删除此应用将使 Key 和访问凭证失效，会话记录保留，应用 ID 不可重新使用。继续？"))return;
   await api(`/v1/management/external-applications/${encodeURIComponent(app.client_id)}`,"DELETE",{expected_revision:app.revision});
   clearSecret();$<HTMLDialogElement>("external-key-dialog").close();
   if(selected?.client_id===app.client_id)fill(null);
   await reload();
   message("应用已删除");
  });
  const actions=node("div","");actions.className="external-actions";
  const history=node("a","查看对话");history.className="button secondary";history.href=`/external-conversations?client_id=${encodeURIComponent(app.client_id)}`;
  actions.append(edit,view,history,remove);row.append(info,actions);list.append(row);
 }
}
async function reload(){const data=await api<{applications:Application[]}>("/v1/management/external-applications");applications=data.applications;render();}
function form(){return {client_id:input("external-client").value.trim(),name:input("external-name").value.trim(),assistant_id:$<HTMLSelectElement>("external-assistant").value,redirect_uris:selected?.redirect_uris||[],scopes:[...document.querySelectorAll<HTMLInputElement>("#external-scopes input:checked")].map(c=>c.value),enabled:input("external-enabled").checked,require_device_id:requiredDevice||$<HTMLSelectElement>("external-assistant").value==="carenest",allow_backend_identity:selected?.allow_backend_identity??true,access_ttl_seconds:Number(input("external-access-ttl").value),grant_ttl_seconds:Number(input("external-grant-ttl").value)};}
for(const [scope,label] of Object.entries(scopeLabels)){const wrapper=node("label","");wrapper.className="external-check";const check=node("input","");check.type="checkbox";check.value=scope;check.checked=true;wrapper.append(check,node("span",label));$("external-scopes").append(wrapper);}
$("external-form").addEventListener("submit",event=>{event.preventDefault();void guarded(async()=>{if(selected&&!confirm("修改配置将使该应用的原用户授权失效，继续？"))return;const body=form();const issued=selected?await api<Issued>(`/v1/management/external-applications/${encodeURIComponent(selected.client_id)}`,"PUT",{application:body,expected_revision:selected.revision,rotate_secret:false}):await api<Issued>("/v1/management/external-applications","POST",body);fill(issued.application);showKey(issued);await reload();await grants(issued.application);message("应用已保存");});});
$("external-new").onclick=()=>void guarded(async()=>{await assistants();fill(null);});
$("external-refresh").onclick=()=>void guarded(async()=>{await assistants();await reload();if(selected){const app=applications.find(a=>a.client_id===selected?.client_id);if(app){fill(app);await grants(app);}else{fill(null);clearSecret();}}});
$("external-rotate").onclick=()=>void guarded(async()=>{if(!selected||!confirm("轮换 Key 将使该应用的原用户授权失效，继续？"))return;const issued=await api<Issued>(`/v1/management/external-applications/${encodeURIComponent(selected.client_id)}`,"PUT",{application:{...selected,revision:undefined},expected_revision:selected.revision,rotate_secret:true});fill(issued.application);showKey(issued);await reload();await grants(issued.application);});
$("external-template").onclick=()=>void guarded(async()=>{const response=await fetch("/assets/carenest-assistant.json");if(!response.ok)throw Error("无法读取助理模板");const template=await response.json();const existing=await api<Assistant[]>("/v1/agent-profiles");if(!existing.some(p=>p.id===template.id))await api("/v1/agent-profiles","POST",template);await assistants();const select=$<HTMLSelectElement>("external-assistant");select.value=template.id;requiredDevice=true;message(select.value===template.id?"CareNest 示例助理已准备好，请按需配置能力。":"CareNest 助理已存在，但尚未启用，请在智能助理页面配置。");});
$("external-copy").onclick=()=>{void navigator.clipboard.writeText(input("external-issued-secret").value).then(()=>message("Key 已复制")).catch(()=>message("请手动复制 Key"));};
$("external-key-close").onclick=()=>{$<HTMLDialogElement>("external-key-dialog").close();clearSecret();};
$("external-key-dialog").addEventListener("close",clearSecret);window.addEventListener("pagehide",clearSecret);
void guarded(async()=>{await assistants();await reload();fill(null);});

export {};
