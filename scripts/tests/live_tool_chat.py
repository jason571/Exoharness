"""Explicit real configured model + registered Skill + OCI + PPTX delivery.
Not part of the offline gate; no media-model or image-generation calls allowed.
"""
import importlib.util
import io
import json
from pathlib import Path
import time
import urllib.request
import zipfile

root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('live', root / 'scripts/tests/live_skills.py')
live = importlib.util.module_from_spec(spec)
spec.loader.exec_module(live)
status, session = live.request('POST', '/v1/sessions', {'title': '临时插件闭环验证'})
assert status == 201
sid = session['id']
run = None
started = time.monotonic()
try:
    prompt = '请实际生成一个只有1页的中文PPTX文件：标题“ExoHarness测试”，正文三条“插件接入”“沙盒执行”“文件交付”。查找并读取已登记的pptx技能，使用当前真实工具Schema在沙盒内执行代码。只交付final.pptx，不需要图片、网络搜索、音频或视频，不调用任何付费媒体生成。'
    status, result = live.request('POST', f'/v1/sessions/{sid}/turns', {'prompt': prompt, 'model': None, 'attachments': [], 'capability_ids': []})
    assert status == 201, status
    run = result['run_id']
    request = urllib.request.Request(f'http://127.0.0.1:9797/v1/runs/{run}/events', headers={'Authorization': 'Bearer ' + live.key})
    terminal = None
    calls = []
    with urllib.request.urlopen(request, timeout=180) as stream:
        for line in stream:
            if not line.startswith(b'data: '):
                continue
            event = json.loads(line[6:])
            if event.get('type') == 'tool_call':
                calls.append(event['tool'])
            if time.monotonic() - started > 180:
                raise TimeoutError('real model workflow exceeded the validation deadline')
            if event.get('type') in ('complete', 'failed', 'cancelled'):
                terminal = event
                break
    assert terminal and terminal['type'] == 'complete', {'run': run, 'status': terminal.get('type') if terminal else None, 'message': terminal.get('message') if terminal else None, 'calls': calls}
    response = terminal['response']
    artifacts = response.get('artifacts', [])
    pptx = next(a for a in artifacts if a['filename'].endswith('.pptx'))
    with urllib.request.urlopen(pptx['download_url'], timeout=30) as downloaded:
        assert downloaded.status == 200
        data = downloaded.read(5 * 1024 * 1024 + 1)
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        slides = [n for n in archive.namelist() if n.startswith('ppt/slides/slide') and n.endswith('.xml')]
        assert len(slides) == 1, slides
        assert 'ExoHarness测试' in archive.read(slides[0]).decode('utf-8')
    assert 'read_skill' in calls and 'sandbox_run' in calls, calls
    print(json.dumps({'passed': True, 'run_id': run, 'calls': calls, 'seconds': round(time.monotonic() - started, 2), 'pptx_bytes': len(data), 'slides': len(slides), 'usage': response.get('usage')}, ensure_ascii=False))
finally:
    if run:
        live.request('POST', f'/v1/runs/{run}/cancel', {})
    request = urllib.request.Request(f'http://127.0.0.1:9797/v1/sessions/{sid}', method='DELETE', data=b'{}', headers={'Authorization': 'Bearer ' + live.key, 'Content-Type': 'application/json'})
    with urllib.request.urlopen(request, timeout=20) as deleted:
        assert deleted.status == 204
