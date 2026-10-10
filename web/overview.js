"use strict";
(() => {
  const $ = (id) => document.getElementById(id);
  const node = (tag, text, cls) => {const el=document.createElement(tag);el.textContent=text??"";if(cls)el.className=cls;return el;};
  const n = (value) => Number(value||0).toLocaleString();
  const statuses = {running:"执行中",pending:"准备中",queued:"排队中",waiting_authorization:"等待授权",recoverable:"可恢复",failed:"失败",succeeded:"已完成",cancelled:"已取消"};
  let data, capabilities=[], dimension="users", loading=false, pendingLoad=false, filterTimer;
  const initial=new URLSearchParams(location.search);
  $("overview-user").value=(initial.get("user")||"").slice(0,128);
  if(["1","7","30","90"].includes(initial.get("days")))$("overview-period").value=initial.get("days");
  function table(target, headings, rows) {
    target.replaceChildren(); if(!rows.length){target.append(node("p","此范围内暂无记录","settings-empty"));return;}
    const t=node("table"), head=node("tr");headings.forEach(h=>head.append(node("th",h)));const thead=node("thead");thead.append(head);t.append(thead);
    const body=node("tbody");for(const cells of rows){const row=node("tr");for(const cell of cells){const td=node("td");if(cell instanceof Node)td.append(cell);else td.textContent=cell;row.append(td);}body.append(row);}t.append(body);target.append(t);
  }
  function usage(){if(!data)return;document.querySelectorAll("[data-usage]").forEach(b=>b.setAttribute("aria-selected",String(b.dataset.usage===dimension)));
    const names=new Map(data.assistants.map(p=>[p.id,p.name]));
    if(dimension==="models")table($("overview-usage"),["模型 / 服务商","请求次数","Token","累计请求耗时","用量缺失"],data.models.map(x=>[`${x.model} / ${x.provider}`,n(x.requests),n(x.tokens),`${Math.round(x.duration_ms/1000)} 秒`,x.unreported_requests?`${x.unreported_requests} 次未上报`:"—"]));
    else table($("overview-usage"),[dimension==="users"?"用户":"助理","执行次数（含重试）","成功尝试","失败 / 可恢复尝试","Token","已结束运行耗时"],data.groups[dimension].map(x=>[(dimension==="users"?data.user_labels?.[x.id]:names.get(x.id))||({management:"管理端",unassigned:"未指定助理"}[x.id])||x.id,n(x.runs),n(x.succeeded),n(x.attention),n(x.tokens),`${Math.round(x.completed_duration_ms/60000)} 分钟`]));
  }
  function allowed(profile,item){const kind=item.manifest.spec.kind;const access=profile.access;
    if(!access)return kind!=="mcp_server"&&(kind!=="skill"||!profile.skill_ids.length||profile.skill_ids.includes(item.id));
    const rule=access[{skill:"skills",mcp_server:"mcp",tool:"tools",model:"models"}[kind]];
    return rule?.mode==="all"||(rule?.mode==="selected"&&rule.ids.includes(item.id));}
  function readiness(){const target=$("overview-readiness");target.replaceChildren();
    const blocked=capabilities.filter(c=>!c.availability.available);const summary=node("div",null,"readiness-item");summary.append(node("strong",`${capabilities.length} 项能力 · ${blocked.length} 项未就绪`));
    summary.append(node("p",!capabilities.length?"尚未登记能力":blocked.length?blocked.map(c=>`${c.manifest.name}：${c.availability.message||c.availability.status}`).join("；"):"所有已登记能力均已就绪"));target.append(summary);
    for(const p of data.assistants){const chosen=capabilities.filter(c=>allowed(p,c));const issues=[];
      if(!p.enabled)issues.push("助理已停用");
      for(const c of chosen){if(!c.availability.available)issues.push(`${c.manifest.name}：${c.availability.message||c.availability.status}`);
        if(c.dependency_error)issues.push(`${c.manifest.name}：${c.dependency_error}`);
        for(const d of c.dependencies||[]){const dependency=capabilities.find(x=>x.id===d.id);if(!dependency||!allowed(p,dependency))issues.push(`${c.manifest.name} 缺少允许使用的依赖：${d.name}`);}}
      for(const rule of Object.values(p.access||{})){if(rule?.mode==="selected")for(const id of rule.ids){if(!capabilities.some(c=>c.id===id))issues.push(`已选择能力不存在：${id}`);}}
      const box=node("div",null,"readiness-item");box.append(node("strong",p.name),node("span",`${chosen.length} 项能力`,"count-badge"),node("p",issues.length?[...new Set(issues)].join("；"):"能力范围及声明依赖检查通过"));
      const link=node("a","查看助理 →");link.href=`/agents?profile=${encodeURIComponent(p.id)}`;box.append(link);target.append(box);
    }
  }
  function render(){const c=data.counts;const active=["running","pending","waiting_authorization"].reduce((v,s)=>v+(c[s]||0),0);
    $("overview-metrics").replaceChildren(...[["活跃任务",active],["正在排队",c.queued||0],["失败 / 可恢复",(c.failed||0)+(c.recoverable||0)],["期间执行次数",data.runs],["期间 Token",data.tokens]].map(([label,value])=>{const el=node("article",null,"metric-tile");el.append(node("span",label),node("strong",n(value)));return el;}));
    table($("overview-activity"),["任务","最新状态","用户","提交时间","追踪"],data.activity.map(r=>{
      const link=node("a","查看 →");link.href=`/trace?session=${encodeURIComponent(r.session_id)}&run=${encodeURIComponent(r.id)}`;
      link.setAttribute("aria-label",`查看任务 Trace：${r.title||r.id}`);
      const task=node("div",null,"overview-task");task.append(node("strong",r.title||"未命名对话"));
      task.append(node("small",`第 ${r.turn_ordinal||1} 轮 · 第 ${r.attempt||1} 次执行`));task.title=r.id;
      const status=node("span",statuses[r.status]||r.status,"overview-task-status");status.dataset.status=r.status;
      return[task,status,r.user_id==="management"?"管理端":data.user_labels?.[r.user_id]||r.user_id,new Date(r.created_at).toLocaleString(),link];
    }));
    const s=data.scheduler;$("overview-resources").replaceChildren();if(s.ready===false){$("overview-resources").append(node("p",s.message||"暂无执行资源信息","settings-empty"));}else{for(const [label,used,limit] of [["全局 Agent",s.active,s.global_limit],...s.pools.map(p=>[p.name,p.active,p.limit])]){const el=node("div",null,"resource-item");const progress=node("progress");progress.max=limit;progress.value=used;progress.setAttribute("aria-label",label);el.append(node("strong",label),node("span",`${used} / ${limit}`),progress);$("overview-resources").append(el);}$("overview-resources").append(node("p",`每用户最多 ${s.per_user_limit} 个活跃 Run`));}usage();readiness();
  }
  async function load(){if(loading){pendingLoad=true;return;}loading=true;pendingLoad=false;$("overview-refresh").disabled=true;
    const days=$("overview-period").value,user=$("overview-user").value.trim();
    const query=new URLSearchParams({days});if(user)query.set("user",user);
    try{
    const fetchJson=async url=>{const r=await window.ExoAuth.fetch(url);if(!r.ok)throw new Error(r.status===404?"运行总览暂未开放，请稍后再试。":r.status===403?"当前账号无权查看运行总览。":"暂时无法读取运行记录，请稍后重试。");return r.json();};
    const result=await Promise.all([fetchJson(`/v1/management/overview?${query}`),fetchJson("/v1/assistant-capabilities")]);
    if(days!==$("overview-period").value||user!==$("overview-user").value.trim()){pendingLoad=true;return;}
    [data,capabilities]=result;render();history.replaceState(null,"",`${location.pathname}?${query}`);
    $("overview-error").hidden=true;$("overview-status").textContent=!data.activity.length&&!data.runs?user?"此用户在所选时间范围内暂无执行记录。":"所选时间范围内暂无执行记录。":`${user?`用户筛选：${user}`:"全部用户"} · 更新于 ${new Date(data.generated_at).toLocaleTimeString()} · 当前任务按最新执行计数，用量包含历史重试 · 执行资源与能力检查为全局 · 每 30 秒更新`;
  }catch(error){$("overview-error").hidden=false;$("overview-error").textContent=error instanceof Error&&/^(运行总览暂未开放|当前账号无权|暂时无法读取)/.test(error.message)?error.message:"暂时无法读取运行记录，请稍后重试。";$("overview-status").textContent=data?"暂时无法刷新，以下为上次读取的记录。":"运行记录暂不可用。";}finally{loading=false;$("overview-refresh").disabled=false;if(pendingLoad)void load();}}
  $("overview-filters").addEventListener("submit",event=>{event.preventDefault();clearTimeout(filterTimer);void load();});
  $("overview-user").addEventListener("input",()=>{clearTimeout(filterTimer);filterTimer=setTimeout(load,300);});
  $("overview-period").addEventListener("change",load);document.querySelectorAll("[data-usage]").forEach(b=>b.addEventListener("click",()=>{dimension=b.dataset.usage;usage();}));void load();setInterval(()=>{if(!document.hidden)void load();},30000);
})();
