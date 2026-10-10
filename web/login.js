"use strict";

const form = document.querySelector("#login-form");
const password = document.querySelector("#login-password");
const submit = document.querySelector("#login-submit");
const errorView = document.querySelector("#login-error");

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  submit.disabled = true;
  errorView.classList.add("hidden");
  try {
    const response = await fetch("/v1/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ password: password.value }),
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error?.message || "登录失败");
    password.value = "";
    window.location.replace(payload?.role === "admin" ? "/overview" : "/services");
  } catch (error) {
    errorView.textContent = error.message;
    errorView.classList.remove("hidden");
    password.select();
  } finally {
    submit.disabled = false;
  }
});
