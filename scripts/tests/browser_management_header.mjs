import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { browser, until } from './support/browser.mjs';

const root = resolve(import.meta.dirname, '../..');
const pages = { '/overview': 'overview.html', '/services': 'index.html', '/imports': 'imports.html', '/models': 'models.html', '/agents': 'agents.html', '/knowledge': 'knowledge.html', '/trace': 'trace.html', '/audit': 'audit.html', '/users': 'users.html', '/security': 'security.html', '/chat': 'portal-chat.html' };
const navigation = ['/overview', '/services', '/imports', '/models', '/agents', '/knowledge', '/trace', '/audit', '/users'];
let role = 'admin';
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/v1/auth/session') {
      // Reproduce account controls mounting after the language selector.
      await new Promise(resolve => setTimeout(resolve, 150));
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify({ role, authentication_enabled: true }));
    } else if (req.url?.startsWith('/assets/')) {
      const name = req.url.slice('/assets/'.length);
      if (!['auth.js', 'styles.css', 'management.css', 'management-theme.js', 'management-theme.css', 'i18n.js'].includes(name)) { res.writeHead(404).end(); return; }
      res.setHeader('content-type', name.endsWith('.css') ? 'text/css' : 'text/javascript');
      res.end(await readFile(resolve(root, 'web', name)));
    } else if (pages[req.url]) {
      const html = (await readFile(resolve(root, 'web', pages[req.url]), 'utf8'))
        .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, tag => ['/assets/auth.js', '/assets/i18n.js', '/assets/management-theme.js'].some(asset => tag.includes(asset)) ? tag : '');
      res.setHeader('content-type', 'text/html');
      res.end(html);
    } else res.writeHead(404).end();
  } catch { res.writeHead(500).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const chrome = await browser();
try {
  for (const width of [1920, 1440, 1280, 1181, 1024, 390]) {
    await chrome.send('Emulation.setDeviceMetricsOverride', { width, height: 900, deviceScaleFactor: 1, mobile: width === 390 });
    for (const language of ['zh-CN', 'en']) {
      let baseline;
      for (const path of Object.keys(pages).filter(path => path !== '/chat')) {
        await chrome.send('Page.navigate', { url: `${base}${path}` });
        await until(() => chrome.evaluate('!!document.querySelector(".ui-language-switch select") && !!document.querySelector(".auth-account")'), `${path}: async account`);
        await chrome.evaluate(`document.querySelector('.ui-language-switch select').value=${JSON.stringify(language)};document.querySelector('.ui-language-switch select').dispatchEvent(new Event('change'));`);
        if (width <= 1180) await until(() => chrome.evaluate(`document.querySelector('.main-nav').getAnimations().every(animation=>animation.playState!=='running')`), `${path}: drawer transition settled`);
        const layout = await chrome.evaluate(`(()=>{const rect=e=>{const r=e.getBoundingClientRect();return {x:r.x,y:r.y,width:r.width,height:r.height,right:r.right};};const header=document.querySelector('.topbar');const selector=document.querySelector('.ui-language-switch');const nav=header.querySelector('.main-nav');return {header:rect(header),nav:rect(nav),main:rect(document.querySelector('main.page')),language:rect(selector),account:rect(header.querySelector('.auth-account')),links:[...header.querySelectorAll('.main-nav>a')].map(e=>e.getAttribute('href')),active:header.querySelector('.main-nav>a.active')?.getAttribute('href'),linkSizes:[...header.querySelectorAll('.main-nav>a')].map(e=>rect(e)),navOverflow:nav.scrollWidth>nav.clientWidth,missingIcons:[...nav.querySelectorAll(':scope>a')].some(e=>!e.querySelector('svg')),overflow:document.documentElement.scrollWidth>innerWidth};})()`);
        assert.deepEqual(layout.links, navigation, `${path}: common menu order`);
        assert.equal(layout.active, path === '/security' ? undefined : path, `${path}: correct active item`);
        assert.ok(layout.language.right <= width && layout.language.x >= 0, `${path}: switch inside viewport`);
        assert.equal(layout.overflow, false, `${path}: no horizontal page overflow`);
        assert.equal(layout.missingIcons, false, `${path}: every menu item has an icon`);
        if (width > 1180) {
          const status = await chrome.evaluate(`(()=>{const service=document.querySelector('.management-header > .service-state');if(!service)return null;const r=service.getBoundingClientRect();const theme=document.querySelector('.management-theme-toggle').getBoundingClientRect();return {x:r.x,right:r.right,themeX:theme.x};})()`);
          if (status) {
            assert.ok(status.right <= layout.account.x && layout.account.x - status.right <= 17, `${path}: status sits next to the compact right action group`);
            assert.ok(layout.account.right <= status.themeX && status.themeX - layout.account.right <= 17, `${path}: account actions sit immediately before theme/language controls`);
          }
          assert.ok(layout.language.x >= layout.account.right, `${path}: language is after account`);
          assert.equal(layout.nav.x, 0, `${path}: navigation is on the left`);
          assert.equal(layout.nav.width, 216, `${path}: fixed expanded sidebar width`);
          assert.ok(layout.main.x >= layout.nav.right, `${path}: content clears sidebar`);
          assert.equal(layout.navOverflow, false, `${path}: no horizontal menu scrollbar`);
        }
        if (!baseline) baseline = layout;
        assert.deepEqual(layout.header, baseline.header, `${path}: common header geometry`);
        assert.deepEqual(layout.nav, baseline.nav, `${path}: common navigation geometry`);
        assert.deepEqual(layout.language, baseline.language, `${path}: common language geometry`);
        assert.deepEqual([layout.main.x, layout.main.width, layout.main.right], [baseline.main.x, baseline.main.width, baseline.main.right], `${path}: identical page content width and margins`);
        assert.deepEqual(layout.linkSizes.map(({width,height}) => ({width,height})), baseline.linkSizes.map(({width,height}) => ({width,height})), `${path}: common menu item sizes`);
        if (process.env.EXOHARNESS_HEADER_SCREENSHOT && width === 1440 && path === '/imports') {
          const shot = await chrome.send('Page.captureScreenshot', { format: 'png' });
          await writeFile(`${process.env.EXOHARNESS_HEADER_SCREENSHOT}-${language}.png`, Buffer.from(shot.data, 'base64'));
        }
      }
    }
  }
  await chrome.send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
  await chrome.send('Page.navigate', { url: `${base}/overview` });
  await until(() => chrome.evaluate('!!window.ExoAuth?.sidebar'), 'sidebar controls');
  await chrome.evaluate(`document.querySelector('.management-nav-collapse').click();`);
  for (const path of navigation) {
    await chrome.send('Page.navigate', { url: `${base}${path}` });
    await until(() => chrome.evaluate('!!window.ExoAuth?.sidebar'), 'persisted sidebar state');
    const collapsed = await chrome.evaluate(`(()=>{const nav=document.querySelector('.main-nav'),toggle=document.querySelector('.management-nav-collapse');return {width:nav.getBoundingClientRect().width,expanded:toggle.getAttribute('aria-expanded'),overflow:nav.scrollWidth>nav.clientWidth,labelsHidden:[...nav.querySelectorAll('.management-nav-label')].every(e=>getComputedStyle(e).display==='none'),accessible:[...nav.querySelectorAll(':scope>a')].every(e=>!!e.getAttribute('aria-label')&&!!e.title),accountInHeader:document.querySelector('.auth-account').parentElement===document.querySelector('.topbar'),languageInHeader:document.querySelector('.ui-language-switch').parentElement===document.querySelector('.topbar')};})()`);
    assert.equal(collapsed.width, 72, `${path}: collapsed width persists after navigation`);
    assert.equal(collapsed.expanded, 'false');
    assert.equal(collapsed.overflow, false);
    assert.equal(collapsed.labelsHidden, true);
    assert.equal(collapsed.accessible, true, 'icon links keep accessible names and tooltips');
    assert.equal(collapsed.accountInHeader, true, 'account actions stay at the top');
    assert.equal(collapsed.languageInHeader, true, 'language stays at the top');
  }
  await chrome.send('Page.bringToFront');
  await chrome.evaluate(`document.querySelector('.management-nav-collapse').focus();`);
  await chrome.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13, text: '\r' });
  await chrome.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Enter', code: 'Enter', windowsVirtualKeyCode: 13 });
  assert.equal(await chrome.evaluate(`document.querySelector('.management-nav-collapse').getAttribute('aria-expanded')`), 'true', 'keyboard expands sidebar');
  assert.equal(await chrome.evaluate(`document.querySelector('.main-nav').getBoundingClientRect().width`), 216);
  await chrome.evaluate(`document.querySelector('.management-nav-collapse').click();`);
  await chrome.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 760, deviceScaleFactor: 1, mobile: true });
  await chrome.send('Page.navigate', { url: `${base}/imports` });
  await until(() => chrome.evaluate('!!window.ExoAuth?.sidebar'), 'mobile sidebar');
  await chrome.evaluate(`document.querySelector('.mobile-nav-toggle').click();`);
  await until(() => chrome.evaluate(`document.querySelector('.main-nav').getBoundingClientRect().x>=0`), 'mobile drawer animation');
  assert.equal(await chrome.evaluate(`getComputedStyle(document.querySelector('.management-nav-collapse')).display`), 'none');
  assert.equal(await chrome.evaluate(`[...document.querySelectorAll('.main-nav>a .management-nav-label')].every(e=>getComputedStyle(e).display!=='none')`), true, 'desktop collapse does not hide mobile labels');
  assert.ok(Math.abs(await chrome.evaluate(`document.querySelector('.main-nav').getBoundingClientRect().width`) - 320) < .1, 'mobile width tolerates fractional transform pixels');
  await chrome.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
  assert.equal(await chrome.evaluate(`document.body.classList.contains('mobile-menu-open')`), false, 'Escape closes mobile drawer');
  // A delivery action added after language initialization also stays to its left.
  await chrome.send('Page.navigate', { url: `${base}/chat` });
  await until(() => chrome.evaluate('!!document.querySelector(".ui-language-switch")'), 'chat language selector');
  await chrome.evaluate(`const button=document.createElement('button');button.textContent='交付';button.className='delivery-toggle';document.querySelector('.chat-header-actions').append(button);`);
  assert.equal(await chrome.evaluate(`document.querySelector('.delivery-toggle').getBoundingClientRect().right <= document.querySelector('.ui-language-switch').getBoundingClientRect().x`), true, 'late chat action stays left of language');
  role = 'readonly';
  await chrome.send('Page.navigate', { url: `${base}/services` });
  await until(() => chrome.evaluate('window.ExoAuth?.session?.role === "readonly"'), 'read-only account');
  assert.deepEqual(await chrome.evaluate('[...document.querySelectorAll(".main-nav>a")].map(e=>e.getAttribute("href"))'), ['/services', '/imports', '/models', '/audit'], 'administrator-only links are not granted to read-only users');
  await chrome.evaluate(`document.querySelector('.mobile-nav-toggle').click();`);
  assert.equal(await chrome.evaluate('document.body.classList.contains("mobile-menu-open")'), true, 'mobile menu still opens');
  await chrome.evaluate(`document.querySelector('.mobile-nav-close').click();`);
  assert.equal(await chrome.evaluate('document.body.classList.contains("mobile-menu-open")'), false, 'mobile menu still closes');
  assert.deepEqual(chrome.errors, []);
  console.log('PASS: left sidebar across 10 pages, two languages and 6 widths; persistent collapse, keyboard, icon labels, fixed top controls, mobile drawer and read-only authorization.');
} finally {
  await chrome.close();
  await new Promise(resolve => server.close(resolve));
}
