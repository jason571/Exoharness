"use strict";
(() => {
  // src/external-conversations.ts
  var el = (id) => document.getElementById(id);
  function text(tag, value, data = false) {
    const node = document.createElement(tag);
    node.textContent = value;
    if (data) node.dataset.i18n = "off";
    return node;
  }
  function field(value, extra) {
    const cell = text("td", "");
    cell.append(text("span", value, true));
    if (extra) cell.append(text("small", extra, true));
    return cell;
  }
  function fileLink(name, value) {
    if (!value) return null;
    try {
      const parsed = new URL(value);
      if (!["http:", "https:"].includes(parsed.protocol) || parsed.username || parsed.password) return null;
      const link = text("a", name, true);
      link.href = value;
      link.target = "_blank";
      link.rel = "noopener noreferrer";
      return link;
    } catch {
      return null;
    }
  }
  function mountExternalConversations(api2, client2) {
    let page = 1, hasMore = false, total = 0, listRevision = 0, detailRevision = 0;
    let active = null, before = null, older = false;
    let loading = false, detailLoading = false;
    let applications = [];
    const missingDevice = (client3) => applications.find((app2) => app2.client_id === client3)?.require_device_id ? "\u8BBE\u5907\u5173\u8054\u5F85\u8865\u5168" : "\u672A\u8981\u6C42\u8BBE\u5907";
    const app = el("external-conversation-app");
    app.value = client2;
    const dialog = el("external-conversation-dialog");
    function pagination() {
      el("external-conversation-prev").disabled = loading || page <= 1;
      el("external-conversation-next").disabled = loading || !hasMore;
      el("external-conversation-total").textContent = String(total);
      el("external-conversation-page").textContent = String(page);
      el("external-conversation-search").disabled = loading;
    }
    function detailButtons() {
      el("external-conversation-older").disabled = detailLoading || !older;
      el("external-conversation-latest").disabled = detailLoading;
    }
    async function load(next) {
      const revision = ++listRevision;
      loading = true;
      pagination();
      el("external-conversation-rows").replaceChildren();
      el("external-conversation-note").textContent = "\u6B63\u5728\u8BFB\u53D6\u5916\u90E8\u5BF9\u8BDD";
      const params = new URLSearchParams({ page: String(next), limit: "10" });
      for (const [key, id] of [["q", "external-conversation-query"], ["user", "external-conversation-user"], ["device", "external-conversation-device"]]) {
        const value = el(id).value.trim();
        if (value) params.set(key, value);
      }
      params.set("client_id", client2);
      try {
        const result = await api2(`/v1/management/external-conversations?${params}`);
        if (revision !== listRevision) return;
        page = result.page;
        total = result.total;
        hasMore = result.has_more;
        const body = el("external-conversation-rows");
        body.replaceChildren();
        for (const session of result.sessions.filter((session2) => session2.client_id === client2)) {
          const row = document.createElement("tr");
          const title = field(session.title, session.id);
          const open = text("button", "\u67E5\u770B\u5BF9\u8BDD");
          open.className = "button secondary";
          open.type = "button";
          open.onclick = () => {
            active = session;
            before = null;
            void show();
          };
          title.append(open);
          const device = session.device_id ? field(session.device_id) : text("td", missingDevice(session.client_id));
          row.append(
            title,
            field(session.client_id, session.assistant_id),
            field(session.user_label || session.user_id, session.external_user_id || session.user_id),
            device,
            field(String(session.turn_count)),
            field(new Date(session.updated_at).toLocaleString())
          );
          body.append(row);
        }
        el("external-conversation-note").textContent = result.sessions.length ? "\u6BCF\u9875\u6700\u591A\u663E\u793A 10 \u6761\u5BF9\u8BDD" : "\u6CA1\u6709\u7B26\u5408\u6761\u4EF6\u7684\u5916\u90E8\u5BF9\u8BDD";
      } catch (error) {
        if (revision === listRevision) el("external-conversation-note").textContent = error instanceof Error ? error.message : "\u8BFB\u53D6\u5916\u90E8\u5BF9\u8BDD\u5931\u8D25";
      } finally {
        if (revision === listRevision) {
          loading = false;
          pagination();
        }
      }
    }
    async function show() {
      if (!active) return;
      const session = active, revision = ++detailRevision;
      detailLoading = true;
      older = false;
      detailButtons();
      el("external-conversation-content").replaceChildren();
      el("external-conversation-detail-info").replaceChildren();
      el("external-conversation-detail-note").textContent = "\u6B63\u5728\u8BFB\u53D6\u5BF9\u8BDD\u5185\u5BB9";
      el("external-conversation-detail-title").textContent = session.title;
      if (!dialog.open) dialog.showModal();
      const params = new URLSearchParams({ turn_limit: "10" });
      if (before !== null) params.set("before_ordinal", String(before));
      try {
        const detail = await api2(`/v1/management/external-applications/${encodeURIComponent(session.client_id)}/sessions/${encodeURIComponent(session.id)}?${params}`);
        if (revision !== detailRevision || !dialog.open) return;
        const info = el("external-conversation-detail-info");
        info.replaceChildren();
        const fields = [["\u5E94\u7528 ID", detail.external.client_id], ["\u7528\u6237", detail.external.user_label], ["\u7528\u6237 ID", detail.external.user_id], ["\u5916\u90E8\u7528\u6237 ID", detail.external.external_user_id || null], ["\u8BBE\u5907 ID", detail.external.device_id], ["\u4F1A\u8BDD ID", detail.id], ["\u5BF9\u8BDD\u8F6E\u6570", String(detail.external.turn_count)]];
        for (const [label, value] of fields) {
          const item = text("div", "");
          item.append(text("strong", label));
          item.append(value ? text("span", value, true) : text("span", label === "\u8BBE\u5907 ID" ? missingDevice(session.client_id) : "\u672A\u63D0\u4F9B"));
          info.append(item);
        }
        const content = el("external-conversation-content");
        content.replaceChildren();
        for (const turn of detail.turns) {
          const article = text("article", "");
          article.className = "external-conversation-turn";
          const heading = text("header", "");
          heading.append(text("strong", "\u8F6E\u6B21"), text("span", String(turn.ordinal), true));
          if (turn.run_status) heading.append(text("span", turn.run_status, true));
          if (turn.run_id) heading.append(text("small", turn.run_id, true));
          article.append(heading);
          article.append(text("h3", "\u7528\u6237"));
          const prompt = text("div", turn.prompt, true);
          prompt.className = "external-conversation-prose";
          article.append(prompt);
          for (const attachment of turn.attachments || []) {
            const link = fileLink(attachment.name || "attachment", attachment.download_url || attachment.url);
            if (link) article.append(link);
          }
          article.append(text("h3", "\u52A9\u7406"));
          const value = turn.answer || turn.response?.answer || "";
          const answer = text("div", value, true);
          answer.className = "external-conversation-prose";
          const direct = /^https?:\/\/\S+$/.test(value.trim()) ? fileLink(value.trim(), value.trim()) : null;
          if (direct) answer.replaceChildren(direct);
          article.append(answer);
          const notice = turn.response?.public_notice?.message;
          if (notice) article.append(text("p", notice));
          const files = text("div", "");
          files.className = "external-conversation-files";
          for (const artifact of turn.response?.artifacts || []) {
            const link = fileLink(artifact.filename || "file", artifact.download_url || artifact.url);
            if (link) files.append(link);
          }
          article.append(files);
          content.append(article);
        }
        older = detail.history_page.has_more;
        before = detail.history_page.before_ordinal;
        el("external-conversation-detail-note").textContent = detail.turns.length ? "\u6BCF\u9875\u6700\u591A\u663E\u793A 10 \u8F6E\u6D88\u606F" : "\u6682\u65E0\u5BF9\u8BDD\u5185\u5BB9";
      } catch (error) {
        if (revision === detailRevision) el("external-conversation-detail-note").textContent = error instanceof Error ? error.message : "\u8BFB\u53D6\u5BF9\u8BDD\u5185\u5BB9\u5931\u8D25";
      } finally {
        if (revision === detailRevision) {
          detailLoading = false;
          detailButtons();
        }
      }
    }
    el("external-conversation-filter").onsubmit = (event) => {
      event.preventDefault();
      void load(1);
    };
    el("external-conversation-prev").onclick = () => {
      void load(Math.max(1, page - 1));
    };
    el("external-conversation-next").onclick = () => {
      void load(page + 1);
    };
    el("external-conversation-older").onclick = () => {
      void show();
    };
    el("external-conversation-latest").onclick = () => {
      before = null;
      void show();
    };
    el("external-conversation-close").onclick = () => dialog.close();
    dialog.addEventListener("close", () => {
      ++detailRevision;
      active = null;
      detailLoading = false;
      el("external-conversation-content").replaceChildren();
    });
    pagination();
    detailButtons();
    return {
      applications(values) {
        applications = values.filter((item) => item.client_id === client2);
        if (!applications.length) {
          el("external-conversation-note").textContent = "\u627E\u4E0D\u5230\u6B64\u5E94\u7528";
          return;
        }
        el("external-conversation-application-name").textContent = `${applications[0].name} \xB7 ${client2}`;
        void load(1);
      }
    };
  }
  async function api(path) {
    const auth = window.ExoAuth;
    const response = await auth.fetch(path);
    const value = await response.json();
    if (!response.ok) throw Error(value?.error?.message || value?.message || String(response.status));
    return value;
  }
  var client = new URLSearchParams(location.search).get("client_id");
  if (!client || !/^[A-Za-z0-9_.-]{1,128}$/.test(client)) {
    el("external-conversation-note").textContent = "\u8BF7\u4ECE\u5916\u90E8\u5E94\u7528\u5217\u8868\u9009\u62E9\u5E94\u7528";
  } else {
    const conversations = mountExternalConversations(api, client);
    void api("/v1/management/external-applications").then((result) => conversations.applications(result.applications)).catch(() => {
      el("external-conversation-note").textContent = "\u8BFB\u53D6\u5916\u90E8\u5BF9\u8BDD\u5931\u8D25";
    });
  }
})();
