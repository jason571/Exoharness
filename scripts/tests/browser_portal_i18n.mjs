import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';
import { browser, until } from './support/browser.mjs';

const root = resolve(import.meta.dirname, '../..');
const { build } = createRequire(resolve(root, 'web-console/package.json'))('esbuild');
// Compile the real UI component, without chat/model/authentication side effects.
const component = (await build({ entryPoints: [resolve(root, 'web-console/src/delivery-workbench.ts')], bundle: true, format: 'iife', globalName: 'WorkbenchForTest', write: false })).outputFiles[0].text;
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/assets/test-workbench.js') {
      res.setHeader('content-type', 'text/javascript');
      res.end(component);
    } else if (req.url?.startsWith('/assets/')) {
      const name = req.url.slice('/assets/'.length);
      if (!['i18n.js', 'styles.css'].includes(name)) { res.writeHead(404).end(); return; }
      res.setHeader('content-type', name.endsWith('.css') ? 'text/css' : 'text/javascript');
      res.end(await readFile(resolve(root, 'web', name)));
    } else if (req.url === '/chat') {
      const html = (await readFile(resolve(root, 'web/portal-chat.html'), 'utf8'))
        .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, tag => tag.includes('/assets/i18n.js') ? tag : '')
        .replace('</head>', '<script src="/assets/test-workbench.js" defer></script></head>');
      res.setHeader('content-type', 'text/html');
      res.end(html);
    } else res.writeHead(404).end();
  } catch { res.writeHead(500).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const chrome = await browser();
const change = language => chrome.evaluate(`document.querySelector('.ui-language-switch select').value=${JSON.stringify(language)};document.querySelector('.ui-language-switch select').dispatchEvent(new Event('change'));`);
try {
  for (const width of [1440, 390]) {
    await chrome.send('Emulation.setDeviceMetricsOverride', { width, height: 900, deviceScaleFactor: 1, mobile: width === 390 });
    await chrome.send('Page.navigate', { url: `${base}/chat` });
    await until(() => chrome.evaluate('!!window.WorkbenchForTest && !!document.querySelector(".ui-language-switch select")'), 'portal interface');
    await change('en');
    // Both controls update after language initialization, as in a real chat startup.
    await chrome.evaluate(`WorkbenchForTest.mountDeliveryWorkbench();document.querySelector('#service-label').textContent='服务在线';`);
    await until(() => chrome.evaluate('document.querySelector(".delivery-toggle").textContent === "Execution & delivery" && document.querySelector("#service-label").textContent === "Service online"'), 'dynamic portal interface is translated');
    await chrome.evaluate('document.querySelector(".delivery-toggle").click();');
    await until(() => chrome.evaluate('document.querySelector("#delivery-heading").textContent === "Execution & delivery" && !/[\u4e00-\u9fff]/.test(document.querySelector(".delivery-list").textContent)'), 'workbench dialog title and empty state');
    await change('zh-CN');
    assert.equal(await chrome.evaluate('document.querySelector("#delivery-heading").textContent'), '执行与交付');
    assert.equal(await chrome.evaluate('document.querySelector("#service-label").textContent'), '服务在线');
    await change('en');
    await chrome.send('Page.reload');
    await until(() => chrome.evaluate('document.documentElement.lang === "en" && !!window.WorkbenchForTest && !!document.querySelector(".ui-language-switch select")'), 'persisted English');
    await chrome.evaluate(`WorkbenchForTest.mountDeliveryWorkbench();document.querySelector('#service-label').textContent='服务在线';`);
    await until(() => chrome.evaluate('document.querySelector(".delivery-toggle").textContent === "Execution & delivery" && document.querySelector("#service-label").textContent === "Service online"'), 'portal English after reload');
  }
  assert.deepEqual(chrome.errors, []);
  console.log('PASS: real portal markup/workbench component; dynamic title/service state; dialog empty state; language reversal/reload; desktop/mobile.');
} finally {
  await chrome.close();
  await new Promise(resolve => server.close(resolve));
}
