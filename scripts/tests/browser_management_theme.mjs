import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { browser, until } from './support/browser.mjs';

const root = resolve(import.meta.dirname, '../..');
const pages = { '/overview': 'overview.html', '/services': 'index.html', '/imports': 'imports.html', '/models': 'models.html', '/agents': 'agents.html', '/knowledge': 'knowledge.html', '/trace': 'trace.html', '/audit': 'audit.html', '/users': 'users.html', '/security': 'security.html' };
const assets = ['auth.js', 'i18n.js', 'styles.css', 'management.css', 'management-theme.js', 'management-theme.css'];
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
try {
  await chrome.send('Emulation.setDeviceMetricsOverride', { width: 1920, height: 900, deviceScaleFactor: 1, mobile: false });
  await open('/overview');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'dark', 'default theme is dark');
  await click('.management-theme-toggle');
  assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'light', 'native pointer click changes theme');
  for (const path of Object.keys(pages)) {
    await open(path);
    assert.equal(await chrome.evaluate('document.documentElement.dataset.managementTheme'), 'light', `${path}: preference persists`);
    const palette = await chrome.evaluate(`(()=>{const header=document.querySelector('.topbar'),nav=document.querySelector('.main-nav'),button=document.querySelector('.management-theme-toggle'),language=document.querySelector('.ui-language-switch');return {header:getComputedStyle(header).backgroundColor,sidebar:getComputedStyle(nav).backgroundColor,foreground:getComputedStyle(document.documentElement).color,scheme:getComputedStyle(document.documentElement).colorScheme,pressed:button.getAttribute('aria-pressed'),position:button.getBoundingClientRect().right<=language.getBoundingClientRect().x,width:document.querySelector('main.page').getBoundingClientRect().width};})()`);
    assert.equal(palette.header, 'rgb(255, 255, 255)');
    assert.equal(palette.sidebar, 'rgb(255, 255, 255)');
    assert.equal(palette.foreground, 'rgb(28, 51, 41)');
    assert.equal(palette.scheme, 'light');
    assert.equal(palette.pressed, 'true');
    assert.equal(palette.position, true, 'theme is before the language switch');
    assert.equal(palette.width, 1480, 'shared content width replaces page-specific caps');
    if (path === '/security') await checkConcurrencyFields('light');
    if (path === '/imports') {
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
