// Original administrator forms. Only the UUID-named test library is mutated.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {randomUUID} from 'node:crypto';
import {resolve} from 'node:path';
import {browser,until} from './support/browser.mjs';

const data=execFileSync('python3',['scripts/local-service.py','data-path'],{encoding:'utf8'}).trim();
const credentials=JSON.parse(await readFile(resolve(data,'credentials.json'),'utf8'));
const token=credentials.find(record=>record.id==='admin').token;
const name='Browser library '+randomUUID();
const chrome=await browser();
const fixtureSource=await readFile('modules/api/tests/support/document_fixtures.py','utf8');
const fixtures=JSON.parse(execFileSync('docker',['run','--rm','--pull=never','--read-only','--network','none','--cap-drop','ALL','--security-opt','no-new-privileges','--user','1000:1000','--memory','536870912','--pids-limit','64','--entrypoint','python','-i','skill-workspace:typescript-20260913','-B','-','fixtures'],{input:fixtureSource,encoding:'utf8',timeout:60000}));
try {
  const {send,evaluate}=chrome;
  await send('Page.navigate',{url:'http://127.0.0.1:9797/exoharness-login'});
  await until(()=>evaluate("document.readyState==='complete'&&!!document.querySelector('#login-password')"),'administrator login');
  await evaluate(`document.querySelector('#login-password').value=${JSON.stringify(token)};document.querySelector('#login-form').requestSubmit()`);
  await until(()=>evaluate("location.pathname==='/overview'&&document.body.dataset.authRole==='admin'"),'administrator authenticated');
  await send('Page.navigate',{url:'http://127.0.0.1:9797/knowledge'});
  await until(()=>evaluate("!!window.ExoAuth&&document.querySelector('#library-list')?.childElementCount>0"),'library page');
  await evaluate(`document.querySelector('#library-new').click();document.querySelector('#library-name').value=${JSON.stringify(name)};document.querySelector('#library-form').requestSubmit()`);
  await until(()=>evaluate(`document.querySelector('#library-name')?.value===${JSON.stringify(name)}&&!!document.querySelector('#library-id')?.value&&!document.querySelector('#library-content').inert`),'library saved');
  const id=await evaluate("document.querySelector('#library-id').value");
  await evaluate("document.querySelector('#library-category-management').open=true;document.querySelector('#library-category-name').value='测试分类';document.querySelector('#library-category-form').requestSubmit()");
  await until(()=>evaluate("document.querySelector('#library-category-list')?.textContent?.includes('测试分类')"),'category saved');
  // A real browser File object passes through the original upload decoder.
  const content='文件导入超过64KiB仍须完整保存。'.repeat(4000);
  await evaluate(`(()=>{const input=document.querySelector('#library-file-import'),transfer=new DataTransfer();transfer.items.add(new File([${JSON.stringify(content)}],'browser-upload.txt',{type:'text/plain'}));input.files=transfer.files;input.dispatchEvent(new Event('change',{bubbles:true}));})()`);
  await until(()=>evaluate("document.querySelector('#library-document-list')?.textContent?.includes('browser-upload')"),'original file import',20000);
  const documents=await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/knowledge/libraries/${id}/documents');return response.json()})()`);
  assert.equal(documents.length,1);
  const detail=await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/knowledge/libraries/${id}/documents/${documents[0].id}');return response.json()})()`);
  assert.equal(detail.content,content);
  for(const [kind,filename] of [['docx','browser-table.docx'],['pdf','browser-layout.pdf']]) {
    await evaluate(`(()=>{const input=document.querySelector('#library-file-import'),transfer=new DataTransfer();const bytes=Uint8Array.from(atob(${JSON.stringify(fixtures[kind])}),c=>c.charCodeAt(0));transfer.items.add(new File([bytes],${JSON.stringify(filename)}));input.files=transfer.files;input.dispatchEvent(new Event('change',{bubbles:true}));})()`);
    await until(()=>evaluate(`document.querySelector('#library-document-list')?.textContent?.includes(${JSON.stringify(filename)})`),'binary document imported',60000);
  }
  assert.match(await evaluate("document.querySelector('#library-file-message').textContent"),/PDF 表格行列未验证/);
  const imported=await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/knowledge/libraries/${id}/documents');return response.json()})()`);
  assert.equal(imported.length,3);
  const word=imported.find(d=>d.title==='browser-table.docx');
  const manifest=await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/knowledge/libraries/${id}/documents/${word.id}/blocks');return response.json()})()`);
  assert.equal(manifest.blocks[1].row_count,200);
  assert.equal(manifest.source_file.filename,'browser-table.docx');
  assert.equal('path' in manifest.source_file,false);
  const view=await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/knowledge/libraries/${id}/documents/${word.id}/blocks/table-1?revision=1&offset=2&limit=1');return response.json()})()`);
  assert.equal(view.block.partial,true);
  assert.equal(view.block.merged_anchors[0].text,'农业');
  assert.equal(view.block.headers[0][0].text,'名称与金额（万元）');
  await until(()=>evaluate("!!document.querySelector('#library-assistants input[value=xiaolan]')"),'enabled assistant grant option');
  await evaluate("document.querySelector('#library-assistants input[value=xiaolan]').checked=true;document.querySelector('#library-grants-save').click()");
  await until(()=>evaluate("document.querySelector('#library-message')?.textContent?.includes('助理授权已保存')"),'assistant grant saved');
  const library=await evaluate(`(async()=>{const response=await window.ExoAuth.fetch('/v1/knowledge/libraries/${id}');return response.json()})()`);
  assert.deepEqual(library.assistant_ids,['xiaolan']);
  await send('Page.reload');
  await until(()=>evaluate(`document.querySelector('#library-list')?.textContent?.includes(${JSON.stringify(name)})`),'library persisted refresh');
  assert.equal(await evaluate("document.querySelector('#knowledge-error')?.classList.contains('hidden')"),true);
  assert.deepEqual(chrome.errors,[]);
  console.log('PASS original administrator forms: plaintext, real DOCX/PDF File uploads, intact 200-row table and PDF warning, grants and refresh. Agent retrieval remains pending.');
} finally {
  try {
    const headers={authorization:'Bearer '+token};
    const response=await fetch('http://127.0.0.1:9797/v1/knowledge/libraries',{headers});
    if(!response.ok)throw new Error('test cleanup inventory unavailable');
    for(const library of await response.json()) {
      if(library.name!==name)continue;
      const deleted=await fetch('http://127.0.0.1:9797/v1/knowledge/libraries/'+encodeURIComponent(library.id),{method:'DELETE',headers});
      assert.equal(deleted.status,204);
    }
  } finally {await chrome.close();}
}
