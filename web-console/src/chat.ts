// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

import DOMPurify from "dompurify";
import { marked } from "marked";
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import css from "highlight.js/lib/languages/css";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import { mountChatPet, setChatPetLifecycle, syncChatPetFromEvent, unmountChatPet } from "./pet";
import { mountSpeechInput, speechInputHint } from "./speech-input";
import { mountDeliveryWorkbench } from "./delivery-workbench";
import { VirtualTurnList } from "./virtual-turn-list";
import { projectPreview } from "./project-preview";
import { remoteMcpInstallIntent, skillCreationIntent, skillInstallIntent, toolPackageInstallIntent, type McpInstallIntent, type SkillInstallIntent } from "./capability-install";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("css", css);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("python", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("xml", xml);
hljs.registerAliases(["html"], { languageName: "xml" });
marked.setOptions({ gfm: true, breaks: true });

type ApiAttachment =
  | { type: "image"; name: string; mime_type: string; data: string }
  | { type: "audio"; name: string; mime_type: string; data: string }
  | { type: "text"; name: string; mime_type: string; content: string }
  | { type: "file"; name: string; mime_type: string; size: number };

type AttachmentKind = "image" | "audio" | "video" | "html" | "text" | "file";

interface LocalAttachment {
  id: string;
  name: string;
  mimeType: string;
  size: number;
  kind: AttachmentKind;
  objectUrl?: string;
  text?: string;
  base64?: string;
}

interface HistoryMessage {
  role: "user" | "assistant";
  content: string;
}

interface ChatResult {
  answer: string;
  model: { name: string; provider: string; model_id: string; api: string };
  usage: { total_tokens?: number; cost?: { total?: number } };
  tool_calls: string[];
  warnings: string[];
  artifacts?: ChatArtifact[];
}

interface ChatArtifact {
  kind: "video" | "audio" | "image" | "file";
  filename: string;
  content_type: string;
  download_url: string;
  url?: string;
  preview_url?: string | null;
  url_ttl_seconds?: number | null;
  original_download_url?: string | null;
  job_id?: string | null;
  width?: number | null;
  height?: number | null;
  duration_seconds?: number | null;
  aspect_ratio?: string | null;
  resolution?: string | null;
  model_id?: string | null;
  source_tool?: string;
  identity?: { key: string; revision: number } | null;
}

type ChatStreamEvent =
  | { type: "run_created"; session_id: string; turn_id: string; run_id: string; status: string }
  | { type: "assistant_start" }
  | { type: "text_delta"; delta: string }
  | { type: "phase"; phase: string }
  | { type: "status"; status?: string; message: string }
  | { type: "tool_call"; tool: string }
  | { type: "tool_output"; tool_call_id?: string; delta: string }
  | { type: "tool_progress"; tool_call_id: string; progress: number; total?: number | null; message?: string | null }
  | { type: "resource_queue"; tool_call_id?: string; state: "waiting" | "acquired"; message: string }
  | { type: "artifact"; tool_call_id: string; artifact: ChatArtifact }
  | { type: "tool_result"; tool_call_id?: string; tool: string; is_error: boolean }
  | { type: "upstream_error"; message: string }
  | { type: "turn_end" }
  | { type: "complete"; response: ChatResult }
  | { type: "error"; message: string }
  | { type: "failed"; status: string; message: string; artifacts?: ChatArtifact[] }
  | { type: "cancelled"; status: string; message: string }
  | { type: "recoverable"; status: string; message: string };

interface ExecutionSession {
  id: string;
  title: string;
  pinned: boolean;
  created_at: number;
  updated_at: number;
  turn_count: number;
  latest_run_id: string | null;
  latest_run_status: string | null;
  managed_coding: boolean;
  parent_id: string | null;
  tagline: string | null;
  cwd: string | null;
  project_name: string | null;
  agent_profile: PublicAgentProfile | null;
}

interface AgentAvatar {
  asset_id: string;
  content_type: string;
}

interface PublicAgentProfile {
  id: string;
  revision: number;
  display_name: string;
  avatar_text: string;
  avatar?: AgentAvatar | null;
  description: string;
  digest: string;
  software_engineering?: boolean;
}

interface ExecutionNotice {
  id: string;
  kind: string;
  content: string;
  sender: string | null;
  created_at: number;
}

interface PersistedAttachment {
  type: "image" | "audio" | "text" | "file";
  name: string;
  mime_type: string;
  size: number;
  content?: string;
  attachment_id?: string | null;
  sha256?: string | null;
  download_url?: string | null;
}

interface PersistedTurn {
  id: string;
  ordinal: number;
  prompt: string;
  attachments: PersistedAttachment[];
  status: string;
  answer: string | null;
  response: ChatResult | null;
  run_id: string;
  run_status: string;
  run_error: string | null;
  regenerated_from_run_id?: string | null;
  created_at: number;
  updated_at: number;
}

interface SessionDetail extends ExecutionSession {
  turns: PersistedTurn[];
  notices: ExecutionNotice[];
  history_page?: { has_more: boolean; before_ordinal: number | null; limit: number };
  active_turns?: PersistedTurn[];
}

interface StartedRun {
  session_id: string;
  turn_id: string;
  run_id: string;
  status: string;
  events_url: string;
}

interface StreamingMessage {
  message: HTMLElement;
  content: HTMLElement;
  activity: HTMLElement;
  artifacts: HTMLElement;
  meta: HTMLElement;
  actions: HTMLElement;
  runId: string | null;
  progress: {
    toolCallId: string | null;
    displayedPercentage: number | null;
    total: number | null;
  };
}

interface ModelCatalog {
  default_model: string | null;
  models: Array<{ name: string; display_name: string; provider: string; model_id: string }>;
}

interface PublicChatConfiguration {
  model_display_name: string | null;
  configuration_managed: boolean;
}

const PORTAL_MODE = document.body.dataset.portal === "true";

function mountPortalSettings(): void {
  if (!PORTAL_MODE) return;
  const workspace = document.querySelector<HTMLElement>("#portal-settings-workspace");
  const chat = document.querySelector<HTMLElement>("#chat-drop-zone");
  const frame = document.querySelector<HTMLIFrameElement>("#portal-settings-frame");
  if (!workspace || !chat || !frame) return;
  let lastTrigger: HTMLElement | null = null;
  const close = () => {
    if (workspace.hidden) return;
    workspace.hidden = true;
    chat.hidden = false;
    frame.removeAttribute("src");
    mountChatPet();
    lastTrigger?.focus();
    lastTrigger = null;
  };
  document.querySelectorAll<HTMLAnchorElement>('#portal-account-menu a[href^="/settings"]').forEach((link) => {
    link.addEventListener("click", (event) => {
      if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      lastTrigger = document.querySelector<HTMLElement>("#portal-account-toggle");
      frame.src = link.getAttribute("href") || "/settings";
      unmountChatPet();
      chat.hidden = true;
      workspace.hidden = false;
    });
  });
  document.querySelector("#session-list")?.addEventListener("click", close, true);
  document.querySelector("#new-session")?.addEventListener("click", close, true);
  document.querySelector(".portal-brand")?.addEventListener("click", (event) => {
    if (workspace.hidden) return;
    event.preventDefault();
    close();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !workspace.hidden) { event.preventDefault(); close(); }
  });
  window.addEventListener("message", (event) => {
    if (event.origin === window.location.origin && event.source === frame.contentWindow &&
        event.data?.type === "exoharness:close-settings") close();
  });
}

const MAX_ATTACHMENTS = 8;
const MAX_IMAGE_BYTES = 5 * 1024 * 1024;
const MAX_AUDIO_BYTES = 32 * 1024 * 1024;
const MAX_TEXT_BYTES = 512 * 1024;
const IMAGE_MIMES = new Set(["image/png", "image/jpeg", "image/gif", "image/webp"]);
const TEXT_EXTENSIONS = new Set([
  "txt", "md", "markdown", "html", "htm", "css", "js", "mjs", "cjs", "ts", "tsx", "jsx",
  "json", "jsonl", "py", "rs", "go", "java", "kt", "swift", "sh", "bash", "zsh", "yaml", "yml",
  "toml", "xml", "csv", "sql", "log",
]);

const state = {
  history: [] as HistoryMessage[],
  pending: [] as LocalAttachment[],
  objectUrls: new Set<string>(),
  running: false,
  sessions: [] as ExecutionSession[],
  currentSessionId: null as string | null,
  currentRunId: null as string | null,
  eventSources: new Map<string, EventSource>(),
  cancellingRuns: new Set<string>(),
  followOutput: true,
  defaultAgentProfile: null as PublicAgentProfile | null,
  currentAgentProfile: null as PublicAgentProfile | null,
  responseActions: {} as Record<string, ResponseActionState>,
};

const HISTORY_BATCH_SIZE = 40;
let sessionLoadVersion = 0;
let historyRenderTarget: DocumentFragment | HTMLElement | null = null;
let virtualHistory: VirtualTurnList<PersistedTurn> | null = null;

function messageContainer(): HTMLElement | DocumentFragment {
  return historyRenderTarget || $("#chat-messages");
}

function appendHistoryBatch(turns: PersistedTurn[], prepend = false): void {
  const container = $("#chat-messages") as HTMLElement;
  container.querySelector(".chat-empty")?.remove();
  const history = state.history.slice();
  const anchor = container.querySelector<HTMLElement>(".chat-message");
  const top = anchor?.getBoundingClientRect().top;
  const fragment = document.createDocumentFragment();
  historyRenderTarget = fragment;
  try { renderPersistedTurns(turns); }
  finally { historyRenderTarget = null; if (prepend) state.history = history; }
  for (const row of Array.from(fragment.children)) row.classList.add("chat-history-row");
  if (prepend) {
    container.querySelector(".chat-history-load")?.after(fragment);
    if (anchor && top !== undefined) container.scrollTo({ top: container.scrollTop + anchor.getBoundingClientRect().top - top, behavior: "instant" });
  } else container.append(fragment);
}

function closeRunStream(runId: string): void {
  state.eventSources.get(runId)?.close();
  state.eventSources.delete(runId);
  state.cancellingRuns.delete(runId);
  if (state.currentRunId === runId) {
    state.currentRunId = state.eventSources.keys().next().value || null;
  }
  updateSendButton();
}

function closeAllRunStreams(): void {
  for (const source of state.eventSources.values()) source.close();
  state.eventSources.clear();
  state.cancellingRuns.clear();
  state.currentRunId = null;
}

const $ = <T extends Element>(selector: string): T => {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error(`Missing UI element: ${selector}`);
  return node;
};

function setPortalSessionDrawer(open: boolean): void {
  if (!PORTAL_MODE) return;
  document.body.classList.toggle("session-drawer-open", open);
  document.querySelector<HTMLButtonElement>("#portal-history-toggle")
    ?.setAttribute("aria-expanded", String(open));
}

function setPortalSidebarCollapsed(collapsed: boolean): void {
  if (!PORTAL_MODE) return;
  document.body.classList.toggle("portal-sidebar-collapsed", collapsed);
  const button = document.querySelector<HTMLButtonElement>("#portal-sidebar-collapse");
  const label = collapsed ? "展开对话侧栏" : "折叠对话侧栏";
  button?.setAttribute("aria-label", label);
  button?.setAttribute("aria-pressed", String(collapsed));
  if (button) button.title = label;
  window.localStorage.setItem("exoharness.portal-sidebar-collapsed", String(collapsed));
}

function setManagementSessionDrawer(open: boolean): void {
  if (PORTAL_MODE) return;
  document.body.classList.toggle("chat-session-drawer-open", open);
  const toggle = document.querySelector<HTMLButtonElement>("#chat-history-toggle");
  const sidebar = document.querySelector<HTMLElement>(".session-sidebar");
  toggle?.setAttribute("aria-expanded", String(open));
  toggle?.setAttribute("aria-label", open ? "关闭对话记录" : "打开对话记录");
  sidebar?.setAttribute("aria-hidden", String(!open && window.innerWidth <= 720));
  if (open) {
    (window as unknown as { ExoAuth?: { setMobileMenu: (value: boolean) => void } })
      .ExoAuth?.setMobileMenu(false);
    document.querySelector<HTMLButtonElement>("#chat-history-close")?.focus();
  }
}

interface ApiRequestInit extends Omit<RequestInit, "body"> {
  body?: unknown;
}

async function api<T>(path: string, options: ApiRequestInit = {}): Promise<T> {
  const { body, ...requestOptions } = options;
  const init: RequestInit = { ...requestOptions, headers: { ...(options.headers || {}) } };
  if (body !== undefined) {
    if (typeof body !== "string") {
      (init.headers as Record<string, string>)["content-type"] = "application/json";
      init.body = JSON.stringify(body);
    } else {
      init.body = body;
    }
  }
  const response = await (window as unknown as {
    ExoAuth: { fetch: typeof fetch };
  }).ExoAuth.fetch(path, init);
  const text = await response.text();
  let payload: unknown = null;
  if (text) {
    try { payload = JSON.parse(text); } catch { payload = text; }
  }
  if (!response.ok) {
    const message = typeof payload === "object" && payload && "error" in payload
      ? (payload as { error?: { message?: string } }).error?.message
      : null;
    throw new Error(message || `${response.status} ${response.statusText}`);
  }
  return payload as T;
}

async function checkService(): Promise<void> {
  try {
    await api("/health");
    $("#service-dot").classList.add("online");
    $("#service-label").textContent = "服务在线";
  } catch {
    $("#service-dot").classList.add("offline");
    $("#service-label").textContent = "服务不可用";
  }
}

async function loadCatalog(): Promise<void> {
  if (PORTAL_MODE) {
    const configuration = await api<PublicChatConfiguration>("/v1/chat/configuration");
    $("#route-status").textContent = configuration.model_display_name || "暂不可用";
    document.querySelector(".route-indicator")?.classList.toggle("unconfigured", !configuration.model_display_name);
    return;
  }
  const catalog = await api<ModelCatalog>("/v1/model-catalog");
  const model = catalog.models.find((item) => item.name === catalog.default_model);
  $("#route-status").textContent = model
    ? `${model.display_name} · ${model.provider}/${model.model_id}`
    : "未配置，点击前往模型设置";
  document.querySelector(".route-indicator")?.classList.toggle("unconfigured", !model);
}

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function renderAgentAvatar(target: HTMLElement, profile: PublicAgentProfile | null, fallback = "EH"): void {
  target.replaceChildren();
  target.classList.remove("has-image");
  if (profile?.avatar?.asset_id) {
    const image = element("img");
    image.src = `/v1/agent-assets/${encodeURIComponent(profile.avatar.asset_id)}`;
    image.alt = profile.display_name;
    image.loading = "lazy";
    image.addEventListener("error", () => {
      target.classList.remove("has-image");
      target.replaceChildren(document.createTextNode(profile.avatar_text || fallback));
    }, { once: true });
    target.classList.add("has-image");
    target.append(image);
    return;
  }
  target.textContent = profile?.avatar_text || fallback;
}

function updateConversationIdentity(): void {
  renderEngineeringHint();
  const profile = state.currentAgentProfile;
  renderAgentAvatar($("#conversation-avatar"), profile);
  $("#conversation-title").textContent = profile ? profile.display_name : "Agent 对话";
}

function isNearLatest(container: HTMLElement): boolean {
  return container.scrollHeight - container.scrollTop - container.clientHeight <= 72;
}

function updateScrollLatestButton(): void {
  $("#scroll-latest").classList.toggle("hidden", state.followOutput);
}

function scrollChatToLatest(force = false): void {
  if (historyRenderTarget) return;
  if (!force && !state.followOutput) return;
  const container = $("#chat-messages") as HTMLElement;
  container.scrollTop = container.scrollHeight;
  state.followOutput = true;
  updateScrollLatestButton();
}

const URL_ONLY_FENCE = /```[^\n]*\n((?:[ \t]*https?:\/\/\S+[ \t]*\r?\n)+)[ \t]*```/g;
const URL_ONLY_INLINE_CODE = /`(https?:\/\/[^`\s]+)`/g;

/** Models sometimes wrap delivery URLs in code fences, which renders them as
 *  non-clickable code. A block or inline code span that is nothing but URLs is
 *  a link list, so present it as explicit autolinks. Other code is untouched. */
function unwrapUrlOnlyCode(content: string): string {
  return content
    .replace(URL_ONLY_FENCE, (_match, body: string) =>
      body.split(/\r?\n/).map((line) => line.trim()).filter(Boolean).map((url) => `<${url}>`).join("\n\n") + "\n")
    .replace(URL_ONLY_INLINE_CODE, "<$1>");
}

function renderRichText(target: HTMLElement, content: string): void {
  const rendered = marked.parse(unwrapUrlOnlyCode(content || "") || "（模型未返回文本）", { async: false }) as string;
  target.innerHTML = DOMPurify.sanitize(rendered, {
    USE_PROFILES: { html: true },
    ADD_ATTR: ["target", "rel"],
    FORBID_TAGS: ["video", "audio", "source"],
  });
  target.querySelectorAll<HTMLAnchorElement>("a").forEach((link) => {
    link.target = "_blank";
    link.rel = "noopener noreferrer";
  });
  target.querySelectorAll<HTMLElement>("pre code").forEach((code) => {
    hljs.highlightElement(code);
    const pre = code.parentElement;
    if (!pre || pre.querySelector(".copy-code")) return;
    const copy = element("button", "copy-code", "复制");
    copy.type = "button";
    copy.addEventListener("click", async () => {
      await navigator.clipboard.writeText(code.textContent || "");
      copy.textContent = "已复制";
      window.setTimeout(() => { copy.textContent = "复制"; }, 1200);
    });
    pre.append(copy);
  });
}

function safeArtifactUrl(value: string): URL | null {
  try {
    const url = new URL(value, window.location.href);
    return ["http:", "https:"].includes(url.protocol) ? url : null;
  } catch {
    return null;
  }
}

function answerWithoutArtifactDownloadDetails(content: string, artifacts: ChatArtifact[]): string {
  const mediaArtifacts = artifacts.filter((artifact) => ["video", "audio", "image"].includes(artifact.kind));
  if (!content.trim() || !mediaArtifacts.length) return content;

  const urls = mediaArtifacts
    .flatMap((artifact) => [artifact.download_url, artifact.original_download_url || ""])
    .map((url) => url ? safeArtifactUrl(url)?.href : null)
    .filter((url): url is string => Boolean(url));
  const mediaLocations = new Set(urls.map((value) => {
    const url = new URL(value);
    return `${url.origin}${url.pathname}`;
  }));
  const filenames = mediaArtifacts
    .map((artifact) => artifact.filename.trim())
    .filter(Boolean);
  const lines = content.split(/\r?\n/);
  const removed = new Set<number>();
  const downloadHeading = /(?:下载|download).*(?:链接|link)|(?:链接|link).*(?:下载|download)/i;
  const fileLabel = /视频文件|音频文件|图片文件|video\s*file|audio\s*file|image\s*file|下载|download/i;
  const expiryLabel = /链接有效期|有效期|到期|过期|expires?|expiry|expiration/i;
  const artifactLines: number[] = [];

  lines.forEach((line, index) => {
    // A model may omit or alter a signed query when repeating a file link.
    // Hide that redundant Markdown link; keep the receipt's exact URL in its card.
    const mentionsUrl = urls.some((url) => line.includes(url))
      || Array.from(line.matchAll(/\]\((https?:\/\/[^\s)]+)\)/g)).some((match) => {
        const url = safeArtifactUrl(match[1]);
        return url !== null && mediaLocations.has(`${url.origin}${url.pathname}`);
      });
    const mentionsFile = filenames.some((filename) => line.includes(filename)) && fileLabel.test(line);
    if (mentionsUrl || mentionsFile) artifactLines.push(index);
  });
  if (!artifactLines.length) return content;

  for (const index of artifactLines) {
    removed.add(index);
    let previous = index - 1;
    while (previous >= 0 && !lines[previous].trim()) removed.add(previous--);
    if (previous >= 0 && lines[previous].length < 120 && downloadHeading.test(lines[previous])) {
      removed.add(previous--);
      while (previous >= 0 && !lines[previous].trim()) removed.add(previous--);
      if (previous >= 0 && /^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/.test(lines[previous])) {
        removed.add(previous);
      }
    }

    let next = index + 1;
    while (next < lines.length && !lines[next].trim()) removed.add(next++);
    if (next < lines.length && lines[next].length < 220 && expiryLabel.test(lines[next])) {
      removed.add(next++);
      while (next < lines.length && !lines[next].trim()) removed.add(next++);
    }
  }

  return lines.filter((_, index) => !removed.has(index)).join("\n").trim();
}

function artifactExpiryLabel(artifact: ChatArtifact, url: URL): string {
  if (url.origin === location.origin && /^\/v1\/run-artifacts\/artifact-[a-f0-9]{64}$/.test(url.pathname)) return "正在核对文件状态…";
  const expires = Number(url.searchParams.get("expires"));
  if (Number.isFinite(expires) && expires > 0) {
    const expiresAt = new Date(expires * 1000);
    if (expiresAt.getTime() <= Date.now()) return "下载链接已过期；播放器可能无法继续加载";
    return `链接有效至 ${expiresAt.toLocaleString()}`;
  }
  const ttl = artifact.url_ttl_seconds || 0;
  if (ttl > 0 && ttl % 3600 === 0) return `链接有效期：${ttl / 3600} 小时`;
  if (ttl > 0 && ttl % 60 === 0) return `链接有效期：${ttl / 60} 分钟`;
  return ttl > 0 ? `链接有效期：${ttl} 秒` : "";
}

async function showManagedArtifactExpiry(label: HTMLElement, url: URL): Promise<void> {
  if (url.origin !== location.origin || !/^\/v1\/run-artifacts\/artifact-[a-f0-9]{64}$/.test(url.pathname)) return;
  try {
    const response = await fetch(`${url.pathname}/metadata`, { headers: { accept: "application/json" } });
    if (!response.ok) throw new Error("metadata unavailable");
    const metadata = await response.json();
    label.textContent = metadata.expires_at
      ? Number(metadata.expires_at) <= Date.now() ? "访问有效期已结束，请联系管理员" : `有效至 ${new Date(metadata.expires_at).toLocaleString()}`
      : metadata.saved ? "已保存 · 永久有效" : "尚未完成持久化保存，请稍后重试";
  } catch { label.textContent = "文件状态暂时无法读取"; }
}

function renderVideoArtifact(container: HTMLElement, artifact: ChatArtifact): void {
  const url = safeArtifactUrl(artifact.download_url);
  if (!url) return;
  if (Array.from(container.children).some((item) => (item as HTMLElement).dataset.artifactUrl === url.href)) return;
  const card = element("article", "chat-video-artifact");
  card.dataset.artifactUrl = url.href;
  const heading = element("div", "video-artifact-heading");
  const title = element("div");
  title.append(element("strong", "", artifact.filename || "result.mp4"));
  const dimensions = artifact.width && artifact.height ? `${artifact.width}×${artifact.height}` : "";
  const duration = artifact.duration_seconds ? `${artifact.duration_seconds.toFixed(1)} 秒` : "";
  const model = artifact.model_id ? `模型 ${artifact.model_id}` : "";
  const metadata = [dimensions, duration, model].filter(Boolean).join(" · ");
  if (metadata) title.append(element("small", "", metadata));
  const badge = element("span", "video-artifact-badge", "MP4");
  heading.append(title, badge);
  const video = element("video");
  video.src = url.href;
  video.controls = true;
  video.playsInline = true;
  video.preload = "metadata";
  video.setAttribute("aria-label", `播放 ${artifact.filename || "生成视频"}`);
  const footer = element("div", "video-artifact-footer");
  const download = element("a", "video-download-link", "下载视频");
  download.href = url.href;
  if (url.origin === location.origin && url.pathname.startsWith("/v1/run-artifacts/")) {
    const downloadUrl = new URL(url);
    downloadUrl.searchParams.set("download", "1");
    download.href = downloadUrl.href;
  }
  download.target = "_blank";
  download.rel = "noopener noreferrer";
  download.setAttribute("download", artifact.filename || "result.mp4");
  const expiry = element("small", "video-artifact-expiry", artifactExpiryLabel(artifact, url));
  void showManagedArtifactExpiry(expiry, url);
  const playbackError = element("small", "video-playback-error hidden", "无法直接播放此地址，请使用下载入口。 ");
  video.addEventListener("error", () => playbackError.classList.remove("hidden"), { once: true });
  footer.append(download);
  if (expiry.textContent) footer.append(expiry);
  footer.append(playbackError);
  card.append(heading, video, footer);
  container.append(card);
  container.classList.remove("hidden");
}

function renderAudioArtifact(container: HTMLElement, artifact: ChatArtifact): void {
  const url = safeArtifactUrl(artifact.url || artifact.download_url);
  if (!url) return;
  if (Array.from(container.children).some((item) => (item as HTMLElement).dataset.artifactUrl === url.href)) return;
  const card = element("article", "chat-video-artifact chat-audio-artifact");
  card.dataset.artifactUrl = url.href;
  const heading = element("div", "video-artifact-heading");
  const title = element("div");
  title.append(element("strong", "", artifact.filename || "speech.mp3"));
  if (artifact.model_id) title.append(element("small", "", `模型 ${artifact.model_id}`));
  heading.append(title, element("span", "video-artifact-badge", "AUDIO"));
  const audio = element("audio");
  audio.src = url.href;
  audio.controls = true;
  audio.preload = "metadata";
  audio.setAttribute("aria-label", `播放 ${artifact.filename || "生成音频"}`);
  const footer = element("div", "video-artifact-footer");
  const download = element("a", "video-download-link", "下载音频");
  download.href = safeArtifactUrl(artifact.download_url)?.href || url.href;
  download.target = "_blank";
  download.rel = "noopener noreferrer";
  download.setAttribute("download", artifact.filename || "speech.mp3");
  const playbackError = element("small", "video-playback-error hidden", "无法直接播放此音频，请使用下载入口。");
  audio.addEventListener("error", () => playbackError.classList.remove("hidden"), { once: true });
  footer.append(download, playbackError);
  card.append(heading, audio, footer);
  container.append(card);
  container.classList.remove("hidden");
}

function renderImageArtifact(container: HTMLElement, artifact: ChatArtifact): void {
  const url = safeArtifactUrl(artifact.download_url);
  if (!url) return;
  if (Array.from(container.children).some((item) => (item as HTMLElement).dataset.artifactUrl === url.href)) return;
  const card = element("article", "chat-video-artifact chat-image-artifact");
  card.dataset.artifactUrl = url.href;
  const heading = element("div", "video-artifact-heading");
  heading.append(element("strong", "", artifact.filename || "image"), element("span", "video-artifact-badge", "IMAGE"));
  const image = element("img");
  image.src = url.href;
  image.alt = artifact.filename || "生成图片";
  image.loading = "lazy";
  const footer = element("div", "video-artifact-footer");
  const download = element("a", "video-download-link", "下载图片");
  download.href = url.href;
  download.target = "_blank";
  download.rel = "noopener noreferrer";
  download.setAttribute("download", artifact.filename || "image");
  footer.append(download);
  card.append(heading, image, footer);
  container.append(card);
  container.classList.remove("hidden");
}

function renderArtifacts(container: HTMLElement, artifacts: ChatArtifact[]): void {
  artifacts.forEach((artifact) => {
    const safeUrl = safeArtifactUrl(artifact.download_url);
    if (!safeUrl) return;
    const identity = artifact.identity && Number.isSafeInteger(artifact.identity.revision) && artifact.identity.revision >= 0
      ? artifact.identity : null;
    if (identity) {
      const previous = Array.from(container.children).find(item => (item as HTMLElement).dataset.artifactIdentity === identity.key) as HTMLElement | undefined;
      if (previous && Number(previous.dataset.artifactRevision) >= identity.revision) return;
      previous?.remove();
    }
    if (artifact.kind === "video") renderVideoArtifact(container, artifact);
    else if (artifact.kind === "audio") renderAudioArtifact(container, artifact);
    else if (artifact.kind === "image") renderImageArtifact(container, artifact);
    else if (artifact.kind === "file") {
      const url = safeArtifactUrl(artifact.download_url);
      if (!url || Array.from(container.children).some(item => (item as HTMLElement).dataset.artifactUrl === url.href)) return;
      const card = element("article", "chat-video-artifact chat-file-artifact");
      card.dataset.artifactUrl = url.href;
      const heading = element("div", "video-artifact-heading");
      heading.append(element("strong", "", artifact.filename || "生成文件"));
      const footer = element("div", "video-artifact-footer");
      const link = element("a", "video-download-link", "下载文件");
      link.href = url.href;
      link.download = artifact.filename || "artifact";
      link.target = "_blank";
      link.rel = "noopener noreferrer";
      footer.append(link);
      // Signed projects and authenticated local HTML stay opaque-origin frames.
      const project = projectPreview(url, artifact.preview_url);
      const localHtml = artifact.content_type.split(";")[0].trim().toLowerCase() === "text/html"
          && url.origin === location.origin && /^\/v1\/run-artifacts\/artifact-[a-f0-9]{64}$/.test(url.pathname);
      const previewSource = project?.href || (localHtml ? `${url.pathname}/preview` : null);
      if (previewSource) {
        const preview = element("button", "video-download-link", "预览网页");
        preview.type = "button";
        preview.setAttribute("aria-expanded", "false");
        let frame: HTMLIFrameElement | undefined;
        preview.addEventListener("click", () => {
          if (frame) {
            frame.remove(); frame = undefined;
            preview.textContent = "预览网页"; preview.setAttribute("aria-expanded", "false");
          } else {
            frame = element("iframe", "chat-html-preview");
            frame.title = `网页预览：${artifact.filename}`;
            frame.setAttribute("sandbox", "allow-scripts");
            frame.referrerPolicy = "no-referrer";
            frame.src = previewSource;
            card.append(frame);
            preview.textContent = "收起预览"; preview.setAttribute("aria-expanded", "true");
          }
        });
        footer.prepend(preview);
        const open = element("a", "video-download-link", "新窗口打开");
        open.href = previewSource;
        open.target = "_blank";
        open.rel = "noopener noreferrer";
        footer.insertBefore(open, link);
        heading.append(element("span", "video-artifact-badge", project ? "WEBSITE" : "HTML"));
      }
      card.append(heading, footer); container.append(card);
      container.classList.remove("hidden");
    }
    if (identity) {
      const card = Array.from(container.children).find(item => (item as HTMLElement).dataset.artifactUrl === safeUrl.href) as HTMLElement | undefined;
      if (card) { card.dataset.artifactIdentity = identity.key; card.dataset.artifactRevision = String(identity.revision); }
    }
  });
}

function progressElapsedSeconds(message: string): number | null {
  const match = message.match(/(?:已运行|elapsed)\s*[:：]?\s*(\d+)\s*(?:秒|s(?:ec(?:ond)?s?)?)/i);
  return match ? Number(match[1]) : null;
}

function formatProgressElapsed(seconds: number): string {
  if (seconds < 60) return `${seconds}秒`;
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return remainder ? `${minutes}分${remainder}秒` : `${minutes}分钟`;
}

interface ToolProgressView {
  title: HTMLElement;
  value: HTMLElement;
  track: HTMLElement;
  fill: HTMLElement;
  detail: HTMLElement;
}

function toolProgressView(activity: HTMLElement): ToolProgressView {
  const existingTitle = activity.querySelector<HTMLElement>(".tool-progress-title");
  const existingValue = activity.querySelector<HTMLElement>(".tool-progress-value");
  const existingTrack = activity.querySelector<HTMLElement>(".tool-progress-track");
  const existingFill = activity.querySelector<HTMLElement>(".tool-progress-fill");
  const existingDetail = activity.querySelector<HTMLElement>(".tool-progress-message");
  if (existingTitle && existingValue && existingTrack && existingFill && existingDetail) {
    return {
      title: existingTitle,
      value: existingValue,
      track: existingTrack,
      fill: existingFill,
      detail: existingDetail,
    };
  }

  const heading = element("div", "tool-progress-heading");
  const title = element("strong", "tool-progress-title");
  const value = element("span", "tool-progress-value");
  const track = element("div", "tool-progress-track");
  const fill = element("span", "tool-progress-fill");
  const detail = element("small", "tool-progress-message");
  heading.append(title, value);
  track.append(fill);
  activity.replaceChildren(heading, track, detail);
  return { title, value, track, fill, detail };
}

function renderToolProgress(streaming: StreamingMessage, event: Extract<ChatStreamEvent, { type: "tool_progress" }>): void {
  const sameToolCall = streaming.progress.toolCallId === event.tool_call_id;
  const reportedTotal = typeof event.total === "number" && Number.isFinite(event.total) && event.total > 0
    ? event.total
    : null;
  const total = reportedTotal ?? (sameToolCall ? streaming.progress.total : null);
  const rawProgress = Number.isFinite(event.progress) ? event.progress : 0;
  const rawPercentage = total === null
    ? null
    : Math.max(0, Math.min(100, (rawProgress / total) * 100));
  const previousPercentage = sameToolCall ? streaming.progress.displayedPercentage : null;
  const displayedPercentage = rawPercentage === null
    ? null
    : Math.max(previousPercentage ?? 0, rawPercentage);
  streaming.progress.toolCallId = event.tool_call_id;
  streaming.progress.displayedPercentage = displayedPercentage;
  streaming.progress.total = total;
  const rawMessage = event.message?.trim() || "MCP 服务正在处理";
  const elapsedSeconds = progressElapsedSeconds(rawMessage);
  const completed = displayedPercentage !== null && displayedPercentage >= 100;
  streaming.content.classList.remove("hidden");
  streaming.content.textContent = completed ? "正在整理结果..." : "正在生成...";
  streaming.activity.classList.remove("hidden");
  streaming.activity.dataset.activityType = "tool-progress";
  const view = toolProgressView(streaming.activity);
  const progressLabel = displayedPercentage === null
    ? "进行中"
    : `${Math.round(displayedPercentage)}%${elapsedSeconds === null ? "" : ` · ${formatProgressElapsed(elapsedSeconds)}`}`;
  view.title.textContent = completed ? "生成完成" : "生成进度";
  view.value.textContent = progressLabel;
  view.track.classList.toggle("is-indeterminate", displayedPercentage === null);
  view.track.setAttribute("role", "progressbar");
  view.track.setAttribute("aria-valuetext", rawMessage);
  if (displayedPercentage !== null) {
    view.track.setAttribute("aria-valuemin", "0");
    view.track.setAttribute("aria-valuemax", "100");
    view.track.setAttribute("aria-valuenow", String(Math.round(displayedPercentage)));
    view.fill.style.setProperty("--tool-progress-ratio", String(displayedPercentage / 100));
  } else {
    view.track.removeAttribute("aria-valuemin");
    view.track.removeAttribute("aria-valuemax");
    view.track.removeAttribute("aria-valuenow");
    view.fill.style.removeProperty("--tool-progress-ratio");
  }
  view.detail.textContent = rawMessage;
  streaming.meta.textContent = completed ? "生成完成，正在整理结果..." : "正在生成...";
}

function renderResourceQueue(streaming: StreamingMessage, event: Extract<ChatStreamEvent, { type: "resource_queue" }>): void {
  if (event.state === "acquired") {
    if (streaming.activity.dataset.activityType === "resource-queue") {
      streaming.activity.classList.add("hidden");
      streaming.activity.replaceChildren();
      delete streaming.activity.dataset.activityType;
    }
    streaming.content.classList.remove("hidden");
    streaming.content.textContent = "正在思考...";
    streaming.meta.textContent = "正在思考...";
    return;
  }
  streaming.content.classList.remove("hidden");
  streaming.content.textContent = "正在排队...";
  streaming.activity.classList.remove("hidden");
  streaming.activity.dataset.activityType = "resource-queue";
  const view = toolProgressView(streaming.activity);
  view.title.textContent = "等待执行资源";
  view.value.textContent = "排队中";
  view.track.classList.add("is-indeterminate");
  view.track.setAttribute("role", "progressbar");
  view.track.setAttribute("aria-valuetext", event.message);
  view.track.removeAttribute("aria-valuemin");
  view.track.removeAttribute("aria-valuemax");
  view.track.removeAttribute("aria-valuenow");
  view.fill.style.removeProperty("--tool-progress-ratio");
  view.detail.textContent = event.message;
  streaming.meta.textContent = event.message;
}

function finalizeStreamingMessage(streaming: StreamingMessage, result: ChatResult): void {
  streaming.message.classList.remove("streaming");
  streaming.activity.classList.add("hidden");
  const displayAnswer = answerWithoutArtifactDownloadDetails(result.answer || "", result.artifacts || []);
  streaming.content.classList.toggle("hidden", !displayAnswer);
  if (displayAnswer) renderRichText(streaming.content, displayAnswer);
  else streaming.content.replaceChildren();
  // Only execution receipts declare deliverables. Prose URLs are not files.
  renderArtifacts(streaming.artifacts, result.artifacts || []);
  streaming.meta.textContent = resultMeta(result);
  if (streaming.runId) {
    streaming.actions.replaceChildren(createResponseActions(streaming.runId, displayAnswer));
    streaming.actions.classList.remove("hidden");
  }
}

function languageFor(name: string): string {
  const extension = name.toLowerCase().split(".").pop() || "";
  const aliases: Record<string, string> = {
    htm: "html", mjs: "javascript", cjs: "javascript", js: "javascript", jsx: "javascript",
    ts: "typescript", tsx: "typescript", py: "python", rs: "rust", sh: "bash", zsh: "bash",
    yml: "yaml", md: "markdown",
  };
  return aliases[extension] || extension;
}

function renderAttachment(container: HTMLElement, attachment: LocalAttachment): void {
  const card = element("article", `chat-attachment ${attachment.kind}`);
  const header = element("div", "attachment-heading");
  header.append(
    element("strong", "", attachment.name),
    element("span", "", formatBytes(attachment.size)),
  );
  card.append(header);
  if (attachment.kind === "image" && attachment.objectUrl) {
    const image = element("img");
    image.src = attachment.objectUrl;
    image.alt = attachment.name;
    image.loading = "lazy";
    card.append(image);
  } else if (attachment.kind === "video" && attachment.objectUrl) {
    const video = element("video");
    video.src = attachment.objectUrl;
    video.controls = true;
    video.preload = "metadata";
    card.append(video, element("small", "attachment-boundary", "视频仅在本地预览，二进制未发送给模型"));
  } else if (attachment.kind === "audio" && attachment.objectUrl) {
    const audio = element("audio");
    audio.src = attachment.objectUrl;
    audio.controls = true;
    audio.preload = "metadata";
    card.append(audio, element("small", "attachment-boundary", "音频已作为不可变附件保存，可由已授权的媒体能力读取"));
  } else if (attachment.kind === "html" && attachment.text !== undefined) {
    const frame = element("iframe");
    frame.title = `${attachment.name} HTML 预览`;
    frame.setAttribute("sandbox", "");
    frame.loading = "lazy";
    frame.srcdoc = attachment.text;
    card.append(frame);
  } else if (attachment.kind === "text" && attachment.text !== undefined) {
    const pre = element("pre");
    const code = element("code");
    const language = languageFor(attachment.name);
    if (language) code.className = `language-${language}`;
    code.textContent = attachment.text;
    pre.append(code);
    card.append(pre);
    try { hljs.highlightElement(code); } catch { /* unknown language stays plain */ }
  } else {
    card.append(element("p", "attachment-boundary", "文件内容未发送给模型，仅提供名称、类型和大小"));
  }
  container.append(card);
}

function renderChatMessage(
  role: "user" | "assistant" | "system" | "notice",
  content: string,
  meta = "",
  attachments: LocalAttachment[] = [],
  artifacts: ChatArtifact[] = [],
  runId?: string,
): HTMLElement {
  const container = messageContainer();
  container.querySelector(".chat-empty")?.remove();
  $("#chat-drop-zone").classList.remove("is-empty");
  const message = element("article", `chat-message ${role}`);
  if (runId) { message.dataset.runId = runId; message.dataset.runStatus = "succeeded"; }
  const avatar = element(
    "span",
    "chat-avatar",
    role === "user" ? "你" : role === "notice" ? "T" : role === "system" ? "!" : "",
  );
  if (role === "assistant") renderAgentAvatar(avatar, state.currentAgentProfile);
  const body = element("div", "chat-message-body");
  body.append(element(
    "span",
    "chat-role",
    role === "user" ? "你" : role === "assistant" ? state.currentAgentProfile?.display_name || "ExoHarness Agent" : role === "notice" ? "编码工作流" : "系统",
  ));
  const displayContent = role === "assistant"
    ? answerWithoutArtifactDownloadDetails(content, artifacts)
    : content;
  let rich: HTMLElement | null = null;
  if (displayContent) {
    rich = element("div", "chat-content rich-content");
    renderRichText(rich, displayContent);
    body.append(rich);
  }
  if (attachments.length) {
    const gallery = element("div", "chat-attachment-gallery");
    attachments.forEach((attachment) => renderAttachment(gallery, attachment));
    body.append(gallery);
  }
  if (role === "assistant") {
    const artifactGallery = element("div", "chat-artifact-gallery hidden");
    renderArtifacts(artifactGallery, artifacts);
    if (artifactGallery.childElementCount) body.append(artifactGallery);
  }
  if (meta || (runId && role === "assistant")) {
    const footer = element("div", "chat-message-footer");
    if (meta) footer.append(element("small", "chat-meta", meta));
    if (runId && role === "assistant") footer.append(createResponseActions(runId, displayContent));
    body.append(footer);
  }
  message.append(avatar, body);
  container.append(message);
  scrollChatToLatest(role === "user");
  return message;
}

function renderRunStatus(runId: string, status: string, text: string): void {
  const message = renderChatMessage("system", text);
  message.dataset.runId = runId;
  message.dataset.runStatus = status;
}

function renderSavedOutputs(artifacts: ChatArtifact[]): void {
  if (!artifacts.length) return;
  // Saved material may be a background or another intermediate. Keep it
  // accessible without presenting it as a finished requested deliverable.
  const message = renderChatMessage("system", "");
  const details = element("details", "chat-saved-outputs");
  details.append(element("summary", "", "查看已保存素材与文件（任务未完成，非最终成品）"));
  const gallery = element("div", "chat-artifact-gallery");
  renderArtifacts(gallery, artifacts);
  details.append(gallery);
  message.querySelector(".chat-message-body")?.append(details);
}

function createStreamingAssistant(): StreamingMessage {
  const container = messageContainer();
  container.querySelector(".chat-empty")?.remove();
  const message = element("article", "chat-message assistant streaming");
  const avatar = element("span", "chat-avatar");
  renderAgentAvatar(avatar, state.currentAgentProfile);
  const body = element("div", "chat-message-body");
  const content = element("div", "chat-content rich-content stream-content", "正在思考...");
  const activity = element("div", "chat-tool-activity hidden");
  const artifacts = element("div", "chat-artifact-gallery hidden");
  const meta = element("small", "chat-meta", "正在思考...");
  const footer = element("div", "chat-message-footer");
  const actions = element("div", "hidden");
  footer.append(meta, actions);
  body.append(element("span", "chat-role", state.currentAgentProfile?.display_name || "ExoHarness Agent"), content, activity, artifacts, footer);
  message.append(avatar, body);
  container.append(message);
  scrollChatToLatest(true);
  return {
    message,
    content,
    activity,
    artifacts,
    meta,
    actions,
    runId: null,
    progress: { toolCallId: null, displayedPercentage: null, total: null },
  };
}

interface ResponseActionState {
  feedback: "like" | "dislike" | null;
  reason: string | null;
  detail: string;
  requires_tool_confirmation: boolean;
}

const ACTION_ICONS: Record<string, string> = {
  copy: '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/>',
  like: '<path d="M7 10v10H4a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1h3Zm0 0 5-7c2 0 2 2 1 6h6a2 2 0 0 1 2 2l-2 7a3 3 0 0 1-3 2H7"/>',
  dislike: '<path d="M7 14V4H4a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h3Zm0 0 5 7c2 0 2-2 1-6h6a2 2 0 0 0 2-2l-2-7a3 3 0 0 0-3-2H7"/>',
  regenerate: '<path d="M20 7v5h-5M4 17v-5h5"/><path d="M6.1 7a7 7 0 0 1 11.6-1L20 9M4 15l2.3 3A7 7 0 0 0 18 17"/>',
};
const pendingResponseActions = new Map<string, { body: string; eventId: string }>();
let feedbackPopover: { panel: HTMLElement; anchor: HTMLButtonElement } | null = null;

function actionButton(action: string, label: string): HTMLButtonElement {
  const button = element("button", "response-action");
  button.type = "button";
  button.dataset.action = action;
  button.title = label;
  button.setAttribute("aria-label", label);
  button.innerHTML = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ACTION_ICONS[action]}</svg>`;
  return button;
}

async function responseAction(runId: string, action: string, fields: Record<string, unknown> = {}): Promise<{ state: ResponseActionState; started?: StartedRun }> {
  const key = `${runId}:${action}`;
  const body = JSON.stringify({ action, ...fields });
  const previous = pendingResponseActions.get(key);
  const pending = previous?.body === body ? previous : { body, eventId: crypto.randomUUID() };
  // Keep the same event ID after a lost response, so a retry cannot start a
  // second generation or record the same click twice.
  pendingResponseActions.set(key, pending);
  const feedbackChange = ["like", "dislike", "clear_feedback"].includes(action);
  const feedbackButtons = [...document.querySelectorAll<HTMLButtonElement>(".response-actions button")]
    .filter((button) => button.parentElement?.dataset.runId === runId && ["like", "dislike"].includes(button.dataset.action || ""));
  if (feedbackChange) feedbackButtons.forEach((button) => { button.disabled = true; });
  try {
    const result = await api<{ state: ResponseActionState; started?: StartedRun }>(`/v1/runs/${encodeURIComponent(runId)}/response-actions`, {
      method: "POST", body: { event_id: pending.eventId, action, ...fields },
    });
    pendingResponseActions.delete(key);
    if (feedbackChange) for (const name of ["like", "dislike", "clear_feedback"]) pendingResponseActions.delete(`${runId}:${name}`);
    state.responseActions[runId] = result.state;
    updateResponseActions(runId);
    return result;
  } finally {
    if (feedbackChange) feedbackButtons.forEach((button) => { button.disabled = false; });
  }
}

function updateResponseActions(runId: string): void {
  const feedback = state.responseActions[runId]?.feedback;
  document.querySelectorAll<HTMLElement>(".response-actions").forEach((toolbar) => {
    if (toolbar.dataset.runId !== runId) return;
    for (const action of ["like", "dislike"]) {
      const button = toolbar.querySelector<HTMLButtonElement>(`[data-action="${action}"]`);
      button?.setAttribute("aria-pressed", String(feedback === action));
    }
  });
}

async function copyReply(content: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(content);
    return;
  }
  // LAN HTTP pages may not expose the Clipboard API.
  const field = element("textarea", "response-copy-buffer");
  field.value = content;
  field.setAttribute("aria-hidden", "true");
  document.body.append(field);
  field.select();
  const copied = document.execCommand("copy");
  field.remove();
  if (!copied) throw new Error("浏览器不允许复制，请手动选择回复内容");
}

function createResponseActions(runId: string, content: string): HTMLElement {
  const toolbar = element("div", "response-actions");
  toolbar.dataset.runId = runId;
  toolbar.setAttribute("role", "group");
  toolbar.setAttribute("aria-label", "回复操作");
  for (const [action, label] of [["copy", "复制回复"], ["like", "点赞"], ["dislike", "点踩"], ["regenerate", "重新生成"]]) {
    const button = actionButton(action, label);
    if (action === "copy") button.disabled = !content;
    if (action === "like" || action === "dislike") button.setAttribute("aria-pressed", String(state.responseActions[runId]?.feedback === action));
    if (action === "dislike") button.setAttribute("aria-haspopup", "dialog");
    button.addEventListener("click", guarded(async () => {
      if (action === "dislike") {
        await openFeedbackPopover(button, runId);
        return;
      }
      button.disabled = true;
      try {
        if (action === "copy") {
          await copyReply(content);
          await responseAction(runId, "copy");
          showToast("已复制回复");
        } else if (action === "like") {
          closeFeedbackPopover();
          await responseAction(runId, state.responseActions[runId]?.feedback === "like" ? "clear_feedback" : "like");
        } else {
          if (!PORTAL_MODE && state.running) { showToast("当前会话正在执行，完成后可重新生成"); return; }
          const current = await api<ResponseActionState>(`/v1/runs/${encodeURIComponent(runId)}/response-actions`);
          if (current.requires_tool_confirmation && !window.confirm("原回复调用过工具。重新生成可能再次执行工具、产生费用或其他操作。确定继续吗？")) return;
          const result = await responseAction(runId, "regenerate", { confirm_tool_execution: current.requires_tool_confirmation });
          if (result.started && state.currentSessionId === result.started.session_id) await loadSession(result.started.session_id);
          showToast(result.started?.status === "queued" ? "重新生成已加入队列" : "已开始重新生成，原回复已保留");
        }
      } finally { button.disabled = action === "copy" && !content; }
    }));
    toolbar.append(button);
  }
  return toolbar;
}

function closeFeedbackPopover(restoreFocus = false): void {
  if (!feedbackPopover) return;
  const { panel, anchor } = feedbackPopover;
  feedbackPopover = null;
  panel.remove();
  anchor.setAttribute("aria-expanded", "false");
  if (restoreFocus && anchor.isConnected) anchor.focus();
}

function positionFeedbackPopover(): void {
  if (!feedbackPopover) return;
  const { panel, anchor } = feedbackPopover;
  const bounds = anchor.getBoundingClientRect();
  const width = panel.offsetWidth;
  const height = panel.offsetHeight;
  panel.style.left = `${Math.max(12, Math.min(bounds.left, window.innerWidth - width - 12))}px`;
  const top = bounds.bottom + 9 + height <= window.innerHeight - 12 ? bounds.bottom + 9 : bounds.top - height - 9;
  panel.style.top = `${Math.max(12, top)}px`;
}

async function openFeedbackPopover(anchor: HTMLButtonElement, runId: string): Promise<void> {
  if (feedbackPopover?.anchor === anchor) { closeFeedbackPopover(); return; }
  closeFeedbackPopover();
  window.dispatchEvent(new CustomEvent("exoharness:response-feedback-open"));
  const panel = element("section", "response-feedback-popover");
  panel.setAttribute("role", "dialog");
  panel.setAttribute("aria-label", "反馈原因");
  panel.append(element("strong", "", "哪里需要改进？"), element("p", "", "你的反馈会帮助我们改进回复体验。"));
  const choices = element("div", "response-feedback-choices");
  const editor = element("div", "response-feedback-editor hidden");
  const input = element("textarea");
  input.rows = 3;
  input.maxLength = 400; // Unicode-aware 200-character limit below, including emoji.
  input.placeholder = "请描述具体问题（最多 200 字）";
  input.setAttribute("aria-label", "其他原因，最多 200 字");
  const counter = element("span", "response-feedback-counter", "0 / 200");
  const submit = element("button", "response-feedback-submit", "提交反馈");
  submit.type = "button";
  submit.disabled = true;
  const updateInput = (): void => {
    input.value = Array.from(input.value).slice(0, 200).join("");
    counter.textContent = `${Array.from(input.value).length} / 200`;
    submit.disabled = !input.value.trim();
  };
  input.addEventListener("input", (event) => { if (!(event as InputEvent).isComposing) updateInput(); });
  input.addEventListener("compositionend", updateInput);
  editor.append(input, counter, submit);
  const send = async (reason: string, detail = ""): Promise<void> => {
    panel.querySelectorAll<HTMLButtonElement>("button").forEach((button) => { button.disabled = true; });
    try {
      await responseAction(runId, "dislike", { reason, detail });
      if (feedbackPopover?.panel === panel) closeFeedbackPopover(true);
      showToast("感谢反馈，已记录");
    } finally { panel.querySelectorAll<HTMLButtonElement>("button").forEach((button) => { button.disabled = false; }); updateInput(); }
  };
  for (const [reason, label] of [["incorrect", "内容不准确"], ["irrelevant", "没有理解需求"], ["incomplete", "内容不完整"], ["poor_style", "表达不合适"], ["other", "其他"]]) {
    const button = element("button", "response-feedback-choice", label);
    button.type = "button";
    button.dataset.reason = reason;
    button.addEventListener("click", guarded(async () => {
      if (reason !== "other") { await send(reason); return; }
      editor.classList.remove("hidden");
      choices.querySelectorAll("button").forEach((choice) => choice.classList.toggle("selected", choice === button));
      positionFeedbackPopover();
      input.focus();
    }));
    choices.append(button);
  }
  submit.addEventListener("click", guarded(() => send("other", input.value.trim())));
  const clear = element("button", "response-feedback-clear", "取消点踩");
  clear.type = "button";
  clear.addEventListener("click", guarded(async () => {
    clear.disabled = true;
    try { await responseAction(runId, "clear_feedback"); if (feedbackPopover?.panel === panel) closeFeedbackPopover(true); }
    finally { clear.disabled = false; }
  }));
  panel.append(choices, editor, clear);
  document.body.append(panel);
  feedbackPopover = { panel, anchor };
  anchor.setAttribute("aria-expanded", "true");
  positionFeedbackPopover();
  if (state.responseActions[runId]?.reason === "other") {
    input.value = state.responseActions[runId].detail;
    editor.classList.remove("hidden");
    updateInput(); positionFeedbackPopover();
  }
  if (state.responseActions[runId]?.feedback !== "dislike") {
    panel.querySelectorAll<HTMLButtonElement>("button").forEach((button) => { button.disabled = true; });
    try { await responseAction(runId, "dislike"); }
    catch (error) { if (feedbackPopover?.panel === panel) closeFeedbackPopover(); throw error; }
    finally { panel.querySelectorAll<HTMLButtonElement>("button").forEach((button) => { button.disabled = false; }); updateInput(); }
  }
}

document.addEventListener("pointerdown", (event) => {
  if (feedbackPopover && !feedbackPopover.panel.contains(event.target as Node) && !feedbackPopover.anchor.contains(event.target as Node)) closeFeedbackPopover();
});
document.addEventListener("keydown", (event) => { if (event.key === "Escape") closeFeedbackPopover(true); });
window.addEventListener("resize", positionFeedbackPopover);
window.addEventListener("exoharness:account-menu-open", () => closeFeedbackPopover());
window.addEventListener("scroll", positionFeedbackPopover, true);

function renderCapabilityWarnings(warnings: string[]): void {
  if (!warnings.length) return;
  const container = messageContainer();
  const notice = element("aside", "chat-capability-warning");
  const icon = element("span", "", "!");
  icon.setAttribute("aria-hidden", "true");
  const copy = element("div");
  copy.append(element("strong", "", "服务提示"));
  if (PORTAL_MODE) {
    for (const warning of warnings) {
      // Older responses may contain transport internals; never expose them in the portal.
      const legacy = warning.match(/^(?:MCP|能力)\s+([^\s]{1,80})/);
      copy.append(element("p", "", legacy
        ? `「${legacy[1]}」${warning.includes("权限") ? "尚未获得所需权限。" : "暂时无法连接，请稍后重试。"}`
        : warning.startsWith("暂时无法连接「") || warning.startsWith("「") ? warning : "某项服务暂时不可用，请稍后重试。"));
    }
    notice.append(icon, copy);
    container.append(notice);
    scrollChatToLatest();
    return;
  }
  const list = element("ul");
  for (const warning of warnings) list.append(element("li", "", warning));
  copy.append(list);
  const action = element("a", "", "检查服务与权限 →");
  action.href = "/services";
  notice.append(icon, copy, action);
  container.append(notice);
  scrollChatToLatest();
}

function phaseLabel(phase: string): string {
  return ({
    waiting: "等待中",
    preparing: "准备上下文",
    connecting: "正在连接模型",
    thinking: "正在思考",
    responding: "正在生成",
    tool_exec: "正在执行工具",
    compacting: "正在整理上下文",
    rate_limited: "请求受限，等待重试",
  } as Record<string, string>)[phase] || phase;
}

// Terminal and error events are easy to miss when a long stream is scrolled
// away from the composer. Keep the conversation as the source of detail, but
// also provide a short toast for the state transition. The key is normalized
// per Run so a status=succeeded event followed by complete does not produce
// two notifications.
const runToastKeys = new Map<string, string>();

function notifyRunEvent(runId: string, event: ChatStreamEvent): void {
  let key: string | null = null;
  let message = "";
  let error = false;
  if (event.type === "complete") {
    key = "completed";
    message = "运行已完成";
  } else if (event.type === "error" || event.type === "failed") {
    key = "failed";
    message = event.message || "运行失败";
    error = true;
  } else if (event.type === "cancelled") {
    key = "cancelled";
    message = "运行已停止";
  } else if (event.type === "recoverable") {
    key = "recoverable";
    message = "运行已暂停，可以继续恢复";
  } else if (event.type === "upstream_error") {
    key = `upstream:${event.message}`;
    message = PORTAL_MODE ? "上游服务暂时不可用，请稍后重试" : event.message || "上游服务返回错误";
    error = true;
  } else if (event.type === "status") {
    const status = (event.status || "").toLowerCase();
    if (["succeeded", "success", "completed", "complete"].includes(status)) {
      key = "completed";
      message = event.message || "运行已完成";
    } else if (["failed", "error", "errored"].includes(status)) {
      key = "failed";
      message = event.message || "运行失败";
      error = true;
    } else if (["cancelled", "canceled", "stopped"].includes(status)) {
      key = "cancelled";
      message = event.message || "运行已停止";
    } else if (status === "recoverable") {
      key = "recoverable";
      message = event.message || "运行已暂停，可以继续恢复";
    }
  }
  if (!key || runToastKeys.get(runId) === key) return;
  runToastKeys.set(runId, key);
  showToast(message, error);
  window.setTimeout(() => {
    if (runToastKeys.get(runId) === key) runToastKeys.delete(runId);
  }, 10_000);
}

async function ensureSession(): Promise<string> {
  if (state.currentSessionId) return state.currentSessionId;
  const session = await api<ExecutionSession>("/v1/sessions", {
    method: "POST",
    body: { title: null, actor: "web-console" },
  });
  state.currentSessionId = session.id;
  state.currentAgentProfile = session.agent_profile || state.defaultAgentProfile;
  updateConversationIdentity();
  window.localStorage.setItem("exoharness.session", session.id);
  updateSessionLocation(session.id);
  await refreshSessions();
  return session.id;
}

async function startTurn(
  sessionId: string,
  prompt: string,
  attachments: LocalAttachment[],
): Promise<StartedRun> {
  return api<StartedRun>(`/v1/sessions/${encodeURIComponent(sessionId)}/turns`, {
    method: "POST",
    body: {
      model: null,
      prompt,
      attachments: attachments.map(attachmentPayload),
      capability_ids: [],
      actor: "web-console",
    },
  });
}

function applyRunEvent(
  event: ChatStreamEvent,
  streaming: StreamingMessage,
  streamed: { answer: string; toolCalled: boolean },
): void {
  if (event.type === "assistant_start") {
    streamed.answer = "";
    streamed.toolCalled = false;
    streaming.content.classList.remove("hidden");
    streaming.content.textContent = "正在思考...";
    streaming.meta.textContent = "正在思考...";
  } else if (event.type === "text_delta") {
    streamed.answer += event.delta;
    if (!streamed.toolCalled) {
      streaming.content.classList.remove("hidden");
      streaming.content.textContent = streamed.answer;
    }
    streaming.meta.textContent = "正在回答...";
  } else if (event.type === "phase") {
    streaming.meta.textContent = phaseLabel(event.phase);
    if (!streamed.answer && !streamed.toolCalled) streaming.content.textContent = "正在思考...";
  } else if (event.type === "status") {
    streaming.meta.textContent = event.message;
  } else if (event.type === "tool_call") {
    streamed.toolCalled = true;
    streamed.answer = "";
    streaming.content.textContent = "正在思考...";
    streaming.meta.textContent = PORTAL_MODE ? "正在调用能力" : `正在调用 ${event.tool}`;
  } else if (event.type === "tool_output") {
    streaming.content.textContent = "正在处理结果...";
    streaming.meta.textContent = "工具正在返回结果";
  } else if (event.type === "tool_progress") {
    renderToolProgress(streaming, event);
  } else if (event.type === "resource_queue") {
    renderResourceQueue(streaming, event);
  } else if (event.type === "artifact") {
    renderArtifacts(streaming.artifacts, [event.artifact]);
  } else if (event.type === "tool_result") {
    streaming.activity.classList.add("hidden");
    streaming.content.textContent = "正在整理结果...";
    streaming.meta.textContent = event.is_error
      ? (PORTAL_MODE ? "正在检查能力返回并继续处理" : `${event.tool} 返回错误，正在检查`)
      : (PORTAL_MODE ? "能力调用完成，正在整理" : `${event.tool} 调用完成，模型正在整理`);
  } else if (event.type === "upstream_error") {
    streaming.meta.textContent = event.message;
  }
  scrollChatToLatest();
}

function followRun(runId: string, streaming: StreamingMessage): Promise<ChatResult | null> {
  streaming.runId = runId;
  streaming.message.dataset.runId = runId;
  closeRunStream(runId);
  const streamed = { answer: "", toolCalled: false };
  return new Promise((resolve, reject) => {
    const source = new EventSource(`/v1/runs/${encodeURIComponent(runId)}/events`);
    state.eventSources.set(runId, source);
    // A queued follow-up must not steal Stop from the active Run ahead of it.
    state.currentRunId = state.eventSources.keys().next().value || runId;
    updateSendButton();
    source.addEventListener("chat", (rawEvent) => {
      try {
        const event = JSON.parse((rawEvent as MessageEvent<string>).data) as ChatStreamEvent;
        applyRunEvent(event, streaming, streamed);
        notifyRunEvent(runId, event);
        syncChatPetFromEvent(event);
        if (event.type === "complete") {
          closeRunStream(runId);
          resolve(event.response);
        } else if (event.type === "error" || event.type === "failed") {
          closeRunStream(runId);
          if (event.type === "failed") renderSavedOutputs(event.artifacts || []);
          reject(new Error(event.message));
        } else if (event.type === "cancelled") {
          closeRunStream(runId);
          streaming.message.remove();
          renderRunStatus(runId, "cancelled", "运行已停止。此前完成的消息和事件已经保留。");
          resolve(null);
        } else if (event.type === "recoverable") {
          closeRunStream(runId);
          streaming.message.remove();
          renderRecoveryMessage(runId, event.message);
          resolve(null);
        }
      } catch (error) {
        closeRunStream(runId);
        reject(error);
      }
    });
    source.onerror = () => {
      if (source.readyState !== EventSource.CLOSED) {
        streaming.meta.textContent = "连接中断，正在从上次事件继续…";
      }
    };
  });
}

function extension(name: string): string {
  return name.toLowerCase().split(".").pop() || "";
}

function normalizedMime(file: File): string {
  if (file.type) return file.type.toLowerCase();
  const ext = extension(file.name);
  if (ext === "html" || ext === "htm") return "text/html";
  if (ext === "md" || ext === "markdown") return "text/markdown";
  if (ext === "json" || ext === "jsonl") return "application/json";
  if (ext === "mp3") return "audio/mpeg";
  if (TEXT_EXTENSIONS.has(ext)) return "text/plain";
  return "application/octet-stream";
}

function attachmentKind(file: File, mimeType: string): AttachmentKind {
  if (IMAGE_MIMES.has(mimeType)) return "image";
  if (mimeType === "audio/mpeg") return "audio";
  if (mimeType.startsWith("video/")) return "video";
  if (mimeType === "text/html" || ["html", "htm"].includes(extension(file.name))) return "html";
  if (mimeType.startsWith("text/") || ["application/json", "application/xml"].includes(mimeType) || TEXT_EXTENSIONS.has(extension(file.name))) return "text";
  return "file";
}

async function fileToBase64(file: File): Promise<string> {
  const result = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error(`无法读取 ${file.name}`));
    reader.onload = () => resolve(String(reader.result));
    reader.readAsDataURL(file);
  });
  const comma = result.indexOf(",");
  if (comma < 0) throw new Error(`无法编码 ${file.name}`);
  return result.slice(comma + 1);
}

async function createAttachment(file: File): Promise<LocalAttachment> {
  const mimeType = normalizedMime(file);
  const kind = attachmentKind(file, mimeType);
  const attachment: LocalAttachment = {
    id: crypto.randomUUID(),
    name: file.name,
    mimeType,
    size: file.size,
    kind,
  };
  if (kind === "image") {
    if (file.size > MAX_IMAGE_BYTES) throw new Error(`${file.name} 超过图片 5 MiB 限制`);
    attachment.base64 = await fileToBase64(file);
    attachment.objectUrl = URL.createObjectURL(file);
  } else if (kind === "audio") {
    if (file.size > MAX_AUDIO_BYTES) throw new Error(`${file.name} 超过音频 32 MiB 限制`);
    attachment.base64 = await fileToBase64(file);
    attachment.objectUrl = URL.createObjectURL(file);
  } else if (kind === "video") {
    attachment.objectUrl = URL.createObjectURL(file);
  } else if (kind === "html" || kind === "text") {
    if (file.size > MAX_TEXT_BYTES) throw new Error(`${file.name} 超过文本附件 512 KiB 限制`);
    attachment.text = await file.text();
    if (["md", "py", "js", "mjs", "cjs", "ts", "tsx", "mts", "cts", "sh", "bash"].includes(extension(file.name))) attachment.base64 = await fileToBase64(file);
  } else if (extension(file.name) === "zip") {
    if (file.size > 32 * 1024 * 1024) throw new Error(`${file.name} 超过 ZIP 包 32 MiB 限制`);
    attachment.base64 = await fileToBase64(file);
  }
  if (attachment.objectUrl?.startsWith("blob:")) state.objectUrls.add(attachment.objectUrl);
  return attachment;
}

async function addFiles(files: File[]): Promise<void> {
  const available = MAX_ATTACHMENTS - state.pending.length;
  if (available <= 0) throw new Error(`一次最多添加 ${MAX_ATTACHMENTS} 个附件`);
  for (const file of files.slice(0, available)) state.pending.push(await createAttachment(file));
  if (files.length > available) showToast(`只添加了前 ${available} 个文件，一次最多 ${MAX_ATTACHMENTS} 个`, true);
  renderPendingAttachments();
}

function renderPendingAttachments(): void {
  const container = $("#pending-attachments");
  container.replaceChildren();
  container.classList.toggle("hidden", state.pending.length === 0);
  for (const attachment of state.pending) {
    const chip = element("div", "pending-attachment");
    if (attachment.kind === "image" && attachment.objectUrl) {
      const preview = element("img");
      preview.src = attachment.objectUrl;
      preview.alt = "";
      chip.append(preview);
    } else {
      chip.append(element("span", "pending-kind", attachment.kind.toUpperCase()));
    }
    const text = element("span", "pending-name");
    text.append(element("strong", "", attachment.name), element("small", "", formatBytes(attachment.size)));
    const remove = element("button", "pending-remove", "×");
    remove.type = "button";
    remove.title = `移除 ${attachment.name}`;
    remove.addEventListener("click", () => removePendingAttachment(attachment.id));
    chip.append(text, remove);
    container.append(chip);
  }
  updateSendButton();
}

function removePendingAttachment(id: string): void {
  const index = state.pending.findIndex((attachment) => attachment.id === id);
  if (index < 0) return;
  const [attachment] = state.pending.splice(index, 1);
  if (attachment.objectUrl) {
    URL.revokeObjectURL(attachment.objectUrl);
    state.objectUrls.delete(attachment.objectUrl);
  }
  renderPendingAttachments();
}

function attachmentPayload(attachment: LocalAttachment): ApiAttachment {
  if (attachment.kind === "image" || attachment.kind === "audio") {
    return { type: attachment.kind, name: attachment.name, mime_type: attachment.mimeType, data: attachment.base64 || "" };
  }
  if (attachment.kind === "html" || attachment.kind === "text") {
    return { type: "text", name: attachment.name, mime_type: attachment.mimeType, content: attachment.text || "" };
  }
  return { type: "file", name: attachment.name, mime_type: attachment.mimeType, size: attachment.size };
}

function historyText(prompt: string, attachments: LocalAttachment[]): string {
  const summaries = attachments.map((attachment) => {
    if (attachment.kind === "html" || attachment.kind === "text") {
      return `\n\n[附件 ${attachment.name} (${attachment.mimeType})]\n${attachment.text || ""}`;
    }
    return `\n\n[附件 ${attachment.name} (${attachment.mimeType}, ${attachment.size} bytes)]`;
  });
  return `${prompt}${summaries.join("")}`;
}

async function submitPortalMcpInstall(intent: Exclude<McpInstallIntent, { kind: "guidance" }>): Promise<void> {
  const sessionId = await ensureSession();
  if (intent.kind === "github") {
    const job = await submitIdempotentGithubInstall("mcp", intent.content, sessionId);
    renderChatMessage("assistant", "已提交公开 GitHub MCP 项目的隔离安装；来源固定、运行环境和协议检查完成后才能使用。可在「设置 → 我的能力」查看进度或取消登记前的下载。");
    void followPortalInstallJob(job.id, sessionId);
    await refreshSessions();
    return;
  }
  await api(intent.kind === "json" ? "/v1/user-capabilities/mcp/json" : "/v1/user-capabilities/mcp/http", {
    method: "POST",
    body: intent.kind === "json"
      ? { content: intent.content, session_id: sessionId }
      : { url: intent.content, session_id: sessionId },
  });
  renderChatMessage("assistant", "已提交远程 MCP 连接检查。检查通过后才能使用；若需要认证，请到「设置 → 我的能力」填写完整 Authorization 值。");
  await refreshSessions();
}

async function submitPortalSkillInstall(intent: Exclude<SkillInstallIntent, { kind: "guidance" }>): Promise<void> {
  const sessionId = await ensureSession();
  const job = await submitIdempotentGithubInstall("skill", intent.content, sessionId);
  renderChatMessage("assistant", "已提交公开 GitHub Skill 项目的隔离安装；检查通过后才能使用。可在「设置 → 我的能力」查看进度或取消登记前的下载。");
  void followPortalInstallJob(job.id, sessionId);
  await refreshSessions();
}

interface PortalInstallJob {
  id: string;
  status: "queued" | "downloading" | "registering" | "preparing" | "ready" | "blocked" | "cancelled";
  message: string;
}

async function skillInstallationAllowed(): Promise<boolean> {
  const catalog = await api<{ policy: { install_skills: boolean } }>("/v1/user-capabilities");
  return catalog.policy.install_skills;
}

async function submitIdempotentGithubInstall(kind: "skill" | "mcp", url: string, sessionId: string): Promise<PortalInstallJob> {
  const submit = () => api<PortalInstallJob>("/v1/user-capabilities/install/github", {
    method: "POST", body: { kind, url, session_id: sessionId },
  });
  try {
    return await submit();
  } catch (error) {
    if (!(error instanceof TypeError)) throw error;
    // The server keys this installation by user, kind and URL. Repeating the
    // exact POST after a transport failure reattaches to the same durable job.
    await new Promise<void>((resolve) => window.setTimeout(resolve, 350));
    return submit();
  }
}

async function followPortalInstallJob(jobId: string, sessionId: string): Promise<void> {
  for (let attempt = 0; attempt < 360; attempt += 1) {
    await new Promise<void>((resolve) => window.setTimeout(resolve, 5000));
    try {
      const jobs = await api<{ jobs: PortalInstallJob[] }>("/v1/user-capabilities/install-jobs");
      const job = jobs.jobs.find((item) => item.id === jobId);
      if (!job) return;
      if (["ready", "blocked", "cancelled"].includes(job.status)) {
        if (state.currentSessionId === sessionId) {
          renderChatMessage("system", job.status === "ready" ? "能力安装检查通过，可以在当前授权范围内使用。" : job.message);
        }
        await refreshSessions();
        return;
      }
    } catch {
      // A temporary network loss does not cancel the persisted install job.
    }
  }
  if (state.currentSessionId === sessionId) showToast("安装仍在后台处理，请到设置 → 我的能力查看状态。", true);
}

async function submitPortalToolInstall(attachment: LocalAttachment): Promise<void> {
  const sessionId = await ensureSession();
  const archive = extension(attachment.name) === "zip";
  await api("/v1/user-capabilities/tool/package", {
    method: "POST",
    body: archive
      ? { file_name: attachment.name, archive_base64: attachment.base64, session_id: sessionId }
      : { file_name: attachment.name, files: [{ path: attachment.name, content_base64: attachment.base64 }], session_id: sessionId },
  });
  renderChatMessage("assistant", "Tool 已登记，正在隔离环境中检查依赖与入口；验证通过后才能使用。");
  await refreshSessions();
}

async function runChat(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (speechInput.busy()) { showToast("请先结束语音输入或取消录音"); return; }
  const promptInput = $("#chat-prompt") as HTMLTextAreaElement;
  const prompt = promptInput.value.trim();
  const attachments = [...state.pending];
  if (!prompt && !attachments.length) throw new Error("请输入消息或添加附件");
  let install: McpInstallIntent | null;
  let skillInstall: SkillInstallIntent | null;
  let toolInstall: LocalAttachment | null;
  try {
    install = PORTAL_MODE ? remoteMcpInstallIntent(prompt, attachments.length) : null;
    skillInstall = PORTAL_MODE ? skillInstallIntent(prompt, attachments.length) : null;
    toolInstall = PORTAL_MODE ? toolPackageInstallIntent(prompt, attachments) : null;
  } catch (error) {
    promptInput.value = "";
    resizeComposer();
    updateSendButton();
    throw error;
  }
  if (install) {
    if (install.kind === "guidance") {
      renderChatMessage("user", prompt);
      promptInput.value = "";
      resizeComposer();
      updateSendButton();
      renderChatMessage("assistant", "我可以帮你安装这个 MCP，但需要先确定可信来源。请打开该项目的官方页面，复制公开 GitHub 仓库链接，然后发送「安装 MCP https://github.com/owner/repository」；如果你已有运行中的 MCP 服务，则发送其 HTTPS 地址，或粘贴地址型 mcpServers JSON。无需上传文件，也不要在对话中发送密钥；安装后如需认证，请到「设置 → 我的能力」填写。收到链接后我会自动检查来源、权限和运行环境，再告诉你是否可用。");
      return;
    }
    renderChatMessage("user", "安装 MCP（配置内容不在对话中显示）");
    promptInput.value = "";
    resizeComposer();
    updateSendButton();
    try {
      await submitPortalMcpInstall(install);
    } catch (error) {
      renderChatMessage("system", error instanceof Error ? error.message : String(error));
      throw error;
    }
    return;
  }
  if (skillInstall) {
    if (skillInstall.kind === "guidance") {
      const allowed = await skillInstallationAllowed();
      renderChatMessage("user", prompt);
      promptInput.value = "";
      resizeComposer();
      updateSendButton();
      renderChatMessage("assistant", allowed
        ? "安装 Skill 需要明确的公开 GitHub 来源。请发送「安装 Skill https://github.com/owner/repository/tree/main/skills/name」；若 Skill 位于仓库根目录，也可发送仓库链接。服务端会自动下载、检查并在隔离沙盒准备运行环境，无需手工操作终端。"
        : "当前账号没有安装 Skill 的权限，本次未安装。请管理员在用户管理中开启“安装 Skill”权限；无需为这个权限问题操作“导入准备”。已有 Skill 的使用权限与安装权限分别控制。");
      return;
    }
    renderChatMessage("user", "从 GitHub 安装 Skill");
    promptInput.value = "";
    resizeComposer();
    updateSendButton();
    try {
      await submitPortalSkillInstall(skillInstall);
    } catch (error) {
      renderChatMessage("system", error instanceof Error ? error.message : String(error));
      throw error;
    }
    return;
  }
  if (PORTAL_MODE && skillCreationIntent(prompt)) {
    const allowed = await skillInstallationAllowed();
    renderChatMessage("user", prompt);
    promptInput.value = "";
    resizeComposer();
    updateSendButton();
    renderChatMessage("assistant", allowed
      ? "当前对话不能直接把新写的 Skill 登记启用；受管安装需要公开 GitHub 仓库或 Skill 子目录链接。提供链接后，服务端会自动检查并在隔离沙盒准备环境，无需管理员手工安装。若只想先编写 SKILL.md 内容，请明确说“只写草稿，不安装”。"
      : "当前账号没有安装 Skill 的权限，不能创建并启用新的 Skill；本次未安装。请管理员在用户管理中开启“安装 Skill”权限，不必为这个权限问题操作“导入准备”。若只需要 SKILL.md 文字草稿，请明确说“只写草稿，不安装”。");
    return;
  }
  if (toolInstall) {
    renderChatMessage("user", "安装 Tool（包内容不在对话中显示）");
    state.pending = [];
    renderPendingAttachments();
    promptInput.value = "";
    resizeComposer();
    updateSendButton();
    try {
      await submitPortalToolInstall(toolInstall);
    } catch (error) {
      renderChatMessage("system", error instanceof Error ? error.message : String(error));
      throw error;
    }
    return;
  }
  renderChatMessage("user", prompt, "", attachments);
  state.pending = [];
  renderPendingAttachments();
  promptInput.value = "";
  resizeComposer();
  state.running = true;
  updateSendButton();
  setChatPetLifecycle("generating");
  const streaming = createStreamingAssistant();
  try {
    const sessionId = await ensureSession();
    const started = await startTurn(sessionId, prompt, attachments);
    const completion = followRun(started.run_id, streaming);
    void refreshSessions().catch(() => {});
    const result = await completion;
    if (!result) return;
    finalizeStreamingMessage(streaming, result);
    renderCapabilityWarnings(result.warnings);
    state.history.push(
      { role: "user", content: historyText(prompt, attachments) },
      { role: "assistant", content: result.answer || "" },
    );
    state.history = state.history.slice(-100);
    $("#route-status").textContent = PORTAL_MODE
      ? result.model.name
      : `${result.model.name} · ${result.model.provider}/${result.model.model_id}`;
    document.querySelector(".route-indicator")?.classList.remove("unconfigured");
    await refreshSessions();
  } catch (error) {
    streaming.message.remove();
    if (streaming.runId) renderRunStatus(streaming.runId, "failed", error instanceof Error ? error.message : String(error));
    else renderChatMessage("system", error instanceof Error ? error.message : String(error));
    setChatPetLifecycle("failed");
    throw error;
  } finally {
    state.running = state.eventSources.size > 0;
    updateSendButton();
  }
}

function persistedAttachment(attachment: PersistedAttachment): LocalAttachment {
  const kind: AttachmentKind = attachment.type === "text"
    ? (attachment.mime_type === "text/html" ? "html" : "text")
    : attachment.type === "image" ? "image" : attachment.type === "audio" ? "audio" : "file";
  return {
    id: attachment.attachment_id || crypto.randomUUID(),
    name: attachment.name,
    mimeType: attachment.mime_type,
    size: attachment.size,
    kind,
    objectUrl: attachment.download_url || undefined,
    text: attachment.content,
  };
}

function resultMeta(result: ChatResult | null): string {
  if (!result) return "";
  if (PORTAL_MODE) return result.usage.total_tokens ? `${result.usage.total_tokens} tokens` : "已完成";
  const tools = result.tool_calls.length ? ` · 调用：${result.tool_calls.join(", ")}` : "";
  const totalCost = result.usage.cost?.total || 0;
  const cost = totalCost > 0 ? ` · $${totalCost.toFixed(4)}` : "";
  return `${result.model.name} · ${result.model.provider}/${result.model.model_id} · ${result.usage.total_tokens || 0} tokens${cost}${tools}`;
}

function renderRecoveryMessage(runId: string, message: string): void {
  const container = messageContainer();
  container.querySelector(".chat-empty")?.remove();
  $("#chat-drop-zone").classList.remove("is-empty");
  const card = element("article", "chat-recovery");
  card.dataset.runId = runId;
  const text = element("div");
  text.append(
    element("strong", "", "这次运行可以恢复"),
    element("p", "", message),
    element("small", "", "从已保存的检查点继续，保留已完成的结果；若存在结果未知的外部调用，会先停止恢复并说明原因。"),
  );
  const resume = element("button", "button secondary", "继续运行");
  resume.type = "button";
  resume.addEventListener("click", guarded(async () => {
    resume.disabled = true;
    try {
      const started = await api<StartedRun>(`/v1/runs/${encodeURIComponent(runId)}/resume`, {
        method: "POST",
        body: { actor: "web-console" },
      });
      card.remove();
      state.running = true;
      updateSendButton();
      setChatPetLifecycle("generating");
      const streaming = createStreamingAssistant();
      try {
        const result = await followRun(started.run_id, streaming);
        if (result) {
          finalizeStreamingMessage(streaming, result);
          renderCapabilityWarnings(result.warnings);
        }
      } finally {
        state.running = state.eventSources.size > 0;
        updateSendButton();
        await refreshSessions();
      }
    } finally {
      if (card.isConnected) resume.disabled = false;
    }
  }));
  card.append(text, resume);
  container.append(card);
  scrollChatToLatest();
}

function updateSessionLocation(sessionId: string | null): void {
  const url = new URL(window.location.href);
  if (sessionId) url.searchParams.set("session", sessionId);
  else url.searchParams.delete("session");
  window.history.replaceState({}, "", url);
}

function statusLabel(status: string | null): string {
  return ({
    pending: "等待运行",
    queued: "排队中",
    running: "正在运行",
    waiting_authorization: "等待授权",
    succeeded: "已完成",
    failed: "失败",
    cancelled: "已停止",
    recoverable: "可恢复",
  } as Record<string, string>)[status || ""] || "暂无消息";
}

let portalHistoryExpanded = false;
let portalSearchMatches: ExecutionSession[] | null = null;
let portalSearchPending = false;
let portalSearchSequence = 0;

function renderPortalSessionList(list: Element): void {
  const search = (document.querySelector<HTMLInputElement>("#session-search")?.value || "").trim();
  if (portalSearchPending) {
    list.append(element("p", "session-list-empty", "正在搜索…"));
    return;
  }
  const sessions = search ? (portalSearchMatches || []) : state.sessions;
  if (!sessions.length) {
    list.append(element("p", "session-list-empty", search ? "没有找到相关对话" : "发送第一条消息后，会话会保存在这里。"));
    return;
  }
  const appendGroup = (title: string, items: ExecutionSession[]): void => {
    if (!items.length) return;
    list.append(element("p", "portal-session-group", title));
    for (const session of items) {
      const entry = element("div", "session-entry");
      entry.classList.toggle("active", session.id === state.currentSessionId);
      const open = element("button", "session-item");
      open.type = "button";
      open.classList.toggle("active", session.id === state.currentSessionId);
      open.title = session.title;
      open.setAttribute("aria-label", `打开对话：${session.title}`);
      const heading = element("span", "session-item-heading");
      heading.append(element("strong", "", session.title));
      open.append(heading);
      open.addEventListener("click", guarded(async () => {
        await loadSession(session.id);
        setPortalSessionDrawer(false);
      }));
      const actions = element("div", "portal-session-actions");
      const toggle = element("button", "portal-session-menu-toggle", "⋯");
      toggle.type = "button";
      toggle.title = `对话操作：${session.title}`;
      toggle.setAttribute("aria-label", toggle.title);
      toggle.setAttribute("aria-expanded", "false");
      const menu = element("div", "portal-session-menu");
      menu.setAttribute("role", "menu");
      menu.hidden = true;
      toggle.addEventListener("click", () => {
        const opening = menu.hidden;
        list.querySelectorAll<HTMLElement>(".portal-session-menu").forEach((other) => { other.hidden = true; });
        list.querySelectorAll<HTMLElement>(".portal-session-menu-toggle").forEach((other) => other.setAttribute("aria-expanded", "false"));
        menu.hidden = !opening;
        toggle.setAttribute("aria-expanded", String(opening));
      });
      const action = (label: string, handler: () => Promise<void> | void, danger = false): HTMLButtonElement => {
        const item = element("button", danger ? "danger" : "", label);
        item.type = "button";
        item.setAttribute("role", "menuitem");
        item.addEventListener("click", guarded(async () => { menu.hidden = true; toggle.setAttribute("aria-expanded", "false"); await handler(); }));
        menu.append(item);
        return item;
      };
      action(session.pinned ? "取消置顶" : "置顶", async () => {
        await api(`/v1/sessions/${encodeURIComponent(session.id)}`, { method: "PATCH", body: { pinned: !session.pinned } });
        await refreshSessions();
      });
      action("重命名", () => {
        const input = element("input", "portal-session-rename") as HTMLInputElement;
        input.value = session.title;
        input.maxLength = 80;
        input.setAttribute("aria-label", "新对话名称，按回车保存");
        open.replaceWith(input);
        input.focus();
        input.select();
        input.addEventListener("keydown", guarded(async (event: KeyboardEvent) => {
          if (event.key === "Escape") { renderSessionList(); return; }
          if (event.key !== "Enter") return;
          event.preventDefault();
          const title = input.value.trim();
          if (!title || title === session.title) { renderSessionList(); return; }
          input.disabled = true;
          await api(`/v1/sessions/${encodeURIComponent(session.id)}`, { method: "PATCH", body: { title } });
          await refreshSessions();
        }));
      });
      const remove = action("删除", () => deleteSession(session), true);
      remove.disabled = ["queued", "pending", "running", "waiting_authorization"].includes(session.latest_run_status || "");
      if (remove.disabled) remove.title = "请先停止正在运行的对话";
      actions.append(toggle, menu);
      entry.append(open, actions);
      list.append(entry);
    }
  };
  if (search) { appendGroup("搜索结果", sessions); return; }
  const sorted = [...sessions].sort((a, b) => b.updated_at - a.updated_at);
  const cutoff = Date.now() - 3 * 24 * 60 * 60 * 1000;
  const pinned = sorted.filter((session) => session.pinned);
  const recent = sorted.filter((session) => !session.pinned && session.updated_at >= cutoff);
  const older = sorted.filter((session) => !session.pinned && session.updated_at < cutoff);
  appendGroup("置顶", pinned);
  appendGroup("近 3 天", recent);
  if (older.length) {
    if (portalHistoryExpanded) appendGroup("更早", older);
    const more = element("button", "portal-history-more", portalHistoryExpanded ? "收起较早对话" : `更多对话（${older.length}）`);
    more.type = "button";
    more.addEventListener("click", () => { portalHistoryExpanded = !portalHistoryExpanded; renderSessionList(); });
    list.append(more);
  }
}

function renderSessionList(): void {
  const list = $("#session-list");
  list.replaceChildren();
  if (PORTAL_MODE) { renderPortalSessionList(list); return; }
  if (!state.sessions.length) {
    list.append(element("p", "session-list-empty", "发送第一条消息后，会话会保存在这里。"));
    return;
  }
  const byId = new Map(state.sessions.map((session) => [session.id, session]));
  const children = new Map<string, ExecutionSession[]>();
  const roots: ExecutionSession[] = [];
  for (const session of state.sessions) {
    if (session.parent_id && byId.has(session.parent_id)) {
      const siblings = children.get(session.parent_id) || [];
      siblings.push(session);
      children.set(session.parent_id, siblings);
    } else {
      roots.push(session);
    }
  }
  const visited = new Set<string>();
  const renderSession = (session: ExecutionSession, depth: number): void => {
    if (visited.has(session.id)) return;
    visited.add(session.id);
    const entry = element("div", "session-entry");
    entry.style.setProperty("--session-depth", String(Math.min(depth, 5)));
    entry.classList.toggle("active", session.id === state.currentSessionId);
    entry.classList.toggle("managed-coding", session.managed_coding);
    const button = element("button", "session-item");
    button.type = "button";
    button.classList.toggle("active", session.id === state.currentSessionId);
    button.setAttribute("aria-label", `打开对话：${session.title}`);
    button.title = session.title;
    const heading = element("span", "session-item-heading");
    heading.append(element("strong", "", session.title));
    if (session.managed_coding) heading.append(element("span", "session-kind", "编码"));
    heading.append(element("small", `session-status ${session.latest_run_status || "idle"}`, statusLabel(session.latest_run_status)));
    const context = session.tagline || session.project_name;
    const metadata = [
      context,
      `${session.turn_count} 条消息`,
      new Date(session.updated_at).toLocaleString(),
    ].filter(Boolean).join(" · ");
    button.append(
      heading,
      element("span", "session-item-meta", metadata),
    );
    button.addEventListener("click", guarded(async () => {
      await loadSession(session.id);
      setPortalSessionDrawer(false);
      setManagementSessionDrawer(false);
    }));
    const remove = element("button", "session-delete-button", "删除");
    remove.type = "button";
    remove.title = `删除对话：${session.title}`;
    remove.setAttribute("aria-label", `删除对话：${session.title}`);
    remove.disabled = ["queued", "pending", "running", "waiting_authorization"].includes(session.latest_run_status || "");
    if (remove.disabled) remove.title = "请先停止正在运行的对话";
    remove.addEventListener("click", guarded(async () => deleteSession(session)));
    entry.append(button, remove);
    list.append(entry);
    for (const child of children.get(session.id) || []) renderSession(child, depth + 1);
  }
  for (const session of roots) renderSession(session, 0);
  for (const session of state.sessions) renderSession(session, 0);
}

async function refreshSessions(): Promise<void> {
  state.sessions = await api<ExecutionSession[]>("/v1/sessions");
  if (PORTAL_MODE && document.querySelector<HTMLInputElement>("#session-search")?.value.trim()) {
    await searchPortalSessions();
    return;
  }
  renderSessionList();
}

async function searchPortalSessions(): Promise<void> {
  const query = document.querySelector<HTMLInputElement>("#session-search")?.value.trim() || "";
  const sequence = ++portalSearchSequence;
  if (!query) { portalSearchMatches = null; portalSearchPending = false; renderSessionList(); return; }
  portalSearchPending = true;
  renderSessionList();
  try {
    const matches = await api<ExecutionSession[]>(`/v1/sessions?q=${encodeURIComponent(query)}`);
    if (sequence !== portalSearchSequence) return;
    portalSearchMatches = matches;
  } finally {
    if (sequence === portalSearchSequence) { portalSearchPending = false; renderSessionList(); }
  }
}

async function deleteSession(session: ExecutionSession): Promise<void> {
  if (["queued", "pending", "running", "waiting_authorization"].includes(session.latest_run_status || "")) {
    showToast("请先停止正在运行的对话，再删除记录", true);
    return;
  }
  if (!window.confirm(`删除对话“${session.title}”及其全部消息和运行记录？此操作无法撤销。`)) return;
  await api(`/v1/sessions/${encodeURIComponent(session.id)}`, {
    method: "DELETE",
    body: { actor: "web-console", confirm: true },
  });

  const deletedCurrent = state.currentSessionId === session.id;
  if (deletedCurrent) {
    sessionLoadVersion += 1;
    closeAllRunStreams();
    state.currentSessionId = null;
    state.running = false;
    window.localStorage.removeItem("exoharness.session");
    updateSessionLocation(null);
    resetConversationView();
    $("#conversation-status").textContent = "新会话 · 发送后持久保存";
    updateSendButton();
    setChatPetLifecycle("idle");
  }

  await refreshSessions();
  showToast("对话记录已删除");
}

function resetConversationView(): void {
  virtualHistory?.destroy();
  virtualHistory = null;
  closeFeedbackPopover();
  for (const url of state.objectUrls) URL.revokeObjectURL(url);
  state.objectUrls.clear();
  state.history = [];
  state.pending = [];
  state.followOutput = true;
  renderPendingAttachments();
  $("#chat-messages").replaceChildren(createEmptyState());
  $("#chat-drop-zone").classList.add("is-empty");
  updateScrollLatestButton();
}

async function loadSession(sessionId: string): Promise<void> {
  const version = ++sessionLoadVersion;
  speechInput.cancel();
  closeAllRunStreams();
  state.running = false;
  updateSendButton();
  setChatPetLifecycle("idle");
  const [detail, actions] = await Promise.all([
    api<SessionDetail>(`/v1/sessions/${encodeURIComponent(sessionId)}?turn_limit=${HISTORY_BATCH_SIZE}`),
    api<Record<string, ResponseActionState>>(`/v1/sessions/${encodeURIComponent(sessionId)}/response-actions?turn_limit=${HISTORY_BATCH_SIZE}`),
  ]);
  if (version !== sessionLoadVersion) return;
  state.responseActions = actions;
  state.currentSessionId = sessionId;
  state.currentAgentProfile = detail.agent_profile;
  updateConversationIdentity();
  window.localStorage.setItem("exoharness.session", sessionId);
  updateSessionLocation(sessionId);
  $("#conversation-status").textContent = detail.managed_coding
    ? `${detail.title} · ${detail.project_name || "编码工作流"}`
    : `${detail.title} · 持久会话`;
  resetConversationView();
  for (const notice of detail.notices) {
    const source = notice.sender ? `来自 ${notice.sender}` : "ExoHarness managed workflow";
    renderChatMessage(
      "notice",
      notice.content,
      `${source} · ${new Date(notice.created_at).toLocaleString()}`,
    );
  }
  const turns = detail.turns;
  let page = detail.history_page;
  let legacyStart = page ? 0 : Math.max(0, turns.length - HISTORY_BATCH_SIZE);
  let loaded = turns.slice(legacyStart);
  const recentIds = new Set(loaded.map(turn => turn.id));
  loaded = [...(detail.active_turns || []).filter(turn => !recentIds.has(turn.id)), ...loaded].sort((a, b) => a.ordinal - b.ordinal);
  virtualHistory = new VirtualTurnList<PersistedTurn>($("#chat-messages") as HTMLElement,
    turn => turn.id, turn => {
      const row = element("div");
      const saved = state.history;
      state.history = [];
      historyRenderTarget = row;
      try { renderPersistedTurns([turn]); }
      finally { historyRenderTarget = null; state.history = saved; }
      return row;
    }, turn => ["queued", "pending", "running", "waiting_authorization"].includes(turn.run_status));
  const list = virtualHistory;
  if (page?.has_more || legacyStart > 0) {
    const older = element("button", "chat-history-load");
    older.type = "button";
    older.textContent = "加载更早消息";
    older.addEventListener("click", guarded(async () => {
      if (version !== sessionLoadVersion) return;
      older.disabled = true;
      try {
        let extra: PersistedTurn[];
        if (page) {
          const cursor = page.before_ordinal;
          const [previous, actions] = await Promise.all([
            api<SessionDetail>(`/v1/sessions/${encodeURIComponent(sessionId)}?turn_limit=${HISTORY_BATCH_SIZE}&before_ordinal=${cursor}`),
            api<Record<string, ResponseActionState>>(`/v1/sessions/${encodeURIComponent(sessionId)}/response-actions?turn_limit=${HISTORY_BATCH_SIZE}&before_ordinal=${cursor}`),
          ]);
          if (version !== sessionLoadVersion || list !== virtualHistory) return;
          page = previous.history_page;
          extra = previous.turns;
          Object.assign(state.responseActions, actions);
        } else {
          const next = Math.max(0, legacyStart - HISTORY_BATCH_SIZE);
          extra = turns.slice(next, legacyStart);
          legacyStart = next;
        }
        const seen = new Set(loaded.map(turn => turn.id));
        loaded = [...extra.filter(turn => !seen.has(turn.id)), ...loaded].sort((a, b) => a.ordinal - b.ordinal);
        list.set(loaded, true);
        if (page ? !page.has_more : legacyStart === 0) older.remove();
      } finally { older.disabled = false; }
    }));
    $("#chat-messages").append(older);
  }
  $("#chat-messages").querySelector(".chat-empty")?.remove();
  $("#chat-messages").append(list.root);
  list.set(loaded);
  state.history = loaded.slice(-50).flatMap(turn => [{role: "user", content: turn.prompt}, {role: "assistant", content: turn.answer || ""}]);
  list.scrollToLatest();
  state.followOutput = true;
  updateScrollLatestButton();
  updateSendButton();
  renderSessionList();
}

function renderPersistedTurns(turns: PersistedTurn[], projectionOnly = false): void {
  for (const turn of turns) {
    const attachments = turn.attachments.map(persistedAttachment);
    if (!turn.regenerated_from_run_id) renderChatMessage("user", turn.prompt, "", attachments);
    state.history.push({ role: "user", content: historyText(turn.prompt, attachments) });
    if (turn.run_status === "failed") {
      renderSavedOutputs(turn.response?.artifacts || []);
      renderRunStatus(turn.run_id, "failed", `本次运行失败：${turn.run_error || "运行记录和已产生的事件已经保留。"}`);
    } else if (turn.answer || (turn.run_status === "succeeded" && turn.response)) {
      renderChatMessage(
        "assistant",
        turn.answer || "",
        `${turn.regenerated_from_run_id ? "重新生成 · " : ""}${resultMeta(turn.response)}`,
        [],
        turn.response?.artifacts || [],
        turn.run_status === "succeeded" ? turn.run_id : undefined,
      );
      renderCapabilityWarnings(turn.response?.warnings || []);
      state.history.push({ role: "assistant", content: turn.answer || "" });
    } else if (["queued", "pending", "running", "waiting_authorization"].includes(turn.run_status)) {
      if (projectionOnly) { renderRunStatus(turn.run_id, turn.run_status, "执行中"); continue; }
      state.running = true;
      state.currentRunId = turn.run_id;
      setChatPetLifecycle(turn.run_status === "waiting_authorization" ? "waiting" : "generating");
      const streaming = createStreamingAssistant();
      void followRun(turn.run_id, streaming)
        .then(async (result) => {
          if (!streaming.message.isConnected) return;
          if (result) {
            finalizeStreamingMessage(streaming, result);
            renderCapabilityWarnings(result.warnings);
          }
          await refreshSessions();
        })
        .catch((error) => {
          if (!streaming.message.isConnected) return;
          streaming.message.remove();
          renderRunStatus(turn.run_id, "failed", error instanceof Error ? error.message : String(error));
          setChatPetLifecycle("failed");
        })
        .finally(() => {
          state.running = state.eventSources.size > 0;
          updateSendButton();
        });
    } else if (turn.run_status === "recoverable") {
      if (!projectionOnly) setChatPetLifecycle("waiting");
      renderRecoveryMessage(
        turn.run_id,
        turn.run_error || "服务在本次运行完成前中断；已保存的事件仍然可读。",
      );
    } else if (turn.run_status === "failed") {
      renderRunStatus(
        turn.run_id, "failed",
        `本次运行失败：${turn.run_error || "运行记录和已产生的事件已经保留。"}`,
      );
    } else if (turn.run_status === "cancelled") {
      renderRunStatus(turn.run_id, "cancelled", "本次运行已停止。");
    }
  }
  state.history = state.history.slice(-100);
}

function createEmptyState(): HTMLElement {
  const empty = element("div", "chat-empty");
  const mark = element("span", "empty-mark");
  mark.setAttribute("aria-hidden", "true");
  const markInner = element("span");
  renderAgentAvatar(markInner, state.currentAgentProfile);
  mark.append(markInner);
  const kicker = element("p", "chat-empty-kicker", state.currentAgentProfile?.display_name || "EXOHARNESS AGENT");
  const heading = element("strong", "", PORTAL_MODE ? "今天想一起做什么？" : "有什么可以帮你？");
  const copy = element("p", "chat-empty-copy", PORTAL_MODE
    ? "制作宣传视频、设计海报、创作音乐，或撰写文档与 PPT——告诉我你的主题和要求。"
    : speechInputHint());
  if (PORTAL_MODE) copy.dataset.staticCopy = "true";
  empty.append(mark, kicker, heading, copy);
  return empty;
}

async function startNewConversation(): Promise<void> {
  sessionLoadVersion += 1;
  speechInput.cancel();
  const sessionSearch = document.querySelector<HTMLInputElement>("#session-search");
  if (sessionSearch) sessionSearch.value = "";
  portalSearchSequence += 1;
  portalSearchMatches = null;
  portalSearchPending = false;
  setPortalSessionDrawer(false);
  setManagementSessionDrawer(false);
  closeAllRunStreams();
  state.currentSessionId = null;
  renderSessionList();
  state.currentAgentProfile = state.defaultAgentProfile;
  updateConversationIdentity();
  state.running = false;
  window.localStorage.removeItem("exoharness.session");
  updateSessionLocation(null);
  resetConversationView();
  setChatPetLifecycle("idle");
  $("#conversation-status").textContent = "新会话 · 发送后持久保存";
  const prompt = $("#chat-prompt") as HTMLTextAreaElement;
  prompt.value = "";
  resizeComposer();
  updateSendButton();
  renderSessionList();
  await refreshSessions();
  prompt.focus();
}

async function stopCurrentRun(): Promise<void> {
  const runId = state.currentRunId;
  if (!runId || state.cancellingRuns.has(runId)) return;
  state.cancellingRuns.add(runId);
  updateSendButton();
  try {
    await api(`/v1/runs/${encodeURIComponent(runId)}/cancel`, {
      method: "POST",
      body: { actor: "web-console" },
    });
    showToast("已发送停止请求，正在等待运行安全结束");
  } catch (error) {
    state.cancellingRuns.delete(runId);
    throw error;
  } finally {
    updateSendButton();
  }
}

function resizeComposer(): void {
  const textarea = $("#chat-prompt") as HTMLTextAreaElement;
  textarea.style.height = "auto";
  textarea.style.height = `${Math.min(textarea.scrollHeight, 180)}px`;
}

function updateSendButton(): void {
  const button = $("#chat-send") as HTMLButtonElement;
  const clear = $("#clear-chat") as HTMLButtonElement;
  const newSession = $("#new-session") as HTMLButtonElement;
  const stop = $("#chat-stop") as HTMLButtonElement;
  const prompt = $("#chat-prompt") as HTMLTextAreaElement;
  button.disabled = speechInput.busy() || (!PORTAL_MODE && state.running)
    || (!prompt.value.trim() && state.pending.length === 0);
  clear.disabled = false;
  newSession.disabled = false;
  stop.classList.toggle("hidden", !state.running);
  const cancelling = !!state.currentRunId && state.cancellingRuns.has(state.currentRunId);
  stop.disabled = !state.currentRunId || cancelling;
  stop.title = cancelling ? "正在等待运行停止" : "停止当前运行";
  const stopLabel = stop.querySelector("span:last-child");
  if (stopLabel) stopLabel.textContent = cancelling ? "正在停止…" : "停止";
  button.classList.remove("is-loading");
  const icon = button.querySelector("span");
  if (icon) icon.textContent = "↑";
  const sendLabel = PORTAL_MODE && state.running ? "发送消息并加入队列" : "发送消息";
  button.ariaLabel = sendLabel;
  button.title = sendLabel;
}

async function bootstrapSessions(): Promise<void> {
  await refreshSessions();
  const requested = new URL(window.location.href).searchParams.get("session");
  const sessionId = requested && state.sessions.some((session) => session.id === requested)
    ? requested
    : null;
  if (sessionId) await loadSession(sessionId);
  else {
    window.localStorage.removeItem("exoharness.session");
    updateSessionLocation(null);
    $("#conversation-status").textContent = "新会话 · 发送后持久保存";
  }
}

async function loadDefaultAgentProfile(): Promise<void> {
  state.defaultAgentProfile = await api<PublicAgentProfile | null>("/v1/agent-profile");
  if (!state.currentSessionId) {
    state.currentAgentProfile = state.defaultAgentProfile;
    updateConversationIdentity();
    resetConversationView();
  }
}

function renderEngineeringHint(): void {
  const hint=document.querySelector<HTMLElement>("#portal-engineering-hint");
  if(hint){hint.hidden=!state.currentAgentProfile?.software_engineering;hint.title="可以直接要求助理创建或打开项目、修改代码、运行测试和查看差异。项目与账号绑定；不会开放服务器文件或网络。";}
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`;
}

let toastTimer: number | null = null;
function showToast(message: string, error = false): void {
  const toast = $("#toast");
  toast.textContent = message;
  toast.classList.toggle("error", error);
  toast.classList.add("visible");
  if (toastTimer !== null) window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => toast.classList.remove("visible"), 5000);
}

function guarded<T extends Event>(handler: (event: T) => Promise<void> | void): (event: T) => Promise<void> {
  return async (event: T) => {
    try { await handler(event); } catch (error) { showToast(error instanceof Error ? error.message : String(error), true); }
  };
}

const attachmentInput = $("#chat-attachments") as HTMLInputElement;
attachmentInput.addEventListener("change", guarded(async () => {
  await addFiles(Array.from(attachmentInput.files || []));
  attachmentInput.value = "";
}));

const promptInput = $("#chat-prompt") as HTMLTextAreaElement;
const speechInput = mountSpeechInput({ input: promptInput, changed: () => { resizeComposer(); updateSendButton(); }, notify: showToast });
void speechInput.initialize();
promptInput.addEventListener("input", () => {
  resizeComposer();
  updateSendButton();
});
promptInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey && !event.isComposing) {
    event.preventDefault();
    ($("#chat-form") as HTMLFormElement).requestSubmit();
  }
});
promptInput.addEventListener("paste", guarded(async (event: ClipboardEvent) => {
  const files = Array.from(event.clipboardData?.files || []);
  if (files.length) await addFiles(files);
}));

const dropZone = $("#chat-drop-zone") as HTMLElement;
const dropOverlay = $("#drop-overlay") as HTMLElement;
for (const eventName of ["dragenter", "dragover"]) {
  dropZone.addEventListener(eventName, (event) => {
    event.preventDefault();
    dropOverlay.classList.remove("hidden");
  });
}
dropZone.addEventListener("dragleave", (event) => {
  if (!dropZone.contains(event.relatedTarget as Node | null)) dropOverlay.classList.add("hidden");
});
dropZone.addEventListener("drop", guarded(async (event: DragEvent) => {
  event.preventDefault();
  dropOverlay.classList.add("hidden");
  await addFiles(Array.from(event.dataTransfer?.files || []));
}));

$("#chat-form").addEventListener("submit", guarded((event) => runChat(event as SubmitEvent)));
$("#clear-chat").addEventListener("click", guarded(() => startNewConversation()));
$("#new-session").addEventListener("click", guarded(() => startNewConversation()));
const sessionSearch = document.querySelector<HTMLInputElement>("#session-search");
let sessionSearchTimer: number | null = null;
sessionSearch?.addEventListener("input", () => {
  if (sessionSearchTimer !== null) window.clearTimeout(sessionSearchTimer);
  sessionSearchTimer = window.setTimeout(() => { void searchPortalSessions().catch((error) => showToast(error instanceof Error ? error.message : String(error), true)); }, 220);
});
document.addEventListener("click", (event) => {
  if (event.target instanceof Element && event.target.closest(".portal-session-actions")) return;
  document.querySelectorAll<HTMLElement>(".portal-session-menu").forEach((menu) => { menu.hidden = true; });
  document.querySelectorAll<HTMLElement>(".portal-session-menu-toggle").forEach((toggle) => toggle.setAttribute("aria-expanded", "false"));
});
$("#chat-stop").addEventListener("click", guarded(() => stopCurrentRun()));
document.querySelector<HTMLButtonElement>("#chat-history-toggle")?.addEventListener("click", () => {
  setManagementSessionDrawer(!document.body.classList.contains("chat-session-drawer-open"));
});
document.querySelector<HTMLButtonElement>("#chat-history-close")?.addEventListener("click", () => {
  setManagementSessionDrawer(false);
  document.querySelector<HTMLButtonElement>("#chat-history-toggle")?.focus();
});
document.querySelector<HTMLButtonElement>("#chat-session-scrim")?.addEventListener("click", () => {
  setManagementSessionDrawer(false);
});
document.querySelector<HTMLButtonElement>("#portal-history-toggle")?.addEventListener("click", () => {
  setPortalSessionDrawer(!document.body.classList.contains("session-drawer-open"));
});
document.querySelector<HTMLButtonElement>("#portal-sidebar-collapse")?.addEventListener("click", () => {
  setPortalSidebarCollapsed(!document.body.classList.contains("portal-sidebar-collapsed"));
});
document.querySelector<HTMLButtonElement>("#portal-session-scrim")?.addEventListener("click", () => {
  setPortalSessionDrawer(false);
});
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    setPortalSessionDrawer(false);
    setManagementSessionDrawer(false);
  }
});
window.addEventListener("exoharness:mobile-menu-open", () => setManagementSessionDrawer(false));
window.matchMedia("(min-width: 801px)").addEventListener("change", (event) => {
  if (event.matches) setPortalSessionDrawer(false);
});
window.matchMedia("(min-width: 721px)").addEventListener("change", () => {
  setManagementSessionDrawer(false);
});
const chatMessages = $("#chat-messages") as HTMLElement;
mountDeliveryWorkbench(() => {
  const list = virtualHistory;
  if (!list) return [];
  return list.recent(50).map(({item, node}) => {
    if (!node) {
      node = element("div");
      const saved = state.history;
      historyRenderTarget = node;
      state.history = [];
      try { renderPersistedTurns([item], true); }
      finally { historyRenderTarget = null; state.history = saved; }
    }
    return {node, resolve: () => { list.reveal(item.id); return list.mounted(item.id); }};
  });
});
chatMessages.addEventListener("scroll", () => {
  state.followOutput = isNearLatest(chatMessages);
  updateScrollLatestButton();
}, { passive: true });
$("#scroll-latest").addEventListener("click", () => scrollChatToLatest(true));
window.addEventListener("beforeunload", () => {
  closeAllRunStreams();
  for (const url of state.objectUrls) URL.revokeObjectURL(url);
});

mountChatPet();
mountPortalSettings();
void checkService();
void loadCatalog().catch((error) => showToast(error instanceof Error ? error.message : String(error), true));
void loadDefaultAgentProfile()
  .then(() => bootstrapSessions())
  .catch((error) => showToast(error instanceof Error ? error.message : String(error), true));
setManagementSessionDrawer(false);
if (PORTAL_MODE) setPortalSidebarCollapsed(window.localStorage.getItem("exoharness.portal-sidebar-collapsed") === "true");
resizeComposer();
updateSendButton();
