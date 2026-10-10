"use strict";

// Runs before the stylesheet to avoid flashing dark when light was selected.
(() => {
  const key = "exoharness.portal.theme";
  const theme = {
    value: "dark",
    apply(value, persist = true) {
      this.value = value === "light" ? "light" : "dark";
      document.documentElement.dataset.portalTheme = this.value;
      document.documentElement.style.colorScheme = this.value;
      document.querySelector('meta[name="theme-color"]')?.setAttribute("content", this.value === "light" ? "#ffffff" : "#0b0b0c");
      if (persist) { try { localStorage.setItem(key, this.value); } catch { /* Browser storage may be unavailable. */ } }
      window.dispatchEvent(new CustomEvent("exoharness:theme-change", { detail: this.value }));
    },
  };
  window.ExoTheme = theme;
  let saved = "dark";
  try { saved = localStorage.getItem(key) || "dark"; } catch { /* Use default. */ }
  theme.apply(saved, false);
  window.addEventListener("storage", (event) => { if (event.key === key) theme.apply(event.newValue, false); });
})();
