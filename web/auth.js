"use strict";

(() => {
  const MOBILE_MENU_BREAKPOINT = 1180;
  const MANAGEMENT_NAVIGATION = [
    ["/overview", "运行总览", true],
    ["/services", "技能中心", false],
    ["/imports", "技能导入", false],
    ["/models", "模型配置", false],
    ["/agents", "智能助理", true],
    ["/knowledge", "资料管理", true],
    ["/trace", "运行追踪", true],
    ["/audit", "操作记录", false],
    ["/users", "用户管理", true],
  ];
  const NAV_ICONS = {
    "/overview": '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',
    "/services": '<path d="m12 2 3 7 7 3-7 3-3 7-3-7-7-3 7-3z"/>',
    "/imports": '<path d="M12 3v12m-4-4 4 4 4-4M4 17v3h16v-3"/>',
    "/models": '<path d="M4 7h16M4 17h16M9 4v6m6 4v6"/>',
    "/agents": '<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4 3-7 8-7s8 3 8 7"/>',
    "/knowledge": '<path d="M4 5.5C7 4 10 4 12 5.5c2-1.5 5-1.5 8 0V20c-3-1.5-6-1.5-8 0-2-1.5-5-1.5-8 0z"/><path d="M12 5.5V20"/>',
    "/trace": '<circle cx="5" cy="5" r="2"/><circle cx="19" cy="7" r="2"/><circle cx="12" cy="19" r="2"/><path d="m7 6 10 1M6 7l5 10m7-8-5 8"/>',
    "/audit": '<path d="M8 3h8v4H8zM8 5H5v16h14V5h-3M8 12h8M8 16h5"/>',
    "/users": '<circle cx="9" cy="8" r="3"/><path d="M2 20c0-4 3-7 7-7s7 3 7 7M17 5a3 3 0 0 1 0 6m1 3c2 1 4 3 4 6"/>',
    "/security": '<path d="M12 2 4 5v6c0 5 3 9 8 11 5-2 8-6 8-11V5z"/><path d="m9 12 2 2 4-4"/>',
  };

  const auth = {
    session: null,
    ready: null,
    mobileMenu: null,
    sidebar: null,
    async load() {
      const response = await fetch("/v1/auth/session", { headers: { accept: "application/json" } });
      if (response.status === 401) {
        window.location.replace("/exoharness-login");
        throw new Error("请先登录");
      }
      if (!response.ok) throw new Error("无法读取登录会话");
      this.session = await response.json();
      document.body.dataset.authRole = this.session.role;
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
      if (response.status === 401) window.location.replace("/exoharness-login");
      return response;
    },
    decorate() {
      const topbar = document.querySelector(".topbar");
      const nav = topbar?.querySelector(".main-nav");
      if (topbar && nav) {
        document.documentElement.classList.add("management-console");
        topbar.classList.add("management-header");
        const brand = topbar.querySelector(".brand");
        if (brand) {
          brand.href = this.session.role === "admin" ? "/overview" : "/services";
          brand.setAttribute("aria-label", "ExoHarness");
          const subtitle = brand.querySelector("small");
          if (subtitle) subtitle.textContent = "管理控制台";
        }
        const currentPath = location.pathname === "/" ? "/services" : location.pathname;
        const links = MANAGEMENT_NAVIGATION.filter(([, , adminOnly]) => !adminOnly || this.session.role === "admin")
          .map(([path, title, adminOnly]) => {
            const link = document.createElement("a");
            link.href = path;
            link.title = title;
            link.setAttribute("aria-label", title);
            const label = document.createElement("span");
            label.className = "management-nav-label";
            label.textContent = title;
            link.append(label);
            if (adminOnly) link.dataset.adminOnly = "";
            if (path === currentPath) { link.className = "active"; link.setAttribute("aria-current", "page"); }
            return link;
          });
        nav.replaceChildren(...links);
        nav.setAttribute("aria-label", "主导航");
      }
      nav?.querySelectorAll("a[href]").forEach((link) => {
        if (link.querySelector(".main-nav-icon")) return;
        const icon = NAV_ICONS[link.getAttribute("href")];
        if (!icon) return;
        const wrapper = document.createElement("span");
        wrapper.className = "main-nav-icon";
        wrapper.setAttribute("aria-hidden", "true");
        wrapper.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${icon}</svg>`;
        link.prepend(wrapper);
      });
      this.installMobileMenu(topbar);
      this.installSidebar(topbar);
      window.ExoManagementTheme?.install(topbar);
      if (!this.session.authentication_enabled) return;
      if (topbar && !topbar.querySelector(".auth-account")) {
        const account = document.createElement("div");
        account.className = "auth-account";
        const label = this.session.role === "admin" ? "管理员" : "只读用户";
        const settings = this.session.role === "admin" ? '<a href="/security">系统设置</a>' : "";
        account.innerHTML = `<span class="auth-role">${label}</span>${settings}<button type="button">退出</button>`;
        account.querySelector("button").addEventListener("click", () => this.logout());
        topbar.append(account);
      }
      this.decorateMobileAccount();
      document.querySelectorAll(".local-badge").forEach((node) => node.remove());
      if (this.session.role === "chat") this.applyReadOnlyUi();
    },
    installSidebar(topbar) {
      if (!topbar?.classList.contains("management-header") || this.sidebar) return;
      const nav = topbar.querySelector(".main-nav");
      const toggle = document.createElement("button");
      toggle.type = "button";
      toggle.className = "management-nav-collapse";
      toggle.setAttribute("aria-controls", nav.id);
      toggle.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m14 7-5 5 5 5"/></svg><span class="management-nav-label"></span>';
      nav.insertBefore(toggle, this.mobileMenu?.account || null);
      const storageKey = "exoharness.management-nav";
      const setCollapsed = (collapsed) => {
        document.documentElement.dataset.managementNav = collapsed ? "collapsed" : "expanded";
        toggle.setAttribute("aria-expanded", String(!collapsed));
        const title = collapsed ? "展开菜单" : "收起菜单";
        toggle.setAttribute("aria-label", title);
        toggle.title = title;
        toggle.querySelector("span").textContent = title;
      };
      let collapsed = false;
      try { collapsed = localStorage.getItem(storageKey) === "collapsed"; } catch { /* Storage can be disabled. */ }
      setCollapsed(collapsed);
      toggle.addEventListener("click", () => {
        const collapsed = document.documentElement.dataset.managementNav !== "collapsed";
        setCollapsed(collapsed);
        try { localStorage.setItem(storageKey, collapsed ? "collapsed" : "expanded"); } catch { /* Keep the current page usable. */ }
      });
      this.sidebar = { toggle };
    },
    installMobileMenu(topbar) {
      if (!topbar || this.mobileMenu) return;
      const nav = topbar.querySelector(".main-nav");
      if (!nav) return;

      nav.id ||= "management-navigation";
      const navHeader = document.createElement("div");
      navHeader.className = "mobile-nav-header";
      navHeader.innerHTML = '<span><strong>ExoHarness</strong><small>管理菜单</small></span><button class="mobile-nav-close" type="button" aria-label="关闭菜单"><span aria-hidden="true"></span></button>';
      nav.prepend(navHeader);

      const account = document.createElement("div");
      account.className = "mobile-nav-account";
      nav.append(account);

      const toggle = document.createElement("button");
      toggle.className = "mobile-nav-toggle";
      toggle.type = "button";
      toggle.setAttribute("aria-controls", nav.id);
      toggle.setAttribute("aria-expanded", "false");
      toggle.setAttribute("aria-label", "打开管理菜单");
      toggle.innerHTML = '<span aria-hidden="true"></span><span aria-hidden="true"></span><span aria-hidden="true"></span>';
      topbar.insertBefore(toggle, nav);

      const scrim = document.createElement("button");
      scrim.className = "mobile-nav-scrim";
      scrim.type = "button";
      scrim.tabIndex = -1;
      scrim.setAttribute("aria-label", "关闭菜单");
      document.body.append(scrim);

      const close = () => this.setMobileMenu(false);
      toggle.addEventListener("click", () => this.setMobileMenu(!document.body.classList.contains("mobile-menu-open")));
      navHeader.querySelector("button").addEventListener("click", close);
      scrim.addEventListener("click", close);
      nav.querySelectorAll("a").forEach((link) => link.addEventListener("click", close));
      document.addEventListener("keydown", (event) => {
        if (event.key === "Escape" && document.body.classList.contains("mobile-menu-open")) close();
      });
      window.addEventListener("resize", () => {
        if (window.innerWidth > MOBILE_MENU_BREAKPOINT) close();
        else if (!document.body.classList.contains("mobile-menu-open")) nav.setAttribute("aria-hidden", "true");
      });

      this.mobileMenu = { nav, toggle, closeButton: navHeader.querySelector("button"), account };
      this.setMobileMenu(false);
    },
    setMobileMenu(open) {
      if (!this.mobileMenu) return;
      if (open) window.dispatchEvent(new CustomEvent("exoharness:mobile-menu-open"));
      document.body.classList.toggle("mobile-menu-open", open);
      this.mobileMenu.toggle.setAttribute("aria-expanded", String(open));
      this.mobileMenu.toggle.setAttribute("aria-label", open ? "关闭管理菜单" : "打开管理菜单");
      this.mobileMenu.nav.setAttribute("aria-hidden", String(!open && window.innerWidth <= MOBILE_MENU_BREAKPOINT));
      if (open) this.mobileMenu.closeButton.focus();
    },
    decorateMobileAccount() {
      if (!this.mobileMenu || !this.session.authentication_enabled) return;
      const label = this.session.role === "admin" ? "管理员" : "只读用户";
      this.mobileMenu.account.replaceChildren();

      const identity = document.createElement("div");
      identity.className = "mobile-nav-identity";
      identity.innerHTML = `<span class="status-dot online" aria-hidden="true"></span><span><small>当前身份</small><strong>${label}</strong></span>`;
      this.mobileMenu.account.append(identity);

      if (this.session.role === "admin") {
        const settings = document.createElement("a");
        settings.href = "/security";
        settings.textContent = "系统设置";
        settings.addEventListener("click", () => this.setMobileMenu(false));
        this.mobileMenu.account.append(settings);
      }
      const logout = document.createElement("button");
      logout.type = "button";
      logout.textContent = "退出登录";
      logout.addEventListener("click", () => this.logout());
      this.mobileMenu.account.append(logout);
    },
    applyReadOnlyUi() {
      const adminSelectors = "[data-admin-only],.add-capability-button,.capability-edit-link,.capability-danger-zone,#validate-capability,#toggle-enabled,#allow-permissions,#discover-tools,#test-section,#secret-section,#new-model,.save-configuration";
      document.querySelectorAll(adminSelectors).forEach((node) => node.classList.add("admin-hidden"));
      if (location.pathname === "/imports") {
        document.querySelectorAll("main input,main textarea,main select,main button").forEach((node) => { node.disabled = true; });
      }
      if (location.pathname === "/models") {
        document.querySelectorAll("#model-config-form input,#model-config-form textarea,#model-config-form select,#model-config-form button").forEach((node) => { node.disabled = true; });
      }
      if (!document.querySelector(".readonly-banner") && location.pathname !== "/audit") {
        const banner = document.createElement("div");
        banner.className = "readonly-banner";
        banner.textContent = "当前使用只读用户：可以查看获授权的管理页面，配置修改需要管理员登录。";
        document.querySelector("main")?.prepend(banner);
      }
    },
    async logout() {
      await this.fetch("/v1/auth/logout", { method: "POST" });
      window.location.replace("/exoharness-login");
    },
  };
  window.ExoAuth = auth;
  auth.ready = auth.load();
})();
