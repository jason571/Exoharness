import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { createServer } from 'node:http';
import { resolve } from 'node:path';
import { browser, until } from './support/browser.mjs';

const root = resolve(import.meta.dirname, '../..');
const pages = (await readdir(resolve(root, 'web'))).filter(name => name.endsWith('.html') && !name.startsWith('local-'));
// Interface fixture: real markup and compiled language asset, without authentication/API side effects.
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/assets/i18n.js') {
      res.setHeader('content-type', 'text/javascript');
      res.end(await readFile(resolve(root, 'web/i18n.js')));
    } else if (req.url?.startsWith('/assets/')) {
      const name = req.url.slice('/assets/'.length);
      if (!['styles.css', 'management.css', 'management-theme.css'].includes(name)) { res.writeHead(404).end(); return; }
      res.setHeader('content-type', 'text/css');
      res.end(await readFile(resolve(root, 'web', name)));
    } else {
      const name = req.url?.slice(1);
      if (!pages.includes(name)) { res.writeHead(404).end(); return; }
      const html = (await readFile(resolve(root, 'web', name), 'utf8'))
        .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, tag => tag.includes('/assets/i18n.js') ? tag : '');
      res.setHeader('content-type', 'text/html');
      res.end(html);
    }
  } catch { res.writeHead(500).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const chrome = await browser();
const change = lang => chrome.evaluate(`document.querySelector('.ui-language-switch select').value=${JSON.stringify(lang)};document.querySelector('.ui-language-switch select').dispatchEvent(new Event('change'));`);
const missing = new Set();
try {
  for (const page of pages) {
    await chrome.send('Page.navigate', { url: `${base}/${page}` });
    await until(() => chrome.evaluate('!!document.querySelector(".ui-language-switch select")'), `${page} switch`);
    await change('zh-CN');
    const before = await chrome.evaluate(`({title:document.title,hints:[...document.querySelectorAll('[placeholder]')].map(e=>[e.id,e.getAttribute('placeholder')]),labels:[...document.querySelectorAll('button,nav a,h1,h2,label')].filter(e=>!e.closest('.ui-language-switch')).map(e=>e.textContent)})`);
    await change('en');
    if (page === 'users.html' || page === 'trace.html') {
      assert.equal(await chrome.evaluate('document.body.innerText.includes("Administrator view")'), false, `${page}: no redundant administrator-view badge`);
    }
    assert.equal(await chrome.evaluate('document.documentElement.lang'), 'en');
    assert.equal(await chrome.evaluate('localStorage.getItem("exoharness.ui-language")'), 'en');
    const untranslated = await chrome.evaluate(`(()=>{const values=[];const walker=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT);while(walker.nextNode()){const n=walker.currentNode;if(!n.parentElement.closest('pre,code,textarea,script,style,.ui-language-switch,#portal-user-name')){const s=n.data.trim();if(/[\u4e00-\u9fff]/.test(s))values.push(s);}}return values;})()`);
    untranslated.forEach(value => missing.add(value));
    await chrome.send('Page.reload');
    await until(() => chrome.evaluate('document.documentElement.lang === "en" && !!document.querySelector(".ui-language-switch")'), 'persisted language');
    await change('zh-CN');
    const after = await chrome.evaluate(`({title:document.title,hints:[...document.querySelectorAll('[placeholder]')].map(e=>[e.id,e.getAttribute('placeholder')]),labels:[...document.querySelectorAll('button,nav a,h1,h2,label')].filter(e=>!e.closest('.ui-language-switch')).map(e=>e.textContent)})`);
    assert.deepEqual(after, before, `${page}: reversible UI translation`);
  }
  await chrome.evaluate(`document.body.insertAdjacentHTML('beforeend','<div id="language-fixture"><button id="dynamic-label">正在读取</button><div class="rich-content" id="user-content">登录</div><div class="tool-progress-message" id="mcp-content">正在读取</div><pre id="trace-payload">运行总览</pre><div class="session-item-heading"><strong id="user-title">登录</strong></div><input id="user-value" value="密码" placeholder="密码"><select id="schema-choice"><option value="streamableHttp">连接 MCP</option></select><button id="event-button">保存配置</button></div>');window.fixtureClicks=0;document.querySelector('#event-button').addEventListener('click',()=>window.fixtureClicks++);`);
  await change('en');
  assert.equal(await chrome.evaluate('document.querySelector("#dynamic-label").textContent'), 'Loading');
  assert.deepEqual(await chrome.evaluate('["user-content","mcp-content","trace-payload","user-title"].map(id=>document.getElementById(id).textContent)'), ['登录', '正在读取', '运行总览', '登录']);
  assert.deepEqual(await chrome.evaluate('[document.querySelector("#user-value").value,document.querySelector("#user-value").placeholder,document.querySelector("#schema-choice").value]'), ['密码', 'Password', 'streamableHttp']);
  await chrome.evaluate('document.querySelector("#event-button").click();document.querySelector("#dynamic-label").textContent="已完成";');
  await until(() => chrome.evaluate('document.querySelector("#dynamic-label").textContent === "Completed"'), 'dynamic status update');
  assert.equal(await chrome.evaluate('window.fixtureClicks'), 1);
  const dynamicCopy = [
    ['活跃任务', 'Active tasks'],
    ['全部用户 · 更新于 21:38 · 当前任务按最新执行计数，用量包含历史重试 · 执行资源与能力检查为全局 · 每 30 秒更新', 'All users · Updated at 21:38 · Tasks count the latest run; usage includes retries · Resources and capability checks are global · Refreshes every 30 seconds'],
    ['第 2 轮 · 第 3 次执行', 'Turn 2 · Attempt 3'],
    ['20 项能力 · 1 项未就绪', '20 capabilities · 1 not ready'],
    ['MCP 服务 · v0.1.0 · 第 4 版', 'MCP service · v0.1.0 · Revision 4'],
    ['配置名称：测试配置\n接口：https://example.com/v1\n上下文 128,000 · 最大输出 16,384 tokens', 'Configuration: 测试配置\nEndpoint: https://example.com/v1\nContext 128,000 · Maximum output 16,384 tokens'],
    ['助理：小澜', 'Assistant: 小澜'],
    ['共 154 条记录', '154 records'],
    ['1 轮消息 · 54,295 Token', '1 turns · 54,295 tokens'],
    ['修改智能助理 · 小澜', 'Edit assistant · 小澜'],
    ['测试 API 连接', 'Test API connection'],
    ['无需额外权限', 'No additional permissions required'],
    ['邮件服务未配置', 'Mail service not configured'],
    ['邮件服务已连接', 'Mail service connected'],
    ['正在同步操作记录', 'Syncing audit records'],
    ['执行进度', 'Execution progress'],
    ['正在同步 Registry', 'Syncing registry'],
    ['正在同步会话记录', 'Syncing conversations'],
    ['暂无会话记录', 'No conversations yet'],
    ['上次保存：2026/9/4 17:57:11', 'Last saved: 2026/9/4 17:57:11'],
    ['41 条展示 · 256 条原始事件 · 当前事件 #1–#256 / 最新 #1303', '41 displayed · 256 raw events · Current events #1–#256 / Latest #1303'],
    ['7 次模型请求 · 7 次能力调用 · 已完成', '7 model requests · 7 capability calls · Completed'],
    ['准备调用（尚未提交） · read_skill', 'Invocation prepared (not submitted) · read_skill'],
    ['排队中 → 运行中', 'Queued → Running'],
    ['查看 16 条原始事件', 'View 16 raw events'],
    ['运行轨迹已同步', 'Trace synchronized'],
    ['读取 128 · 写入 未提供', 'Read 128 · Write Not provided'],
    ['系统说明 2 · 4,096 字节 · 已脱敏', 'System instructions 2 · 4,096 bytes · Redacted'],
    ['成功 / stop', 'Succeeded / stop'],
    ['未记录 / 未记录', 'Not recorded / Not recorded'],
    ['变换 3 · 技能资源去重 · 原 message 2', 'Transformation 3 · Deduplicated Skill resources · Original message 2'],
    ['相比上次请求：新增 2 项，移除 1 项，复用 3 项。逻辑输入可核验。 复用指日志存储去重，不代表模型没有接收或消耗 Token。 已记录 4 个上下文变换，可核验变换前后的来源。', 'Compared with the previous request: 2 added, 1 removed, 3 retained. Logical input is verifiable. Retention refers to journal deduplication, not avoided model input or tokens. 4 context transformations recorded with verifiable sources.'],
  ];
  await chrome.evaluate(`document.querySelector('#language-fixture').insertAdjacentHTML('beforeend','<div id="dynamic-management-copy"></div><div class="overview-task"><strong id="task-data">活跃任务</strong></div><div class="agent-list-copy"><span id="assistant-description">测试 API 连接</span></div><div class="model-item-title"><strong id="model-name">配置已保存</strong></div>');const target=document.querySelector('#dynamic-management-copy');for(const [source] of ${JSON.stringify(dynamicCopy)}){const label=document.createElement('span');label.textContent=source;target.append(label);}`);
  await until(() => chrome.evaluate('document.querySelector("#dynamic-management-copy span")?.textContent === "Active tasks"'), 'asynchronously loaded management labels');
  assert.deepEqual(await chrome.evaluate('[...document.querySelectorAll("#dynamic-management-copy span")].map(e=>e.textContent)'), dynamicCopy.map(([, expected])=>expected));
  assert.deepEqual(await chrome.evaluate('["task-data","assistant-description","model-name"].map(id=>document.getElementById(id).textContent)'), ['活跃任务','测试 API 连接','配置已保存'], 'UI words inside user-defined content remain untouched');
  await chrome.evaluate(`document.querySelector('#language-fixture').insertAdjacentHTML('beforeend','<div class="agent-list-copy"><span class="agent-list-heading"><strong>小澜</strong><small id="assistant-badge">默认</small></span></div><p class="trace-step-text" data-i18n="off" id="trace-evidence">运行状态 · 排队中</p>');`);
  await until(() => chrome.evaluate('document.querySelector("#assistant-badge").textContent === "Default"'), 'assistant badges are interface text, unlike names');
  assert.equal(await chrome.evaluate('document.querySelector("#trace-evidence").textContent'), '运行状态 · 排队中', 'raw event content remains unchanged');
  await change('zh-CN');
  assert.equal(await chrome.evaluate('document.querySelector("#dynamic-label").textContent'), '已完成');
  assert.deepEqual(await chrome.evaluate('[...document.querySelectorAll("#dynamic-management-copy span")].map(e=>e.textContent)'), dynamicCopy.map(([source])=>source), 'dynamic templates reverse without changing names or values');
  await chrome.send('Emulation.setDeviceMetricsOverride', { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
  for (const page of ['overview.html', 'portal-chat.html', 'portal-login.html', 'portal-register.html']) {
    await chrome.send('Page.navigate', { url: `${base}/${page}` });
    await until(() => chrome.evaluate('!!document.querySelector(".ui-language-switch select")'), 'mobile switch');
    const rect = await chrome.evaluate('(()=>{const r=document.querySelector(".ui-language-switch select").getBoundingClientRect();return {x:r.x,right:r.right,width:r.width,height:r.height};})()');
    assert.ok(rect.width >= 50 && rect.height >= 16 && rect.x >= 0 && rect.right <= 390, `${page}: mobile language control is visible: ${JSON.stringify(rect)}`);
  }
  assert.deepEqual(chrome.errors, []);
  console.log(`PASS: ${pages.length} pages; language persistence/reversal; dynamic labels; protected content; form values; click handlers; mobile controls.`);
  console.log(`UNTRANSLATED_STATIC: ${JSON.stringify([...missing])}`);
  assert.equal(missing.size, 0, 'All static interface text must be translated');
} finally {
  await chrome.close();
  await new Promise(resolve => server.close(resolve));
}
