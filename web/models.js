// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

"use strict";
(() => {
  // src/models.ts
  var state = {
    catalog: { providers: [], models: [], default_model: null, default_models: {} },
    selectedModel: null
  };
  var $ = (selector) => {
    const node = document.querySelector(selector);
    if (!node) throw new Error(`Missing UI element: ${selector}`);
    return node;
  };
  async function api(path, options = {}) {
    const headers = new Headers(options.headers);
    let body;
    if (options.body !== void 0) {
      headers.set("content-type", "application/json");
      body = JSON.stringify(options.body);
    }
    const response = await window.ExoAuth.fetch(
      path,
      { ...options, headers, body }
    );
    const text = await response.text();
    let payload = null;
    if (text) {
      try {
        payload = JSON.parse(text);
      } catch {
        payload = text;
      }
    }
    if (!response.ok) {
      const message = typeof payload === "object" && payload && "error" in payload ? payload.error?.message : null;
      throw new Error(message || `${response.status} ${response.statusText}`);
    }
    return payload;
  }
  async function checkService() {
    try {
      await api("/health");
      $("#service-dot").classList.add("online");
      $("#service-label").textContent = "\u670D\u52A1\u5728\u7EBF";
    } catch {
      $("#service-dot").classList.add("offline");
      $("#service-label").textContent = "\u670D\u52A1\u4E0D\u53EF\u7528";
    }
  }
  function element(tag, className = "", text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== void 0) node.textContent = text;
    return node;
  }
  function selectedType() {
    return $("#model-type").value;
  }
  function typeLabel(modelType) {
    return { llm: "LLM", tts: "TTS", asr: "ASR" }[modelType];
  }
  function providerFor(modelType) {
    return state.catalog.providers.find((provider) => (provider.model_type || "llm") === modelType) || null;
  }
  function providerNamed(name) {
    return state.catalog.providers.find((provider) => provider.name === name) || null;
  }
  function selectedProvider(modelType) {
    const model = state.catalog.models.find((entry) => entry.name === state.selectedModel);
    return model?.model_type === modelType ? providerNamed(model.provider) : providerFor(modelType);
  }
  function updateProviderKeyField() {
    const provider = providerNamed($("#provider-name").value.trim());
    const keyInput = $("#provider-api-key");
    keyInput.required = !provider?.api_key_configured;
    keyInput.placeholder = provider?.api_key_configured ? "\u5DF2\u914D\u7F6E\uFF1B\u7559\u7A7A\u8868\u793A\u4E0D\u4FEE\u6539" : "\u8BF7\u8F93\u5165 API Key";
    $("#provider-api-key-help").textContent = provider?.api_key_configured ? "API Key \u5DF2\u914D\u7F6E\uFF1B\u9875\u9762\u548C API \u5747\u4E0D\u4F1A\u56DE\u663E" : "\u5FC5\u586B\uFF1B\u4FDD\u5B58\u540E\u8FDB\u5165\u672C\u673A Secret Store\uFF0C\u4E0D\u5199\u5165\u6A21\u578B\u76EE\u5F55";
  }
  function defaultNameFor(modelType) {
    return state.catalog.default_models?.[modelType] || (modelType === "llm" ? state.catalog.default_model : null) || null;
  }
  function effectiveModelEnabled(model) {
    return model.enabled && providerNamed(model.provider)?.enabled === true;
  }
  function actionButton(text, action, name, style = "ghost") {
    const button = element("button", `button ${style}`, text);
    button.type = "button";
    button.dataset.action = action;
    button.dataset.name = name;
    return button;
  }
  function configureOutputTokenField() {
    const applicable = selectedType() === "llm" && $("#provider-api").value === "openai_compatible";
    $("#provider-output-token-field").toggleAttribute("hidden", !applicable);
    $("#provider-output-token-parameter").disabled = !applicable;
  }
  function configureTtsProtocolFields(applyDefaults = false) {
    if (selectedType() !== "tts") return;
    const api2 = $("#provider-api").value;
    const aliyunHttp = api2 === "aliyun_qwen_tts_http";
    const aliyunRealtime = api2 === "aliyun_qwen_audio_tts_websocket";
    $("#tts-language-field").toggleAttribute("hidden", !aliyunHttp);
    $("#tts-stream-fields").toggleAttribute("hidden", !aliyunRealtime);
    $("#tts-language-type").disabled = !aliyunHttp;
    $("#tts-audio-format").disabled = !aliyunRealtime;
    $("#tts-sample-rate").disabled = !aliyunRealtime;
    $("#tts-speaker-label").textContent = aliyunHttp || aliyunRealtime ? "\u97F3\u8272\uFF08voice\uFF09" : "\u97F3\u8272\uFF08Speaker\uFF09";
    $("#tts-speaker-help").textContent = aliyunHttp ? "\u4F8B\u5982 Cherry\uFF1B\u5C06\u4F5C\u4E3A input.voice \u53D1\u9001" : aliyunRealtime ? "\u4F8B\u5982 longanhuan_v3.6\uFF1B\u5C06\u4F5C\u4E3A parameters.voice \u53D1\u9001" : "\u706B\u5C71\u5F15\u64CE\u97F3\u8272\u6807\u8BC6";
    $("#connection-section-copy").textContent = aliyunHttp ? "\u8FDE\u63A5\u963F\u91CC\u4E91\u5343\u95EE TTS \u975E\u5B9E\u65F6 HTTP\uFF1B\u6574\u6BB5\u6587\u672C\u63D0\u4EA4\u540E\u4E0B\u8F7D\u5E76\u6301\u4E45\u5316\u6700\u7EC8\u97F3\u9891\u3002" : aliyunRealtime ? "\u8FDE\u63A5\u963F\u91CC\u4E91 Qwen-Audio TTS WebSocket\uFF1B\u5B9E\u65F6\u63A5\u6536\u4E8C\u8FDB\u5236\u97F3\u9891\u3002" : "\u8FDE\u63A5\u706B\u5C71\u5F15\u64CE TTS SSE \u670D\u52A1\uFF1BAPI Key \u4EC5\u5199\u5165\u672C\u673A\u5BC6\u94A5\u5B58\u50A8\u3002";
    $("#model-section-copy").textContent = aliyunHttp ? "\u6A21\u578B\u3001\u97F3\u8272\u4E0E\u8BED\u8A00\u5747\u53EF\u914D\u7F6E\uFF1Blanguage_type \u4F1A\u968F\u6BCF\u6B21\u5408\u6210\u53D1\u9001\u3002" : aliyunRealtime ? "\u6A21\u578B\u3001\u97F3\u8272\u3001\u683C\u5F0F\u4E0E\u91C7\u6837\u7387\u5747\u53EF\u914D\u7F6E\uFF1B\u97F3\u9891\u901A\u8FC7 WebSocket \u5B9E\u65F6\u63A5\u6536\u3002" : "Resource ID \u9009\u62E9\u8BED\u97F3\u6A21\u578B\uFF0CProfile ID \u6807\u8BC6\u58F0\u97F3\u89D2\u8272\uFF0CSpeaker \u6307\u5B9A\u5B9E\u9645\u97F3\u8272\u3002";
    $("#model-id-label").textContent = aliyunHttp || aliyunRealtime ? "\u6A21\u578B\u540D\u79F0" : "Resource ID";
    $("#model-id-help").textContent = aliyunHttp ? "\u4F8B\u5982 qwen3-tts-flash" : aliyunRealtime ? "\u4F8B\u5982 qwen-audio-3.0-tts-flash" : "\u4F8B\u5982 seed-tts-2.0";
    $("#provider-base-url-help").textContent = aliyunHttp ? "\u963F\u91CC\u4E91\u767E\u70BC\u591A\u6A21\u6001\u751F\u6210\u975E\u5B9E\u65F6 HTTP \u5730\u5740" : aliyunRealtime ? "\u963F\u91CC\u4E91\u767E\u70BC\u5B9E\u65F6\u8BED\u97F3\u5408\u6210 WebSocket \u5730\u5740\uFF08wss://\uFF09" : "\u706B\u5C71\u5F15\u64CE V3 Server-Sent Events \u5355\u5411\u6D41\u5F0F\u5730\u5740";
    if (!applyDefaults) return;
    if (aliyunHttp) {
      $("#provider-base-url").value = "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
      $("#model-display-name").value = "qwen3-tts-flash";
      $("#tts-profile-id").value = "aliyun-qwen3-voice";
      $("#tts-speaker").value = "Cherry";
      $("#tts-language-type").value = "English";
    } else if (aliyunRealtime) {
      $("#provider-base-url").value = "wss://dashscope.aliyuncs.com/api-ws/v1/inference";
      $("#model-display-name").value = "qwen-audio-3.0-tts-flash";
      $("#tts-profile-id").value = "aliyun-qwen-audio-voice";
      $("#tts-speaker").value = "longanhuan_v3.6";
      $("#tts-audio-format").value = "mp3";
      $("#tts-sample-rate").value = "22050";
    } else {
      $("#provider-base-url").value = "https://openspeech.bytedance.com/api/v3/tts/unidirectional/sse";
      $("#model-display-name").value = "seed-tts-2.0";
      $("#tts-profile-id").value = "persona-nova";
      $("#tts-speaker").value = "zh_female_vv_uranus_bigtts";
    }
  }
  function configureTypeFields(modelType, hydrateProvider = true) {
    const tts = modelType === "tts";
    const asr = modelType === "asr";
    const typeSelect = $("#model-type");
    typeSelect.value = modelType;
    typeSelect.disabled = state.selectedModel !== null;
    const apiSelect = $("#provider-api");
    for (const option of apiSelect.options) {
      option.hidden = option.dataset.modelType !== modelType;
      option.disabled = option.hidden;
    }
    $("#tts-model-fields").toggleAttribute("hidden", !tts);
    $("#llm-advanced-fields").toggleAttribute("hidden", modelType !== "llm");
    $("#llm-advanced-fields").querySelectorAll("input, select").forEach((input) => {
      input.disabled = modelType !== "llm";
    });
    $("#tts-profile-id").required = tts;
    $("#tts-speaker").required = tts;
    $("#connection-section-copy").textContent = tts ? "\u8FDE\u63A5 TTS \u670D\u52A1\uFF1BAPI Key \u4EC5\u5199\u5165\u672C\u673A\u5BC6\u94A5\u5B58\u50A8\u3002" : asr ? "\u8FDE\u63A5\u8BED\u97F3\u8F6C\u6587\u5B57\u670D\u52A1\uFF0C\u5F55\u97F3\u8BC6\u522B\u4E0D\u7ECF\u8FC7\u5927\u8BED\u8A00\u6A21\u578B\u3002" : "\u8FDE\u63A5 LLM \u670D\u52A1\u6240\u9700\u7684\u4FE1\u606F\u3002";
    $("#model-section-copy").textContent = tts ? "\u6A21\u578B\u3001\u58F0\u97F3\u89D2\u8272\u4E0E\u97F3\u8272\u7531\u6240\u9009 TTS \u534F\u8BAE\u51B3\u5B9A\u3002" : "\u914D\u7F6E\u540D\u79F0\u7528\u4E8E\u533A\u5206\u4FDD\u5B58\u7684\u914D\u7F6E\uFF1B\u6A21\u578B\u540D\u79F0\u4F1A\u76F4\u63A5\u53D1\u9001\u7ED9 API \u670D\u52A1\u3002";
    $("#model-id-label").textContent = tts ? "Resource ID" : "\u6A21\u578B\u540D\u79F0";
    $("#model-id-help").textContent = tts ? "\u4F8B\u5982 seed-tts-2.0" : "\u586B\u5199\u6A21\u578B\u670D\u52A1\u63D0\u4F9B\u7684\u6A21\u578B\u540D\u79F0";
    $("#model-enabled-copy").textContent = `\u542F\u7528\u8FD9\u4E2A ${typeLabel(modelType)}\uFF1B\u4FDD\u5B58\u540E\u81EA\u52A8\u505C\u7528\u540C\u7C7B\u578B\u7684\u5176\u4ED6\u914D\u7F6E\uFF0C\u4E0D\u5F71\u54CD\u5176\u4ED6\u7C7B\u578B`;
    if (!hydrateProvider) return;
    const provider = selectedProvider(modelType);
    const providerName = $("#provider-name");
    providerName.value = provider?.name || (tts ? "volcengine-tts" : asr ? "speech-api" : "default-api");
    providerName.readOnly = state.selectedModel !== null;
    const names = $("#provider-names");
    names.replaceChildren(...state.catalog.providers.filter((entry) => entry.model_type === modelType).map((entry) => {
      const option = document.createElement("option");
      option.value = entry.name;
      return option;
    }));
    apiSelect.value = provider?.api || (tts ? "volcengine_tts_sse" : asr ? "openai_transcription" : "openai_compatible");
    $("#provider-output-token-parameter").value = provider?.output_token_parameter || "";
    configureOutputTokenField();
    $("#provider-base-url").value = provider?.base_url || (tts ? "https://openspeech.bytedance.com/api/v3/tts/unidirectional/sse" : asr ? "http://127.0.0.1:8796/v1" : "https://api.openai.com/v1");
    $("#provider-base-url-help").textContent = tts ? "\u706B\u5C71\u5F15\u64CE V3 Server-Sent Events \u5355\u5411\u6D41\u5F0F\u5730\u5740" : asr ? "\u586B\u5199 API \u6839\u5730\u5740\uFF08\u542B /v1\uFF09\uFF0C\u7CFB\u7EDF\u901A\u8FC7 /audio/transcriptions \u4E0A\u4F20\u5F55\u97F3" : "\u4F8B\u5982 https://api.openai.com/v1";
    $("#provider-api-key").value = "";
    updateProviderKeyField();
    if (tts) configureTtsProtocolFields(false);
  }
  function renderModels() {
    const list = $("#model-list");
    list.replaceChildren();
    $("#model-count").textContent = `${state.catalog.models.length} \u4E2A\u6A21\u578B`;
    const activeLlm = state.catalog.models.find((model) => model.name === defaultNameFor("llm"));
    const activeTts = state.catalog.models.find((model) => model.name === defaultNameFor("tts"));
    const activeAsr = state.catalog.models.find((model) => model.name === defaultNameFor("asr"));
    $("#default-model-summary").textContent = `LLM ${activeLlm?.display_name || "\u2014"} \xB7 TTS ${activeTts?.tts?.profile_id || activeTts?.display_name || "\u2014"} \xB7 ASR ${activeAsr?.display_name || "\u2014"}`;
    if (!state.catalog.models.length) {
      const empty = element("div", "settings-empty");
      empty.append(element("strong", "", "\u8FD8\u6CA1\u6709\u914D\u7F6E\u6A21\u578B"), element("span", "", "\u9009\u62E9 LLM\u3001TTS \u6216 ASR\uFF0C\u586B\u5199\u5DE6\u4FA7\u914D\u7F6E\u5E76\u4FDD\u5B58\u3002"));
      list.append(empty);
      return;
    }
    for (const modelType of ["llm", "tts", "asr"]) {
      const models = state.catalog.models.filter((model) => (model.model_type || "llm") === modelType);
      if (!models.length) continue;
      const group = element("section", "model-type-group");
      const groupHeading = element("div", "model-type-group-heading");
      groupHeading.append(element("strong", "", typeLabel(modelType)), element("span", "", `${models.length} \u4E2A\u914D\u7F6E`));
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
        heading.append(title, element("span", current ? "state-enabled" : "state-disabled", current ? modelType === "asr" ? "\u5DF2\u542F\u7528 \xB7 \u8BED\u97F3\u8F93\u5165" : "\u5DF2\u542F\u7528 \xB7 \u5BF9\u8BDD\u53EF\u7528" : "\u672A\u542F\u7528"));
        const provider = providerNamed(model.provider);
        const detail = modelType === "tts" ? `\u914D\u7F6E\u540D\u79F0\uFF1A${model.name}
\u534F\u8BAE\uFF1A${provider?.api || "\u2014"}
\u63A5\u53E3\uFF1A${provider?.base_url || "\u2014"}
Profile\uFF1A${model.tts?.profile_id || "\u2014"}
\u97F3\u8272\uFF1A${model.tts?.speaker || "\u2014"}${model.tts?.language_type ? `
\u8BED\u8A00\uFF1A${model.tts.language_type}` : ""}` : modelType === "asr" ? `\u914D\u7F6E\u540D\u79F0\uFF1A${model.name}
\u63A5\u53E3\uFF1A${provider?.base_url || "\u2014"}
\u7528\u4E8E\u804A\u5929\u5F55\u97F3\u8F6C\u6587\u5B57\uFF0C\u4E0D\u6D88\u8017 LLM Token` : `\u914D\u7F6E\u540D\u79F0\uFF1A${model.name}
\u63A5\u53E3\uFF1A${provider?.base_url || "\u2014"}
Temperature ${model.temperature ?? 0.7} \xB7 Top P ${model.top_p ?? 0.9} \xB7 Top K ${model.top_k ?? 40}
\u4E0A\u4E0B\u6587 ${model.context_window.toLocaleString()} \xB7 ${provider?.output_token_parameter === "omit" ? "\u8F93\u51FA\u957F\u5EA6\u7531\u6A21\u578B\u670D\u52A1\u51B3\u5B9A" : `\u6700\u5927\u8F93\u51FA ${model.max_tokens.toLocaleString()} tokens`}`;
        const actions = element("div", "model-item-actions");
        if (document.body.dataset.authRole !== "chat") {
          actions.append(
            actionButton(modelType === "tts" ? "\u751F\u6210\u77ED\u97F3\u9891\u6D4B\u8BD5" : modelType === "asr" ? "\u6D4B\u8BD5\u8F6C\u5F55\u63A5\u53E3" : "\u6D4B\u8BD5 API \u8FDE\u63A5", "model-test", model.name, "secondary"),
            actionButton("\u5220\u9664", "model-delete", model.name, "ghost danger-text")
          );
        }
        item.append(heading, element("p", "model-item-meta", detail), actions);
        group.append(item);
      }
      list.append(group);
    }
  }
  async function loadCatalog() {
    state.catalog = await api("/v1/model-catalog");
    state.catalog.providers = state.catalog.providers.map((provider) => ({ ...provider, model_type: provider.model_type || "llm" }));
    state.catalog.models = state.catalog.models.map((model) => ({ ...model, model_type: model.model_type || "llm" }));
    configureTypeFields(selectedType(), true);
    renderModels();
  }
  function optionalNumber(selector) {
    const value = $(selector).value.trim();
    return value ? Number(value) : null;
  }
  async function saveConfiguration(event) {
    event.preventDefault();
    const modelType = selectedType();
    const identifier = $("#model-name").value.trim();
    const modelId = $("#model-display-name").value.trim();
    const apiKey = $("#provider-api-key").value.trim();
    const providerName = $("#provider-name").value.trim();
    const provider = providerNamed(providerName);
    if (provider && provider.model_type !== modelType) throw new Error("\u63A5\u53E3\u540D\u79F0\u5DF2\u88AB\u53E6\u4E00\u79CD\u6A21\u578B\u7C7B\u578B\u4F7F\u7528");
    if (!apiKey && !provider?.api_key_configured) throw new Error("\u8BF7\u8F93\u5165 API Key");
    const enabled = $("#model-enabled").checked;
    await api("/v1/model-configuration", {
      method: "POST",
      body: {
        actor: "web-console",
        api_key: apiKey || null,
        provider: {
          name: providerName,
          model_type: modelType,
          api: $("#provider-api").value,
          output_token_parameter: modelType === "llm" && $("#provider-api").value === "openai_compatible" ? $("#provider-output-token-parameter").value || null : null,
          base_url: $("#provider-base-url").value.trim(),
          secret_env: null,
          api_key_configured: provider?.api_key_configured || Boolean(apiKey),
          enabled: true
        },
        model: {
          name: identifier,
          model_id: modelId,
          display_name: modelId,
          provider: providerName,
          model_type: modelType,
          context_window: modelType === "llm" ? Number($("#model-context").value) : 1,
          max_tokens: modelType === "llm" ? Number($("#model-max-tokens").value) : 1,
          thinking: modelType === "llm" ? $("#model-thinking").value : "none",
          temperature: modelType === "llm" ? optionalNumber("#model-temperature") : null,
          top_p: modelType === "llm" ? optionalNumber("#model-top-p") : null,
          top_k: modelType === "llm" ? optionalNumber("#model-top-k") : null,
          tts: modelType === "tts" ? {
            profile_id: $("#tts-profile-id").value.trim(),
            speaker: $("#tts-speaker").value.trim(),
            language_type: $("#provider-api").value === "aliyun_qwen_tts_http" ? $("#tts-language-type").value : null,
            audio_format: $("#provider-api").value === "aliyun_qwen_audio_tts_websocket" ? $("#tts-audio-format").value : null,
            sample_rate: $("#provider-api").value === "aliyun_qwen_audio_tts_websocket" ? Number($("#tts-sample-rate").value) : null
          } : null,
          enabled
        }
      }
    });
    state.selectedModel = identifier;
    await loadCatalog();
    showToast(enabled ? `${typeLabel(modelType)} \u914D\u7F6E\u5DF2\u4FDD\u5B58\u5E76\u542F\u7528\uFF1B\u5176\u4ED6\u7C7B\u578B\u672A\u53D7\u5F71\u54CD` : `${typeLabel(modelType)} \u914D\u7F6E\u5DF2\u4FDD\u5B58`);
  }
  function populateModel(model) {
    const modelType = model.model_type || "llm";
    state.selectedModel = model.name;
    configureTypeFields(modelType, true);
    $("#model-name").value = model.name;
    $("#model-name").readOnly = true;
    $("#model-display-name").value = model.model_id;
    $("#model-temperature").value = String(model.temperature ?? 0.7);
    $("#model-top-p").value = String(model.top_p ?? 0.9);
    $("#model-top-k").value = String(model.top_k ?? 40);
    $("#model-context").value = String(model.context_window || 128e3);
    $("#model-max-tokens").value = String(model.max_tokens || 16384);
    $("#model-thinking").value = model.thinking;
    $("#tts-profile-id").value = model.tts?.profile_id || "persona-nova";
    $("#tts-speaker").value = model.tts?.speaker || "zh_female_vv_uranus_bigtts";
    $("#tts-language-type").value = model.tts?.language_type || "Auto";
    $("#tts-audio-format").value = model.tts?.audio_format || "mp3";
    $("#tts-sample-rate").value = String(model.tts?.sample_rate || 22050);
    if (modelType === "tts") configureTtsProtocolFields(false);
    $("#model-enabled").checked = model.enabled;
    $("#model-form-title").textContent = `\u4FEE\u6539 ${typeLabel(modelType)} \xB7 ${model.display_name}`;
    renderModels();
    $("#model-display-name").focus();
  }
  function resetModelForm(modelType = selectedType()) {
    state.selectedModel = null;
    const name = $("#model-name");
    name.value = "";
    name.readOnly = false;
    $("#model-display-name").value = modelType === "tts" ? "seed-tts-2.0" : "";
    $("#model-temperature").value = "0.7";
    $("#model-top-p").value = "0.9";
    $("#model-top-k").value = "40";
    $("#model-context").value = "128000";
    $("#model-max-tokens").value = "16384";
    $("#model-thinking").value = "none";
    $("#tts-profile-id").value = "persona-nova";
    $("#tts-speaker").value = "zh_female_vv_uranus_bigtts";
    $("#tts-language-type").value = "Auto";
    $("#tts-audio-format").value = "mp3";
    $("#tts-sample-rate").value = "22050";
    $("#model-enabled").checked = false;
    $("#model-form-title").textContent = `\u6DFB\u52A0 ${typeLabel(modelType)} \u914D\u7F6E`;
    configureTypeFields(modelType, true);
    renderModels();
    name.focus();
  }
  async function modelAction(event) {
    const target = event.target;
    const button = target.closest("button[data-action]");
    const item = target.closest(".model-item[data-name]");
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
        const result = await api(
          `/v1/models/${encodeURIComponent(model.name)}/test`,
          { method: "POST", body: { actor: "web-console", confirm: true } }
        );
        showToast(model.model_type === "asr" ? "ASR \u8F6C\u5F55\u63A5\u53E3\u901A\u8FC7\uFF1B\u8BC6\u522B\u6548\u679C\u8BF7\u5230\u5BF9\u8BDD\u9875\u9762\u5F55\u97F3\u6D4B\u8BD5" : `${typeLabel(model.model_type || "llm")} \u6D4B\u8BD5\u901A\u8FC7\uFF1A${result.model.name} \xB7 ${result.model.provider}/${result.model.model_id}`);
      } finally {
        button.disabled = false;
      }
    }
    if (button.dataset.action === "model-delete") {
      if (!window.confirm(`\u5220\u9664 ${typeLabel(model.model_type || "llm")} \u914D\u7F6E ${model.display_name}\uFF1F`)) return;
      await api(`/v1/models/${encodeURIComponent(model.name)}`, {
        method: "DELETE",
        body: { actor: "web-console", confirm: true }
      });
      if (state.selectedModel === model.name) resetModelForm(model.model_type || "llm");
    }
    await loadCatalog();
  }
  var toastTimer = null;
  function showToast(message, error = false) {
    const toast = $("#toast");
    toast.textContent = message;
    toast.classList.toggle("error", error);
    toast.classList.add("visible");
    if (toastTimer !== null) window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => toast.classList.remove("visible"), 5e3);
  }
  function guarded(handler) {
    return async (event) => {
      try {
        await handler(event);
      } catch (error) {
        showToast(error instanceof Error ? error.message : String(error), true);
      }
    };
  }
  $("#model-config-form").addEventListener("submit", guarded((event) => saveConfiguration(event)));
  $("#model-list").addEventListener("click", guarded((event) => modelAction(event)));
  $("#model-list").addEventListener("keydown", guarded((event) => {
    if ((event.key === "Enter" || event.key === " ") && !event.target.closest("button")) {
      event.preventDefault();
      return modelAction(event);
    }
  }));
  $("#model-type").addEventListener("change", () => resetModelForm(selectedType()));
  $("#provider-api").addEventListener("change", () => {
    if (selectedType() === "tts" && state.selectedModel === null) {
      const api2 = $("#provider-api").value;
      const name = $("#provider-name");
      const existing = providerNamed(name.value.trim());
      const base = api2 === "aliyun_qwen_tts_http" ? "aliyun-tts-http" : api2 === "aliyun_qwen_audio_tts_websocket" ? "aliyun-tts-realtime" : "volcengine-tts";
      const automaticName = ["volcengine-tts", "aliyun-tts-http", "aliyun-tts-realtime"].includes(name.value.trim());
      if (existing && existing.api !== api2 || automaticName && name.value.trim() !== base) {
        let candidate = base;
        for (let index = 2; providerNamed(candidate) && providerNamed(candidate)?.api !== api2; index++) {
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
    const provider = providerNamed($("#provider-name").value.trim());
    if (provider?.model_type === selectedType()) {
      $("#provider-api").value = provider.api;
      $("#provider-base-url").value = provider.base_url;
      configureOutputTokenField();
      configureTtsProtocolFields(false);
    }
    updateProviderKeyField();
  });
  $("#new-model").addEventListener("click", () => resetModelForm(selectedType()));
  void checkService();
  configureTypeFields("llm", true);
  void loadCatalog().catch((error) => showToast(error instanceof Error ? error.message : String(error), true));
})();
