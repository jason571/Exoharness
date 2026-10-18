import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { browser, until } from './support/browser.mjs';

const root = resolve(import.meta.dirname, '../..');
const pages = { '/overview': 'overview.html', '/services': 'index.html', '/imports': 'imports.html', '/models': 'models.html', '/agents': 'agents.html', '/external-applications': 'external-applications.html', '/external-conversations': 'external-conversations.html', '/knowledge': 'knowledge.html', '/trace': 'trace.html', '/audit': 'audit.html', '/users': 'users.html', '/security': 'security.html' };
const assets = ['auth.js', 'i18n.js', 'styles.css', 'management.css', 'management-theme.js', 'management-theme.css', 'external-applications.css'];
let blockStorage = false;
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/v1/auth/session') {
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify({ role: 'admin', authentication_enabled: true }));
    } else if (req.url?.startsWith('/assets/') && assets.includes(req.url.slice(8))) {
      const name = req.url.slice(8);
      res.setHeader('content-type', name.endsWith('.css') ? 'text/css' : 'text/javascript');
      res.end(await readFile(resolve(root, 'web', name)));
    } else if (pages[req.url]) {
      let html = (await readFile(resolve(root, 'web', pages[req.url]), 'utf8')).replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, tag => ['auth.js', 'i18n.js', 'management-theme.js'].some(name => tag.includes(`/assets/${name}`)) ? tag : '');
      // Capture the palette while parsing the head, before deferred auth can decorate it.
      html = html.replace('</head>', '<script>window.paletteBeforeAuth={management:document.documentElement.classList.contains("management-console"),theme:document.documentElement.dataset.managementTheme,background:getComputedStyle(document.documentElement).getPropertyValue("--bg").trim(),authReady:!!window.ExoAuth};</script></head>');
      html = html.replace('</body>', '<script>window.headerBeforeAuth={header:getComputedStyle(document.querySelector(".topbar")).backgroundColor,sidebar:getComputedStyle(document.querySelector(".main-nav")).backgroundColor,authReady:!!window.ExoAuth};</script></body>');
      if (blockStorage) html = html.replace('<head>', '<head><script>Storage.prototype.getItem=()=>{throw new Error("storage disabled")};Storage.prototype.setItem=()=>{throw new Error("storage disabled")};</script>');
      res.setHeader('content-type', 'text/html');
      res.end(html);
    } else res.writeHead(404).end();
  } catch { res.writeHead(500).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const chrome = await browser();
async function open(path) {
  await chrome.send('Page.navigate', { url: `${base}${path}` });
  await until(() => chrome.evaluate('!!document.querySelector(".management-theme-toggle") && !!document.querySelector(".ui-language-switch")'), `${path}: theme control`);
}
async function click(selector) {
  const point = await chrome.evaluate(`(()=>{const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2};})()`);
  await chrome.send('Input.dispatchMouseEvent', { type: 'mousePressed', button: 'left', clickCount: 1, ...point });
  await chrome.send('Input.dispatchMouseEvent', { type: 'mouseReleased', button: 'left', clickCount: 1, ...point });
}
async function checkConcurrencyFields(theme) {
  const fields = await chrome.evaluate(`(()=>{const rgb=color=>color.match(/[\\d.]+/g).slice(0,3).map(Number);const luminance=rgb=>rgb.map(v=>{v/=255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4}).reduce((sum,v,i)=>sum+v*[.2126,.7152,.0722][i],0);const contrast=(a,b)=>{a=luminance(rgb(a));b=luminance(rgb(b));return (Math.max(a,b)+.05)/(Math.min(a,b)+.05)};return [...document.querySelectorAll('.scheduler-limit-card')].map((card,index)=>{const label=card.querySelector('span'),note=card.querySelector('small'),input=card.querySelector('input');input.value=String(index?4:2);const field=getComputedStyle(input),background=getComputedStyle(card).backgroundColor,r=input.getBoundingClientRect();return {value:input.value,width:r.width,height:r.height,font:parseFloat(field.fontSize),labelFont:parseFloat(getComputedStyle(label).fontSize),noteFont:parseFloat(getComputedStyle(note).fontSize),numberContrast:contrast(field.color,field.backgroundColor),borderContrast:contrast(field.borderColor,field.backgroundColor),labelContrast:contrast(getComputedStyle(label).color,background),noteContrast:contrast(getComputedStyle(note).color,background),overflow:card.scrollWidth>card.clientWidth};});})()`);
  assert.equal(fields.length, 2, `${theme}: separate user and global limits`);
  for (const field of fields) {
    assert.ok(field.width >= 100 && field.height >= 60 && field.font >= 28, `${theme}: clearly sized numeric control`);
    assert.ok(field.labelFont >= 15 && field.noteFont >= 13, `${theme}: readable label and explanation ${JSON.stringify(field)}`);
    assert.ok(field.numberContrast >= 4.5 && field.labelContrast >= 4.5 && field.noteContrast >= 4.5, `${theme}: accessible text contrast ${JSON.stringify(field)}`);
    assert.ok(field.borderContrast >= 3, `${theme}: visible control boundary ${JSON.stringify(field)}`);
    assert.equal(field.overflow, false, `${theme}: no clipped field content`);
  }
}
async function checkTraceContrast() {
  await chrome.evaluate(`document.body.insertAdjacentHTML('beforeend', '<section id="trace-theme-fixture" class="panel"><p class="trace-prompt">Prompt</p><div class="trace-metrics"><div><strong>123</strong></div></div><div class="trace-flow"><div class="trace-flow-row"><span class="trace-flow-sequence">01</span><div class="trace-flow-track"><button class="trace-flow-node skill selected"><span class="trace-flow-icon">SK</span><span class="trace-flow-copy"><small>SKILL</small><strong>Example capability</strong><em>Completed</em></span></button></div></div></div><div class="trace-usage-row"><span class="trace-usage-value"><strong>100</strong></span></div><article class="trace-step"><div class="trace-step-copy"><p>Timeline message</p><time>12:00</time><details class="trace-event-data" open><summary>Event data</summary><pre>{}</pre></details></div></article></section><aside id="trace-theme-drawer" class="trace-detail-drawer open"><header class="trace-drawer-header"><h2>Invocation details</h2><span class="trace-detail-kind" data-kind="skill">SKILL</span></header><div class="trace-drawer-toolbar"><span>1 / 2</span></div><div class="trace-detail-tabs"><button class="active">Input</button></div><div class="trace-drawer-body"><dl class="trace-facts"><div><dt>Invocation ID</dt><dd>sample-call</dd></div></dl><section class="trace-json-section"><div><h3>Input</h3><button>Copy</button></div><pre>{}</pre></section></div></aside>');`);
  try {
    const samples = await chrome.evaluate(`(()=>{const rgb=c=>c.match(/[\\d.]+/g).slice(0,3).map(Number),lum=c=>rgb(c).map(v=>{v/=255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4}).reduce((sum,v,i)=>sum+v*[.2126,.7152,.0722][i],0);return [...document.querySelectorAll('#trace-theme-fixture p,#trace-theme-fixture strong,#trace-theme-fixture small,#trace-theme-fixture em,#trace-theme-fixture time,#trace-theme-fixture summary,#trace-theme-fixture pre,#trace-theme-fixture .trace-flow-icon,#trace-theme-fixture .trace-flow-sequence,#trace-theme-drawer h2,#trace-theme-drawer span,#trace-theme-drawer button,#trace-theme-drawer dt,#trace-theme-drawer dd,#trace-theme-drawer h3,#trace-theme-drawer pre')].map(e=>{let parent=e;while(getComputedStyle(parent).backgroundColor==='rgba(0, 0, 0, 0)')parent=parent.parentElement;const fg=lum(getComputedStyle(e).color),bg=lum(getComputedStyle(parent).backgroundColor);return {text:e.textContent,contrast:(Math.max(fg,bg)+.05)/(Math.min(fg,bg)+.05)};});})()`);
    for (const sample of samples) assert.ok(sample.contrast >= 4.5, `trace light text contrast: ${sample.text} (${sample.contrast})`);
    assert.equal(await chrome.evaluate('getComputedStyle(document.querySelector("#trace-theme-drawer")).backgroundColor'), 'rgb(255, 255, 255)', 'actual detail drawer uses a light surface');
    assert.equal(await chrome.evaluate('getComputedStyle(document.querySelector("#trace-theme-fixture .trace-flow")).backgroundColor'), 'rgb(247, 250, 248)', 'call path uses a light surface');
  } finally {
    await chrome.evaluate('document.querySelector("#trace-theme-fixture").remove();document.querySelector("#trace-theme-drawer").remove();');
  }
}
try {
  await chrome.send('Emulation.setDeviceMetricsOverride', { width: 1920, height: 900, deviceScaleFactor: 1, mobile: false });
  await open('/overview');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'dark', 'default theme is dark');
  await click('.management-theme-toggle');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'light', 'native pointer click changes theme');
  for (const path of Object.keys(pages)) {
    await open(path);
    assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'light', `${path}: preference persists`);
    assert.deepEqual(await chrome.evaluate('window.paletteBeforeAuth'), { management: true, theme: 'light', background: '#f4f7f6', authReady: false }, `${path}: light palette applies before authentication and body parsing`);
    assert.deepEqual(await chrome.evaluate('window.headerBeforeAuth'), { header: 'rgb(255, 255, 255)', sidebar: 'rgb(255, 255, 255)', authReady: false }, `${path}: header and sidebar use light backgrounds before authentication`);
    const palette = await chrome.evaluate(`(()=>{const header=document.querySelector('.topbar'),nav=document.querySelector('.main-nav'),button=document.querySelector('.management-theme-toggle'),language=document.querySelector('.ui-language-switch');return {header:getComputedStyle(header).backgroundColor,sidebar:getComputedStyle(nav).backgroundColor,foreground:getComputedStyle(document.documentElement).color,scheme:getComputedStyle(document.documentElement).colorScheme,pressed:button.getAttribute('aria-pressed'),position:button.getBoundingClientRect().right<=language.getBoundingClientRect().x,width:document.querySelector('main.page').getBoundingClientRect().width};})()`);
    assert.equal(palette.header, 'rgb(255, 255, 255)');
    assert.equal(palette.sidebar, 'rgb(255, 255, 255)');
    assert.equal(palette.foreground, 'rgb(28, 51, 41)');
    assert.equal(palette.scheme, 'light');
    assert.equal(palette.pressed, 'true');
    assert.equal(palette.position, true, 'theme is before the language switch');
    assert.equal(palette.width, 1480, 'shared content width replaces page-specific caps');
    if (path === '/security') await checkConcurrencyFields('light');
    if (path === '/trace') await checkTraceContrast();
    if (path === '/imports') {
      const previews = await chrome.evaluate(`[...document.querySelectorAll('.import-page-shell .selected-files,.import-page-shell .file-preview')].map(e=>({background:getComputedStyle(e).backgroundColor,color:getComputedStyle(e).color}))`);
      assert.ok(previews.length > 0, 'local import file preview panels exist');
      for (const preview of previews) assert.deepEqual(preview, { background: 'rgb(247, 250, 248)', color: 'rgb(28, 51, 41)' }, 'local project waiting and selected-file previews use the light palette');
      const contrast = await chrome.evaluate(`(()=>{const rgb=color=>color.match(/[\\d.]+/g).slice(0,3).map(Number);const luminance=rgb=>rgb.map(v=>{v/=255;return v<=.04045?v/12.92:((v+.055)/1.055)**2.4}).reduce((sum,v,i)=>sum+v*[.2126,.7152,.0722][i],0);const ratio=(a,b)=>{a=luminance(a);b=luminance(b);return (Math.max(a,b)+.05)/(Math.min(a,b)+.05)};return [...document.querySelectorAll('.import-kind-tab strong,.import-kind-tab small,.json-guide-copy code')].map(e=>({text:e.textContent,contrast:ratio(rgb(getComputedStyle(e).color),[255,255,255])}));})()`);
      for (const item of contrast) assert.ok(item.contrast >= 4.5, `readable light-theme label: ${item.text} (${item.contrast})`);
      assert.equal(await chrome.evaluate(`getComputedStyle(document.querySelector('.import-kind-tabs')).backgroundColor`), 'rgb(255, 255, 255)', 'import selector uses the light palette');
    }
    await chrome.evaluate(`document.querySelector('.ui-language-switch select').value='en';document.querySelector('.ui-language-switch select').dispatchEvent(new Event('change'));`);
    assert.equal(await chrome.evaluate('document.querySelector(".management-theme-toggle").getAttribute("aria-label")'), 'Switch to dark theme');
    if (process.env.EXOHARNESS_THEME_SCREENSHOT && path === '/imports') {
      const screenshot = await chrome.send('Page.captureScreenshot', { format: 'png' });
      await writeFile(`${process.env.EXOHARNESS_THEME_SCREENSHOT}.png`, Buffer.from(screenshot.data, 'base64'));
    }
  }
  await click('.management-nav-collapse');
  assert.equal(await chrome.evaluate('document.querySelector(".main-nav").getBoundingClientRect().width'), 72, 'collapse works in light theme');
  await click('.management-theme-toggle');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'dark');
  await open('/security');
  await checkConcurrencyFields('dark');
  await open('/imports');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'dark', 'dark preference persists');
  await chrome.evaluate(`dispatchEvent(new StorageEvent('storage',{key:'exoharness.management.theme',newValue:'light'}));`);
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'light', 'other-tab changes synchronize');
  await chrome.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 800, deviceScaleFactor: 1, mobile: true });
  await open('/imports');
  const mobile = await chrome.evaluate(`(()=>{const r=document.querySelector('.management-theme-toggle').getBoundingClientRect();return {x:r.x,right:r.right,width:innerWidth,overflow:document.documentElement.scrollWidth>390};})()`);
  assert.ok(mobile.x >= 0 && mobile.right <= 390 && mobile.width === 390);
  assert.equal(mobile.overflow, false, 'mobile theme control does not expand the viewport');
  await open('/security');
  await checkConcurrencyFields('mobile dark');
  await open('/imports');
  await click('.management-theme-toggle');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'light');
  blockStorage = true;
  await open('/imports');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'dark');
  await click('.management-theme-toggle');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'light', 'blocked storage does not disable switching');
  assert.deepEqual(chrome.errors, []);
  console.log('PASS: 10 management pages; uniform content width; native theme/collapse clicks; light/dark persistence; English labels; tab sync; mobile; disabled storage.');
} finally {
  await chrome.close();
  await new Promise(resolve => server.close(resolve));
}
