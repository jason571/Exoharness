// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

export {};

interface AgentAvatar {
  asset_id: string;
  content_type: string;
}

type Scope = "skills" | "mcp" | "tools" | "models";
type Mode = "all" | "selected" | "none";
type Selection = { mode: "all" | "none" } | { mode: "selected"; ids: string[] };
type AgentAccess = Record<Scope, Selection> & { published: boolean; software_engineering?: {runtime_capability_id:string} | null };
interface AgentProfile {
  id: string;
  enabled: boolean;
  is_default: boolean;
  active_revision: number;
  display_name: string;
  avatar_text: string;
  avatar?: AgentAvatar | null;
  description: string;
  system_prompt: string;
  default_llm?: string | null;
  skill_ids: string[];
  access?: AgentAccess | null;
  digest: string;
  created_at: number;
  updated_at: number;
}

type AgentProfileDetail = AgentProfile;
interface ModelCatalog {
  models: {name:string;display_name?:string;model_type?:string;enabled:boolean;provider:string}[];
  providers: {name:string;enabled:boolean}[];
  model_capabilities?: {name:string;agent_callable:boolean}[];
}

interface CapabilityOverview {
  id: string;
  enabled: boolean;
  manifest: {
    name: string;
    description: string;
    spec: { kind: string };
  };
  dependencies?: { name: string; id: string | null }[];
  dependency_error?: string;
  availability: {
    status: string;
    available: boolean;
    message?: string | null;
  };
}

interface ApiOptions extends Omit<RequestInit, "body"> {
  body?: unknown;
}
function withModels(capabilities:CapabilityOverview[]):CapabilityOverview[] {
  const catalog=state.modelCatalog;
  if(!catalog)return capabilities;
  return [...capabilities,...catalog.models.filter(m=>(m.model_type||"llm")!=="llm").map(m=>{
    const enabled=m.enabled && catalog.providers.some(p=>p.name===m.provider && p.enabled);
    const ready=enabled && !!catalog.model_capabilities?.find(c=>c.name===m.name)?.agent_callable;
    return {id:m.name,enabled,manifest:{name:m.display_name||m.name,description:`${(m.model_type||"").toUpperCase()} · ${m.name} · 使用模型配置中的参数`,spec:{kind:"model"}},availability:{status:ready?"available":enabled?"unavailable":"disabled",available:ready,message:ready?"已接入Agent，按授权和任务需要调用":enabled?"已配置，但当前执行适配器尚未就绪":"模型或供应商未启用"}};
  })];
}
function renderDefaultLlm(selectedName:string):void {
  const select=$<HTMLSelectElement>("#agent-default-llm");
  select.replaceChildren(element("option","","跟随全局默认"));
  select.options[0]!.value="";
  const catalog=state.modelCatalog;
  for(const model of catalog?.models||[]) {
    if((model.model_type||"llm")!=="llm")continue;
    const available=model.enabled && !!catalog?.providers.some(p=>p.name===model.provider && p.enabled);
    if(!available && model.name!==selectedName)continue;
    const option=element("option","",`${model.display_name||model.name}${available?"":" · 未启用"}`);
    option.value=model.name;option.disabled=!available;select.append(option);
  }
  if(selectedName && !Array.from(select.options).some(o=>o.value===selectedName)) {
    const option=element("option","",`${selectedName} · 配置不可用`);option.value=selectedName;option.disabled=true;select.append(option);
  }
  select.value=selectedName;
}

const state: {
  profiles: AgentProfile[];
  capabilities: CapabilityOverview[];
  skillSelection: Set<string>;
  selected: AgentProfileDetail | null;
  pendingAvatar: File | null;
  avatar: AgentAvatar | null;
  previewUrl: string | null;
  modes: Record<Scope, Mode>;
  selections: Record<Exclude<Scope,"skills">, Set<string>>;
  activeScope: Scope;
  filters: Record<Scope, { query: string; selectedOnly: boolean; scrollTop: number }>;
  dirty: boolean;
  modelCatalog: ModelCatalog | null;
} = {
  profiles: [],
  capabilities: [],
  skillSelection: new Set(),
  selected: null,
  pendingAvatar: null,
  avatar: null,
  previewUrl: null,
  modes: { skills: "selected", mcp: "selected", tools: "none", models: "none" },
  selections: { mcp: new Set(), tools: new Set(), models: new Set() },
  activeScope: "skills",
  filters: {
    skills: { query: "", selectedOnly: false, scrollTop: 0 },
    mcp: { query: "", selectedOnly: false, scrollTop: 0 },
    tools: { query: "", selectedOnly: false, scrollTop: 0 },
    models: { query: "", selectedOnly: false, scrollTop: 0 },
  },
  dirty: false,
  modelCatalog: null,
};

const $ = <T extends Element = HTMLElement>(selector: string): T => {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error(`Missing UI element: ${selector}`);
  return node;
};

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

async function api<T = unknown>(path: string, options: ApiOptions = {}): Promise<T> {
  const headers = new Headers(options.headers);
  let body: BodyInit | undefined;
  if (options.body !== undefined) {
    headers.set("content-type", "application/json");
    body = JSON.stringify(options.body);
  }
  const response = await (window as unknown as { ExoAuth: { fetch: typeof fetch } }).ExoAuth.fetch(
    path,
    { ...options, headers, body },
  );
  const text = await response.text();
  let payload: unknown = null;
  if (text) {
    try { payload = JSON.parse(text); } catch { payload = text; }
  }
  if (!response.ok) {
    const message = typeof payload === "object" && payload && "error" in payload
      ? (payload as { error?: { message?: string } }).error?.message
      : null;
    throw new Error(message || `${response.status} ${response.statusText}`);
  }
  return payload as T;
}

function avatarUrl(avatar?: AgentAvatar | null): string | null {
  return avatar?.asset_id ? `/v1/agent-assets/${encodeURIComponent(avatar.asset_id)}` : null;
}

function setAvatar(target: HTMLElement, avatar: AgentAvatar | null, text: string, source?: string | null): void {
  target.replaceChildren();
  target.classList.remove("has-image");
  const url = source || avatarUrl(avatar);
  if (url) {
    const image = element("img");
    image.src = url;
    image.alt = "智能助理头像预览";
    image.addEventListener("error", () => {
      target.classList.remove("has-image");
      target.textContent = text || "AI";
    }, { once: true });
    target.classList.add("has-image");
    target.append(image);
  } else {
    target.textContent = text || "AI";
  }
}

function clearPreviewUrl(): void {
  if (state.previewUrl) URL.revokeObjectURL(state.previewUrl);
  state.previewUrl = null;
}

function updateAvatarPreview(): void {
  setAvatar(
    $("#agent-avatar-preview"),
    state.avatar,
    $<HTMLInputElement>("#agent-avatar-text").value.trim() || "AI",
    state.previewUrl,
  );
}

function availabilityLabel(skill: CapabilityOverview): string {
  return ({
    available: "可用",
    instructions_only: "说明已登记",
    execution_ready: "执行已就绪",
    degraded: "部分可用",
    pending_permission: "等待授权",
    permission_denied: "权限拒绝",
    runtime_missing: "环境未准备",
    unverified: "尚未验证",
    unavailable: "不可用",
    disabled: "未启用",
  } as Record<string, string>)[skill.availability.status] || "状态未知";
}

function capabilityMatches(capability: CapabilityOverview, query: string): boolean {
  return !query || [capability.id, capability.manifest.name, capability.manifest.description]
    .some((value) => value.toLocaleLowerCase().includes(query));
}

function capabilityCopy(capability: CapabilityOverview): HTMLElement {
  const copy = element("span");
  const title = element("span", "agent-skill-heading");
  const status = element("small", capability.availability.available ? "state-enabled" : "state-disabled", availabilityLabel(capability));
  status.title = capability.availability.message || availabilityLabel(capability);
  title.append(element("strong", "", capability.manifest.name), status);
  const description = element("small", "agent-skill-description", capability.manifest.description);
  description.title = capability.manifest.description;
  copy.append(title, description);
  return copy;
}

const scopes: [Scope, string, string, string][] = [
  ["skills","skill","#agent-skill-list","Skill"], ["mcp","mcp_server","#agent-mcp-list","MCP"],
  ["tools","tool","#agent-tools-list","Tool"], ["models","model","#agent-models-list","模型"],
];
function selected(scope: Scope): Set<string> { return scope==="skills" ? state.skillSelection : state.selections[scope]; }
function isAllowed(scope: Scope, id: string): boolean { return state.modes[scope]==="all" || (state.modes[scope]==="selected" && selected(scope).has(id)); }
function setDirty(dirty: boolean): void {state.dirty=dirty;$("#agent-save-status").textContent=dirty?"有未保存的修改":"配置已保存";}
function setTab(name: string): void {
  document.querySelectorAll<HTMLElement>("[data-agent-panel]").forEach(p=>p.hidden=p.dataset.agentPanel!==name);
  document.querySelectorAll<HTMLElement>("[data-agent-tab]").forEach(b=>{
    b.setAttribute("aria-selected",String(b.dataset.agentTab===name));
    b.tabIndex=b.dataset.agentTab===name?0:-1;
  });
}
function rememberCapabilityScroll(): void {
  for (const [scope,,target] of scopes) {
    const list = $(target);
    if (list.getClientRects().length) state.filters[scope].scrollTop = list.scrollTop;
  }
}
function updateScopeResults(): void {
  const scope = scopes.find(([name]) => name === state.activeScope)!;
  const count = $(scope[2]).querySelectorAll("input").length;
  $("#agent-scope-results").textContent = `当前显示 ${count} 项 ${scope[3]} · 标签数量为范围内 / 已登记`;
}
function setCapabilityTab(scope: Scope): void {
  rememberCapabilityScroll();
  state.activeScope = scope;
  document.querySelectorAll<HTMLElement>("[data-capability-panel]").forEach(panel=>panel.hidden=panel.dataset.capabilityPanel!==scope);
  document.querySelectorAll<HTMLElement>("[data-capability-tab]").forEach(button=>{
    const active=button.dataset.capabilityTab===scope;
    button.setAttribute("aria-selected",String(active));
    button.tabIndex=active?0:-1;
  });
  const category=scopes.find(([name])=>name===scope)!;
  $("#agent-search-label").textContent=`搜索 ${category[3]}`;
  $<HTMLInputElement>("#agent-capability-search").value=state.filters[scope].query;
  $<HTMLInputElement>("#agent-selected-only").checked=state.filters[scope].selectedOnly;
  $(category[2]).scrollTop=state.filters[scope].scrollTop;
  updateScopeResults();
}
function accessInput(): AgentAccess {
  const selection=(scope:Scope):Selection => state.modes[scope]==="selected" ? {mode:"selected",ids:selectedRegisteredIds(scope)} : {mode:state.modes[scope] as "all"|"none"};
  const software_engineering=$<HTMLInputElement>("#agent-engineering-enabled").checked
    ? {runtime_capability_id:$<HTMLSelectElement>("#agent-engineering-runtime").value} : null;
  if(software_engineering && !software_engineering.runtime_capability_id)throw new Error("请选择软件工程沙盒服务");
  return {skills:selection("skills"),mcp:selection("mcp"),tools:selection("tools"),models:selection("models"),published:$<HTMLInputElement>("#agent-published").checked,software_engineering};
}
function selectedRegisteredIds(scope:Scope):string[] {
  const registered=new Set(state.capabilities.map(capability=>capability.id));
  return Array.from(selected(scope)).filter(id=>registered.has(id));
}
function staleSelectionCount():number {
  return scopes.reduce((count,[scope])=>count+(state.modes[scope]==="selected"
    ? selected(scope).size-selectedRegisteredIds(scope).length : 0),0);
}
function renderEngineeringRuntime(id?:string):void {
  const select=$<HTMLSelectElement>("#agent-engineering-runtime");
  const value=id ?? select.value;
  select.replaceChildren(new Option("请选择已启用的沙盒服务", ""), ...state.capabilities
    .filter(c=>c.enabled && c.manifest.spec.kind==="mcp_server")
    .map(c=>new Option(c.manifest.name,c.id)));
  if(value && !Array.from(select.options).some(option=>option.value===value))select.add(new Option("原运行环境不可用，请重新选择",value));
  select.value=value;
  select.disabled=!$<HTMLInputElement>("#agent-engineering-enabled").checked;
}
$("#agent-engineering-enabled").addEventListener("change",()=>{renderEngineeringRuntime();setDirty(true);});
function renderCapabilities(): void {
  renderEngineeringRuntime();
  rememberCapabilityScroll();
  const focused=document.activeElement instanceof HTMLInputElement && document.activeElement.dataset.selectionScope
    ? { scope: document.activeElement.dataset.selectionScope, id: document.activeElement.value } : null;
  const counts=scopes.map(([scope,kind,,label])=>`${state.capabilities.filter(c=>c.manifest.spec.kind===kind).length} 个 ${label}`);
  $("#agent-capability-summary").textContent=`共 ${state.capabilities.length} 项能力 · ${counts.join(" · ")}`;
  for(const [scope,kind,target,label] of scopes) {
    const query=state.filters[scope].query.trim().toLocaleLowerCase();
    const only=state.filters[scope].selectedOnly;
    $<HTMLSelectElement>(`[data-scope="${scope}"]`).value=state.modes[scope];
    const list=$(target);list.replaceChildren();
    const items=state.capabilities.filter(c=>c.manifest.spec.kind===kind);
    const count=state.modes[scope]==="all"?items.length:state.modes[scope]==="none"?0:selected(scope).size;
    $(`[data-selection-count="${scope}"]`).textContent=`范围内 ${count} 项`;
    $(`[data-scope-count="${scope}"]`).textContent=`${count} / ${items.length}`;
    $(`[data-capability-tab="${scope}"]`).setAttribute("aria-label",`${label}，范围内 ${count} 项，已登记 ${items.length} 项`);
    for(const item of items.filter(c=>capabilityMatches(c,query)&&(!only||isAllowed(scope,c.id)))) {
      const row=element("label","agent-skill-option");row.dataset.capabilityId=item.id;
      const input=element("input");input.type="checkbox";input.value=item.id;input.dataset.selectionScope=scope;
      input.checked=isAllowed(scope,item.id);input.disabled=state.modes[scope]!=="selected" || (!item.enabled&&!input.checked);
      row.append(input,capabilityCopy(item));list.append(row);
    }
    for(const id of selected(scope)) {
      if(state.modes[scope]!=="selected" || items.some(c=>c.id===id) || (query&&!id.toLocaleLowerCase().includes(query)))continue;
      const row=element("label","agent-skill-option missing");const input=element("input");
      input.type="checkbox";input.value=id;input.checked=true;input.dataset.selectionScope=scope;
      row.append(input,element("span","",id+" · 已删除登记，保存时自动移除"));list.append(row);
    }
    if(!list.childElementCount)list.append(element("p","settings-empty",!items.length&&!selected(scope).size
      ? `尚未登记 ${label} 能力。可在服务与能力中查看。`
      : only&&!query?`此类别没有已选能力。取消“只看已选”后可继续选择。`
      : `没有匹配的 ${label}。可调整搜索或“只看已选”。`));
    list.scrollTop=state.filters[scope].scrollTop;
    if(focused?.scope===scope) {
      const replacement=Array.from(list.querySelectorAll<HTMLInputElement>("input")).find(input=>input.value===focused.id);
      (replacement||list).focus({preventScroll:true});
    }
  }
  updateScopeResults();
  const issues:string[]=[];
  for(const item of state.capabilities) {
    const scope=scopes.find(([,kind])=>kind===item.manifest.spec.kind)?.[0];
    if(!scope||!isAllowed(scope,item.id))continue;
    if(!item.availability.available)issues.push(`${item.manifest.name}：${item.availability.message||availabilityLabel(item)}`);
    if(item.dependency_error)issues.push(`${item.manifest.name}：${item.dependency_error}`);
    for(const dependency of item.dependencies||[]) {
      const other=state.capabilities.find(c=>c.id===dependency.id);
      const group=scopes.find(([,kind])=>kind===other?.manifest.spec.kind)?.[0];
      if(!other||!group||!isAllowed(group,other.id))issues.push(`${item.manifest.name} 缺少授权依赖：${dependency.name}`);
    }
  }
  const unique=[...new Set(issues)];
  $("#agent-readiness-summary").textContent=unique.length?`${unique.length} 项可用性提醒 · 展开查看`:"能力范围检查通过";
  $("#agent-readiness-issues").replaceChildren(...unique.map(issue=>element("li","",issue)));
  $("#agent-readiness").hidden=!unique.length;
}
function renderProfiles(): void {
  const list = $("#agent-profile-list");
  list.replaceChildren();
  $("#agent-count").textContent = `${state.profiles.length} 个助理`;
  const currentDefault = state.profiles.find((profile) => profile.is_default);
  $("#default-agent-summary").dataset.i18n = currentDefault ? "off" : "on";
  $("#default-agent-summary").textContent = currentDefault?.display_name || "尚未设置";
  if (!state.profiles.length) {
    list.append(element("p", "settings-empty", "还没有智能助理。左侧填写后保存即可创建。"));
    return;
  }
  for (const profile of state.profiles) {
    const item = element("button", `agent-profile-item${state.selected?.id === profile.id ? " selected" : ""}`);
    item.type = "button";
    item.dataset.profileId = profile.id;
    const avatar = element("span", "agent-list-avatar");
    setAvatar(avatar, profile.avatar || null, profile.avatar_text);
    const copy = element("span", "agent-list-copy");
    const heading = element("span", "agent-list-heading");
    heading.append(element("strong", "", profile.display_name));
    if (profile.is_default) heading.append(element("small", "state-enabled", "默认"));
    else if (!profile.enabled) heading.append(element("small", "state-disabled", "停用"));
    copy.append(
      heading,
      element("small", "", profile.id),
      element("span", "", profile.description),
    );
    item.append(avatar, copy);
    list.append(item);
  }
}

function renderProfileActions(): void {
  const panel = $("#agent-profile-actions-panel");
  panel.querySelector(".agent-profile-actions")?.remove();
  panel.classList.toggle("hidden", !state.selected);
  if (!state.selected) return;
  const actions = element("div", "agent-profile-actions");
  if (!state.selected.is_default) {
    const makeDefault = element("button", "button secondary", "设为默认");
    makeDefault.type = "button";
    makeDefault.dataset.action = "default";
    actions.append(makeDefault);
  }
  const remove = element("button", "button ghost danger-text", "删除这个助理");
  remove.type = "button";
  remove.dataset.action = "delete";
  actions.append(remove);
  panel.append(actions);
}

function populateForm(profile: AgentProfileDetail): void {
  state.selected = profile;
  state.pendingAvatar = null;
  state.avatar = profile.avatar || null;
  clearPreviewUrl();
  $<HTMLInputElement>("#agent-id").value = profile.id;
  $<HTMLInputElement>("#agent-id").readOnly = true;
  $<HTMLInputElement>("#agent-display-name").value = profile.display_name;
  $<HTMLInputElement>("#agent-avatar-text").value = profile.avatar_text;
  $<HTMLTextAreaElement>("#agent-description").value = profile.description;
  $<HTMLTextAreaElement>("#agent-system-prompt").value = profile.system_prompt;
  renderDefaultLlm(profile.default_llm || "");
  $<HTMLInputElement>("#agent-enabled").checked = profile.enabled;
  $<HTMLInputElement>("#agent-enabled").disabled = profile.is_default;
  $<HTMLInputElement>("#agent-default").checked = profile.is_default;
  $<HTMLInputElement>("#agent-default").disabled = profile.is_default;
  $("#agent-form-title").textContent = `修改智能助理 · ${profile.display_name}`;
  $("#agent-avatar-upload-label").textContent = "更换头像";
  $("#save-agent-profile").textContent = `更新当前助理 · ${profile.display_name}`;
  state.skillSelection = new Set(profile.access?.skills.mode==="selected" ? profile.access.skills.ids : profile.skill_ids);
  for(const [scope] of scopes) {
    const rule=profile.access?.[scope];
    state.modes[scope]=rule?.mode||(scope==="mcp" || (scope==="skills"&&profile.skill_ids.length)?"selected":"all");
    if(scope!=="skills")state.selections[scope]=new Set(rule?.mode==="selected"?rule.ids:[]);
  }
  $<HTMLInputElement>("#agent-published").checked=profile.is_default || !!profile.access?.published;
  $<HTMLInputElement>("#agent-engineering-enabled").checked=!!profile.access?.software_engineering;
  renderEngineeringRuntime(profile.access?.software_engineering?.runtime_capability_id || "");
  $<HTMLInputElement>("#agent-published").disabled=profile.is_default;
  $("#agent-policy-note").textContent=profile.access?"只在允许的范围内按需调用。收紧范围会作用于后续调用和恢复检查。":"MCP 尚未单独授权，请勾选后保存。其他类别保留原有范围。";
  setDirty(false);
  renderCapabilities();
  updateAvatarPreview();
  renderProfiles();
  renderProfileActions();
}

function resetForm(): void {
  if(state.dirty&&!window.confirm("放弃尚未保存的修改？"))return;
  state.selected = null;
  state.pendingAvatar = null;
  state.avatar = null;
  clearPreviewUrl();
  $<HTMLFormElement>("#agent-profile-form").reset();
  renderDefaultLlm("");
  $<HTMLInputElement>("#agent-id").readOnly = false;
  $<HTMLInputElement>("#agent-avatar-text").value = "AI";
  $<HTMLInputElement>("#agent-enabled").checked = true;
  $<HTMLInputElement>("#agent-enabled").disabled = false;
  $<HTMLInputElement>("#agent-default").disabled = false;
  $("#agent-form-title").textContent = "创建智能助理";
  $("#agent-avatar-upload-label").textContent = "选择头像";
  $("#save-agent-profile").textContent = "创建智能助理";
  state.skillSelection.clear();
  state.modes={skills:"selected",mcp:"selected",tools:"none",models:"none"};
  $<HTMLInputElement>("#agent-engineering-enabled").checked=false;
  renderEngineeringRuntime("");
  state.selections={mcp:new Set(),tools:new Set(),models:new Set()};
  for(const [scope] of scopes) state.filters[scope]={query:"",selectedOnly:false,scrollTop:0};
  $<HTMLInputElement>("#agent-published").disabled=false;
  $("#agent-policy-note").textContent="新助理默认不开放执行能力，请按任务需要选择。";
  setDirty(false);setTab("identity");setCapabilityTab("skills");
  renderCapabilities();
  updateAvatarPreview();
  renderProfiles();
  renderProfileActions();
}

async function refresh(): Promise<void> {
  const [profiles, capabilities, modelCatalog] = await Promise.all([
    api<AgentProfile[]>("/v1/agent-profiles"),
    api<CapabilityOverview[]>("/v1/assistant-capabilities"),
    api<ModelCatalog>("/v1/model-catalog"),
  ]);
  state.profiles = profiles;
  state.modelCatalog=modelCatalog;
  state.capabilities = withModels(capabilities);
  renderProfiles();
  if (state.selected) {
    const updated = state.profiles.find((profile) => profile.id === state.selected?.id);
    if (updated) await selectProfile(updated.id);
    else resetForm();
  } else {
    const requested=new URLSearchParams(location.search).get("profile");
    const preferred = state.profiles.find(p=>p.id===requested) || state.profiles.find((profile) => profile.is_default) || state.profiles[0];
    if (preferred) await selectProfile(preferred.id);
    else renderCapabilities();
  }
}

async function selectProfile(id: string): Promise<void> {
  if(state.dirty&&!window.confirm("放弃尚未保存的修改并切换助理？"))return;
  const detail = await api<AgentProfileDetail>(`/v1/agent-profiles/${encodeURIComponent(id)}`);
  populateForm(detail);
}

function fileAsBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || "").split(",")[1] || "");
    reader.onerror = () => reject(reader.error || new Error("读取头像失败"));
    reader.readAsDataURL(file);
  });
}

async function uploadAvatar(): Promise<AgentAvatar | null> {
  if (!state.pendingAvatar) return state.avatar;
  return api<AgentAvatar>("/v1/agent-assets", {
    method: "POST",
    body: {
      content_type: state.pendingAvatar.type,
      content_base64: await fileAsBase64(state.pendingAvatar),
      actor: "web-console",
    },
  });
}

function selectedSkillIds(): string[] {
  // Search only changes visibility, never the persisted selection.
  return selectedRegisteredIds("skills");
}

async function saveProfile(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const form=$<HTMLFormElement>("#agent-profile-form");
  if(!form.checkValidity()){setTab("identity");form.reportValidity();return;}
  const updating = state.selected !== null;
  const removedStale = staleSelectionCount();
  const submit = $<HTMLButtonElement>("#agent-profile-form button[type=submit]");
  submit.disabled = true;
  try {
    const avatar = await uploadAvatar();
    const common = {
      display_name: $<HTMLInputElement>("#agent-display-name").value.trim(),
      avatar_text: $<HTMLInputElement>("#agent-avatar-text").value.trim(),
      avatar,
      description: $<HTMLTextAreaElement>("#agent-description").value.trim(),
      system_prompt: $<HTMLTextAreaElement>("#agent-system-prompt").value.trim(),
      default_llm: $<HTMLSelectElement>("#agent-default-llm").value || null,
      skill_ids: state.modes.skills === "selected" ? selectedSkillIds() : [],
      access: accessInput(),
      actor: "web-console",
    };
    if (state.selected) {
      const id = state.selected.id;
      const expectedRevision = state.selected.active_revision;
      await api(`/v1/agent-profiles/${encodeURIComponent(id)}`, {
        method: "PUT",
        body: { ...common, expected_revision: expectedRevision },
      });
      const enabled = $<HTMLInputElement>("#agent-enabled").checked;
      if (enabled !== state.selected.enabled) {
        await api(`/v1/agent-profiles/${encodeURIComponent(id)}/enabled`, {
          method: "PUT",
          body: { enabled, actor: "web-console" },
        });
      }
      if ($<HTMLInputElement>("#agent-default").checked && !state.selected.is_default) {
        await setDefault(id);
      }
      state.selected = await api<AgentProfileDetail>(`/v1/agent-profiles/${encodeURIComponent(id)}`);
    } else {
      const id = $<HTMLInputElement>("#agent-id").value.trim();
      state.selected = await api<AgentProfileDetail>("/v1/agent-profiles", {
        method: "POST",
        body: {
          id,
          ...common,
          enabled: $<HTMLInputElement>("#agent-enabled").checked,
          make_default: $<HTMLInputElement>("#agent-default").checked,
        },
      });
    }
    state.pendingAvatar = null;
    clearPreviewUrl();
    setDirty(false);
    await refresh();
    showToast((updating
      ? `已更新当前助理“${state.selected.display_name}”；已有会话仍保留原快照`
      : `已创建智能助理“${state.selected.display_name}”`)
      +(removedStale?`；已自动移除 ${removedStale} 项已删除的能力登记`:""));
  } finally {
    submit.disabled = false;
  }
}

async function setDefault(id: string): Promise<void> {
  await api(`/v1/agent-profiles/${encodeURIComponent(id)}/default`, {
    method: "PUT",
    body: { actor: "web-console", confirm: true },
  });
}

async function handleProfileList(event: MouseEvent): Promise<void> {
  const item = (event.target as Element).closest<HTMLElement>("[data-profile-id]");
  if (item?.dataset.profileId) await selectProfile(item.dataset.profileId);
}

async function handleProfileActions(event: MouseEvent): Promise<void> {
  if (!state.selected) return;
  const button = (event.target as Element).closest<HTMLButtonElement>("button");
  if (!button) return;
  const id = state.selected.id;
  if (button.dataset.action === "default") {
    await setDefault(id);
  } else if (button.dataset.action === "delete") {
    if (!window.confirm(`删除智能助理“${state.selected.display_name}”？已有对话仍保留创建时的助理快照。此操作无法撤销。`)) return;
    await api(`/v1/agent-profiles/${encodeURIComponent(id)}`, {
      method: "DELETE",
      body: { actor: "web-console", confirm: true },
    });
    resetForm();
  } else {
    return;
  }
  await refresh();
}

async function checkService(): Promise<void> {
  try {
    await api("/health");
    $("#service-dot").classList.add("online");
    $("#service-label").textContent = "服务在线";
  } catch {
    $("#service-dot").classList.add("offline");
    $("#service-label").textContent = "服务不可用";
  }
}

let toastTimer: number | null = null;
function showToast(message: string, error = false): void {
  const toast = $("#toast");
  toast.textContent = message;
  toast.classList.toggle("error", error);
  toast.classList.add("visible");
  if (toastTimer !== null) window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toast.classList.remove("visible"), 5000);
}

function guarded<T extends Event>(handler: (event: T) => Promise<void> | void): (event: T) => Promise<void> {
  return async (event: T) => {
    try { await handler(event); } catch (error) { showToast(error instanceof Error ? error.message : String(error), true); }
  };
}

$<HTMLFormElement>("#agent-profile-form").addEventListener("submit", guarded((event) => saveProfile(event)));
$("#agent-profile-list").addEventListener("click", guarded((event: MouseEvent) => handleProfileList(event)));
$("#agent-profile-actions-panel").addEventListener("click", guarded((event: MouseEvent) => handleProfileActions(event)));
$("#new-agent").addEventListener("click", () => resetForm());
function filterCapabilities(): void {
  state.filters[state.activeScope].query=$<HTMLInputElement>("#agent-capability-search").value;
  state.filters[state.activeScope].selectedOnly=$<HTMLInputElement>("#agent-selected-only").checked;
  const target=scopes.find(([scope])=>scope===state.activeScope)![2];
  $(target).scrollTop=0;
  state.filters[state.activeScope].scrollTop=0;
  renderCapabilities();
}
$("#agent-capability-search").addEventListener("input", filterCapabilities);
$("#agent-capability-search").addEventListener("keydown", (event) => {
  // Enter in a search box must not submit the surrounding profile editor.
  if (event instanceof KeyboardEvent && event.key === "Enter") event.preventDefault();
});
$("#agent-profile-form").addEventListener("input",event=>{if(!["agent-capability-search","agent-selected-only"].includes((event.target as HTMLElement).id))setDirty(true);});
$("#agent-profile-form").addEventListener("change",(event)=>{
  const input=event.target;
  if(input instanceof HTMLElement && ["agent-capability-search","agent-selected-only"].includes(input.id))return;
  if(input instanceof HTMLSelectElement && input.dataset.scope) {
    state.modes[input.dataset.scope as Scope]=input.value as Mode;renderCapabilities();
  }
  if(input instanceof HTMLInputElement && input.dataset.selectionScope) {
    const set=selected(input.dataset.selectionScope as Scope);if(input.checked)set.add(input.value);else set.delete(input.value);
    renderCapabilities();
  }
  setDirty(true);
});
$("#agent-selected-only").addEventListener("change",filterCapabilities);
function bindTabs(selector: string, activate: (button: HTMLElement) => void): void {
  const tabs=Array.from(document.querySelectorAll<HTMLElement>(selector));
  tabs.forEach((button,index)=>{
    button.addEventListener("click",()=>activate(button));
    button.addEventListener("keydown",event=>{
      const target=event.key==="Home"?0:event.key==="End"?tabs.length-1:event.key==="ArrowRight"?(index+1)%tabs.length:event.key==="ArrowLeft"?(index+tabs.length-1)%tabs.length:-1;
      if(target<0)return;
      event.preventDefault();activate(tabs[target]);tabs[target].focus({preventScroll:true});
    });
  });
}
bindTabs("[data-agent-tab]",button=>setTab(button.dataset.agentTab||"identity"));
bindTabs("[data-capability-tab]",button=>setCapabilityTab(button.dataset.capabilityTab as Scope));
setTab("identity");
window.addEventListener("beforeunload",event=>{if(state.dirty){event.preventDefault();event.returnValue="";}});
$("#refresh-agent-capabilities").addEventListener("click", guarded(async () => {
  const button = $<HTMLButtonElement>("#refresh-agent-capabilities");
  button.disabled = true;
  try {
    state.modelCatalog=await api<ModelCatalog>("/v1/model-catalog");
    state.capabilities = withModels(await api<CapabilityOverview[]>("/v1/assistant-capabilities"));
    renderCapabilities();
    showToast("能力列表已刷新，未保存的助理配置已保留");
  } finally {
    button.disabled = false;
  }
}));
$<HTMLInputElement>("#agent-avatar-text").addEventListener("input", updateAvatarPreview);
$<HTMLInputElement>("#agent-avatar-file").addEventListener("change", guarded(async () => {
  const input = $<HTMLInputElement>("#agent-avatar-file");
  const file = input.files?.[0];
  if (!file) return;
  if (!["image/png", "image/jpeg", "image/webp"].includes(file.type)) throw new Error("头像必须是 PNG、JPEG 或 WebP");
  if (file.size > 2 * 1024 * 1024) throw new Error("头像不能超过 2 MiB");
  state.pendingAvatar = file;
  clearPreviewUrl();
  state.previewUrl = URL.createObjectURL(file);
  updateAvatarPreview();
  input.value = "";
}));
$("#remove-agent-avatar").addEventListener("click", () => {
  setDirty(true);
  state.pendingAvatar = null;
  state.avatar = null;
  clearPreviewUrl();
  updateAvatarPreview();
});
window.addEventListener("beforeunload", clearPreviewUrl);

void checkService();
void refresh().catch((error) => showToast(error instanceof Error ? error.message : String(error), true));
