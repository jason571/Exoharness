// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

"use strict";

// Apply the saved palette before styles are loaded. Management and chat use separate preferences.
(() => {
  const key = "exoharness.management.theme";
  let value = "dark";
  let button;
  const apply = (next, persist = true) => {
    value = next === "light" ? "light" : "dark";
    document.documentElement.dataset.managementTheme = value;
    document.documentElement.style.colorScheme = value;
    if (button) {
      const title = value === "light" ? "切换为暗色模式" : "切换为明亮模式";
      button.title = title;
      button.setAttribute("aria-label", title);
      button.setAttribute("aria-pressed", String(value === "light"));
    }
    if (persist) try { localStorage.setItem(key, value); } catch { /* Keep the current preference in memory. */ }
  };
  try { value = localStorage.getItem(key) || "dark"; } catch { /* Storage is optional. */ }
  apply(value, false);
  window.addEventListener("storage", event => { if (event.key === key) apply(event.newValue, false); });
  window.ExoManagementTheme = {
    apply,
    install(header) {
      if (!header || button) return;
      button = document.createElement("button");
      button.type = "button";
      button.className = "management-theme-toggle";
      button.innerHTML = '<svg class="management-theme-sun" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.4 1.4m11.2 11.2L19 19M5 19l1.4-1.4M17.6 6.4 19 5"/></svg><svg class="management-theme-moon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 15.4A8.4 8.4 0 0 1 8.6 4a8.4 8.4 0 1 0 11.4 11.4Z"/></svg>';
      button.addEventListener("click", () => apply(value === "dark" ? "light" : "dark"));
      header.append(button);
      apply(value, false);
    },
  };
})();
