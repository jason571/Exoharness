// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

"use strict";
(() => {
  // src/knowledge-import.ts
  var MAX_SOURCE_BYTES = 2 * 1024 * 1024;
  var MAX_CONTENT_BYTES = 1024 * 1024;
  var TEXT_EXTENSIONS = /* @__PURE__ */ new Set(["txt", "md", "markdown", "csv", "json", "jsonl", "html", "htm"]);
  function knowledgeImportWarnings(value) {
    if (!value || typeof value !== "object" || !("warnings" in value) || !Array.isArray(value.warnings)) return [];
    const messages = {
      pdf_table_structure_unverified: "\u5DF2\u5BFC\u5165\u7248\u9762\u6587\u5B57\uFF0C\u4F46 PDF \u8868\u683C\u884C\u5217\u672A\u9A8C\u8BC1\uFF0C\u4E0D\u4F1A\u731C\u6D4B\u6216\u91CD\u5EFA\u8868\u683C",
      page_without_text_layer: "\u90E8\u5206\u9875\u9762\u6CA1\u6709\u6587\u5B57\u5C42\uFF0C\u672A\u8BC6\u522B\u8FD9\u4E9B\u9875\u9762\u4E2D\u7684\u56FE\u7247",
      images_not_interpreted: "\u5DF2\u4FDD\u5B58\u6B63\u6587\uFF0C\u5185\u5D4C\u56FE\u7247\u672A\u8FDB\u884C\u8BC6\u522B",
      external_links_not_fetched: "\u5916\u90E8\u94FE\u63A5\u672A\u8BBF\u95EE\uFF0C\u94FE\u63A5\u5185\u5BB9\u672A\u5BFC\u5165",
      unsupported_word_block: "\u90E8\u5206\u7279\u6B8A Word \u5185\u5BB9\u4E0D\u652F\u6301\u89E3\u6790\uFF0C\u8BF7\u6838\u5BF9\u6B63\u6587"
    };
    return value.warnings.map((code) => typeof code === "string" ? messages[code] || "\u90E8\u5206\u5185\u5BB9\u5B58\u5728\u89E3\u6790\u9650\u5236\uFF0C\u8BF7\u6838\u5BF9\u8D44\u6599" : "");
  }
  async function readKnowledgeTextFile(file) {
    const extension = file.name.split(".").pop()?.toLowerCase() || "";
    if (extension === "docx" || extension === "pdf") {
      if (!file.size || file.size > 4 * 1024 * 1024) throw new Error(`${file.name}\uFF1A\u6587\u4EF6\u4E3A\u7A7A\u6216\u8D85\u8FC7 4 MB\u3002`);
      const bytes = new Uint8Array(await file.arrayBuffer());
      const parts = [];
      for (let offset = 0; offset < bytes.length; offset += 32768) {
        parts.push(String.fromCharCode(...bytes.subarray(offset, offset + 32768)));
      }
      return { title: file.name, filename: file.name, file_base64: btoa(parts.join("")) };
    }
    if (!TEXT_EXTENSIONS.has(extension)) {
      throw new Error(`${file.name}\uFF1A\u652F\u6301 DOCX\u3001\u6587\u5B57\u578B PDF\u3001TXT\u3001Markdown\u3001CSV\u3001JSON/JSONL \u548C HTML\uFF1BDOC \u8BF7\u5148\u8F6C\u6362\u4E3A DOCX\u3002`);
    }
    if (file.size > MAX_SOURCE_BYTES) throw new Error(`${file.name}\uFF1A\u6587\u4EF6\u8D85\u8FC7 2 MB\uFF0C\u8BF7\u62C6\u5206\u540E\u518D\u5BFC\u5165\u3002`);
    let content;
    try {
      content = new TextDecoder("utf-8", { fatal: true }).decode(await file.arrayBuffer());
    } catch {
      throw new Error(`${file.name}\uFF1A\u6587\u4EF6\u4E0D\u662F UTF-8 \u7F16\u7801\uFF0C\u8BF7\u8F6C\u6362\u540E\u91CD\u8BD5\u3002`);
    }
    if (extension === "html" || extension === "htm") {
      const html = new DOMParser().parseFromString(content, "text/html");
      html.querySelectorAll("script, style, noscript, template").forEach((element2) => element2.remove());
      content = html.body.textContent || "";
    }
    content = content.replaceAll("\0", "").trim();
    if (!content) throw new Error(`${file.name}\uFF1A\u672A\u627E\u5230\u53EF\u68C0\u7D22\u7684\u6587\u5B57\u3002`);
    if (new TextEncoder().encode(content).byteLength > MAX_CONTENT_BYTES) {
      throw new Error(`${file.name}\uFF1A\u63D0\u53D6\u7684\u6587\u5B57\u8D85\u8FC7 1 MB\uFF0C\u8BF7\u62C6\u5206\u540E\u518D\u5BFC\u5165\u3002`);
    }
    const title = Array.from(file.name.trim()).slice(0, 160).join("");
    if (!title || Array.from(title).some((character) => /[\u0000-\u001f\u007f]/.test(character))) {
      throw new Error("\u6587\u4EF6\u540D\u65E0\u6548\uFF0C\u8BF7\u91CD\u547D\u540D\u540E\u91CD\u8BD5\u3002");
    }
    return { title, content };
  }

  // src/admin-knowledge.ts
  var state = {
    libraries: [],
    agents: [],
    categories: [],
    selected: null
  };
  var $ = (selector) => {
    const node = document.querySelector(selector);
    if (!node) throw new Error(`Missing element: ${selector}`);
    return node;
  };
  function element(tag, text = "", className = "") {
    const node = document.createElement(tag);
    node.textContent = text;
    node.className = className;
    return node;
  }
  function showError(error) {
    const view = $("#knowledge-error");
    view.textContent = error instanceof Error ? error.message : String(error);
    view.classList.remove("hidden");
  }
  async function api(url, method = "GET", body) {
    const response = await window.ExoAuth.fetch(url, {
      method,
      headers: body === void 0 ? void 0 : { "content-type": "application/json" },
      body: body === void 0 ? void 0 : JSON.stringify(body)
    });
    const text = await response.text();
    const result = text ? JSON.parse(text) : null;
    if (!response.ok) throw new Error(result?.error?.message || `${response.status} ${response.statusText}`);
    return result;
  }
  function guard(callback) {
    return () => {
      $("#knowledge-error").classList.add("hidden");
      void callback().catch(showError);
    };
  }
  function action(label, callback) {
    const button = element("button", label, "button secondary");
    button.type = "button";
    button.addEventListener("click", () => {
      button.disabled = true;
      void callback().catch(showError).finally(() => {
        button.disabled = false;
      });
    });
    return button;
  }
  function deleteAction(callback) {
    const button = action("\u5220\u9664", async () => {
      if (button.dataset.confirm !== "yes") {
        button.dataset.confirm = "yes";
        button.textContent = "\u518D\u6B21\u70B9\u51FB\u786E\u8BA4";
        window.setTimeout(() => {
          button.dataset.confirm = "";
          button.textContent = "\u5220\u9664";
        }, 5e3);
        return;
      }
      await callback();
    });
    return button;
  }
  function clearDocument() {
    $("#library-document-form").reset();
    $("#library-document-id").value = "";
    $("#library-document-revision").value = "";
  }
  function clearCategory() {
    $("#library-category-form").reset();
    $("#library-category-id").value = "";
    $("#library-category-revision").value = "";
  }
  async function loadCategories(library) {
    const categories = await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/categories`);
    if (state.selected?.id !== library.id) return;
    state.categories = categories;
    const parent = $("#library-category-parent");
    const quickParent = $("#library-quick-category-parent");
    const documentCategory = $("#library-document-category");
    const parentValue = parent.value;
    const quickParentValue = quickParent.value;
    const documentValue = documentCategory.value;
    parent.replaceChildren(new Option("\u9876\u5C42\u5206\u7C7B\uFF08\u7B2C\u4E00\u7EA7\uFF09", ""));
    quickParent.replaceChildren(new Option("\u9876\u5C42\u5206\u7C7B\uFF08\u7B2C\u4E00\u7EA7\uFF09", ""));
    documentCategory.replaceChildren(new Option("\u672A\u5206\u7C7B", ""));
    categories.forEach((category) => {
      const depth = category.path.split("/").length;
      if (depth < 3) {
        const label = `${depth + 1} \u7EA7 \xB7 ${category.path} \u4E0B`;
        parent.add(new Option(label, category.id));
        quickParent.add(new Option(label, category.id));
      }
      documentCategory.add(new Option(`${depth} \u7EA7 \xB7 ${category.path}`, category.id));
    });
    parent.value = parentValue;
    quickParent.value = quickParentValue;
    documentCategory.value = documentValue;
    const list = $("#library-category-list");
    list.replaceChildren();
    if (!categories.length) list.append(element("p", "\u5C1A\u65E0\u5206\u7C7B\uFF1B\u53EF\u5148\u521B\u5EFA\u5206\u7C7B\uFF0C\u518D\u7ED9\u8D44\u6599\u5F52\u7C7B\u3002"));
    categories.forEach((category) => {
      const card = element("article", "", "portal-knowledge-item");
      card.append(element("strong", category.path));
      const actions = element("div", "", "portal-knowledge-actions");
      actions.append(
        action("\u7F16\u8F91", async () => {
          $("#library-category-id").value = category.id;
          $("#library-category-revision").value = String(category.revision);
          $("#library-category-name").value = category.name;
          $("#library-category-parent").value = category.parent_id || "";
          $("#library-category-management").open = true;
          $("#library-category-name").focus();
        }),
        deleteAction(async () => {
          await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/categories/${encodeURIComponent(category.id)}`, "DELETE");
          clearCategory();
          await loadCategories(library);
          await loadDocuments(library);
        })
      );
      card.append(actions);
      list.append(card);
    });
  }
  function clearLibrary() {
    state.selected = null;
    $("#library-form").reset();
    $("#library-description-details").open = false;
    $("#library-id").value = "";
    $("#library-revision").value = "";
    $("#library-editor-title").textContent = "\u7B2C\u4E00\u6B65 \xB7 \u65B0\u5EFA\u8D44\u6599\u5E93";
    $("#library-content").inert = true;
    $("#library-content").setAttribute("aria-disabled", "true");
    $("#library-prerequisite").hidden = false;
    $("#library-delete").hidden = true;
    $("#library-message").textContent = "";
    clearDocument();
    clearCategory();
    renderLibraries();
  }
  function renderLibraries() {
    const list = $("#library-list");
    list.replaceChildren();
    if (!state.libraries.length) list.append(element("p", "\u8FD8\u6CA1\u6709\u8D44\u6599\u5E93\u3002\u5728\u53F3\u4FA7\u521B\u5EFA\u540E\uFF0C\u5373\u53EF\u5206\u7C7B\u5E76\u5BFC\u5165\u6587\u4EF6\u3002"));
    state.libraries.forEach((library) => {
      const button = element("button", "", `knowledge-library-choice${state.selected?.id === library.id ? " is-active" : ""}`);
      button.type = "button";
      button.append(element("strong", library.name), element("small", `${library.assistant_ids.length} \u4E2A\u52A9\u7406\u5DF2\u6388\u6743`));
      button.addEventListener("click", () => selectLibrary(library));
      list.append(button);
    });
  }
  async function loadDocuments(library) {
    const records = await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents`);
    const list = $("#library-document-list");
    list.replaceChildren();
    if (!records.length) list.append(element("p", "\u8FD9\u4E2A\u8D44\u6599\u5E93\u8FD8\u6CA1\u6709\u8D44\u6599\u3002"));
    records.forEach((record) => {
      const card = element("article", "", "portal-knowledge-item");
      card.append(element("strong", record.title), element("p", record.excerpt));
      if (record.category_path || record.tags.length) card.append(element("small", [record.category_path, ...record.tags].filter(Boolean).join(" \xB7 ")));
      const actions = element("div", "", "portal-knowledge-actions");
      actions.append(
        action("\u7F16\u8F91", async () => {
          const detail = await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents/${encodeURIComponent(record.id)}`);
          $("#library-manual-editor").open = true;
          $("#library-document-id").value = detail.id;
          $("#library-document-revision").value = String(detail.revision);
          $("#library-document-title").value = detail.title;
          $("#library-document-content").value = detail.content;
          $("#library-document-source").value = detail.source_uri || "";
          $("#library-document-category").value = detail.category_id || "";
          $("#library-document-tags").value = detail.tags.join(", ");
          $("#library-document-title").focus();
        }),
        deleteAction(async () => {
          await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents/${encodeURIComponent(record.id)}`, "DELETE");
          if ($("#library-document-id").value === record.id) clearDocument();
          await loadDocuments(library);
          $("#library-message").textContent = "\u8D44\u6599\u5DF2\u5220\u9664";
        })
      );
      card.append(actions);
      list.append(card);
    });
  }
  function renderGrants(library) {
    const list = $("#library-assistants");
    list.replaceChildren();
    const available = state.agents.filter((agent) => agent.enabled);
    if (!available.length) list.append(element("p", "\u5C1A\u65E0\u5DF2\u542F\u7528\u7684\u52A9\u7406\u3002"));
    available.forEach((agent) => {
      const label = element("label", "", "knowledge-assistant-option");
      const input = document.createElement("input");
      input.type = "checkbox";
      input.value = agent.id;
      input.checked = library.assistant_ids.includes(agent.id);
      label.append(input, element("span", `${agent.display_name} \xB7 ${agent.id}`));
      list.append(label);
    });
  }
  function selectLibrary(library) {
    state.selected = library;
    $("#library-id").value = library.id;
    $("#library-revision").value = String(library.revision);
    $("#library-name").value = library.name;
    $("#library-description").value = library.description;
    $("#library-description-details").open = Boolean(library.description);
    $("#library-editor-title").textContent = `\u7F16\u8F91\u8D44\u6599\u5E93 \xB7 ${library.name}`;
    $("#library-content").inert = false;
    $("#library-content").removeAttribute("aria-disabled");
    $("#library-prerequisite").hidden = true;
    $("#library-delete").hidden = false;
    $("#library-message").textContent = "";
    clearDocument();
    clearCategory();
    renderLibraries();
    renderGrants(library);
    void loadCategories(library).catch(showError);
    void loadDocuments(library).catch(showError);
  }
  async function refresh() {
    const selectedId = state.selected?.id;
    [state.libraries, state.agents] = await Promise.all([
      api("/v1/knowledge/libraries"),
      api("/v1/agent-profiles")
    ]);
    const selected = state.libraries.find((library) => library.id === selectedId);
    if (selected) selectLibrary(selected);
    else clearLibrary();
  }
  $("#library-new").addEventListener("click", clearLibrary);
  $("#library-form").addEventListener("submit", (event) => {
    event.preventDefault();
    guard(async () => {
      const id = $("#library-id").value;
      const result = await api(id ? `/v1/knowledge/libraries/${encodeURIComponent(id)}` : "/v1/knowledge/libraries", id ? "PUT" : "POST", {
        name: $("#library-name").value,
        description: $("#library-description").value,
        expected_revision: id ? Number($("#library-revision").value) : null
      });
      await refresh();
      selectLibrary(state.libraries.find((item) => item.id === result.id) || result);
      $("#library-message").textContent = "\u8D44\u6599\u5E93\u5DF2\u4FDD\u5B58";
      if (!id) $("#library-content").scrollIntoView({ block: "start", behavior: "smooth" });
    })();
  });
  $("#library-delete").addEventListener("click", () => {
    const library = state.selected;
    if (!library) return;
    const button = $("#library-delete");
    if (button.dataset.confirm !== "yes") {
      button.dataset.confirm = "yes";
      button.textContent = "\u518D\u6B21\u70B9\u51FB\uFF0C\u5220\u9664\u8D44\u6599\u5E93\u53CA\u6240\u6709\u8D44\u6599";
      window.setTimeout(() => {
        button.dataset.confirm = "";
        button.textContent = "\u5220\u9664\u8D44\u6599\u5E93";
      }, 5e3);
      return;
    }
    guard(async () => {
      await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}`, "DELETE");
      await refresh();
    })();
  });
  $("#library-grants-save").addEventListener("click", guard(async () => {
    const library = state.selected;
    if (!library) return;
    const ids = [...document.querySelectorAll("#library-assistants input:checked")].map((item) => item.value);
    const result = await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/grants`, "PUT", { assistant_ids: ids });
    state.libraries = state.libraries.map((item) => item.id === result.id ? result : item);
    selectLibrary(result);
    $("#library-message").textContent = "\u52A9\u7406\u6388\u6743\u5DF2\u4FDD\u5B58";
  }));
  $("#library-document-new").addEventListener("click", clearDocument);
  $("#library-category-new").addEventListener("click", clearCategory);
  $("#library-quick-category-toggle").addEventListener("click", () => {
    const form = $("#library-quick-category-form");
    form.hidden = !form.hidden;
    if (!form.hidden) {
      const selected = state.categories.find((item) => item.id === $("#library-document-category").value);
      $("#library-quick-category-parent").value = selected && selected.path.split("/").length < 3 ? selected.id : "";
      $("#library-quick-category-name").focus();
    }
  });
  $("#library-quick-category-cancel").addEventListener("click", () => {
    $("#library-quick-category-form").hidden = true;
    $("#library-quick-category-form").reset();
  });
  $("#library-quick-category-form").addEventListener("submit", (event) => {
    event.preventDefault();
    guard(async () => {
      const library = state.selected;
      if (!library) return;
      const category = await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/categories`, "POST", {
        name: $("#library-quick-category-name").value,
        parent_id: $("#library-quick-category-parent").value || null,
        expected_revision: null
      });
      await loadCategories(library);
      $("#library-document-category").value = category.id;
      $("#library-quick-category-form").reset();
      $("#library-quick-category-form").hidden = true;
      $("#library-message").textContent = `\u5DF2\u521B\u5EFA\u5E76\u9009\u4E2D\u5206\u7C7B\u300C${category.name}\u300D`;
    })();
  });
  $("#library-category-form").addEventListener("submit", (event) => {
    event.preventDefault();
    guard(async () => {
      const library = state.selected;
      if (!library) return;
      const id = $("#library-category-id").value;
      const base = `/v1/knowledge/libraries/${encodeURIComponent(library.id)}/categories`;
      await api(id ? `${base}/${encodeURIComponent(id)}` : base, id ? "PUT" : "POST", {
        name: $("#library-category-name").value,
        parent_id: $("#library-category-parent").value || null,
        expected_revision: id ? Number($("#library-category-revision").value) : null
      });
      clearCategory();
      await loadCategories(library);
      $("#library-message").textContent = "\u5206\u7C7B\u5DF2\u4FDD\u5B58";
    })();
  });
  $("#library-document-form").addEventListener("submit", (event) => {
    event.preventDefault();
    guard(async () => {
      const library = state.selected;
      if (!library) return;
      const id = $("#library-document-id").value;
      const base = `/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents`;
      await api(id ? `${base}/${encodeURIComponent(id)}` : base, id ? "PUT" : "POST", {
        title: $("#library-document-title").value,
        content: $("#library-document-content").value,
        source_uri: $("#library-document-source").value || null,
        category_id: $("#library-document-category").value || null,
        tags: $("#library-document-tags").value.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean),
        expected_revision: id ? Number($("#library-document-revision").value) : null
      });
      clearDocument();
      await loadDocuments(library);
      $("#library-message").textContent = "\u8D44\u6599\u5DF2\u4FDD\u5B58";
    })();
  });
  $("#library-file-import").addEventListener("change", async (event) => {
    const input = event.currentTarget;
    const files = Array.from(input.files || []);
    const message = $("#library-file-message");
    if (!files.length) return;
    if (files.length > 10) {
      message.textContent = "\u6BCF\u6B21\u6700\u591A\u5BFC\u5165 10 \u4E2A\u6587\u4EF6\u3002";
      input.value = "";
      return;
    }
    const library = state.selected;
    if (!library) {
      message.textContent = "\u8BF7\u5148\u9009\u62E9\u8D44\u6599\u5E93\u3002";
      input.value = "";
      return;
    }
    const categoryId = $("#library-document-category").value || null;
    input.disabled = true;
    const failures = [];
    let imported = 0;
    try {
      for (const file of files) {
        try {
          const document2 = await readKnowledgeTextFile(file);
          const result = await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents${document2.file_base64 ? "/import" : ""}`, "POST", {
            ...document2,
            source_uri: null,
            category_id: categoryId,
            tags: [],
            expected_revision: null
          });
          for (const warning of knowledgeImportWarnings(result)) failures.push(`${file.name}\uFF1A${warning}\u3002`);
          imported += 1;
        } catch (error) {
          failures.push(error instanceof Error ? error.message : String(error));
        }
      }
      if (state.selected?.id === library.id) await loadDocuments(library);
      message.textContent = `${imported} \u4E2A\u6587\u4EF6\u5DF2\u5BFC\u5165${failures.length ? `\uFF1B${failures.join("\uFF1B")}` : "\u3002"}`;
    } finally {
      input.value = "";
      input.disabled = false;
    }
  });
  void refresh().catch(showError);
})();
