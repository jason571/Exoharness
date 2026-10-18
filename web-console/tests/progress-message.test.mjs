import assert from 'node:assert/strict';
import {progressMessage} from '../src/progress-message.ts';
for(const message of ['[sampling] ComfyUI 节点 10 采样进度 7/8','正在生成视频，已运行 125 秒',
  'Preparing references','Encoding video · elapsed: 95 seconds','正在排队，前面还有 2 个任务']) {
  assert.equal(progressMessage(message),message);
}
for(const message of [undefined,null,'','  ',123,'X'.repeat(4097),'汉'.repeat(1366),'normal\nTraceback',
  'RuntimeError: private','MCP 503','permission denied','任务执行失败','API_KEY=private',
  'Authorization: Bearer private','saved /home/private/output.mp4','Exception at node',
  'download https://example.test/file?signature=private','{"token":"private"}']) {
  assert.equal(progressMessage(message),'正在处理...');
}
assert.equal(progressMessage('  Encoding video  '),'Encoding video');
console.log('PASS ordinary Chinese/English progress preserved; missing/oversized messages and diagnostics safely hidden.');
