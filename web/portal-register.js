"use strict";

const form = document.querySelector("#portal-register-form");
const email = document.querySelector("#portal-register-email");
const code = document.querySelector("#portal-register-code");
const username = document.querySelector("#portal-register-username");
const displayName = document.querySelector("#portal-register-display-name");
const password = document.querySelector("#portal-register-password");
const confirmPassword = document.querySelector("#portal-register-confirm");

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
const sendCode = document.querySelector("#portal-send-code");
const submit = document.querySelector("#portal-register-submit");
const messageView = document.querySelector("#portal-register-message");
const errorView = document.querySelector("#portal-register-error");
let countdownTimer = null;

function showError(message) {
  errorView.textContent = message;
  errorView.classList.remove("hidden");
  messageView.classList.add("hidden");
}

function startCountdown(seconds) {
  if (countdownTimer) window.clearInterval(countdownTimer);
  let remaining = seconds;
  sendCode.disabled = true;
  sendCode.textContent = `${remaining} 秒后重发`;
  countdownTimer = window.setInterval(() => {
    remaining -= 1;
    if (remaining <= 0) {
      window.clearInterval(countdownTimer);
      countdownTimer = null;
      sendCode.disabled = false;
      sendCode.textContent = "重新发送";
    } else {
      sendCode.textContent = `${remaining} 秒后重发`;
    }
  }, 1000);
}

sendCode.addEventListener("click", async () => {
  errorView.classList.add("hidden");
  messageView.classList.add("hidden");
  if (!email.reportValidity()) return;
  sendCode.disabled = true;
  try {
    const response = await fetch("/v1/auth/registration-code", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ email: email.value }),
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error?.message || "验证码发送失败");
    messageView.textContent = payload?.message || "验证码邮件已发送";
    messageView.classList.remove("hidden");
    startCountdown(payload?.resend_after || 60);
    code.focus();
  } catch (error) {
    sendCode.disabled = false;
    showError(error.message);
  }
});

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  errorView.classList.add("hidden");
  if (password.value !== confirmPassword.value) {
    showError("两次输入的密码不一致");
    confirmPassword.focus();
    return;
  }
  submit.disabled = true;
  try {
    const response = await fetch("/v1/auth/register", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        email: email.value,
        code: code.value,
        username: username.value,
        display_name: displayName.value,
        password: password.value,
      }),
    });
    const payload = await response.json().catch(() => null);
    if (!response.ok) throw new Error(payload?.error?.message || "注册失败");
    password.value = "";
    confirmPassword.value = "";
    window.location.replace("/chat");
  } catch (error) {
    showError(error.message);
  } finally {
    submit.disabled = false;
  }
});
