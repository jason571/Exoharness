import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { browser, until } from './support/browser.mjs';

const root = resolve(import.meta.dirname, '../..');
let healthy = true;
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/v1/auth/session') {
      await new Promise(resolve => setTimeout(resolve, 100));
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify({ role: 'admin', authentication_enabled: true }));
    } else if (req.url === '/health') {
      await new Promise(resolve => setTimeout(resolve, 200));
      res.writeHead(healthy ? 200 : 503, { 'content-type': 'application/json' });
      res.end(JSON.stringify(healthy ? { status: 'ok' } : { error: { message: 'offline' } }));
    } else if (req.url?.startsWith('/assets/')) {
      const name = req.url.slice('/assets/'.length);
      if (!['auth.js', 'imports.js', 'styles.css', 'i18n.js'].includes(name)) { res.writeHead(404).end(); return; }
      res.setHeader('content-type', name.endsWith('.css') ? 'text/css' : 'text/javascript');
      res.end(await readFile(resolve(root, 'web', name)));
    } else if (req.url === '/imports') {
      res.setHeader('content-type', 'text/html');
      res.end(await readFile(resolve(root, 'web/imports.html')));
    } else res.writeHead(404).end();
  } catch { res.writeHead(500).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const chrome = await browser();
const change = language => chrome.evaluate(`document.querySelector('.ui-language-switch select').value=${JSON.stringify(language)};document.querySelector('.ui-language-switch select').dispatchEvent(new Event('change'));`);
try {
  for (const width of [1920, 1440, 1280, 390]) {
    await chrome.send('Emulation.setDeviceMetricsOverride', { width, height: 900, deviceScaleFactor: 1, mobile: width === 390 });
    for (const online of [true, false]) {
      healthy = online;
      await chrome.send('Page.navigate', { url: `${base}/imports` });
      await until(() => chrome.evaluate('!!document.querySelector(".ui-language-switch select")'), 'language selector');
      await change('en');
      const englishStatus = online ? 'Service online' : 'Service unavailable';
      await until(() => chrome.evaluate(`document.querySelector('#service-label').textContent === ${JSON.stringify(englishStatus)}`), 'asynchronous service state is translated');
      const state = await chrome.evaluate(`(()=>{const label=document.querySelector('#service-label');const rect=label.getBoundingClientRect();const style=getComputedStyle(label);return {whiteSpace:style.whiteSpace,height:rect.height,width:rect.width,scrollWidth:label.scrollWidth,visible:getComputedStyle(label.parentElement).display !== 'none',text:label.textContent};})()`);
      assert.equal(state.whiteSpace, 'nowrap', 'status text explicitly stays on one line');
      if (state.visible) {
        assert.ok(state.height > 0 && state.height <= 24, `status never becomes vertical: ${JSON.stringify(state)}`);
        assert.ok(state.width >= state.scrollWidth - 1, `English status remains fully visible: ${JSON.stringify(state)}`);
      }
      await change('zh-CN');
      assert.equal(await chrome.evaluate('document.querySelector("#service-label").textContent'), online ? '服务在线' : '服务不可用');
      await change('en');
      await chrome.send('Page.reload');
      await until(() => chrome.evaluate(`document.documentElement.lang === 'en' && document.querySelector('#service-label')?.textContent === ${JSON.stringify(englishStatus)}`), 'English service state after reload');
    }
  }
  assert.deepEqual(chrome.errors, []);
  console.log('PASS: real import/auth/health scripts; async online/offline translation; one-line complete status; language persistence; four viewport widths.');
} finally {
  await chrome.close();
  await new Promise(resolve => server.close(resolve));
}
