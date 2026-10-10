// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

"use strict";
(() => {
  // src/agents.ts
  function withModels(capabilities) {
    const catalog = state.modelCatalog;
    if (!catalog) return capabilities;
    return [...capabilities, ...catalog.models.filter((m) => (m.model_type || "llm") !== "llm").map((m) => {
      const enabled = m.enabled && catalog.providers.some((p) => p.name === m.provider && p.enabled);
      const ready = enabled && !!catalog.model_capabilities?.find((c) => c.name === m.name)?.agent_callable;
      return { id: m.name, enabled, manifest: { name: m.display_name || m.name, description: `${(m.model_type || "").toUpperCase()} \xB7 ${m.name} \xB7 \u4F7F\u7528\u6A21\u578B\u914D\u7F6E\u4E2D\u7684\u53C2\u6570`, spec: { kind: "model" } }, availability: { status: ready ? "available" : enabled ? "unavailable" : "disabled", available: ready, message: ready ? "\u5DF2\u63A5\u5165Agent\uFF0C\u6309\u6388\u6743\u548C\u4EFB\u52A1\u9700\u8981\u8C03\u7528" : enabled ? "\u5DF2\u914D\u7F6E\uFF0C\u4F46\u5F53\u524D\u6267\u884C\u9002\u914D\u5668\u5C1A\u672A\u5C31\u7EEA" : "\u6A21\u578B\u6216\u4F9B\u5E94\u5546\u672A\u542F\u7528" } };
    })];
  }
  function renderDefaultLlm(selectedName) {
    const select = $("#agent-default-llm");
    select.replaceChildren(element("option", "", "\u8DDF\u968F\u5168\u5C40\u9ED8\u8BA4"));
    select.options[0].value = "";
    const catalog = state.modelCatalog;
    for (const model of catalog?.models || []) {
      if ((model.model_type || "llm") !== "llm") continue;
      const available = model.enabled && !!catalog?.providers.some((p) => p.name === model.provider && p.enabled);
      if (!available && model.name !== selectedName) continue;
      const option = element("option", "", `${model.display_name || model.name}${available ? "" : " \xB7 \u672A\u542F\u7528"}`);
      option.value = model.name;
      option.disabled = !available;
      select.append(option);
    }
    if (selectedName && !Array.from(select.options).some((o) => o.value === selectedName)) {
      const option = element("option", "", `${selectedName} \xB7 \u914D\u7F6E\u4E0D\u53EF\u7528`);
      option.value = selectedName;
      option.disabled = true;
      select.append(option);
    }
    select.value = selectedName;
  }
  var state = {
    profiles: [],
    capabilities: [],
    skillSelection: /* @__PURE__ */ new Set(),
    selected: null,
    pendingAvatar: null,
    avatar: null,
    previewUrl: null,
    modes: { skills: "selected", mcp: "selected", tools: "none", models: "none" },
    selections: { mcp: /* @__PURE__ */ new Set(), tools: /* @__PURE__ */ new Set(), models: /* @__PURE__ */ new Set() },
    activeScope: "skills",
    filters: {
      skills: { query: "", selectedOnly: false, scrollTop: 0 },
      mcp: { query: "", selectedOnly: false, scrollTop: 0 },
      tools: { query: "", selectedOnly: false, scrollTop: 0 },
      models: { query: "", selectedOnly: false, scrollTop: 0 }
    },
    dirty: false,
    modelCatalog: null
  };
  var $ = (selector) => {
    const node = document.querySelector(selector);
    if (!node) throw new Error(`Missing UI element: ${selector}`);
    return node;
  };
  function element(tag, className = "", text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== void 0) node.textContent = text;
    return node;
  }
  async function api(path, options = {}) {
    const headers = new Headers(options.headers);
    let body;
    if (options.body !== void 0) {
      headers.set("content-type", "application/json");
      body = JSON.stringify(options.body);
    }
    const response = await window.ExoAuth.fetch(
      path,
      { ...options, headers, body }
    );
    const text = await response.text();
    let payload = null;
    if (text) {
      try {
        payload = JSON.parse(text);
      } catch {
        payload = text;
      }
    }
    if (!response.ok) {
      const message = typeof payload === "object" && payload && "error" in payload ? payload.error?.message : null;
      throw new Error(message || `${response.status} ${response.statusText}`);
    }
    return payload;
  }
  function avatarUrl(avatar) {
    return avatar?.asset_id ? `/v1/agent-assets/${encodeURIComponent(avatar.asset_id)}` : null;
  }
  function setAvatar(target, avatar, text, source) {
    target.replaceChildren();
    target.classList.remove("has-image");
    const url = source || avatarUrl(avatar);
    if (url) {
      const image = element("img");
      image.src = url;
      image.alt = "\u667A\u80FD\u52A9\u7406\u5934\u50CF\u9884\u89C8";
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
  function clearPreviewUrl() {
    if (state.previewUrl) URL.revokeObjectURL(state.previewUrl);
    state.previewUrl = null;
  }
  function updateAvatarPreview() {
    setAvatar(
      $("#agent-avatar-preview"),
      state.avatar,
      $("#agent-avatar-text").value.trim() || "AI",
      state.previewUrl
    );
  }
  function availabilityLabel(skill) {
    return {
      available: "\u53EF\u7528",
      instructions_only: "\u8BF4\u660E\u5DF2\u767B\u8BB0",
      execution_ready: "\u6267\u884C\u5DF2\u5C31\u7EEA",
      degraded: "\u90E8\u5206\u53EF\u7528",
      pending_permission: "\u7B49\u5F85\u6388\u6743",
      permission_denied: "\u6743\u9650\u62D2\u7EDD",
      runtime_missing: "\u73AF\u5883\u672A\u51C6\u5907",
      unverified: "\u5C1A\u672A\u9A8C\u8BC1",
      unavailable: "\u4E0D\u53EF\u7528",
      disabled: "\u672A\u542F\u7528"
    }[skill.availability.status] || "\u72B6\u6001\u672A\u77E5";
  }
  function capabilityMatches(capability, query) {
    return !query || [capability.id, capability.manifest.name, capability.manifest.description].some((value) => value.toLocaleLowerCase().includes(query));
  }
  function capabilityCopy(capability) {
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
  var scopes = [
    ["skills", "skill", "#agent-skill-list", "Skill"],
    ["mcp", "mcp_server", "#agent-mcp-list", "MCP"],
    ["tools", "tool", "#agent-tools-list", "Tool"],
    ["models", "model", "#agent-models-list", "\u6A21\u578B"]
  ];
  function selected(scope) {
    return scope === "skills" ? state.skillSelection : state.selections[scope];
  }
  function isAllowed(scope, id) {
    return state.modes[scope] === "all" || state.modes[scope] === "selected" && selected(scope).has(id);
  }
  function setDirty(dirty) {
    state.dirty = dirty;
    $("#agent-save-status").textContent = dirty ? "\u6709\u672A\u4FDD\u5B58\u7684\u4FEE\u6539" : "\u914D\u7F6E\u5DF2\u4FDD\u5B58";
  }
  function setTab(name) {
    document.querySelectorAll("[data-agent-panel]").forEach((p) => p.hidden = p.dataset.agentPanel !== name);
    document.querySelectorAll("[data-agent-tab]").forEach((b) => {
      b.setAttribute("aria-selected", String(b.dataset.agentTab === name));
      b.tabIndex = b.dataset.agentTab === name ? 0 : -1;
    });
  }
  function rememberCapabilityScroll() {
    for (const [scope, , target] of scopes) {
      const list = $(target);
      if (list.getClientRects().length) state.filters[scope].scrollTop = list.scrollTop;
    }
  }
  function updateScopeResults() {
    const scope = scopes.find(([name]) => name === state.activeScope);
    const count = $(scope[2]).querySelectorAll("input").length;
    $("#agent-scope-results").textContent = `\u5F53\u524D\u663E\u793A ${count} \u9879 ${scope[3]} \xB7 \u6807\u7B7E\u6570\u91CF\u4E3A\u8303\u56F4\u5185 / \u5DF2\u767B\u8BB0`;
  }
  function setCapabilityTab(scope) {
    rememberCapabilityScroll();
    state.activeScope = scope;
    document.querySelectorAll("[data-capability-panel]").forEach((panel) => panel.hidden = panel.dataset.capabilityPanel !== scope);
    document.querySelectorAll("[data-capability-tab]").forEach((button) => {
      const active = button.dataset.capabilityTab === scope;
      button.setAttribute("aria-selected", String(active));
      button.tabIndex = active ? 0 : -1;
    });
    const category = scopes.find(([name]) => name === scope);
    $("#agent-search-label").textContent = `\u641C\u7D22 ${category[3]}`;
    $("#agent-capability-search").value = state.filters[scope].query;
    $("#agent-selected-only").checked = state.filters[scope].selectedOnly;
    $(category[2]).scrollTop = state.filters[scope].scrollTop;
    updateScopeResults();
  }
  function accessInput() {
    const selection = (scope) => state.modes[scope] === "selected" ? { mode: "selected", ids: selectedRegisteredIds(scope) } : { mode: state.modes[scope] };
    const software_engineering = $("#agent-engineering-enabled").checked ? { runtime_capability_id: $("#agent-engineering-runtime").value } : null;
    if (software_engineering && !software_engineering.runtime_capability_id) throw new Error("\u8BF7\u9009\u62E9\u8F6F\u4EF6\u5DE5\u7A0B\u6C99\u76D2\u670D\u52A1");
    return { skills: selection("skills"), mcp: selection("mcp"), tools: selection("tools"), models: selection("models"), published: $("#agent-published").checked, software_engineering };
  }
  function selectedRegisteredIds(scope) {
    const registered = new Set(state.capabilities.map((capability) => capability.id));
    return Array.from(selected(scope)).filter((id) => registered.has(id));
  }
  function staleSelectionCount() {
    return scopes.reduce((count, [scope]) => count + (state.modes[scope] === "selected" ? selected(scope).size - selectedRegisteredIds(scope).length : 0), 0);
  }
  function renderEngineeringRuntime(id) {
    const select = $("#agent-engineering-runtime");
    const value = id ?? select.value;
    select.replaceChildren(new Option("\u8BF7\u9009\u62E9\u5DF2\u542F\u7528\u7684\u6C99\u76D2\u670D\u52A1", ""), ...state.capabilities.filter((c) => c.enabled && c.manifest.spec.kind === "mcp_server").map((c) => new Option(c.manifest.name, c.id)));
    if (value && !Array.from(select.options).some((option) => option.value === value)) select.add(new Option("\u539F\u8FD0\u884C\u73AF\u5883\u4E0D\u53EF\u7528\uFF0C\u8BF7\u91CD\u65B0\u9009\u62E9", value));
    select.value = value;
    select.disabled = !$("#agent-engineering-enabled").checked;
  }
  $("#agent-engineering-enabled").addEventListener("change", () => {
    renderEngineeringRuntime();
    setDirty(true);
  });
  function renderCapabilities() {
    renderEngineeringRuntime();
    rememberCapabilityScroll();
    const focused = document.activeElement instanceof HTMLInputElement && document.activeElement.dataset.selectionScope ? { scope: document.activeElement.dataset.selectionScope, id: document.activeElement.value } : null;
    const counts = scopes.map(([scope, kind, , label]) => `${state.capabilities.filter((c) => c.manifest.spec.kind === kind).length} \u4E2A ${label}`);
    $("#agent-capability-summary").textContent = `\u5171 ${state.capabilities.length} \u9879\u80FD\u529B \xB7 ${counts.join(" \xB7 ")}`;
    for (const [scope, kind, target, label] of scopes) {
      const query = state.filters[scope].query.trim().toLocaleLowerCase();
      const only = state.filters[scope].selectedOnly;
      $(`[data-scope="${scope}"]`).value = state.modes[scope];
      const list = $(target);
      list.replaceChildren();
      const items = state.capabilities.filter((c) => c.manifest.spec.kind === kind);
      const count = state.modes[scope] === "all" ? items.length : state.modes[scope] === "none" ? 0 : selected(scope).size;
      $(`[data-selection-count="${scope}"]`).textContent = `\u8303\u56F4\u5185 ${count} \u9879`;
      $(`[data-scope-count="${scope}"]`).textContent = `${count} / ${items.length}`;
      $(`[data-capability-tab="${scope}"]`).setAttribute("aria-label", `${label}\uFF0C\u8303\u56F4\u5185 ${count} \u9879\uFF0C\u5DF2\u767B\u8BB0 ${items.length} \u9879`);
      for (const item of items.filter((c) => capabilityMatches(c, query) && (!only || isAllowed(scope, c.id)))) {
        const row = element("label", "agent-skill-option");
        row.dataset.capabilityId = item.id;
        const input = element("input");
        input.type = "checkbox";
        input.value = item.id;
        input.dataset.selectionScope = scope;
        input.checked = isAllowed(scope, item.id);
        input.disabled = state.modes[scope] !== "selected" || !item.enabled && !input.checked;
        row.append(input, capabilityCopy(item));
        list.append(row);
      }
      for (const id of selected(scope)) {
        if (state.modes[scope] !== "selected" || items.some((c) => c.id === id) || query && !id.toLocaleLowerCase().includes(query)) continue;
        const row = element("label", "agent-skill-option missing");
        const input = element("input");
        input.type = "checkbox";
        input.value = id;
        input.checked = true;
        input.dataset.selectionScope = scope;
        row.append(input, element("span", "", id + " \xB7 \u5DF2\u5220\u9664\u767B\u8BB0\uFF0C\u4FDD\u5B58\u65F6\u81EA\u52A8\u79FB\u9664"));
        list.append(row);
      }
      if (!list.childElementCount) list.append(element("p", "settings-empty", !items.length && !selected(scope).size ? `\u5C1A\u672A\u767B\u8BB0 ${label} \u80FD\u529B\u3002\u53EF\u5728\u670D\u52A1\u4E0E\u80FD\u529B\u4E2D\u67E5\u770B\u3002` : only && !query ? `\u6B64\u7C7B\u522B\u6CA1\u6709\u5DF2\u9009\u80FD\u529B\u3002\u53D6\u6D88\u201C\u53EA\u770B\u5DF2\u9009\u201D\u540E\u53EF\u7EE7\u7EED\u9009\u62E9\u3002` : `\u6CA1\u6709\u5339\u914D\u7684 ${label}\u3002\u53EF\u8C03\u6574\u641C\u7D22\u6216\u201C\u53EA\u770B\u5DF2\u9009\u201D\u3002`));
      list.scrollTop = state.filters[scope].scrollTop;
      if (focused?.scope === scope) {
        const replacement = Array.from(list.querySelectorAll("input")).find((input) => input.value === focused.id);
        (replacement || list).focus({ preventScroll: true });
      }
    }
    updateScopeResults();
    const issues = [];
    for (const item of state.capabilities) {
      const scope = scopes.find(([, kind]) => kind === item.manifest.spec.kind)?.[0];
      if (!scope || !isAllowed(scope, item.id)) continue;
      if (!item.availability.available) issues.push(`${item.manifest.name}\uFF1A${item.availability.message || availabilityLabel(item)}`);
      if (item.dependency_error) issues.push(`${item.manifest.name}\uFF1A${item.dependency_error}`);
      for (const dependency of item.dependencies || []) {
        const other = state.capabilities.find((c) => c.id === dependency.id);
        const group = scopes.find(([, kind]) => kind === other?.manifest.spec.kind)?.[0];
        if (!other || !group || !isAllowed(group, other.id)) issues.push(`${item.manifest.name} \u7F3A\u5C11\u6388\u6743\u4F9D\u8D56\uFF1A${dependency.name}`);
      }
    }
    const unique = [...new Set(issues)];
    $("#agent-readiness-summary").textContent = unique.length ? `${unique.length} \u9879\u53EF\u7528\u6027\u63D0\u9192 \xB7 \u5C55\u5F00\u67E5\u770B` : "\u80FD\u529B\u8303\u56F4\u68C0\u67E5\u901A\u8FC7";
    $("#agent-readiness-issues").replaceChildren(...unique.map((issue) => element("li", "", issue)));
    $("#agent-readiness").hidden = !unique.length;
  }
  function renderProfiles() {
    const list = $("#agent-profile-list");
    list.replaceChildren();
    $("#agent-count").textContent = `${state.profiles.length} \u4E2A\u52A9\u7406`;
    const currentDefault = state.profiles.find((profile) => profile.is_default);
    $("#default-agent-summary").dataset.i18n = currentDefault ? "off" : "on";
    $("#default-agent-summary").textContent = currentDefault?.display_name || "\u5C1A\u672A\u8BBE\u7F6E";
    if (!state.profiles.length) {
      list.append(element("p", "settings-empty", "\u8FD8\u6CA1\u6709\u667A\u80FD\u52A9\u7406\u3002\u5DE6\u4FA7\u586B\u5199\u540E\u4FDD\u5B58\u5373\u53EF\u521B\u5EFA\u3002"));
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
      if (profile.is_default) heading.append(element("small", "state-enabled", "\u9ED8\u8BA4"));
      else if (!profile.enabled) heading.append(element("small", "state-disabled", "\u505C\u7528"));
      copy.append(
        heading,
        element("small", "", profile.id),
        element("span", "", profile.description)
      );
      item.append(avatar, copy);
      list.append(item);
    }
  }
  function renderProfileActions() {
    const panel = $("#agent-profile-actions-panel");
    panel.querySelector(".agent-profile-actions")?.remove();
    panel.classList.toggle("hidden", !state.selected);
    if (!state.selected) return;
    const actions = element("div", "agent-profile-actions");
    if (!state.selected.is_default) {
      const makeDefault = element("button", "button secondary", "\u8BBE\u4E3A\u9ED8\u8BA4");
      makeDefault.type = "button";
      makeDefault.dataset.action = "default";
      actions.append(makeDefault);
    }
    const remove = element("button", "button ghost danger-text", "\u5220\u9664\u8FD9\u4E2A\u52A9\u7406");
    remove.type = "button";
    remove.dataset.action = "delete";
    actions.append(remove);
    panel.append(actions);
  }
  function populateForm(profile) {
    state.selected = profile;
    state.pendingAvatar = null;
    state.avatar = profile.avatar || null;
    clearPreviewUrl();
    $("#agent-id").value = profile.id;
    $("#agent-id").readOnly = true;
    $("#agent-display-name").value = profile.display_name;
    $("#agent-avatar-text").value = profile.avatar_text;
    $("#agent-description").value = profile.description;
    $("#agent-system-prompt").value = profile.system_prompt;
    renderDefaultLlm(profile.default_llm || "");
    $("#agent-enabled").checked = profile.enabled;
    $("#agent-enabled").disabled = profile.is_default;
    $("#agent-default").checked = profile.is_default;
    $("#agent-default").disabled = profile.is_default;
    $("#agent-form-title").textContent = `\u4FEE\u6539\u667A\u80FD\u52A9\u7406 \xB7 ${profile.display_name}`;
    $("#agent-avatar-upload-label").textContent = "\u66F4\u6362\u5934\u50CF";
    $("#save-agent-profile").textContent = `\u66F4\u65B0\u5F53\u524D\u52A9\u7406 \xB7 ${profile.display_name}`;
    state.skillSelection = new Set(profile.access?.skills.mode === "selected" ? profile.access.skills.ids : profile.skill_ids);
    for (const [scope] of scopes) {
      const rule = profile.access?.[scope];
      state.modes[scope] = rule?.mode || (scope === "mcp" || scope === "skills" && profile.skill_ids.length ? "selected" : "all");
      if (scope !== "skills") state.selections[scope] = new Set(rule?.mode === "selected" ? rule.ids : []);
    }
    $("#agent-published").checked = profile.is_default || !!profile.access?.published;
    $("#agent-engineering-enabled").checked = !!profile.access?.software_engineering;
    renderEngineeringRuntime(profile.access?.software_engineering?.runtime_capability_id || "");
    $("#agent-published").disabled = profile.is_default;
    $("#agent-policy-note").textContent = profile.access ? "\u53EA\u5728\u5141\u8BB8\u7684\u8303\u56F4\u5185\u6309\u9700\u8C03\u7528\u3002\u6536\u7D27\u8303\u56F4\u4F1A\u4F5C\u7528\u4E8E\u540E\u7EED\u8C03\u7528\u548C\u6062\u590D\u68C0\u67E5\u3002" : "MCP \u5C1A\u672A\u5355\u72EC\u6388\u6743\uFF0C\u8BF7\u52FE\u9009\u540E\u4FDD\u5B58\u3002\u5176\u4ED6\u7C7B\u522B\u4FDD\u7559\u539F\u6709\u8303\u56F4\u3002";
    setDirty(false);
    renderCapabilities();
    updateAvatarPreview();
    renderProfiles();
    renderProfileActions();
  }
  function resetForm() {
    if (state.dirty && !window.confirm("\u653E\u5F03\u5C1A\u672A\u4FDD\u5B58\u7684\u4FEE\u6539\uFF1F")) return;
    state.selected = null;
    state.pendingAvatar = null;
    state.avatar = null;
    clearPreviewUrl();
    $("#agent-profile-form").reset();
    renderDefaultLlm("");
    $("#agent-id").readOnly = false;
    $("#agent-avatar-text").value = "AI";
    $("#agent-enabled").checked = true;
    $("#agent-enabled").disabled = false;
    $("#agent-default").disabled = false;
    $("#agent-form-title").textContent = "\u521B\u5EFA\u667A\u80FD\u52A9\u7406";
    $("#agent-avatar-upload-label").textContent = "\u9009\u62E9\u5934\u50CF";
    $("#save-agent-profile").textContent = "\u521B\u5EFA\u667A\u80FD\u52A9\u7406";
    state.skillSelection.clear();
    state.modes = { skills: "selected", mcp: "selected", tools: "none", models: "none" };
    $("#agent-engineering-enabled").checked = false;
    renderEngineeringRuntime("");
    state.selections = { mcp: /* @__PURE__ */ new Set(), tools: /* @__PURE__ */ new Set(), models: /* @__PURE__ */ new Set() };
    for (const [scope] of scopes) state.filters[scope] = { query: "", selectedOnly: false, scrollTop: 0 };
    $("#agent-published").disabled = false;
    $("#agent-policy-note").textContent = "\u65B0\u52A9\u7406\u9ED8\u8BA4\u4E0D\u5F00\u653E\u6267\u884C\u80FD\u529B\uFF0C\u8BF7\u6309\u4EFB\u52A1\u9700\u8981\u9009\u62E9\u3002";
    setDirty(false);
    setTab("identity");
    setCapabilityTab("skills");
    renderCapabilities();
    updateAvatarPreview();
    renderProfiles();
    renderProfileActions();
  }
  async function refresh() {
    const [profiles, capabilities, modelCatalog] = await Promise.all([
      api("/v1/agent-profiles"),
      api("/v1/assistant-capabilities"),
      api("/v1/model-catalog")
    ]);
    state.profiles = profiles;
    state.modelCatalog = modelCatalog;
    state.capabilities = withModels(capabilities);
    renderProfiles();
    if (state.selected) {
      const updated = state.profiles.find((profile) => profile.id === state.selected?.id);
      if (updated) await selectProfile(updated.id);
      else resetForm();
    } else {
      const requested = new URLSearchParams(location.search).get("profile");
      const preferred = state.profiles.find((p) => p.id === requested) || state.profiles.find((profile) => profile.is_default) || state.profiles[0];
      if (preferred) await selectProfile(preferred.id);
      else renderCapabilities();
    }
  }
  async function selectProfile(id) {
    if (state.dirty && !window.confirm("\u653E\u5F03\u5C1A\u672A\u4FDD\u5B58\u7684\u4FEE\u6539\u5E76\u5207\u6362\u52A9\u7406\uFF1F")) return;
    const detail = await api(`/v1/agent-profiles/${encodeURIComponent(id)}`);
    populateForm(detail);
  }
  function fileAsBase64(file) {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result || "").split(",")[1] || "");
      reader.onerror = () => reject(reader.error || new Error("\u8BFB\u53D6\u5934\u50CF\u5931\u8D25"));
      reader.readAsDataURL(file);
    });
  }
  async function uploadAvatar() {
    if (!state.pendingAvatar) return state.avatar;
    return api("/v1/agent-assets", {
      method: "POST",
      body: {
        content_type: state.pendingAvatar.type,
        content_base64: await fileAsBase64(state.pendingAvatar),
        actor: "web-console"
      }
    });
  }
  function selectedSkillIds() {
    return selectedRegisteredIds("skills");
  }
  async function saveProfile(event) {
    event.preventDefault();
    const form = $("#agent-profile-form");
    if (!form.checkValidity()) {
      setTab("identity");
      form.reportValidity();
      return;
    }
    const updating = state.selected !== null;
    const removedStale = staleSelectionCount();
    const submit = $("#agent-profile-form button[type=submit]");
    submit.disabled = true;
    try {
      const avatar = await uploadAvatar();
      const common = {
        display_name: $("#agent-display-name").value.trim(),
        avatar_text: $("#agent-avatar-text").value.trim(),
        avatar,
        description: $("#agent-description").value.trim(),
        system_prompt: $("#agent-system-prompt").value.trim(),
        default_llm: $("#agent-default-llm").value || null,
        skill_ids: state.modes.skills === "selected" ? selectedSkillIds() : [],
        access: accessInput(),
        actor: "web-console"
      };
      if (state.selected) {
        const id = state.selected.id;
        const expectedRevision = state.selected.active_revision;
        await api(`/v1/agent-profiles/${encodeURIComponent(id)}`, {
          method: "PUT",
          body: { ...common, expected_revision: expectedRevision }
        });
        const enabled = $("#agent-enabled").checked;
        if (enabled !== state.selected.enabled) {
          await api(`/v1/agent-profiles/${encodeURIComponent(id)}/enabled`, {
            method: "PUT",
            body: { enabled, actor: "web-console" }
          });
        }
        if ($("#agent-default").checked && !state.selected.is_default) {
          await setDefault(id);
        }
        state.selected = await api(`/v1/agent-profiles/${encodeURIComponent(id)}`);
      } else {
        const id = $("#agent-id").value.trim();
        state.selected = await api("/v1/agent-profiles", {
          method: "POST",
          body: {
            id,
            ...common,
            enabled: $("#agent-enabled").checked,
            make_default: $("#agent-default").checked
          }
        });
      }
      state.pendingAvatar = null;
      clearPreviewUrl();
      setDirty(false);
      await refresh();
      showToast((updating ? `\u5DF2\u66F4\u65B0\u5F53\u524D\u52A9\u7406\u201C${state.selected.display_name}\u201D\uFF1B\u5DF2\u6709\u4F1A\u8BDD\u4ECD\u4FDD\u7559\u539F\u5FEB\u7167` : `\u5DF2\u521B\u5EFA\u667A\u80FD\u52A9\u7406\u201C${state.selected.display_name}\u201D`) + (removedStale ? `\uFF1B\u5DF2\u81EA\u52A8\u79FB\u9664 ${removedStale} \u9879\u5DF2\u5220\u9664\u7684\u80FD\u529B\u767B\u8BB0` : ""));
    } finally {
      submit.disabled = false;
    }
  }
  async function setDefault(id) {
    await api(`/v1/agent-profiles/${encodeURIComponent(id)}/default`, {
      method: "PUT",
      body: { actor: "web-console", confirm: true }
    });
  }
  async function handleProfileList(event) {
    const item = event.target.closest("[data-profile-id]");
    if (item?.dataset.profileId) await selectProfile(item.dataset.profileId);
  }
  async function handleProfileActions(event) {
    if (!state.selected) return;
    const button = event.target.closest("button");
    if (!button) return;
    const id = state.selected.id;
    if (button.dataset.action === "default") {
      await setDefault(id);
    } else if (button.dataset.action === "delete") {
      if (!window.confirm(`\u5220\u9664\u667A\u80FD\u52A9\u7406\u201C${state.selected.display_name}\u201D\uFF1F\u5DF2\u6709\u5BF9\u8BDD\u4ECD\u4FDD\u7559\u521B\u5EFA\u65F6\u7684\u52A9\u7406\u5FEB\u7167\u3002\u6B64\u64CD\u4F5C\u65E0\u6CD5\u64A4\u9500\u3002`)) return;
      await api(`/v1/agent-profiles/${encodeURIComponent(id)}`, {
        method: "DELETE",
        body: { actor: "web-console", confirm: true }
      });
      resetForm();
    } else {
      return;
    }
    await refresh();
  }
  async function checkService() {
    try {
      await api("/health");
      $("#service-dot").classList.add("online");
      $("#service-label").textContent = "\u670D\u52A1\u5728\u7EBF";
    } catch {
      $("#service-dot").classList.add("offline");
      $("#service-label").textContent = "\u670D\u52A1\u4E0D\u53EF\u7528";
    }
  }
  var toastTimer = null;
  function showToast(message, error = false) {
    const toast = $("#toast");
    toast.textContent = message;
    toast.classList.toggle("error", error);
    toast.classList.add("visible");
    if (toastTimer !== null) window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => toast.classList.remove("visible"), 5e3);
  }
  function guarded(handler) {
    return async (event) => {
      try {
        await handler(event);
      } catch (error) {
        showToast(error instanceof Error ? error.message : String(error), true);
      }
    };
  }
  $("#agent-profile-form").addEventListener("submit", guarded((event) => saveProfile(event)));
  $("#agent-profile-list").addEventListener("click", guarded((event) => handleProfileList(event)));
  $("#agent-profile-actions-panel").addEventListener("click", guarded((event) => handleProfileActions(event)));
  $("#new-agent").addEventListener("click", () => resetForm());
  function filterCapabilities() {
    state.filters[state.activeScope].query = $("#agent-capability-search").value;
    state.filters[state.activeScope].selectedOnly = $("#agent-selected-only").checked;
    const target = scopes.find(([scope]) => scope === state.activeScope)[2];
    $(target).scrollTop = 0;
    state.filters[state.activeScope].scrollTop = 0;
    renderCapabilities();
  }
  $("#agent-capability-search").addEventListener("input", filterCapabilities);
  $("#agent-capability-search").addEventListener("keydown", (event) => {
    if (event instanceof KeyboardEvent && event.key === "Enter") event.preventDefault();
  });
  $("#agent-profile-form").addEventListener("input", (event) => {
    if (!["agent-capability-search", "agent-selected-only"].includes(event.target.id)) setDirty(true);
  });
  $("#agent-profile-form").addEventListener("change", (event) => {
    const input = event.target;
    if (input instanceof HTMLElement && ["agent-capability-search", "agent-selected-only"].includes(input.id)) return;
    if (input instanceof HTMLSelectElement && input.dataset.scope) {
      state.modes[input.dataset.scope] = input.value;
      renderCapabilities();
    }
    if (input instanceof HTMLInputElement && input.dataset.selectionScope) {
      const set = selected(input.dataset.selectionScope);
      if (input.checked) set.add(input.value);
      else set.delete(input.value);
      renderCapabilities();
    }
    setDirty(true);
  });
  $("#agent-selected-only").addEventListener("change", filterCapabilities);
  function bindTabs(selector, activate) {
    const tabs = Array.from(document.querySelectorAll(selector));
    tabs.forEach((button, index) => {
      button.addEventListener("click", () => activate(button));
      button.addEventListener("keydown", (event) => {
        const target = event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1 : event.key === "ArrowRight" ? (index + 1) % tabs.length : event.key === "ArrowLeft" ? (index + tabs.length - 1) % tabs.length : -1;
        if (target < 0) return;
        event.preventDefault();
        activate(tabs[target]);
        tabs[target].focus({ preventScroll: true });
      });
    });
  }
  bindTabs("[data-agent-tab]", (button) => setTab(button.dataset.agentTab || "identity"));
  bindTabs("[data-capability-tab]", (button) => setCapabilityTab(button.dataset.capabilityTab));
  setTab("identity");
  window.addEventListener("beforeunload", (event) => {
    if (state.dirty) {
      event.preventDefault();
      event.returnValue = "";
    }
  });
  $("#refresh-agent-capabilities").addEventListener("click", guarded(async () => {
    const button = $("#refresh-agent-capabilities");
    button.disabled = true;
    try {
      state.modelCatalog = await api("/v1/model-catalog");
      state.capabilities = withModels(await api("/v1/assistant-capabilities"));
      renderCapabilities();
      showToast("\u80FD\u529B\u5217\u8868\u5DF2\u5237\u65B0\uFF0C\u672A\u4FDD\u5B58\u7684\u52A9\u7406\u914D\u7F6E\u5DF2\u4FDD\u7559");
    } finally {
      button.disabled = false;
    }
  }));
  $("#agent-avatar-text").addEventListener("input", updateAvatarPreview);
  $("#agent-avatar-file").addEventListener("change", guarded(async () => {
    const input = $("#agent-avatar-file");
    const file = input.files?.[0];
    if (!file) return;
    if (!["image/png", "image/jpeg", "image/webp"].includes(file.type)) throw new Error("\u5934\u50CF\u5FC5\u987B\u662F PNG\u3001JPEG \u6216 WebP");
    if (file.size > 2 * 1024 * 1024) throw new Error("\u5934\u50CF\u4E0D\u80FD\u8D85\u8FC7 2 MiB");
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
})();
