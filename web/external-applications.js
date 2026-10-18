"use strict";
(() => {
  // src/external-applications.ts
  var $ = (id) => {
    const el = document.getElementById(id);
    if (!el) throw Error(`Missing ${id}`);
    return el;
  };
  var input = (id) => $(id);
  var profiles = [];
  var applications = [];
  var selected = null;
  var busy = false;
  var requiredDevice = false;
  var scopeLabels = { "identity:read": "\u8BFB\u53D6\u8D26\u53F7\u8EAB\u4EFD", "sessions:read": "\u8BFB\u53D6\u5E94\u7528\u4F1A\u8BDD", "sessions:write": "\u521B\u5EFA\u4F1A\u8BDD\u4E0E\u53D1\u9001\u6D88\u606F", "runs:read": "\u8BFB\u53D6\u6267\u884C\u8FDB\u5EA6", "runs:cancel": "\u53D6\u6D88\u5E94\u7528\u4EFB\u52A1" };
  async function api(path, method = "GET", body) {
    const auth = window.ExoAuth;
    const response = await auth.fetch(path, { method, headers: body === void 0 ? void 0 : { "Content-Type": "application/json" }, body: body === void 0 ? void 0 : JSON.stringify(body) });
    const text = await response.text();
    const data = text ? JSON.parse(text) : null;
    if (!response.ok) throw Error(data?.error?.message || data?.message || String(response.status));
    return data;
  }
  function message(value) {
    $("external-message").textContent = value;
  }
  function node(tag, text) {
    const el = document.createElement(tag);
    el.textContent = text;
    return el;
  }
  async function guarded(work) {
    if (busy) return;
    busy = true;
    for (const el of document.querySelectorAll("#external-form button,#external-new,#external-refresh,#external-list button,#external-grants button")) el.disabled = true;
    try {
      message("");
      await work();
    } catch (error) {
      message(error instanceof Error ? error.message : String(error));
    } finally {
      busy = false;
      for (const el of document.querySelectorAll("#external-form button,#external-new,#external-refresh,#external-list button,#external-grants button")) el.disabled = false;
    }
  }
  function clearSecret() {
    input("external-issued-secret").value = "";
    input("external-issued-client").value = "";
  }
  function showKey(issued) {
    if (!issued.client_secret) return;
    clearSecret();
    input("external-issued-client").value = issued.application.client_id;
    input("external-issued-secret").value = issued.client_secret;
    $("external-key-dialog").showModal();
  }
  async function assistants() {
    profiles = await api("/v1/agent-profiles");
    const select = $("external-assistant");
    const old = select.value;
    select.replaceChildren();
    for (const p of profiles.filter((p2) => p2.enabled)) {
      const option = node("option", p.display_name);
      option.value = p.id;
      option.dataset.i18n = "off";
      select.append(option);
    }
    if ([...select.options].some((o) => o.value === old)) select.value = old;
  }
  function fill(app) {
    selected = app;
    $("external-save").textContent = app ? "\u4FDD\u5B58\u5E94\u7528" : "\u751F\u6210 Key \u5E76\u4FDD\u5B58\u5E94\u7528";
    $("external-form-title").textContent = app ? "\u7F16\u8F91\u5E94\u7528" : "\u65B0\u589E\u5E94\u7528";
    input("external-client").value = app?.client_id || "";
    input("external-client").disabled = !!app;
    input("external-name").value = app?.name || "";
    input("external-access-ttl").value = String(app?.access_ttl_seconds || 300);
    input("external-grant-ttl").value = String(app?.grant_ttl_seconds || 2592e3);
    input("external-enabled").checked = app?.enabled ?? true;
    requiredDevice = app?.require_device_id ?? false;
    const select = $("external-assistant");
    if (app) {
      if (![...select.options].some((o) => o.value === app.assistant_id)) {
        const option = node("option", profiles.find((p) => p.id === app.assistant_id)?.display_name || app.assistant_id);
        option.value = app.assistant_id;
        option.dataset.i18n = "off";
        select.append(option);
      }
      select.value = app.assistant_id;
    }
    for (const checkbox of document.querySelectorAll("#external-scopes input")) checkbox.checked = app ? app.scopes.includes(checkbox.value) : true;
    $("external-rotate").hidden = !app;
    $("external-grants").replaceChildren();
    $("external-grants-note").textContent = app ? "\u6B63\u5728\u8BFB\u53D6\u7528\u6237\u6388\u6743" : "\u9009\u62E9\u5E94\u7528\u540E\u67E5\u770B\u7528\u6237\u6388\u6743\u3002";
  }
  async function grants(app) {
    const data = await api(`/v1/management/external-applications/${encodeURIComponent(app.client_id)}/grants`);
    if (selected?.client_id !== app.client_id) return;
    const list = $("external-grants");
    list.replaceChildren();
    $("external-grants-note").textContent = data.grants.length ? "\u4EC5\u663E\u793A\u6700\u8FD1 200 \u6761\u7528\u6237\u6388\u6743\u3002" : "\u6682\u65E0\u7528\u6237\u6388\u6743";
    for (const grant of data.grants) {
      const row = node("div", "");
      row.className = "external-app-row";
      const info = node("div", "");
      const name = node("strong", `${grant.display_name} \xB7 ${grant.username}`);
      name.dataset.i18n = "off";
      info.append(name, node("small", grant.active ? "\u6388\u6743\u6709\u6548" : "\u6388\u6743\u5DF2\u5931\u6548"));
      const expires = node("small", new Date(grant.expires_at * 1e3).toLocaleString());
      expires.dataset.i18n = "off";
      info.append(expires);
      row.append(info);
      if (grant.active) {
        const revoke = node("button", "\u64A4\u9500\u6388\u6743");
        revoke.className = "button secondary";
        revoke.onclick = () => void guarded(async () => {
          if (!confirm("\u64A4\u9500\u6B64\u7528\u6237\u5BF9\u8BE5\u5E94\u7528\u7684\u6388\u6743\uFF1F")) return;
          await api(`/v1/management/external-applications/${encodeURIComponent(app.client_id)}/grants/${encodeURIComponent(grant.id)}`, "DELETE");
          await grants(app);
        });
        row.append(revoke);
      }
      list.append(row);
    }
  }
  function render() {
    const list = $("external-list");
    list.replaceChildren();
    if (!applications.length) {
      list.append(node("p", "\u5C1A\u672A\u767B\u8BB0\u5916\u90E8\u5E94\u7528"));
      return;
    }
    for (const app of applications) {
      const row = node("div", "");
      row.className = "external-app-row";
      const info = node("div", "");
      const name = node("strong", app.name);
      name.dataset.i18n = "off";
      const id = node("small", app.client_id);
      id.dataset.i18n = "off";
      const role = node("small", "");
      const roleName = node("span", profiles.find((p) => p.id === app.assistant_id)?.display_name || app.assistant_id);
      roleName.dataset.i18n = "off";
      role.append(node("span", "\u7ED1\u5B9A\u52A9\u7406"), document.createTextNode("\uFF1A"), roleName);
      info.append(name, id, role, node("small", app.enabled ? "\u5DF2\u542F\u7528" : "\u5DF2\u505C\u7528"));
      const edit = node("button", "\u7F16\u8F91");
      edit.className = "button secondary";
      edit.dataset.action = "edit";
      edit.onclick = () => void guarded(async () => {
        await assistants();
        fill(app);
        await grants(app);
        $("external-form-title").scrollIntoView({ block: "nearest" });
        input("external-name").focus();
      });
      const view = node("button", "\u67E5\u770B Key");
      view.className = "button secondary";
      view.onclick = () => void guarded(async () => {
        if (!confirm("\u786E\u8BA4\u67E5\u770B\u6B64\u5E94\u7528\u7684 Key\uFF1F")) return;
        const result = await api(`/v1/management/external-applications/${encodeURIComponent(app.client_id)}/key`, "POST", { confirm: true });
        showKey({ application: app, client_secret: result.client_secret });
      });
      const remove = node("button", "\u5220\u9664");
      remove.className = "button danger";
      remove.dataset.action = "delete";
      remove.onclick = () => void guarded(async () => {
        if (!confirm("\u5220\u9664\u6B64\u5E94\u7528\u5C06\u4F7F Key \u548C\u8BBF\u95EE\u51ED\u8BC1\u5931\u6548\uFF0C\u4F1A\u8BDD\u8BB0\u5F55\u4FDD\u7559\uFF0C\u5E94\u7528 ID \u4E0D\u53EF\u91CD\u65B0\u4F7F\u7528\u3002\u7EE7\u7EED\uFF1F")) return;
        await api(`/v1/management/external-applications/${encodeURIComponent(app.client_id)}`, "DELETE", { expected_revision: app.revision });
        clearSecret();
        $("external-key-dialog").close();
        if (selected?.client_id === app.client_id) fill(null);
        await reload();
        message("\u5E94\u7528\u5DF2\u5220\u9664");
      });
      const actions = node("div", "");
      actions.className = "external-actions";
      const history = node("a", "\u67E5\u770B\u5BF9\u8BDD");
      history.className = "button secondary";
      history.href = `/external-conversations?client_id=${encodeURIComponent(app.client_id)}`;
      actions.append(edit, view, history, remove);
      row.append(info, actions);
      list.append(row);
    }
  }
  async function reload() {
    const data = await api("/v1/management/external-applications");
    applications = data.applications;
    render();
  }
  function form() {
    return { client_id: input("external-client").value.trim(), name: input("external-name").value.trim(), assistant_id: $("external-assistant").value, redirect_uris: selected?.redirect_uris || [], scopes: [...document.querySelectorAll("#external-scopes input:checked")].map((c) => c.value), enabled: input("external-enabled").checked, require_device_id: requiredDevice || $("external-assistant").value === "carenest", allow_backend_identity: selected?.allow_backend_identity ?? true, access_ttl_seconds: Number(input("external-access-ttl").value), grant_ttl_seconds: Number(input("external-grant-ttl").value) };
  }
  for (const [scope, label] of Object.entries(scopeLabels)) {
    const wrapper = node("label", "");
    wrapper.className = "external-check";
    const check = node("input", "");
    check.type = "checkbox";
    check.value = scope;
    check.checked = true;
    wrapper.append(check, node("span", label));
    $("external-scopes").append(wrapper);
  }
  $("external-form").addEventListener("submit", (event) => {
    event.preventDefault();
    void guarded(async () => {
      if (selected && !confirm("\u4FEE\u6539\u914D\u7F6E\u5C06\u4F7F\u8BE5\u5E94\u7528\u7684\u539F\u7528\u6237\u6388\u6743\u5931\u6548\uFF0C\u7EE7\u7EED\uFF1F")) return;
      const body = form();
      const issued = selected ? await api(`/v1/management/external-applications/${encodeURIComponent(selected.client_id)}`, "PUT", { application: body, expected_revision: selected.revision, rotate_secret: false }) : await api("/v1/management/external-applications", "POST", body);
      fill(issued.application);
      showKey(issued);
      await reload();
      await grants(issued.application);
      message("\u5E94\u7528\u5DF2\u4FDD\u5B58");
    });
  });
  $("external-new").onclick = () => void guarded(async () => {
    await assistants();
    fill(null);
  });
  $("external-refresh").onclick = () => void guarded(async () => {
    await assistants();
    await reload();
    if (selected) {
      const app = applications.find((a) => a.client_id === selected?.client_id);
      if (app) {
        fill(app);
        await grants(app);
      } else {
        fill(null);
        clearSecret();
      }
    }
  });
  $("external-rotate").onclick = () => void guarded(async () => {
    if (!selected || !confirm("\u8F6E\u6362 Key \u5C06\u4F7F\u8BE5\u5E94\u7528\u7684\u539F\u7528\u6237\u6388\u6743\u5931\u6548\uFF0C\u7EE7\u7EED\uFF1F")) return;
    const issued = await api(`/v1/management/external-applications/${encodeURIComponent(selected.client_id)}`, "PUT", { application: { ...selected, revision: void 0 }, expected_revision: selected.revision, rotate_secret: true });
    fill(issued.application);
    showKey(issued);
    await reload();
    await grants(issued.application);
  });
  $("external-template").onclick = () => void guarded(async () => {
    const response = await fetch("/assets/carenest-assistant.json");
    if (!response.ok) throw Error("\u65E0\u6CD5\u8BFB\u53D6\u52A9\u7406\u6A21\u677F");
    const template = await response.json();
    const existing = await api("/v1/agent-profiles");
    if (!existing.some((p) => p.id === template.id)) await api("/v1/agent-profiles", "POST", template);
    await assistants();
    const select = $("external-assistant");
    select.value = template.id;
    requiredDevice = true;
    message(select.value === template.id ? "CareNest \u793A\u4F8B\u52A9\u7406\u5DF2\u51C6\u5907\u597D\uFF0C\u8BF7\u6309\u9700\u914D\u7F6E\u80FD\u529B\u3002" : "CareNest \u52A9\u7406\u5DF2\u5B58\u5728\uFF0C\u4F46\u5C1A\u672A\u542F\u7528\uFF0C\u8BF7\u5728\u667A\u80FD\u52A9\u7406\u9875\u9762\u914D\u7F6E\u3002");
  });
  $("external-copy").onclick = () => {
    void navigator.clipboard.writeText(input("external-issued-secret").value).then(() => message("Key \u5DF2\u590D\u5236")).catch(() => message("\u8BF7\u624B\u52A8\u590D\u5236 Key"));
  };
  $("external-key-close").onclick = () => {
    $("external-key-dialog").close();
    clearSecret();
  };
  $("external-key-dialog").addEventListener("close", clearSecret);
  window.addEventListener("pagehide", clearSecret);
  void guarded(async () => {
    await assistants();
    await reload();
    fill(null);
  });
})();
