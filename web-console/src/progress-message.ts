// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

// Retain ordinary progress from the service, including on reconnect. Protect
// clients connected to an older server that still emits diagnostic progress.
export function progressMessage(value: unknown): string {
  if (typeof value !== "string") return "正在处理...";
  const message = value.trim();
  if (!message || new TextEncoder().encode(message).length > 4096 || /[\u0000-\u001f\u007f-\u009f]/.test(message)) return "正在处理...";
  const lower = message.toLowerCase();
  const privatePatterns = [
    "traceback", "exception", "runtimeerror", "stack trace", "panic", "failed", "failure",
    "error:", " error", "permission denied", "unauthorized", "forbidden", "http status",
    "mcp 503", "authorization", "bearer ", "api_key", "apikey", "password", "secret",
    "access_token", "token=", "token:", "signature=", "http://", "https://", "file://",
    "/home/", "/mnt/", "/tmp/", "/usr/", ":\\", "失败", "异常", "报错", "结果未知",
    "权限", "拒绝", "执行已停止", "查看追踪",
  ];
  return lower.startsWith("error") || lower.startsWith("{") || lower.startsWith("[{")
    || privatePatterns.some(pattern => lower.includes(pattern)) ? "正在处理..." : message;
}
