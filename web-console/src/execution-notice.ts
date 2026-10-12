// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

/** Translate only trusted notice codes; never display provider exception text. */
export function executionNotice(value: unknown, language = document.documentElement.lang): string {
  const code = value && typeof value === "object" && "code" in value ? String(value.code) : "capability_limited";
  const messages: Record<string, [string, string]> = {
    capability_permission_required: ["目前没有获得完成这项任务所需的技能权限，请联系管理员开放相应技能。已完成内容已保留。", "The required Skill permission is unavailable. Please ask your administrator to enable it. Completed work is retained."],
    service_unavailable: ["所需能力暂时不可用，暂时无法完成这项任务。已完成内容已保留。", "The required capability is temporarily unavailable. Completed work is retained."],
    result_pending_verification: ["已有任务的最终结果暂时无法确认。已完成内容已保留，无需重复提交。", "The existing task's final result is not yet confirmed. Completed work is retained; no resubmission is needed."],
    task_scope_limited: ["当前能力暂时无法处理完整任务，请缩小未完成部分的范围。已完成内容已保留。", "The current capability cannot handle the full task. Please reduce the remaining scope. Completed work is retained."],
    task_processing_limited: ["这次任务暂时未能完成，已保存的内容会保留。你可以继续完成未完成的部分。", "This task could not be completed yet. Saved work is retained. You can continue the unfinished part."],
    capability_limited: ["目前可用的能力暂时无法完成这项任务，已完成内容已保留。可联系管理员检查所需技能与权限。", "The available capabilities cannot currently complete this task. Completed work is retained. Your administrator can check the required Skills and permissions."],
  };
  const normalized = code === "capability_not_authorized" ? "capability_permission_required" : code;
  return (Object.hasOwn(messages, normalized) ? messages[normalized] : messages.capability_limited)[language.startsWith("en") ? 1 : 0];
}
