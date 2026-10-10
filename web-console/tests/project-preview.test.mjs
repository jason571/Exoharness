import test from 'node:test';
import assert from 'node:assert/strict';
import {projectPreview} from '../src/project-preview.ts';
const download = new URL('https://media.example.test/artifacts/zip/website.zip?signature=x');
const path = `/projects/artifact-${'a'.repeat(64)}/${'b'.repeat(64)}/pages/index.html`;
test('signed static project accepts nested HTML without changing download', () => {
  assert.equal(projectPreview(download, `https://media.example.test${path}`).pathname, path);
  assert.equal(projectPreview(download, path).origin, download.origin);
  assert.equal(download.pathname, '/artifacts/zip/website.zip');
});
test('arbitrary providers, authenticated API, active schemes and unscoped paths never embed', () => {
  for (const value of [null, '', 'javascript:alert(1)', 'data:text/html,test', `https://evil.test${path}`, '/v1/auth/session', `${path}?x=y`, `${path}#fragment`, `/projects/artifact-${'a'.repeat(64)}/${'b'.repeat(64)}/%2findex.html`, path.replace('index.html','code.js')]) assert.equal(projectPreview(download,value),null);
});
