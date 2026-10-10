"use strict";

(() => {
  const $ = (id) => document.getElementById(id);
  const node = (tag, text, className) => {
    const el = document.createElement(tag);
    if (text !== undefined && text !== null) el.textContent = text;
    if (className) el.className = className;
    return el;
  };
  const number = (value) => Number(value || 0).toLocaleString();
  const date = (value) => {
    const timestamp = Number(value || 0);
    if (!timestamp) return "—";
    // Auth account timestamps are seconds; execution timestamps are millis.
    return new Date(timestamp < 10_000_000_000 ? timestamp * 1000 : timestamp).toLocaleString();
  };
  const statuses = {
    running: "执行中", pending: "准备中", queued: "排队中",
    waiting_authorization: "等待授权", recoverable: "可恢复",
    failed: "失败", succeeded: "已完成", cancelled: "已取消"
  };
  const state = { users: [], assistants: [], defaultAssistant: "xiaolan", selected: null, loading: false, detailLoading: false };

  function toast(message, error = false) {
    const target = $("toast");
    target.textContent = message;
    target.classList.toggle("error", error);
    target.classList.add("visible");
    window.clearTimeout(toast.timer);
    toast.timer = window.setTimeout(() => target.classList.remove("visible"), 3600);
  }

  function assistantOptions() {
    const assignable = (item) => item.assignable ?? (item.enabled && (item.is_default || item.published));
    const options = state.assistants.filter(assignable);
    return options;
  }

  function renderConversation(conversation) {
    const item = node("article", null, "user-conversation");
    const head = node("div", null, "user-conversation-head");
    const title = node("div", null, "user-conversation-title");
    title.append(node("strong", conversation.title || "未命名对话"));
    title.append(node("small", `${conversation.turn_count || 0} 轮消息 · 更新于 ${date(conversation.updated_at)}`));
    const status = node("span", statuses[conversation.latest_run_status] || conversation.latest_run_status || "未执行", "user-conversation-status");
    status.dataset.status = conversation.latest_run_status || "none";
    head.append(title, status);
    const prompt = node("p", conversation.latest_prompt || "暂无执行提示词", "user-conversation-prompt");
    const metrics = node("div", null, "user-conversation-metrics");
    metrics.append(node("span", `${number(conversation.run_count)} 次执行`), node("span", `${number(conversation.total_tokens)} Token`));
    const link = node("a", "查看完整 Trace →", "user-trace-link");
    link.href = `/trace?session=${encodeURIComponent(conversation.id)}`;
    metrics.append(link);
    item.append(head, prompt, metrics);
    return item;
  }

  function renderUserDetail(record) {
    const card = node("article", null, "user-detail-content");
    const header = node("div", null, "user-record-header");
    const identity = node("div", null, "user-identity");
    identity.append(node("span", (record.display_name || record.username || "U").slice(0, 1).toUpperCase(), "user-avatar"));
    const name = node("div");
    name.append(node("h2", record.display_name || record.username));
    name.append(node("p", `${record.username}${record.email ? ` · ${record.email}` : ""}`));
    identity.append(name);
    const created = node("small", `注册于 ${date(record.created_at)}`, "user-created");
    header.append(identity, created);

    const assignment = node("div", null, "user-assignment");
    const label = node("label", "默认助理");
    const select = document.createElement("select");
    select.setAttribute("aria-label", `为 ${record.display_name || record.username} 指定默认助理`);
    const options = assistantOptions();
    const assignableOptions = options.filter((profile) => profile.assignable ?? (profile.enabled && (profile.is_default || profile.published)));
    if (!assignableOptions.some((profile) => profile.id === record.assistant_profile_id)) {
      options.unshift({ id: "", name: assignableOptions.length ? "选择已启用并发布的助理" : "暂无已启用并发布的助理", assignable: false, placeholder: true });
    }
    for (const profile of options) {
      const option = document.createElement("option");
      option.value = profile.id;
      option.textContent = profile.name ? `${profile.name} · ${profile.id}` : profile.id;
      option.disabled = !profile.placeholder && !(profile.assignable ?? (profile.enabled && (profile.is_default || profile.published)));
      select.append(option);
    }
    select.value = assignableOptions.some((profile) => profile.id === record.assistant_profile_id) ? record.assistant_profile_id : "";
    select.disabled = assignableOptions.length === 0;
    const save = node("button", "保存", "button primary user-assignment-save");
    save.type = "button";
    save.disabled = true;
    const availability = node("span", record.assistant_available ? "可用" : assignableOptions.length ? `当前助理“${record.assistant_name || record.assistant_profile_id}”不可用，请选择已启用并发布的助理` : "暂无可分配助理（需先启用并发布）", "user-assistant-availability");
    availability.dataset.available = String(record.assistant_available);
    let original = record.assistant_profile_id;
    select.addEventListener("change", () => { save.disabled = select.value === original; });
    save.addEventListener("click", async () => {
      save.disabled = true;
      save.textContent = "保存中…";
      try {
        const response = await window.ExoAuth.fetch(`/v1/management/users/${encodeURIComponent(record.id)}/assistant`, {
          method: "PUT",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ agent_profile_id: select.value })
        });
        const payload = await response.json().catch(() => ({}));
        if (!response.ok) throw new Error(payload.message || payload.error || `保存失败（${response.status}）`);
        record.assistant_profile_id = payload.assistant_profile_id;
        original = record.assistant_profile_id;
        record.assistant_name = payload.assistant_name;
        record.assistant_available = true;
        const summary = state.users.find((user) => user.id === record.id);
        if (summary) {
          summary.assistant_profile_id = record.assistant_profile_id;
          summary.assistant_name = record.assistant_name;
          summary.assistant_available = true;
        }
        availability.textContent = "可用";
        availability.dataset.available = "true";
        renderUserList();
        toast(`已为 ${record.display_name || record.username} 更新默认助理`);
      } catch (error) {
        toast(error.message, true);
        select.value = record.assistant_profile_id;
      } finally {
        save.textContent = "保存";
        save.disabled = true;
      }
    });
    label.append(select);
    assignment.append(label, save, availability);

    const policySection = node("section", null, "user-capability-policy");
    policySection.append(node("h3", "能力权限"));
    policySection.append(node("p", "安装权限默认关闭；使用权限仍受能力归属、分享和助理授权限制。用户在对话中可从公开 GitHub 安装 Skill、本地 MCP，或连接地址型远程 MCP；Tool 脚本由独立权限控制。用户上传文件不能安装 Skill/MCP，管理员技能导入不受此限制。"));
    const policy = { install_skills: false, install_mcp: false, install_tools: false,
      use_skills: true, use_mcp: true, use_tools: true, ...(record.capability_policy || {}) };
    const policyGrid = node("div", null, "user-capability-policy-grid");
    policyGrid.append(node("span", "能力类型"), node("span", "允许安装"), node("span", "允许使用"));
    const inputs = {};
    const policySave = node("button", "保存权限", "button primary");
    policySave.type = "button";
    policySave.disabled = true;
    for (const [kind, title] of [["skills", "Skill"], ["mcp", "MCP"], ["tools", "Tool"]]) {
      policyGrid.append(node("strong", title));
      for (const action of ["install", "use"]) {
        const field = `${action}_${kind}`;
        const wrapper = node("label", null, "user-policy-checkbox");
        const input = document.createElement("input");
        input.type = "checkbox";
        input.checked = Boolean(policy[field]);
        input.setAttribute("aria-label", `${record.display_name || record.username} ${action === "install" ? "允许安装" : "允许使用"}${title}`);
        inputs[field] = input;
        wrapper.append(input, node("span", input.checked ? "开启" : "关闭"));
        input.addEventListener("change", () => {
          wrapper.lastChild.textContent = input.checked ? "开启" : "关闭";
          policySave.disabled = false;
        });
        policyGrid.append(wrapper);
      }
    }
    policySave.addEventListener("click", async () => {
      policySave.disabled = true;
      try {
        const next = Object.fromEntries(Object.entries(inputs).map(([field, input]) => [field, input.checked]));
        const response = await window.ExoAuth.fetch(`/v1/management/users/${encodeURIComponent(record.id)}/capability-policy`, {
          method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify({ policy: next })
        });
        const payload = await response.json().catch(() => ({}));
        if (!response.ok) throw new Error(payload.message || payload.error || `保存失败（${response.status}）`);
        record.capability_policy = payload.capability_policy;
        const summary = state.users.find((user) => user.id === record.id);
        if (summary) summary.capability_policy = payload.capability_policy;
        toast(`已更新 ${record.display_name || record.username} 的能力权限`);
      } catch (error) {
        policySave.disabled = false;
        toast(error.message, true);
      }
    });
    policySection.append(policyGrid, policySave);

    const installed = node("section", null, "user-installed-capabilities");
    installed.append(node("h3", "用户安装的能力"));
    if (!record.installed_capabilities?.length) {
      installed.append(node("p", "该用户尚未安装 Skill、MCP 或 Tool。", "settings-empty"));
    } else {
      const installedList = node("div", null, "user-installed-list");
      for (const capability of record.installed_capabilities) {
        const row = node("div", null, "user-installed-item");
        row.append(node("strong", capability.name || capability.id));
        row.append(node("small", `${capability.kind || "能力"} · revision ${capability.revision} · ${capability.enabled ? "已启用" : "未启用"} · ${capability.preparation_status || "未检查"}${capability.shareable ? " · 可按用户分享" : " · 私有，不可分享"}`));
        if (capability.source?.url) row.append(node("small", `来源 ${capability.source.url}`));
        if (capability.preparation_message) row.append(node("small", capability.preparation_message));
        if (capability.permissions?.length) row.append(node("small", `声明权限 ${capability.permissions.length} 项`));
        if (capability.shareable) {
          const sharing = document.createElement("details");
          sharing.className = "user-share-picker";
          sharing.append(node("summary", `管理分享 · 已分享 ${capability.shared_user_ids?.length || 0} 人`));
          const search = document.createElement("input");
          search.type = "search";
          search.placeholder = "搜索接收用户";
          search.setAttribute("aria-label", `搜索 ${capability.name} 的接收用户`);
          const targets = node("div", null, "user-share-targets");
          let allUsers = [];
          const renderTargets = () => {
            targets.replaceChildren();
            const term = search.value.trim().toLowerCase();
            const matching = allUsers.filter((user) => user.id !== record.id &&
              (!term || [user.username, user.display_name, user.email].some((value) => String(value || "").toLowerCase().includes(term))));
            for (const user of matching.slice(0, 50)) {
              const label = node("label", null, "user-share-target");
              const input = document.createElement("input");
              input.type = "checkbox";
              input.checked = (capability.shared_user_ids || []).includes(user.id);
              input.addEventListener("change", async () => {
                input.disabled = true;
                try {
                  const response = await window.ExoAuth.fetch(`/v1/capabilities/${encodeURIComponent(capability.id)}/shares/${encodeURIComponent(user.id)}`, {
                    method: "PUT", headers: { "content-type": "application/json" },
                    body: JSON.stringify({ enabled: input.checked })
                  });
                  const payload = await response.json().catch(() => ({}));
                  if (!response.ok) throw new Error(payload.message || payload.error || `更新失败（${response.status}）`);
                  const set = new Set(capability.shared_user_ids || []);
                  if (input.checked) set.add(user.id); else set.delete(user.id);
                  capability.shared_user_ids = [...set];
                  sharing.querySelector("summary").textContent = `管理分享 · 已分享 ${set.size} 人`;
                  toast(`已${input.checked ? "分享给" : "取消分享给"} ${user.display_name || user.username}`);
                } catch (error) {
                  input.checked = !input.checked;
                  toast(error.message, true);
                } finally { input.disabled = false; }
              });
              label.append(input, node("span", `${user.display_name || user.username} · ${user.username}`));
              targets.append(label);
            }
            if (matching.length > 50) targets.append(node("small", "仅显示前 50 位，请继续输入关键词。"));
            if (!matching.length) targets.append(node("small", "没有匹配用户。"));
          };
          search.addEventListener("input", renderTargets);
          sharing.addEventListener("toggle", async () => {
            if (!sharing.open || allUsers.length) return;
            targets.replaceChildren(node("small", "正在读取用户…"));
            try {
              const response = await window.ExoAuth.fetch("/v1/management/users");
              const payload = await response.json().catch(() => ({}));
              if (!response.ok) throw new Error(payload.message || payload.error || `读取失败（${response.status}）`);
              allUsers = payload.users || [];
              renderTargets();
            } catch (error) { targets.replaceChildren(node("small", error.message)); }
          });
          sharing.append(search, targets);
          row.append(sharing);
        }
        installedList.append(row);
      }
      installed.append(installedList);
    }

    const installJobs = node("section", null, "user-installed-capabilities");
    installJobs.append(node("h3", "安装记录"));
    if (!record.install_jobs?.length) {
      installJobs.append(node("p", "暂无 GitHub 安装任务。", "settings-empty"));
    } else {
      const jobList = node("div", null, "user-installed-list");
      for (const job of record.install_jobs) {
        const row = node("div", null, "user-installed-item");
        const status = {
          queued: "排队中", downloading: "下载中", registering: "登记中",
          preparing: "检查中", ready: "可用", blocked: "未就绪", cancelled: "已取消"
        }[job.status] || job.status;
        row.append(
          node("strong", `${job.kind === "mcp" ? "MCP" : "Skill"} · ${status}`),
          node("small", job.url || ""),
          node("small", job.message || ""),
        );
        if (job.commit) row.append(node("small", `固定提交 ${job.commit.slice(0, 12)}`));
        if (job.capability_id) row.append(node("small", `能力 ${job.capability_id}`));
        jobList.append(row);
      }
      installJobs.append(jobList);
    }

    const conversations = node("section", null, "user-conversations");
    const heading = node("div", null, "user-conversations-heading");
    const conversationCount = Number(record.conversation_count ?? record.conversations?.length ?? 0);
    heading.append(node("h3", "执行对话"), node("span", `${number(conversationCount)} 个会话`, "count-badge"));
    conversations.append(heading);
    if (state.detailLoading) {
      conversations.append(node("p", "正在读取该用户的对话记录…", "settings-empty"));
    } else if (!record.conversations?.length) {
      conversations.append(node("p", "该用户还没有执行记录。", "settings-empty"));
    } else {
      const list = node("div", null, "user-conversation-list");
      record.conversations.forEach((conversation) => list.append(renderConversation(conversation)));
      conversations.append(list);
    }
    card.append(header, assignment, policySection, installed, installJobs, conversations);
    return card;
  }

  function renderUserList() {
    const list = $("users-list");
    list.replaceChildren();
    if (!state.users.length) {
      list.append(node("div", "没有匹配的注册用户。", "settings-empty users-empty"));
      return;
    }
    state.users.forEach((record) => {
      const item = node("button", null, "user-list-item");
      item.type = "button";
      item.setAttribute("aria-pressed", String(state.selected?.id === record.id));
      item.classList.toggle("active", state.selected?.id === record.id);
      item.setAttribute("aria-label", `查看用户 ${record.display_name || record.username}`);
      const avatar = node("span", (record.display_name || record.username || "U").slice(0, 1).toUpperCase(), "user-list-avatar");
      const main = node("span", null, "user-list-main");
      main.append(node("strong", record.display_name || record.username || "未命名用户", "user-list-name"));
      main.append(node("small", `${record.username || ""}${record.email ? ` · ${record.email}` : ""}`, "user-list-meta"));
      const assistant = record.assistant_name || record.assistant_profile_id || "未指定助理";
      main.append(node("small", `助理：${assistant}`, "user-list-assistant"));
      const side = node("span", null, "user-list-side");
      side.append(node("strong", number(record.conversation_count), "user-list-count"));
      side.append(node("small", "个会话", "user-list-meta"));
      item.append(avatar, main, side);
      item.addEventListener("click", () => { void selectUser(record); });
      list.append(item);
    });
  }

  function renderDetailEmpty() {
    const detail = $("user-detail");
    detail.replaceChildren();
    const empty = node("div", null, "users-detail-empty");
    empty.append(node("span", "◎"));
    empty.append(node("h2", state.users.length ? "选择一个用户" : "暂无用户"));
    empty.append(node("p", state.users.length ? "用户详情、默认助理和执行对话会显示在这里。" : "没有符合当前筛选条件的注册用户。"));
    detail.append(empty);
  }

  function renderDetail() {
    const detail = $("user-detail");
    if (!state.selected) {
      renderDetailEmpty();
      return;
    }
    detail.replaceChildren(renderUserDetail(state.selected));
  }

  async function selectUser(record) {
    state.selected = { ...record, conversations: [] };
    state.detailLoading = true;
    renderUserList();
    renderDetail();
    await loadUserDetail(record.id);
  }

  let detailRequestId = 0;
  async function loadUserDetail(userId) {
    const requestId = ++detailRequestId;
    try {
      const response = await window.ExoAuth.fetch(`/v1/management/users/${encodeURIComponent(userId)}`);
      const payload = await response.json().catch(() => ({}));
      if (!response.ok) throw new Error(payload.message || payload.error || `读取用户详情失败（${response.status}）`);
      if (requestId !== detailRequestId || state.selected?.id !== userId) return;
      state.selected = { ...state.selected, ...payload, conversations: payload.conversations || [] };
      state.detailLoading = false;
      const summary = state.users.find((user) => user.id === userId);
      if (summary) {
        summary.conversation_count = state.selected.conversation_count;
        summary.assistant_profile_id = state.selected.assistant_profile_id;
        summary.assistant_name = state.selected.assistant_name;
        summary.assistant_available = state.selected.assistant_available;
      }
      renderUserList();
      renderDetail();
    } catch (error) {
      if (requestId !== detailRequestId || state.selected?.id !== userId) return;
      state.detailLoading = false;
      const detail = $("user-detail");
      detail.replaceChildren();
      const failure = node("div", null, "users-detail-empty");
      failure.append(node("span", "!", "users-detail-error-icon"));
      failure.append(node("h2", "用户详情读取失败"));
      failure.append(node("p", error.message));
      const retry = node("button", "重试", "button secondary");
      retry.type = "button";
      retry.addEventListener("click", () => { state.detailLoading = true; renderDetail(); void loadUserDetail(userId); });
      failure.append(retry);
      detail.append(failure);
    }
  }

  function render() {
    $("users-count").textContent = number(state.users.length);
    $("users-active-count").textContent = number(state.users.filter((user) => Number(user.conversation_count) > 0).length);
    $("users-conversation-count").textContent = number(state.users.reduce((sum, user) => sum + Number(user.conversation_count || 0), 0));
    $("users-list-count").textContent = `${number(state.users.length)} 个`;
    renderUserList();
    if (state.selected && !state.users.some((user) => user.id === state.selected.id)) state.selected = null;
    renderDetail();
  }

  async function load() {
    if (state.loading) return;
    state.loading = true;
    $("users-refresh").disabled = true;
    const search = $("users-search").value.trim();
    const query = search ? `?search=${encodeURIComponent(search)}` : "";
    try {
      const response = await window.ExoAuth.fetch(`/v1/management/users${query}`);
      const payload = await response.json().catch(() => ({}));
      if (!response.ok) throw new Error(payload.message || payload.error || `读取失败（${response.status}）`);
      state.users = payload.users || [];
      state.assistants = payload.assistants || [];
      state.defaultAssistant = payload.default_assistant_id || "xiaolan";
      render();
      $("users-status").textContent = `${search ? `筛选：${search} · ` : ""}更新于 ${new Date(payload.generated_at || Date.now()).toLocaleTimeString()}`;
      $("users-error").hidden = true;
    } catch (error) {
      $("users-error").hidden = false;
      $("users-error").textContent = error.message;
      $("users-status").textContent = "用户数据读取失败";
    } finally {
      state.loading = false;
      $("users-refresh").disabled = false;
    }
  }

  $("users-filter").addEventListener("submit", (event) => { event.preventDefault(); void load(); });
  let timer;
  $("users-search").addEventListener("input", () => { clearTimeout(timer); timer = setTimeout(load, 250); });
  void load();
})();
