"use strict";

(() => {
  const goToLogin = () => window.top.location.replace("/login");
  const auth = {
    session: null,
    ready: null,
    async load() {
      const response = await fetch("/v1/auth/session", { headers: { accept: "application/json" } });
      if (response.status === 401) {
        goToLogin();
        throw new Error("请先登录");
      }
      if (!response.ok) throw new Error("无法读取登录会话");
      this.session = await response.json();
      this.decorate();
      return this.session;
    },
    async fetch(path, options = {}) {
      await this.ready;
      const headers = new Headers(options.headers || {});
      const method = (options.method || "GET").toUpperCase();
      if (!["GET", "HEAD", "OPTIONS"].includes(method) && this.session?.csrf_token) {
        headers.set("x-exoharness-csrf", this.session.csrf_token);
      }
      const response = await fetch(path, { ...options, headers });
      if (response.status === 401) goToLogin();
      return response;
    },
    decorate() {
      const account = this.session?.account;
      const name = document.querySelector("#portal-user-name");
      const email = document.querySelector("#portal-user-email");
      if (name) name.textContent = account?.display_name || account?.username || "我的账号";
      if (email) email.textContent = account?.email || "";
      const avatar = document.querySelector("#portal-user-avatar");
      if (avatar) avatar.textContent = Array.from(account?.display_name || account?.username || "U")[0].toUpperCase();
      installAccountMenu();
      document.querySelector("#portal-logout")?.addEventListener("click", () => this.logout());
    },
    async logout() {
      const button = document.querySelector("#portal-logout");
      if (button) button.disabled = true;
      try {
        const response = await this.fetch("/v1/auth/logout", { method: "POST" });
        if (!response.ok) throw new Error("退出失败，请重试");
        goToLogin();
      } catch (error) {
        if (button) { button.disabled = false; button.title = error.message; }
      }
    },
  };
  function installAccountMenu() {
    const toggle = document.querySelector("#portal-account-toggle");
    const menu = document.querySelector("#portal-account-menu");
    if (!toggle || !menu || toggle.dataset.ready) return;
    toggle.dataset.ready = "true";
    const position = () => {
      if (menu.hidden) return;
      const bounds = toggle.getBoundingClientRect();
      menu.style.left = `${Math.max(10, Math.min(bounds.left, innerWidth - menu.offsetWidth - 10))}px`;
      const top = bounds.top > innerHeight / 2 ? bounds.top - menu.offsetHeight - 8 : bounds.bottom + 8;
      menu.style.top = `${Math.max(10, Math.min(top, innerHeight - menu.offsetHeight - 10))}px`;
    };
    const close = (focus = false) => { menu.hidden = true; toggle.setAttribute("aria-expanded", "false"); if (focus) toggle.focus(); };
    const appearance = document.querySelector("#portal-appearance-toggle");
    const options = document.querySelector("#portal-theme-options");
    const refreshTheme = () => {
      const value = window.ExoTheme?.value || "dark";
      document.querySelector("#portal-appearance-label").textContent = value === "light" ? "明亮" : "暗色";
      options.querySelectorAll("button").forEach((button) => button.setAttribute("aria-pressed", String(button.dataset.portalTheme === value)));
    };
    toggle.addEventListener("click", (event) => {
      if (!menu.hidden) { close(); return; }
      window.dispatchEvent(new CustomEvent("exoharness:account-menu-open"));
      menu.hidden = false;
      toggle.setAttribute("aria-expanded", "true");
      refreshTheme(); position();
      if (event.detail === 0) menu.querySelector('[role="menuitem"]')?.focus();
    });
    appearance.addEventListener("click", () => {
      options.hidden = !options.hidden;
      appearance.setAttribute("aria-expanded", String(!options.hidden));
      position();
    });
    options.querySelectorAll("button").forEach((button) => button.addEventListener("click", () => {
      window.ExoTheme?.apply(button.dataset.portalTheme);
      refreshTheme();
    }));
    menu.querySelector('[aria-disabled="true"]')?.addEventListener("click", (event) => event.preventDefault());
    menu.querySelectorAll("a").forEach((link) => link.addEventListener("click", () => close()));
    document.addEventListener("pointerdown", (event) => { if (!menu.hidden && !menu.contains(event.target) && !toggle.contains(event.target)) close(); });
    document.addEventListener("keydown", (event) => {
      if (menu.hidden) return;
      if (event.key === "Escape") { event.preventDefault(); close(true); return; }
      if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
      const items = [...menu.querySelectorAll('a,button:not([aria-disabled="true"])')].filter((item) => item.offsetParent !== null);
      const index = items.indexOf(document.activeElement);
      const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (index + (event.key === "ArrowUp" ? -1 : 1) + items.length) % items.length;
      event.preventDefault(); items[next]?.focus();
    });
    menu.addEventListener("focusout", () => setTimeout(() => { if (!menu.contains(document.activeElement) && document.activeElement !== toggle) close(); }, 0));
    window.addEventListener("resize", position);
    window.addEventListener("exoharness:theme-change", refreshTheme);
    window.addEventListener("exoharness:response-feedback-open", () => close());
  }
  window.ExoAuth = auth;
  auth.ready = auth.load();
})();
