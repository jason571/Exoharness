/** UI projection only: never fetch Trace, invoke a model, or copy media bytes. */
export interface HistoricalDelivery {
  node: HTMLElement;
  resolve: () => HTMLElement | undefined;
}

export function mountDeliveryWorkbench(history: () => HistoricalDelivery[] = () => []): void {
  const messages = document.querySelector<HTMLElement>("#chat-messages");
  const actions = document.querySelector(".chat-header-actions");
  if (!messages || !actions) return;
  const toggle = document.createElement("button");
  toggle.type = "button";
  toggle.className = "delivery-toggle";
  toggle.textContent = "执行与交付";
  toggle.setAttribute("aria-haspopup", "dialog");
  toggle.setAttribute("aria-controls", "delivery-workbench");
  const dialog = document.createElement("dialog");
  dialog.id = "delivery-workbench";
  dialog.className = "delivery-workbench";
  dialog.setAttribute("aria-labelledby", "delivery-heading");
  const heading = document.createElement("header");
  const title = document.createElement("h2");
  title.id = "delivery-heading";
  title.textContent = "执行与交付";
  const close = document.createElement("button");
  close.type = "button";
  close.textContent = "关闭";
  close.addEventListener("click", () => dialog.close());
  heading.append(title, close);
  const list = document.createElement("div");
  list.className = "delivery-list";
  dialog.append(heading, list);
  document.body.append(dialog);
  actions.prepend(toggle);

  function locate(target: HTMLElement): void {
    dialog.close();
    target.scrollIntoView({ block: "center", behavior: "smooth" });
    target.tabIndex = -1;
    target.focus({ preventScroll: true });
  }

  function render(): void {
    const focused = document.activeElement instanceof HTMLElement && list.contains(document.activeElement)
      ? document.activeElement.dataset.deliveryKey : undefined;
    const scrollTop = list.scrollTop;
    const fragment = document.createDocumentFragment();
    const historyRows = history();
    const owners = new Map<HTMLElement, () => HTMLElement | undefined>();
    const runs: HTMLElement[] = [];
    for (const entry of historyRows) {
      for (const run of entry.node.querySelectorAll<HTMLElement>(".chat-message[data-run-id], .chat-recovery[data-run-id]")) {
        owners.set(run, entry.resolve);
        runs.push(run);
      }
    }
    const known = new Set(runs.map(run => run.dataset.runId));
    for (const run of messages!.querySelectorAll<HTMLElement>(".chat-message[data-run-id], .chat-recovery[data-run-id]")) {
      if (historyRows.length && run.closest('.chat-virtual-history')) continue;
      if (!known.has(run.dataset.runId)) runs.push(run);
    }
    // A bounded index of the loaded history. More history is loaded in chat,
    // not by bypassing its ownership boundary or reading administrator Trace.
    for (const run of runs.slice(-50).reverse()) {
      const resolve = (artifact?: string): HTMLElement | undefined => {
        const root = owners.get(run)?.() || (run.isConnected ? run : undefined);
        if (!root) return undefined;
        if (artifact) return Array.from(root.querySelectorAll<HTMLElement>('[data-artifact-url]'))
          .find(card => card.dataset.artifactUrl === artifact);
        return root.matches('[data-run-id]') ? root : Array.from(root.querySelectorAll<HTMLElement>('[data-run-id]'))
          .find(node => node.dataset.runId === run.dataset.runId);
      };
      const row = document.createElement("section");
      row.className = "delivery-run";
      row.dataset.runId = run.dataset.runId;
      let previous = run.previousElementSibling;
      while (previous && !previous.classList.contains("user")) previous = previous.previousElementSibling;
      const prompt = previous?.querySelector(".chat-content")?.textContent;
      if (prompt) {
        const context = document.createElement("p");
        context.className = "delivery-context";
        context.textContent = prompt.slice(0, 160);
        row.append(context);
      }
      const top = document.createElement("div");
      top.className = "delivery-run-heading";
      const label = document.createElement("strong");
      label.textContent = run.classList.contains("streaming") ? "进行中"
        : run.classList.contains("chat-recovery") ? "等待恢复"
        : run.dataset.runStatus === "failed" ? "执行失败"
        : run.dataset.runStatus === "cancelled" ? "已停止" : "已完成";
      const jump = document.createElement("button");
      jump.type = "button";
      jump.dataset.deliveryKey = `${run.dataset.runId}:conversation`;
      jump.textContent = "查看对话";
      jump.addEventListener("click", () => { const target = resolve(); if (target) locate(target); });
      top.append(label, jump);
      row.append(top);
      const status = run.querySelector<HTMLElement>(".chat-meta, .chat-recovery p");
      if (status?.textContent) {
        const text = document.createElement("p");
        text.textContent = status.textContent;
        row.append(text);
      }
      const progress = run.querySelector<HTMLElement>('[data-activity-type="tool-progress"]');
      if (progress && !progress.classList.contains("hidden")) {
        // This is the trusted progress widget built from MCP event data, not
        // remote markup. Reuse its actual value and bar without new polling.
        row.append(progress.cloneNode(true));
      }
      const files = run.querySelectorAll<HTMLElement>("[data-artifact-url]");
      for (const card of files) {
        const file = document.createElement("div");
        file.className = "delivery-file";
        const view = document.createElement("button");
        view.type = "button";
        view.dataset.deliveryKey = `${run.dataset.runId}:${card.dataset.artifactUrl}:view`;
        view.textContent = card.querySelector("strong")?.textContent || "查看文件";
        view.addEventListener("click", () => { const target = resolve(card.dataset.artifactUrl); if (target) locate(target); });
        file.append(view);
        // Reuse only links already validated by the artifact renderer.
        const source = Array.from(card.querySelectorAll<HTMLAnchorElement>("a[download]"))[0];
        if (source) {
          const download = source.cloneNode(true) as HTMLAnchorElement;
          download.textContent = "下载";
          download.dataset.deliveryKey = `${run.dataset.runId}:${card.dataset.artifactUrl}:download`;
          file.append(download);
        }
        const preview = card.querySelector<HTMLButtonElement>("button[aria-expanded]");
        if (preview) {
          const open = document.createElement("button");
          open.type = "button";
          open.dataset.deliveryKey = `${run.dataset.runId}:${card.dataset.artifactUrl}:preview`;
          open.textContent = "预览";
          open.addEventListener("click", () => {
            const target = resolve(card.dataset.artifactUrl);
            const current = target?.querySelector<HTMLButtonElement>('button[aria-expanded]');
            if (current?.getAttribute("aria-expanded") !== "true") current?.click();
            if (target) locate(target);
          });
          file.append(open);
        }
        row.append(file);
      }
      fragment.append(row);
    }
    if (!fragment.childElementCount) {
      const empty = document.createElement("p");
      empty.textContent = "发送消息后，可在这里查看执行状态、预览和下载文件。";
      fragment.append(empty);
    } else if (runs.length > 50) {
      const scope = document.createElement("p");
      scope.textContent = "这里显示已加载历史中最近 50 次执行；更早的文件可在对应历史消息中查看。";
      fragment.append(scope);
    }
    list.replaceChildren(fragment);
    list.scrollTop = scrollTop;
    if (focused) {
      Array.from(list.querySelectorAll<HTMLElement>("[data-delivery-key]"))
        .find(element => element.dataset.deliveryKey === focused)?.focus({ preventScroll: true });
    }
  }

  let frame = 0;
  const observer = new MutationObserver(() => {
    if (!dialog.open || frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      if (dialog.open) render();
    });
  });
  observer.observe(messages, { subtree: true, childList: true, characterData: true, attributes: true,
    attributeFilter: ["class", "data-run-id", "data-run-status"] });
  toggle.addEventListener("click", () => { render(); dialog.showModal(); });
  dialog.addEventListener("click", event => {
    if (event.target === dialog) {
      const box = dialog.getBoundingClientRect();
      if (event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom) dialog.close();
    }
  });
  dialog.addEventListener("close", () => { if (frame) cancelAnimationFrame(frame); frame = 0; toggle.focus(); });
}
