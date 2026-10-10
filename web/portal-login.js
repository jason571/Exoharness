"use strict";

const form = document.querySelector("#portal-login-form");
const identifier = document.querySelector("#portal-login-identifier");
const password = document.querySelector("#portal-login-password");
const submit = document.querySelector("#portal-login-submit");
const errorView = document.querySelector("#portal-login-error");
const registerLink = document.querySelector("#portal-register-link");

for (const toggle of document.querySelectorAll("[data-password-target]")) {
  toggle.addEventListener("click", () => {
    const input = document.getElementById(toggle.dataset.passwordTarget);
    if (!(input instanceof HTMLInputElement)) return;
    const visible = input.type === "password";
    input.type = visible ? "text" : "password";
    toggle.classList.toggle("is-visible", visible);
    toggle.setAttribute("aria-pressed", String(visible));
    toggle.setAttribute("aria-label", visible ? "隐藏密码" : "显示密码");
    input.focus({ preventScroll: true });
  });
}

fetch("/v1/auth/status", { headers: { accept: "application/json" } })
  .then((response) => response.ok ? response.json() : null)
  .then((status) => {
    if (status && !status.registration_available) {
      registerLink.textContent = "注册暂未开放";
      registerLink.removeAttribute("href");
      registerLink.classList.add("disabled");
    }
  })
  .catch(() => {});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  submit.disabled = true;
  errorView.classList.add("hidden");
  try {
    const response = await fetch("/v1/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ identifier: identifier.value, password: password.value }),
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error?.message || "登录失败");
    password.value = "";
    window.location.replace("/chat");
  } catch (error) {
    errorView.textContent = error.message;
    errorView.classList.remove("hidden");
    password.select();
  } finally {
    submit.disabled = false;
  }
});
