// Dictation stays outside the Agent conversation until the user presses Send.
import { mountRecordingWaveform } from "./recording-waveform";
export function speechInputHint(): string {
  return document.querySelector<HTMLElement>("#chat-microphone")?.dataset.availability === "available"
    ? "输入文字、使用语音，或添加参考文件。"
    : "描述你的需求，或添加参考文件。";
}

export function mountSpeechInput(options: {
  input: HTMLTextAreaElement;
  changed: () => void;
  notify: (message: string, error?: boolean) => void;
}) {
  const button = document.querySelector<HTMLButtonElement>("#chat-microphone");
  const panel = document.querySelector<HTMLElement>("#speech-input-status");
  const label = panel?.querySelector<HTMLElement>("[data-speech-label]");
  const cancelButton = panel?.querySelector<HTMLButtonElement>("button");
  const waveform = mountRecordingWaveform(panel);
  let phase: "idle" | "permission" | "recording" | "transcribing" = "idle";
  let recorder: MediaRecorder | null = null;
  let stream: MediaStream | null = null;
  let request: AbortController | null = null;
  let timer: number | null = null;
  let generation = 0;
  let maxSeconds = 120;
  let maxBytes = 8 * 1024 * 1024;
  let availability = { available: false, message: "正在检测语音服务…" };
  let checking = true;
  let permission: PermissionStatus | null = null;
  let deviceError = "";
  let refreshTimer: number | null = null;
  let refreshRequest: Promise<void> | null = null;
  let statusController: AbortController | null = null;
  let active = true;
  let initialized = false;
  const authFetch: typeof fetch = (...args) => (window as unknown as { ExoAuth: { fetch: typeof fetch } }).ExoAuth.fetch(...args);

  function unavailableReason(): string {
    if (!window.isSecureContext) return "语音输入需要通过 HTTPS 域名访问";
    if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") return "当前浏览器不支持录音";
    if (!navigator.onLine) return "网络已断开，语音输入暂不可用";
    if (permission?.state === "denied") return "麦克风权限未开启，请在浏览器设置中允许使用麦克风";
    return deviceError || (!availability.available ? availability.message : "");
  }
  function renderButton() {
    if (!button) return;
    const reason = unavailableReason();
    button.hidden = false;
    button.dataset.availability = reason ? (checking ? "checking" : "unavailable") : "available";
    button.disabled = phase === "permission" || phase === "transcribing";
    button.setAttribute("aria-disabled", String(button.disabled || (!!reason && phase !== "recording")));
    button.classList.toggle("is-recording", phase === "recording");
    button.setAttribute("aria-pressed", String(phase === "recording"));
    button.title = phase === "recording" ? "结束录音并转文字"
      : phase === "transcribing" ? "正在转成文字…" : reason || "语音输入可用，点击开始录音";
    button.setAttribute("aria-label", button.title);
    document.querySelectorAll<HTMLElement>(".chat-empty-copy:not([data-static-copy])").forEach(copy => {
      const text = speechInputHint();
      if (copy.textContent !== text) copy.textContent = text;
    });
  }
  function setPhase(value: typeof phase, message = "") {
    phase = value;
    if (panel) { panel.hidden = value === "idle"; panel.dataset.phase = value; }
    if (label) label.textContent = message;
    renderButton();
    options.changed();
  }
  function releaseMic() {
    waveform.stop();
    if (timer !== null) window.clearInterval(timer);
    timer = null;
    stream?.getTracks().forEach(track => track.stop());
    stream = null;
  }
  function cancel() {
    generation++;
    request?.abort(); request = null;
    if (recorder && recorder.state !== "inactive") recorder.stop();
    recorder = null;
    releaseMic();
    setPhase("idle");
  }
  async function transcribe(blob: Blob, current: number) {
    if (current !== generation) return;
    releaseMic();
    if (!blob.size) { cancel(); options.notify("没有录到声音，请重新录制", true); return; }
    setPhase("transcribing", "正在转成文字…");
    const controller = new AbortController(); request = controller;
    const timeout = window.setTimeout(() => controller.abort(), 125000);
    try {
      const response = await authFetch("/v1/speech-input", {
        method: "POST", headers: { "content-type": blob.type }, body: blob, signal: controller.signal,
      });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error?.message || "语音识别暂时不可用，请重试");
      if (current !== generation) return;
      if (typeof result.text !== "string") throw new Error("语音识别返回格式错误");
      if (!result.text.trim()) options.notify("未识别到语音，请靠近麦克风再试一次");
      else {
        // Read the CURRENT draft, so typing while transcribing is never overwritten.
        const original = options.input.value;
        options.input.value = original + (original && !/\s$/.test(original) ? " " : "") + result.text.trim();
        options.input.dispatchEvent(new Event("input", { bubbles: true }));
        options.input.focus();
        options.notify("已转成文字，确认后点击发送");
      }
    } catch (error) {
      if (current === generation) {
        availability = { available: false, message: "语音识别暂不可用，正在重新检测" };
        renderButton();
        void refreshAvailability();
      }
      if (current === generation) options.notify(error instanceof DOMException && error.name === "AbortError"
        ? "语音识别超时，已有文字已保留，请重试" : error instanceof Error ? error.message : "语音识别失败", true);
    } finally {
      window.clearTimeout(timeout);
      if (current === generation) { request = null; recorder = null; setPhase("idle"); }
    }
  }
  async function start() {
    if (phase === "recording") { if (recorder?.state === "recording") recorder.stop(); releaseMic(); return; }
    if (phase !== "idle") return;
    const reason = unavailableReason();
    if (reason) {
      options.notify(reason, true);
      deviceError = "";
      void refreshAvailability();
      return;
    }
    const current = ++generation;
    setPhase("permission", "请允许使用麦克风…");
    try {
      const acquired = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true }, video: false });
      if (current !== generation) { acquired.getTracks().forEach(track => track.stop()); return; }
      stream = acquired;
      const mime = ["audio/webm;codecs=opus", "audio/mp4", "audio/ogg;codecs=opus", "audio/webm"]
        .find(type => MediaRecorder.isTypeSupported(type));
      if (!mime) throw new Error("当前浏览器的录音格式暂不支持");
      const activeRecorder = new MediaRecorder(acquired, { mimeType: mime, audioBitsPerSecond: 64000 });
      recorder = activeRecorder;
      const chunks: Blob[] = []; let bytes = 0;
      activeRecorder.ondataavailable = event => {
        if (current !== generation) return;
        bytes += event.data.size;
        if (bytes > maxBytes) { cancel(); options.notify("录音文件过大，请缩短录音后重试", true); return; }
        if (event.data.size) chunks.push(event.data);
      };
      activeRecorder.onstop = () => { if (current === generation) void transcribe(new Blob(chunks, { type: activeRecorder.mimeType || mime }), current); };
      activeRecorder.onerror = () => { if (current === generation) { cancel(); options.notify("录音中断，请检查麦克风后重试", true); } };
      acquired.getAudioTracks().forEach(track => track.addEventListener("ended", () => {
        if (current === generation && phase === "recording") { cancel(); options.notify("麦克风连接已断开", true); }
      }));
      activeRecorder.start(1000);
      const began = Date.now();
      setPhase("recording", "正在录音 · 0 秒");
      waveform.start(acquired);
      timer = window.setInterval(() => {
        const seconds = Math.floor((Date.now() - began) / 1000);
        if (seconds >= maxSeconds) { activeRecorder.stop(); releaseMic(); return; }
        if (label) label.textContent = `正在录音 · ${seconds} 秒`;
      }, 250);
    } catch (error) {
      if (current !== generation) return;
      if (error instanceof DOMException && error.name === "NotAllowedError") deviceError = "麦克风权限未开启，请在浏览器的网站设置中允许使用麦克风";
      else if (error instanceof DOMException && error.name === "NotFoundError") deviceError = "没有找到可用的麦克风";
      cancel();
      options.notify(error instanceof DOMException && error.name === "NotAllowedError"
        ? "麦克风权限未开启，请在浏览器的网站设置中允许使用麦克风"
        : error instanceof DOMException && error.name === "NotFoundError" ? "没有找到可用的麦克风"
        : error instanceof Error ? error.message : "无法启动录音", true);
    }
  }
  button?.addEventListener("click", () => void start());
  cancelButton?.addEventListener("click", cancel);
  window.addEventListener("pagehide", () => {
    active = false; cancel(); statusController?.abort();
    if (refreshTimer !== null) window.clearTimeout(refreshTimer);
  });
  window.addEventListener("pageshow", () => { active = true; void refreshAvailability(); });
  window.addEventListener("focus", () => { deviceError = ""; void refreshAvailability(); });
  window.addEventListener("online", () => void refreshAvailability());
  window.addEventListener("offline", renderButton);
  document.addEventListener("visibilitychange", () => { if (!document.hidden) void refreshAvailability(); });
  document.addEventListener("keydown", event => { if (event.key === "Escape" && phase !== "idle") cancel(); });
  function refreshAvailability(): Promise<void> {
    if (!button || !active || !initialized) return Promise.resolve();
    if (refreshRequest) return refreshRequest;
    refreshRequest = (async () => {
      if (refreshTimer !== null) window.clearTimeout(refreshTimer);
      if (!document.hidden && navigator.onLine) {
        const controller = new AbortController(); statusController = controller;
        const timeout = window.setTimeout(() => controller.abort(), 5000);
        try {
          const response = await authFetch("/v1/speech-input/configuration", { signal: controller.signal });
          if (!response.ok) throw new Error("status unavailable");
          const configuration = await response.json();
          if (!active) return;
          availability = { available: configuration.enabled === true && configuration.available === true,
            message: configuration.message || "语音输入暂不可用，请稍后重试" };
          maxSeconds = Math.min(120, configuration.max_recording_seconds || 120);
          maxBytes = Math.min(8 * 1024 * 1024, configuration.max_audio_bytes || 8 * 1024 * 1024);
        } catch {
          if (active) availability = { available: false, message: "无法连接语音服务，请稍后重试" };
        } finally {
          window.clearTimeout(timeout); statusController = null;
        }
      }
    })().finally(() => {
      refreshRequest = null; checking = false; renderButton();
      if (active) refreshTimer = window.setTimeout(() => void refreshAvailability(), 15000);
    });
    return refreshRequest;
  }
  async function initialize() {
    if (!button || initialized) return;
    initialized = true; renderButton();
    void navigator.permissions?.query({ name: "microphone" as PermissionName }).then(result => {
      permission = result;
      permission.onchange = () => { deviceError = ""; renderButton(); };
      renderButton();
    }).catch(() => { /* Not all browsers implement microphone permission queries. */ });
    await refreshAvailability();
  }
  return { initialize, cancel, busy: () => phase !== "idle" };
}
