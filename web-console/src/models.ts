type ModelType = "llm" | "tts" | "asr";
type ModelApi = "openai_compatible" | "anthropic" | "volcengine_tts_sse" | "aliyun_qwen_tts_http" | "aliyun_qwen_audio_tts_websocket" | "openai_transcription";

interface ModelProvider {
  name: string;
  model_type: ModelType;
  api: ModelApi;
  output_token_parameter?: "max_tokens" | "max_completion_tokens" | "omit" | null;
  base_url: string;
  secret_env: string | null;
  api_key_configured: boolean;
  enabled: boolean;
}

interface TtsModelSettings {
  profile_id: string;
  speaker: string;
  language_type?: string | null;
  audio_format?: string | null;
  sample_rate?: number | null;
}

interface ConfiguredModel {
  name: string;
  model_id: string;
  display_name: string;
  provider: string;
  model_type: ModelType;
  context_window: number;
  max_tokens: number;
  thinking: "none" | "openai" | "anthropic" | "qwen";
  temperature?: number | null;
  top_p?: number | null;
  top_k?: number | null;
  tts?: TtsModelSettings | null;
  enabled: boolean;
}

interface ModelCatalog {
  providers: ModelProvider[];
  models: ConfiguredModel[];
  default_model: string | null;
  default_models?: Partial<Record<ModelType, string>>;
}

interface ApiOptions extends Omit<RequestInit, "body"> { body?: unknown; }

const state: {
  catalog: ModelCatalog;
  selectedModel: string | null;
} = {
  catalog: { providers: [], models: [], default_model: null, default_models: {} },
  selectedModel: null,
};

const $ = <T extends Element = HTMLElement>(selector: string): T => {
  const node = document.querySelector<T>(selector);
  if (!node) throw new Error(`Missing UI element: ${selector}`);
  return node;
};

async function api<T = unknown>(path: string, options: ApiOptions = {}): Promise<T> {
  const headers = new Headers(options.headers);
  let body: BodyInit | undefined;
  if (options.body !== undefined) {
    headers.set("content-type", "application/json");
    body = JSON.stringify(options.body);
  }
  const response = await (window as unknown as { ExoAuth: { fetch: typeof fetch } }).ExoAuth.fetch(
    path,
    { ...options, headers, body },
  );
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

function element<K extends keyof HTMLElementTagNameMap>(tag: K, className = "", text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function selectedType(): ModelType {
  return $<HTMLSelectElement>("#model-type").value as ModelType;
}

function typeLabel(modelType: ModelType): string {
  return { llm: "LLM", tts: "TTS", asr: "ASR" }[modelType];
}

function providerFor(modelType: ModelType): ModelProvider | null {
  return state.catalog.providers.find((provider) => (provider.model_type || "llm") === modelType) || null;
}

function providerNamed(name: string): ModelProvider | null {
  return state.catalog.providers.find((provider) => provider.name === name) || null;
}

function selectedProvider(modelType: ModelType): ModelProvider | null {
  const model = state.catalog.models.find((entry) => entry.name === state.selectedModel);
  return model?.model_type === modelType ? providerNamed(model.provider) : providerFor(modelType);
}

function updateProviderKeyField(): void {
  const provider = providerNamed($<HTMLInputElement>("#provider-name").value.trim());
  const keyInput = $<HTMLInputElement>("#provider-api-key");
  keyInput.required = !provider?.api_key_configured;
  keyInput.placeholder = provider?.api_key_configured ? "已配置；留空表示不修改" : "请输入 API Key";
  $("#provider-api-key-help").textContent = provider?.api_key_configured
    ? "API Key 已配置；页面和 API 均不会回显"
    : "必填；保存后进入本机 Secret Store，不写入模型目录";
}

function defaultNameFor(modelType: ModelType): string | null {
  return state.catalog.default_models?.[modelType]
    || (modelType === "llm" ? state.catalog.default_model : null)
    || null;
}

function effectiveModelEnabled(model: ConfiguredModel): boolean {
  return model.enabled && providerNamed(model.provider)?.enabled === true;
}

function actionButton(text: string, action: string, name: string, style = "ghost"): HTMLButtonElement {
  const button = element("button", `button ${style}`, text);
  button.type = "button";
  button.dataset.action = action;
  button.dataset.name = name;
  return button;
}

function configureOutputTokenField(): void {
  const applicable = selectedType() === "llm" && $<HTMLSelectElement>("#provider-api").value === "openai_compatible";
  $("#provider-output-token-field").toggleAttribute("hidden", !applicable);
  $<HTMLSelectElement>("#provider-output-token-parameter").disabled = !applicable;
}

function configureTtsProtocolFields(applyDefaults = false): void {
  if (selectedType() !== "tts") return;
  const api = $<HTMLSelectElement>("#provider-api").value as ModelApi;
  const aliyunHttp = api === "aliyun_qwen_tts_http";
  const aliyunRealtime = api === "aliyun_qwen_audio_tts_websocket";
  $("#tts-language-field").toggleAttribute("hidden", !aliyunHttp);
  $("#tts-stream-fields").toggleAttribute("hidden", !aliyunRealtime);
  $<HTMLSelectElement>("#tts-language-type").disabled = !aliyunHttp;
  $<HTMLSelectElement>("#tts-audio-format").disabled = !aliyunRealtime;
  $<HTMLInputElement>("#tts-sample-rate").disabled = !aliyunRealtime;
  $("#tts-speaker-label").textContent = aliyunHttp || aliyunRealtime ? "音色（voice）" : "音色（Speaker）";
  $("#tts-speaker-help").textContent = aliyunHttp
    ? "例如 Cherry；将作为 input.voice 发送"
    : aliyunRealtime ? "例如 longanhuan_v3.6；将作为 parameters.voice 发送" : "火山引擎音色标识";
  $("#connection-section-copy").textContent = aliyunHttp
    ? "连接阿里云千问 TTS 非实时 HTTP；整段文本提交后下载并持久化最终音频。"
    : aliyunRealtime ? "连接阿里云 Qwen-Audio TTS WebSocket；实时接收二进制音频。" : "连接火山引擎 TTS SSE 服务；API Key 仅写入本机密钥存储。";
  $("#model-section-copy").textContent = aliyunHttp
    ? "模型、音色与语言均可配置；language_type 会随每次合成发送。"
    : aliyunRealtime ? "模型、音色、格式与采样率均可配置；音频通过 WebSocket 实时接收。" : "Resource ID 选择语音模型，Profile ID 标识声音角色，Speaker 指定实际音色。";
  $("#model-id-label").textContent = aliyunHttp || aliyunRealtime ? "模型名称" : "Resource ID";
  $("#model-id-help").textContent = aliyunHttp
    ? "例如 qwen3-tts-flash"
    : aliyunRealtime ? "例如 qwen-audio-3.0-tts-flash" : "例如 seed-tts-2.0";
  $("#provider-base-url-help").textContent = aliyunHttp
    ? "阿里云百炼多模态生成非实时 HTTP 地址"
    : aliyunRealtime ? "阿里云百炼实时语音合成 WebSocket 地址（wss://）" : "火山引擎 V3 Server-Sent Events 单向流式地址";
  if (!applyDefaults) return;
  if (aliyunHttp) {
    $<HTMLInputElement>("#provider-base-url").value = "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
    $<HTMLInputElement>("#model-display-name").value = "qwen3-tts-flash";
    $<HTMLInputElement>("#tts-profile-id").value = "aliyun-qwen3-voice";
    $<HTMLInputElement>("#tts-speaker").value = "Cherry";
    $<HTMLSelectElement>("#tts-language-type").value = "English";
  } else if (aliyunRealtime) {
    $<HTMLInputElement>("#provider-base-url").value = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";
    $<HTMLInputElement>("#model-display-name").value = "qwen-audio-3.0-tts-flash";
    $<HTMLInputElement>("#tts-profile-id").value = "aliyun-qwen-audio-voice";
    $<HTMLInputElement>("#tts-speaker").value = "longanhuan_v3.6";
    $<HTMLSelectElement>("#tts-audio-format").value = "mp3";
    $<HTMLInputElement>("#tts-sample-rate").value = "22050";
  } else {
    $<HTMLInputElement>("#provider-base-url").value = "https://openspeech.bytedance.com/api/v3/tts/unidirectional/sse";
    $<HTMLInputElement>("#model-display-name").value = "seed-tts-2.0";
    $<HTMLInputElement>("#tts-profile-id").value = "persona-nova";
    $<HTMLInputElement>("#tts-speaker").value = "zh_female_vv_uranus_bigtts";
  }
}

function configureTypeFields(modelType: ModelType, hydrateProvider = true): void {
  const tts = modelType === "tts";
  const asr = modelType === "asr";
  const typeSelect = $<HTMLSelectElement>("#model-type");
  typeSelect.value = modelType;
  typeSelect.disabled = state.selectedModel !== null;
  const apiSelect = $<HTMLSelectElement>("#provider-api");
  for (const option of apiSelect.options) {
    option.hidden = option.dataset.modelType !== modelType;
    option.disabled = option.hidden;
  }
  $("#tts-model-fields").toggleAttribute("hidden", !tts);
  $("#llm-advanced-fields").toggleAttribute("hidden", modelType !== "llm");
  $("#llm-advanced-fields").querySelectorAll<HTMLInputElement | HTMLSelectElement>("input, select").forEach(input => { input.disabled = modelType !== "llm"; });
  $<HTMLInputElement>("#tts-profile-id").required = tts;
  $<HTMLInputElement>("#tts-speaker").required = tts;
  $("#connection-section-copy").textContent = tts
    ? "连接 TTS 服务；API Key 仅写入本机密钥存储。"
    : asr ? "连接语音转文字服务，录音识别不经过大语言模型。" : "连接 LLM 服务所需的信息。";
  $("#model-section-copy").textContent = tts
    ? "模型、声音角色与音色由所选 TTS 协议决定。"
    : "配置名称用于区分保存的配置；模型名称会直接发送给 API 服务。";
  $("#model-id-label").textContent = tts ? "Resource ID" : "模型名称";
  $("#model-id-help").textContent = tts ? "例如 seed-tts-2.0" : "填写模型服务提供的模型名称";
  $("#model-enabled-copy").textContent = `启用这个 ${typeLabel(modelType)}；保存后自动停用同类型的其他配置，不影响其他类型`;
  if (!hydrateProvider) return;

  const provider = selectedProvider(modelType);
  const providerName = $<HTMLInputElement>("#provider-name");
  providerName.value = provider?.name || (tts ? "volcengine-tts" : asr ? "speech-api" : "default-api");
  providerName.readOnly = state.selectedModel !== null;
  const names = $<HTMLDataListElement>("#provider-names");
  names.replaceChildren(...state.catalog.providers.filter((entry) => entry.model_type === modelType).map((entry) => {
    const option = document.createElement("option");
    option.value = entry.name;
    return option;
  }));
  apiSelect.value = provider?.api || (tts ? "volcengine_tts_sse" : asr ? "openai_transcription" : "openai_compatible");
  $<HTMLSelectElement>("#provider-output-token-parameter").value = provider?.output_token_parameter || "";
  configureOutputTokenField();
  $<HTMLInputElement>("#provider-base-url").value = provider?.base_url || (tts
    ? "https://openspeech.bytedance.com/api/v3/tts/unidirectional/sse"
    : asr ? "http://127.0.0.1:8796/v1" : "https://api.openai.com/v1");
  $("#provider-base-url-help").textContent = tts
    ? "火山引擎 V3 Server-Sent Events 单向流式地址"
    : asr ? "填写 API 根地址（含 /v1），系统通过 /audio/transcriptions 上传录音" : "例如 https://api.openai.com/v1";
  $<HTMLInputElement>("#provider-api-key").value = "";
  updateProviderKeyField();
  if (tts) configureTtsProtocolFields(false);
}

function renderModels(): void {
  const list = $("#model-list");
  list.replaceChildren();
  $("#model-count").textContent = `${state.catalog.models.length} 个模型`;
  const activeLlm = state.catalog.models.find((model) => model.name === defaultNameFor("llm"));
  const activeTts = state.catalog.models.find((model) => model.name === defaultNameFor("tts"));
  const activeAsr = state.catalog.models.find((model) => model.name === defaultNameFor("asr"));
  $("#default-model-summary").textContent = `LLM ${activeLlm?.display_name || "—"} · TTS ${activeTts?.tts?.profile_id || activeTts?.display_name || "—"} · ASR ${activeAsr?.display_name || "—"}`;

  if (!state.catalog.models.length) {
    const empty = element("div", "settings-empty");
    empty.append(element("strong", "", "还没有配置模型"), element("span", "", "选择 LLM、TTS 或 ASR，填写左侧配置并保存。"));
    list.append(empty);
    return;
  }

  for (const modelType of ["llm", "tts", "asr"] as ModelType[]) {
    const models = state.catalog.models.filter((model) => (model.model_type || "llm") === modelType);
    if (!models.length) continue;
    const group = element("section", "model-type-group");
    const groupHeading = element("div", "model-type-group-heading");
    groupHeading.append(element("strong", "", typeLabel(modelType)), element("span", "", `${models.length} 个配置`));
    group.append(groupHeading);
    for (const model of models) {
      const current = defaultNameFor(modelType) === model.name && effectiveModelEnabled(model);
      const selected = state.selectedModel === model.name;
      const item = element("div", `model-item selectable${current ? " default-model" : ""}${selected ? " selected" : ""}`);
      item.dataset.name = model.name;
      item.tabIndex = 0;
      item.setAttribute("role", "button");
      const heading = element("div", "model-item-heading");
      const title = element("div", "model-item-title");
      title.append(element("span", "model-type-chip", typeLabel(modelType)), element("strong", "", model.display_name));
      heading.append(title, element("span", current ? "state-enabled" : "state-disabled", current ? (modelType === "asr" ? "已启用 · 语音输入" : "已启用 · 对话可用") : "未启用"));
      const provider = providerNamed(model.provider);
      const detail = modelType === "tts"
        ? `配置名称：${model.name}\n协议：${provider?.api || "—"}\n接口：${provider?.base_url || "—"}\nProfile：${model.tts?.profile_id || "—"}\n音色：${model.tts?.speaker || "—"}${model.tts?.language_type ? `\n语言：${model.tts.language_type}` : ""}`
        : modelType === "asr" ? `配置名称：${model.name}\n接口：${provider?.base_url || "—"}\n用于聊天录音转文字，不消耗 LLM Token`
        : `配置名称：${model.name}\n接口：${provider?.base_url || "—"}\nTemperature ${model.temperature ?? 0.7} · Top P ${model.top_p ?? 0.9} · Top K ${model.top_k ?? 40}\n上下文 ${model.context_window.toLocaleString()} · ${provider?.output_token_parameter === "omit" ? "输出长度由模型服务决定" : `最大输出 ${model.max_tokens.toLocaleString()} tokens`}`;
      const actions = element("div", "model-item-actions");
      if (document.body.dataset.authRole !== "chat") {
        actions.append(
          actionButton(modelType === "tts" ? "生成短音频测试" : modelType === "asr" ? "测试转录接口" : "测试 API 连接", "model-test", model.name, "secondary"),
          actionButton("删除", "model-delete", model.name, "ghost danger-text"),
        );
      }
      item.append(heading, element("p", "model-item-meta", detail), actions);
      group.append(item);
    }
    list.append(group);
  }
}

async function loadCatalog(): Promise<void> {
  state.catalog = await api<ModelCatalog>("/v1/model-catalog");
  state.catalog.providers = state.catalog.providers.map((provider) => ({ ...provider, model_type: provider.model_type || "llm" }));
  state.catalog.models = state.catalog.models.map((model) => ({ ...model, model_type: model.model_type || "llm" }));
  configureTypeFields(selectedType(), true);
  renderModels();
}

function optionalNumber(selector: string): number | null {
  const value = $<HTMLInputElement>(selector).value.trim();
  return value ? Number(value) : null;
}

async function saveConfiguration(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const modelType = selectedType();
  const identifier = $<HTMLInputElement>("#model-name").value.trim();
  const modelId = $<HTMLInputElement>("#model-display-name").value.trim();
  const apiKey = $<HTMLInputElement>("#provider-api-key").value.trim();
  const providerName = $<HTMLInputElement>("#provider-name").value.trim();
  const provider = providerNamed(providerName);
  if (provider && provider.model_type !== modelType) throw new Error("接口名称已被另一种模型类型使用");
  if (!apiKey && !provider?.api_key_configured) throw new Error("请输入 API Key");
  const enabled = $<HTMLInputElement>("#model-enabled").checked;
  await api("/v1/model-configuration", {
    method: "POST",
    body: {
      actor: "web-console",
      api_key: apiKey || null,
      provider: {
        name: providerName,
        model_type: modelType,
        api: $<HTMLSelectElement>("#provider-api").value,
        output_token_parameter: modelType === "llm" && $<HTMLSelectElement>("#provider-api").value === "openai_compatible"
          ? ($<HTMLSelectElement>("#provider-output-token-parameter").value || null) : null,
        base_url: $<HTMLInputElement>("#provider-base-url").value.trim(),
        secret_env: null,
        api_key_configured: provider?.api_key_configured || Boolean(apiKey),
        enabled: true,
      },
      model: {
        name: identifier,
        model_id: modelId,
        display_name: modelId,
        provider: providerName,
        model_type: modelType,
        context_window: modelType === "llm" ? Number($<HTMLInputElement>("#model-context").value) : 1,
        max_tokens: modelType === "llm" ? Number($<HTMLInputElement>("#model-max-tokens").value) : 1,
        thinking: modelType === "llm" ? $<HTMLSelectElement>("#model-thinking").value : "none",
        temperature: modelType === "llm" ? optionalNumber("#model-temperature") : null,
        top_p: modelType === "llm" ? optionalNumber("#model-top-p") : null,
        top_k: modelType === "llm" ? optionalNumber("#model-top-k") : null,
        tts: modelType === "tts" ? {
          profile_id: $<HTMLInputElement>("#tts-profile-id").value.trim(),
          speaker: $<HTMLInputElement>("#tts-speaker").value.trim(),
          language_type: $<HTMLSelectElement>("#provider-api").value === "aliyun_qwen_tts_http"
            ? $<HTMLSelectElement>("#tts-language-type").value : null,
          audio_format: $<HTMLSelectElement>("#provider-api").value === "aliyun_qwen_audio_tts_websocket"
            ? $<HTMLSelectElement>("#tts-audio-format").value : null,
          sample_rate: $<HTMLSelectElement>("#provider-api").value === "aliyun_qwen_audio_tts_websocket"
            ? Number($<HTMLInputElement>("#tts-sample-rate").value) : null,
        } : null,
        enabled,
      },
    },
  });
  state.selectedModel = identifier;
  await loadCatalog();
  showToast(enabled ? `${typeLabel(modelType)} 配置已保存并启用；其他类型未受影响` : `${typeLabel(modelType)} 配置已保存`);
}

function populateModel(model: ConfiguredModel): void {
  const modelType = model.model_type || "llm";
  state.selectedModel = model.name;
  configureTypeFields(modelType, true);
  $<HTMLInputElement>("#model-name").value = model.name;
  $<HTMLInputElement>("#model-name").readOnly = true;
  $<HTMLInputElement>("#model-display-name").value = model.model_id;
  $<HTMLInputElement>("#model-temperature").value = String(model.temperature ?? 0.7);
  $<HTMLInputElement>("#model-top-p").value = String(model.top_p ?? 0.9);
  $<HTMLInputElement>("#model-top-k").value = String(model.top_k ?? 40);
  $<HTMLInputElement>("#model-context").value = String(model.context_window || 128000);
  $<HTMLInputElement>("#model-max-tokens").value = String(model.max_tokens || 16384);
  $<HTMLSelectElement>("#model-thinking").value = model.thinking;
  $<HTMLInputElement>("#tts-profile-id").value = model.tts?.profile_id || "persona-nova";
  $<HTMLInputElement>("#tts-speaker").value = model.tts?.speaker || "zh_female_vv_uranus_bigtts";
  $<HTMLSelectElement>("#tts-language-type").value = model.tts?.language_type || "Auto";
  $<HTMLSelectElement>("#tts-audio-format").value = model.tts?.audio_format || "mp3";
  $<HTMLInputElement>("#tts-sample-rate").value = String(model.tts?.sample_rate || 22050);
  if (modelType === "tts") configureTtsProtocolFields(false);
  $<HTMLInputElement>("#model-enabled").checked = model.enabled;
  $("#model-form-title").textContent = `修改 ${typeLabel(modelType)} · ${model.display_name}`;
  renderModels();
  $<HTMLInputElement>("#model-display-name").focus();
}

function resetModelForm(modelType: ModelType = selectedType()): void {
  state.selectedModel = null;
  const name = $<HTMLInputElement>("#model-name");
  name.value = "";
  name.readOnly = false;
  $<HTMLInputElement>("#model-display-name").value = modelType === "tts" ? "seed-tts-2.0" : "";
  $<HTMLInputElement>("#model-temperature").value = "0.7";
  $<HTMLInputElement>("#model-top-p").value = "0.9";
  $<HTMLInputElement>("#model-top-k").value = "40";
  $<HTMLInputElement>("#model-context").value = "128000";
  $<HTMLInputElement>("#model-max-tokens").value = "16384";
  $<HTMLSelectElement>("#model-thinking").value = "none";
  $<HTMLInputElement>("#tts-profile-id").value = "persona-nova";
  $<HTMLInputElement>("#tts-speaker").value = "zh_female_vv_uranus_bigtts";
  $<HTMLSelectElement>("#tts-language-type").value = "Auto";
  $<HTMLSelectElement>("#tts-audio-format").value = "mp3";
  $<HTMLInputElement>("#tts-sample-rate").value = "22050";
  $<HTMLInputElement>("#model-enabled").checked = false;
  $("#model-form-title").textContent = `添加 ${typeLabel(modelType)} 配置`;
  configureTypeFields(modelType, true);
  renderModels();
  name.focus();
}

async function modelAction(event: MouseEvent | KeyboardEvent): Promise<void> {
  const target = event.target as Element;
  const button = target.closest<HTMLButtonElement>("button[data-action]");
  const item = target.closest<HTMLElement>(".model-item[data-name]");
  const name = button?.dataset.name || item?.dataset.name;
  if (!name) return;
  const model = state.catalog.models.find((entry) => entry.name === name);
  if (!model) return;
  if (!button) {
    populateModel(model);
    return;
  }
  if (button.dataset.action === "model-test") {
    button.disabled = true;
    try {
      const result = await api<{ model: { name: string; provider: string; model_id: string; model_type?: ModelType } }>(
        `/v1/models/${encodeURIComponent(model.name)}/test`,
        { method: "POST", body: { actor: "web-console", confirm: true } },
      );
      showToast(model.model_type === "asr" ? "ASR 转录接口通过；识别效果请到对话页面录音测试" : `${typeLabel(model.model_type || "llm")} 测试通过：${result.model.name} · ${result.model.provider}/${result.model.model_id}`);
    } finally {
      button.disabled = false;
    }
  }
  if (button.dataset.action === "model-delete") {
    if (!window.confirm(`删除 ${typeLabel(model.model_type || "llm")} 配置 ${model.display_name}？`)) return;
    await api(`/v1/models/${encodeURIComponent(model.name)}`, {
      method: "DELETE",
      body: { actor: "web-console", confirm: true },
    });
    if (state.selectedModel === model.name) resetModelForm(model.model_type || "llm");
  }
  await loadCatalog();
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

$<HTMLFormElement>("#model-config-form").addEventListener("submit", guarded((event) => saveConfiguration(event)));
$("#model-list").addEventListener("click", guarded((event: MouseEvent) => modelAction(event)));
$("#model-list").addEventListener("keydown", guarded((event: KeyboardEvent) => {
  if ((event.key === "Enter" || event.key === " ") && !(event.target as Element).closest("button")) {
    event.preventDefault();
    return modelAction(event);
  }
}));
$<HTMLSelectElement>("#model-type").addEventListener("change", () => resetModelForm(selectedType()));
$("#provider-api").addEventListener("change", () => {
  if (selectedType() === "tts" && state.selectedModel === null) {
    const api = $<HTMLSelectElement>("#provider-api").value as ModelApi;
    const name = $<HTMLInputElement>("#provider-name");
    const existing = providerNamed(name.value.trim());
    const base = api === "aliyun_qwen_tts_http" ? "aliyun-tts-http"
      : api === "aliyun_qwen_audio_tts_websocket" ? "aliyun-tts-realtime" : "volcengine-tts";
    const automaticName = ["volcengine-tts", "aliyun-tts-http", "aliyun-tts-realtime"].includes(name.value.trim());
    if ((existing && existing.api !== api) || (automaticName && name.value.trim() !== base)) {
      let candidate = base;
      for (let index = 2; providerNamed(candidate) && providerNamed(candidate)?.api !== api; index++) {
        candidate = `${base}-${index}`;
      }
      name.value = candidate;
      updateProviderKeyField();
    }
  }
  configureOutputTokenField();
  configureTtsProtocolFields(true);
});
$("#provider-name").addEventListener("change", () => {
  const provider = providerNamed($<HTMLInputElement>("#provider-name").value.trim());
  if (provider?.model_type === selectedType()) {
    $<HTMLSelectElement>("#provider-api").value = provider.api;
    $<HTMLInputElement>("#provider-base-url").value = provider.base_url;
    configureOutputTokenField();
    configureTtsProtocolFields(false);
  }
  updateProviderKeyField();
});
$("#new-model").addEventListener("click", () => resetModelForm(selectedType()));

void checkService();
configureTypeFields("llm", true);
void loadCatalog().catch((error) => showToast(error instanceof Error ? error.message : String(error), true));
