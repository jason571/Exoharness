import { readKnowledgeTextFile, knowledgeImportWarnings } from "./knowledge-import";

interface Library {
  id: string;
  name: string;
  description: string;
  revision: number;
  assistant_ids: string[];
}
interface AgentProfile { id: string; display_name: string; enabled: boolean; }
interface DocumentSummary { id: string; title: string; excerpt: string; revision: number; source_uri: string | null; category_id: string | null; category_path: string | null; tags: string[]; }
interface DocumentDetail extends DocumentSummary { content: string; }
interface Category { id: string; name: string; parent_id: string | null; path: string; revision: number; }

const state: { libraries: Library[]; agents: AgentProfile[]; categories: Category[]; selected: Library | null } = {
  libraries: [], agents: [], categories: [], selected: null,
};
const $ = <T extends HTMLElement = HTMLElement>(selector: string): T => {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error(`Missing element: ${selector}`);
  return node;
};
function element<K extends keyof HTMLElementTagNameMap>(tag: K, text = "", className = ""): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.textContent = text;
  node.className = className;
  return node;
}
function showError(error: unknown): void {
  const view = $("#knowledge-error");
  view.textContent = error instanceof Error ? error.message : String(error);
  view.classList.remove("hidden");
}
async function api<T>(url: string, method = "GET", body?: unknown): Promise<T> {
  const response = await (window as unknown as { ExoAuth: { fetch: typeof fetch } }).ExoAuth.fetch(url, {
    method,
    headers: body === undefined ? undefined : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await response.text();
  const result = text ? JSON.parse(text) : null;
  if (!response.ok) throw new Error(result?.error?.message || `${response.status} ${response.statusText}`);
  return result as T;
}
function guard(callback: () => Promise<void>): () => void {
  return () => { $("#knowledge-error").classList.add("hidden"); void callback().catch(showError); };
}
function action(label: string, callback: () => Promise<void>): HTMLButtonElement {
  const button = element("button", label, "button secondary");
  button.type = "button";
  button.addEventListener("click", () => {
    button.disabled = true;
    void callback().catch(showError).finally(() => { button.disabled = false; });
  });
  return button;
}
function deleteAction(callback: () => Promise<void>): HTMLButtonElement {
  const button = action("删除", async () => {
    if (button.dataset.confirm !== "yes") {
      button.dataset.confirm = "yes";
      button.textContent = "再次点击确认";
      window.setTimeout(() => { button.dataset.confirm = ""; button.textContent = "删除"; }, 5000);
      return;
    }
    await callback();
  });
  return button;
}
function clearDocument(): void {
  $<HTMLFormElement>("#library-document-form").reset();
  $<HTMLInputElement>("#library-document-id").value = "";
  $<HTMLInputElement>("#library-document-revision").value = "";
}
function clearCategory(): void {
  $<HTMLFormElement>("#library-category-form").reset();
  $<HTMLInputElement>("#library-category-id").value = "";
  $<HTMLInputElement>("#library-category-revision").value = "";
}
async function loadCategories(library: Library): Promise<void> {
  const categories = await api<Category[]>(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/categories`);
  if (state.selected?.id !== library.id) return;
  state.categories = categories;
  const parent = $<HTMLSelectElement>("#library-category-parent");
  const quickParent = $<HTMLSelectElement>("#library-quick-category-parent");
  const documentCategory = $<HTMLSelectElement>("#library-document-category");
  const parentValue = parent.value;
  const quickParentValue = quickParent.value;
  const documentValue = documentCategory.value;
  parent.replaceChildren(new Option("顶层分类（第一级）", ""));
  quickParent.replaceChildren(new Option("顶层分类（第一级）", ""));
  documentCategory.replaceChildren(new Option("未分类", ""));
  categories.forEach((category) => {
    const depth = category.path.split("/").length;
    if (depth < 3) {
      const label = `${depth + 1} 级 · ${category.path} 下`;
      parent.add(new Option(label, category.id));
      quickParent.add(new Option(label, category.id));
    }
    documentCategory.add(new Option(`${depth} 级 · ${category.path}`, category.id));
  });
  parent.value = parentValue;
  quickParent.value = quickParentValue;
  documentCategory.value = documentValue;
  const list = $("#library-category-list");
  list.replaceChildren();
  if (!categories.length) list.append(element("p", "尚无分类；可先创建分类，再给资料归类。"));
  categories.forEach((category) => {
    const card = element("article", "", "portal-knowledge-item");
    card.append(element("strong", category.path));
    const actions = element("div", "", "portal-knowledge-actions");
    actions.append(
      action("编辑", async () => {
        $<HTMLInputElement>("#library-category-id").value = category.id;
        $<HTMLInputElement>("#library-category-revision").value = String(category.revision);
        $<HTMLInputElement>("#library-category-name").value = category.name;
        $<HTMLSelectElement>("#library-category-parent").value = category.parent_id || "";
        $<HTMLDetailsElement>("#library-category-management").open = true;
        $<HTMLInputElement>("#library-category-name").focus();
      }),
      deleteAction(async () => {
        await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/categories/${encodeURIComponent(category.id)}`, "DELETE");
        clearCategory();
        await loadCategories(library);
        await loadDocuments(library);
      }),
    );
    card.append(actions);
    list.append(card);
  });
}
function clearLibrary(): void {
  state.selected = null;
  $<HTMLFormElement>("#library-form").reset();
  $<HTMLDetailsElement>("#library-description-details").open = false;
  $<HTMLInputElement>("#library-id").value = "";
  $<HTMLInputElement>("#library-revision").value = "";
  $("#library-editor-title").textContent = "第一步 · 新建资料库";
  $("#library-content").inert = true;
  $("#library-content").setAttribute("aria-disabled", "true");
  $("#library-prerequisite").hidden = false;
  $("#library-delete").hidden = true;
  $("#library-message").textContent = "";
  clearDocument();
  clearCategory();
  renderLibraries();
}
function renderLibraries(): void {
  const list = $("#library-list");
  list.replaceChildren();
  if (!state.libraries.length) list.append(element("p", "还没有资料库。在右侧创建后，即可分类并导入文件。"));
  state.libraries.forEach((library) => {
    const button = element("button", "", `knowledge-library-choice${state.selected?.id === library.id ? " is-active" : ""}`);
    button.type = "button";
    button.append(element("strong", library.name), element("small", `${library.assistant_ids.length} 个助理已授权`));
    button.addEventListener("click", () => selectLibrary(library));
    list.append(button);
  });
}
async function loadDocuments(library: Library): Promise<void> {
  const records = await api<DocumentSummary[]>(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents`);
  const list = $("#library-document-list");
  list.replaceChildren();
  if (!records.length) list.append(element("p", "这个资料库还没有资料。"));
  records.forEach((record) => {
    const card = element("article", "", "portal-knowledge-item");
    card.append(element("strong", record.title), element("p", record.excerpt));
    if (record.category_path || record.tags.length) card.append(element("small", [record.category_path, ...record.tags].filter(Boolean).join(" · ")));
    const actions = element("div", "", "portal-knowledge-actions");
    actions.append(
      action("编辑", async () => {
        const detail = await api<DocumentDetail>(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents/${encodeURIComponent(record.id)}`);
        $<HTMLDetailsElement>("#library-manual-editor").open = true;
        $<HTMLInputElement>("#library-document-id").value = detail.id;
        $<HTMLInputElement>("#library-document-revision").value = String(detail.revision);
        $<HTMLInputElement>("#library-document-title").value = detail.title;
        $<HTMLTextAreaElement>("#library-document-content").value = detail.content;
        $<HTMLInputElement>("#library-document-source").value = detail.source_uri || "";
        $<HTMLSelectElement>("#library-document-category").value = detail.category_id || "";
        $<HTMLInputElement>("#library-document-tags").value = detail.tags.join(", ");
        $<HTMLInputElement>("#library-document-title").focus();
      }),
      deleteAction(async () => {
        await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents/${encodeURIComponent(record.id)}`, "DELETE");
        if ($<HTMLInputElement>("#library-document-id").value === record.id) clearDocument();
        await loadDocuments(library);
        $("#library-message").textContent = "资料已删除";
      }),
    );
    card.append(actions);
    list.append(card);
  });
}
function renderGrants(library: Library): void {
  const list = $("#library-assistants");
  list.replaceChildren();
  const available = state.agents.filter((agent) => agent.enabled);
  if (!available.length) list.append(element("p", "尚无已启用的助理。"));
  available.forEach((agent) => {
    const label = element("label", "", "knowledge-assistant-option");
    const input = document.createElement("input");
    input.type = "checkbox";
    input.value = agent.id;
    input.checked = library.assistant_ids.includes(agent.id);
    label.append(input, element("span", `${agent.display_name} · ${agent.id}`));
    list.append(label);
  });
}
function selectLibrary(library: Library): void {
  state.selected = library;
  $<HTMLInputElement>("#library-id").value = library.id;
  $<HTMLInputElement>("#library-revision").value = String(library.revision);
  $<HTMLInputElement>("#library-name").value = library.name;
  $<HTMLTextAreaElement>("#library-description").value = library.description;
  $<HTMLDetailsElement>("#library-description-details").open = Boolean(library.description);
  $("#library-editor-title").textContent = `编辑资料库 · ${library.name}`;
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
async function refresh(): Promise<void> {
  const selectedId = state.selected?.id;
  [state.libraries, state.agents] = await Promise.all([
    api<Library[]>("/v1/knowledge/libraries"), api<AgentProfile[]>("/v1/agent-profiles"),
  ]);
  const selected = state.libraries.find((library) => library.id === selectedId);
  if (selected) selectLibrary(selected);
  else clearLibrary();
}

$("#library-new").addEventListener("click", clearLibrary);
$<HTMLFormElement>("#library-form").addEventListener("submit", (event) => {
  event.preventDefault();
  guard(async () => {
    const id = $<HTMLInputElement>("#library-id").value;
    const result = await api<Library>(id ? `/v1/knowledge/libraries/${encodeURIComponent(id)}` : "/v1/knowledge/libraries", id ? "PUT" : "POST", {
      name: $<HTMLInputElement>("#library-name").value,
      description: $<HTMLTextAreaElement>("#library-description").value,
      expected_revision: id ? Number($<HTMLInputElement>("#library-revision").value) : null,
    });
    await refresh();
    selectLibrary(state.libraries.find((item) => item.id === result.id) || result);
    $("#library-message").textContent = "资料库已保存";
    if (!id) $("#library-content").scrollIntoView({ block: "start", behavior: "smooth" });
  })();
});
$("#library-delete").addEventListener("click", () => {
  const library = state.selected;
  if (!library) return;
  const button = $<HTMLButtonElement>("#library-delete");
  if (button.dataset.confirm !== "yes") {
    button.dataset.confirm = "yes";
    button.textContent = "再次点击，删除资料库及所有资料";
    window.setTimeout(() => { button.dataset.confirm = ""; button.textContent = "删除资料库"; }, 5000);
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
  const ids = [...document.querySelectorAll<HTMLInputElement>("#library-assistants input:checked")].map((item) => item.value);
  const result = await api<Library>(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/grants`, "PUT", { assistant_ids: ids });
  state.libraries = state.libraries.map((item) => item.id === result.id ? result : item);
  selectLibrary(result);
  $("#library-message").textContent = "助理授权已保存";
}));
$("#library-document-new").addEventListener("click", clearDocument);
$("#library-category-new").addEventListener("click", clearCategory);
$("#library-quick-category-toggle").addEventListener("click", () => {
  const form = $<HTMLFormElement>("#library-quick-category-form");
  form.hidden = !form.hidden;
  if (!form.hidden) {
    const selected = state.categories.find((item) => item.id === $<HTMLSelectElement>("#library-document-category").value);
    $<HTMLSelectElement>("#library-quick-category-parent").value = selected && selected.path.split("/").length < 3 ? selected.id : "";
    $<HTMLInputElement>("#library-quick-category-name").focus();
  }
});
$("#library-quick-category-cancel").addEventListener("click", () => {
  $<HTMLFormElement>("#library-quick-category-form").hidden = true;
  $<HTMLFormElement>("#library-quick-category-form").reset();
});
$<HTMLFormElement>("#library-quick-category-form").addEventListener("submit", (event) => {
  event.preventDefault();
  guard(async () => {
    const library = state.selected;
    if (!library) return;
    const category = await api<Category>(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/categories`, "POST", {
      name: $<HTMLInputElement>("#library-quick-category-name").value,
      parent_id: $<HTMLSelectElement>("#library-quick-category-parent").value || null,
      expected_revision: null,
    });
    await loadCategories(library);
    $<HTMLSelectElement>("#library-document-category").value = category.id;
    $<HTMLFormElement>("#library-quick-category-form").reset();
    $<HTMLFormElement>("#library-quick-category-form").hidden = true;
    $("#library-message").textContent = `已创建并选中分类「${category.name}」`;
  })();
});
$<HTMLFormElement>("#library-category-form").addEventListener("submit", (event) => {
  event.preventDefault();
  guard(async () => {
    const library = state.selected;
    if (!library) return;
    const id = $<HTMLInputElement>("#library-category-id").value;
    const base = `/v1/knowledge/libraries/${encodeURIComponent(library.id)}/categories`;
    await api(id ? `${base}/${encodeURIComponent(id)}` : base, id ? "PUT" : "POST", {
      name: $<HTMLInputElement>("#library-category-name").value,
      parent_id: $<HTMLSelectElement>("#library-category-parent").value || null,
      expected_revision: id ? Number($<HTMLInputElement>("#library-category-revision").value) : null,
    });
    clearCategory();
    await loadCategories(library);
    $("#library-message").textContent = "分类已保存";
  })();
});
$<HTMLFormElement>("#library-document-form").addEventListener("submit", (event) => {
  event.preventDefault();
  guard(async () => {
    const library = state.selected;
    if (!library) return;
    const id = $<HTMLInputElement>("#library-document-id").value;
    const base = `/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents`;
    await api(id ? `${base}/${encodeURIComponent(id)}` : base, id ? "PUT" : "POST", {
      title: $<HTMLInputElement>("#library-document-title").value,
      content: $<HTMLTextAreaElement>("#library-document-content").value,
      source_uri: $<HTMLInputElement>("#library-document-source").value || null,
      category_id: $<HTMLSelectElement>("#library-document-category").value || null,
      tags: $<HTMLInputElement>("#library-document-tags").value.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean),
      expected_revision: id ? Number($<HTMLInputElement>("#library-document-revision").value) : null,
    });
    clearDocument();
    await loadDocuments(library);
    $("#library-message").textContent = "资料已保存";
  })();
});
$<HTMLInputElement>("#library-file-import").addEventListener("change", async (event) => {
  const input = event.currentTarget as HTMLInputElement;
  const files = Array.from(input.files || []);
  const message = $("#library-file-message");
  if (!files.length) return;
  if (files.length > 10) { message.textContent = "每次最多导入 10 个文件。"; input.value = ""; return; }
  const library = state.selected;
  if (!library) { message.textContent = "请先选择资料库。"; input.value = ""; return; }
  const categoryId = $<HTMLSelectElement>("#library-document-category").value || null;
  input.disabled = true;
  const failures: string[] = [];
  let imported = 0;
  try {
    for (const file of files) {
      try {
        const document = await readKnowledgeTextFile(file);
        const result = await api(`/v1/knowledge/libraries/${encodeURIComponent(library.id)}/documents${document.file_base64 ? "/import" : ""}`, "POST", {
          ...document, source_uri: null, category_id: categoryId, tags: [], expected_revision: null,
        });
        for (const warning of knowledgeImportWarnings(result)) failures.push(`${file.name}：${warning}。`);
        imported += 1;
      } catch (error) { failures.push(error instanceof Error ? error.message : String(error)); }
    }
    if (state.selected?.id === library.id) await loadDocuments(library);
    message.textContent = `${imported} 个文件已导入${failures.length ? `；${failures.join("；")}` : "。"}`;
  } finally { input.value = ""; input.disabled = false; }
});
void refresh().catch(showError);
