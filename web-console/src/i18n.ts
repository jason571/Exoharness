import { english } from './i18n-catalog';

type Language = 'zh-CN' | 'en';
const storageKey = 'exoharness.ui-language';
const protectedContent = [
  'script', 'style', 'pre', 'code', 'textarea', 'input', '[contenteditable]',
  '[data-i18n="off"]', '.rich-content', '.tool-progress-message',
  '.session-item-heading strong', '.session-item-meta', '.user-conversation-title',
  '#conversation-title', '#portal-user-name', '#portal-user-email',
  '#trace-conversation-title', '.video-artifact-heading strong',
  '#skill-instructions', '.skill-instructions', '.artifact-name',
  '.user-conversation-prompt', '.user-identity', '.user-installed-item > strong',
  '.capability-card strong', '.secret-row-heading strong', '.validation-check span',
  '#detail-name', '#detail-description', '#skill-instruction-content',
  '.assistant-card strong', '.agent-card strong', '.model-card strong',
  '.overview-task strong', '.readiness-item:not(:first-child) > strong',
  '.user-list-name', '.user-list-avatar', '.agent-list-heading strong',
  '.agent-list-copy > span:not(.agent-list-heading)', '.agent-list-avatar', '.model-item-title strong',
  '#default-model-summary', '#default-tts-summary', '#default-asr-summary',
  '.trace-run-item-head strong', '.trace-run-prompt', '.trace-turn-prompt', '#trace-prompt',
].join(',');
const attributes = ['placeholder', 'title', 'aria-label'] as const;
const textSources = new WeakMap<Text, { source: string; rendered: string }>();
const attributeSources = new WeakMap<Element, Map<string, { source: string; rendered: string }>>();
const selectors = new Set<HTMLSelectElement>();
let language: Language = 'zh-CN';
try { if (localStorage.getItem(storageKey) === 'en') language = 'en'; } catch { /* Private browsing may disable storage. */ }

function render(source: string): string {
  if (language === 'zh-CN') return source;
  const core = source.trim();
  return core ? source.replace(core, english(core)) : source;
}

function translateText(node: Text): void {
  if (!node.parentElement || node.parentElement.closest(protectedContent)) return;
  const previous = textSources.get(node);
  const source = previous && node.data === previous.rendered ? previous.source : node.data;
  const rendered = render(source);
  textSources.set(node, { source, rendered });
  if (node.data !== rendered) node.data = rendered;
}

function translateAttributes(element: Element): void {
  if (element.closest('[data-i18n="off"],pre,code,.rich-content,.tool-progress-message')) return;
  let sources = attributeSources.get(element);
  if (!sources) { sources = new Map(); attributeSources.set(element, sources); }
  for (const name of attributes) {
    const current = element.getAttribute(name);
    if (current === null) { sources.delete(name); continue; }
    // Session titles and filenames are data. Only explicit action prefixes are localized.
    if (name === 'title' && element.matches('.session-item,.video-artifact-heading strong')) continue;
    const previous = sources.get(name);
    const source = previous && current === previous.rendered ? previous.source : current;
    const rendered = render(source);
    sources.set(name, { source, rendered });
    if (rendered !== current) element.setAttribute(name, rendered);
  }
}

function translateTree(root: Node): void {
  if (root instanceof Text) { translateText(root); return; }
  if (root instanceof Element && root.closest(protectedContent)) {
    if (root instanceof HTMLInputElement || root instanceof HTMLTextAreaElement) translateAttributes(root);
    return;
  }
  if (root instanceof Element) translateAttributes(root);
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      if (node instanceof Element && node.matches(protectedContent)) {
        // Input hints are interface copy; values are never changed.
        if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) translateAttributes(node);
        return NodeFilter.FILTER_REJECT;
      }
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  while (walker.nextNode()) {
    const node = walker.currentNode;
    if (node instanceof Text) translateText(node);
    else if (node instanceof Element) translateAttributes(node);
  }
}

const pending = new Set<Node>();
let scheduled = false;
const observer = new MutationObserver(records => {
  for (const record of records) {
    if (record.type === 'childList') for (const node of record.addedNodes) pending.add(node);
    else pending.add(record.target);
  }
  if (!scheduled && pending.size) {
    scheduled = true;
    queueMicrotask(() => {
      scheduled = false;
      observer.disconnect();
      for (const node of pending) if (node.isConnected) translateTree(node);
      pending.clear();
      observe();
    });
  }
});
function observe(): void {
  observer.observe(document.documentElement, {
    subtree: true, childList: true, characterData: true, attributes: true,
    attributeFilter: [...attributes],
  });
}

function setLanguage(next: Language, persist = true): void {
  language = next;
  if (persist) try { localStorage.setItem(storageKey, language); } catch { /* Keep an in-memory preference. */ }
  observer.disconnect();
  document.documentElement.lang = language;
  translateTree(document.documentElement);
  selectors.forEach(select => { select.value = language; });
  observe();
  document.dispatchEvent(new CustomEvent('exoharness:languagechange', { detail: { language } }));
}

function installSwitcher(): void {
  const label = document.createElement('label');
  label.className = 'ui-language-switch';
  label.dataset.i18n = 'off';
  label.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a18 18 0 0 1 0 18 18 18 0 0 1 0-18Z"/></svg><select aria-label="Language / 语言"><option value="zh-CN">中文</option><option value="en">English</option></select>';
  const select = label.querySelector('select')!;
  selectors.add(select);
  select.value = language;
  select.addEventListener('change', () => setLanguage(select.value === 'en' ? 'en' : 'zh-CN'));
  const anchor = document.querySelector('.chat-header-actions') || document.querySelector('.topbar') || document.querySelector('.portal-access-shell,.login-shell');
  (anchor || document.body).append(label);
  if (!anchor || anchor.matches('.portal-access-shell,.login-shell')) label.classList.add('ui-language-corner');
  const style = document.createElement('style');
  style.dataset.i18n = 'off';
  // Account and delivery actions can mount asynchronously after this control.
  // Visual order must not depend on their response/load timing.
  style.textContent = `.ui-language-switch{display:inline-flex;align-items:center;gap:6px;flex-shrink:0;order:100;margin-left:auto;padding:5px 8px;border:1px solid var(--border,#7775);border-radius:10px;color:inherit;background:var(--surface,#20222b);font:13px/1.4 system-ui}.ui-language-switch svg{width:17px;height:17px;flex-shrink:0}.ui-language-switch select{min-width:70px;max-width:100px;width:auto;padding:2px;border:0;background:transparent;color:inherit;font:inherit;cursor:pointer}.ui-language-switch select option{color:#20222b;background:#fff}.ui-language-switch select:focus-visible{outline:2px solid #7189ed;outline-offset:3px}.ui-language-corner{position:fixed;top:18px;right:18px;z-index:30}.ui-language-switch:not(.ui-language-corner){align-self:center}@media(max-width:600px){.ui-language-switch{gap:3px;padding:4px 5px;font-size:12px}.ui-language-switch select{min-width:62px}.ui-language-corner{top:12px;right:12px}}`;
  document.head.append(style);
}

// Native dialogs are interface copy; their return values and caller behavior stay unchanged.
const nativeConfirm = window.confirm.bind(window);
const nativePrompt = window.prompt.bind(window);
window.confirm = message => nativeConfirm(render(String(message)));
window.prompt = (message, value) => nativePrompt(render(String(message)), value);

window.addEventListener('storage', event => {
  if (event.key === storageKey) setLanguage(event.newValue === 'en' ? 'en' : 'zh-CN', false);
});
function start(): void { installSwitcher(); setLanguage(language, false); }
if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start, { once: true });
else start();
