import assert from 'node:assert/strict';
import {test} from 'node:test';
import {executionNotice} from '../src/execution-notice.ts';

test('raw errors and forged messages never appear in a user notice',()=>{
  for(const value of [new Error('RuntimeError: private'), 'MCP 503', {code:'__proto__'}, {code:'constructor'}, {code:'unknown',message:'private'}, {code:'service_unavailable',message:'Traceback: private'}]) {
    const text=executionNotice(value,'en');assert.ok(!/private|RuntimeError|MCP|503|Traceback/.test(text));
  }
});
test('permission, availability and unknown outcome messages stay distinct in both languages',()=>{
  for(const code of ['capability_permission_required','service_unavailable','result_pending_verification','task_scope_limited','task_processing_limited','capability_limited']) {
    const en=executionNotice({code},'en');const zh=executionNotice({code},'zh-CN');
    assert.ok(en);assert.match(zh,/[\u4e00-\u9fff]/);assert.ok(!/[\u4e00-\u9fff]/.test(en));
  }
  assert.match(executionNotice({code:'capability_permission_required'},'en'),/administrator/);
  assert.match(executionNotice({code:'result_pending_verification'},'en'),/no resubmission/);
  assert.doesNotMatch(executionNotice({code:'task_processing_limited'},'en'),/administrator|permission|trace|error/i);
});
