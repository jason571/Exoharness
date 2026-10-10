import importlib.util
import json
import urllib.request
import time
from pathlib import Path

root = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('live', root / 'scripts/tests/live_skills.py')
live = importlib.util.module_from_spec(spec)
spec.loader.exec_module(live)
status, catalog = live.request('GET', '/v1/model-catalog')
assert status == 200
name = catalog.get('default_models', {}).get('llm') or catalog.get('default_model')
selected = next(model for model in catalog['models'] if model['name'] == name)
assert selected.get('thinking') == 'none', 'test requires configured thinking off'
status, session = live.request('POST', '/v1/sessions', {'title': None, 'actor': 'admin'})
assert status == 201, (status, session)
sid = session['id']
run = None
try:
    status, turn = live.request('POST', f'/v1/sessions/{sid}/turns', {'prompt': '请用四句话简单介绍春夏秋冬，每句约20字，只输出介绍正文，不要工具调用。', 'model': None, 'attachments': [], 'capability_ids': [], 'actor': 'admin'})
    assert status == 201, (status, turn)
    run = turn['run_id']
    request = urllib.request.Request(f'http://127.0.0.1:9797/v1/runs/{run}/events', headers={'Authorization': 'Bearer ' + live.key})
    terminal = None
    deltas = []
    started = time.monotonic()
    first = None
    with urllib.request.urlopen(request, timeout=90) as stream:
        for raw in stream:
            if raw.startswith(b'data: '):
                event = json.loads(raw[6:])
                if event.get('type') == 'text_delta':
                    deltas.append(event['delta'])
                    if first is None:
                        first = time.monotonic() - started
                if event.get('type') in ('complete', 'failed', 'cancelled'):
                    terminal = event
                    break
    assert terminal and terminal['type'] == 'complete', terminal
    response = terminal['response']
    assert len(deltas) > 1, {'delta_count': len(deltas), 'terminal': terminal}
    assert ''.join(deltas) == response['answer']
    status, history = live.request('GET', f'/v1/sessions/{sid}')
    assert status == 200 and history['title'] != '新对话', history.get('title')
    assert history['title'].startswith('请用四句话简单介绍春夏秋冬')
    status, renamed = live.request('PATCH', f'/v1/sessions/{sid}', {'title': '流式测试手动名称'})
    assert status == 200 and renamed['title'] == '流式测试手动名称'
    print(json.dumps({'passed': True, 'delta_count': len(deltas), 'first_delta_seconds': round(first, 3), 'complete_seconds': round(time.monotonic() - started, 3), 'auto_title': history['title'], 'manual_title_preserved': True, 'usage': response.get('usage'), 'tool_calls': len(response.get('tool_calls', []))}, ensure_ascii=False))
finally:
    if run:
        live.request('POST', f'/v1/runs/{run}/cancel', {})
    cleanup = urllib.request.Request(f'http://127.0.0.1:9797/v1/sessions/{sid}', method='DELETE', data=b'{}', headers={'Authorization': 'Bearer ' + live.key, 'Content-Type': 'application/json'})
    with urllib.request.urlopen(cleanup, timeout=20) as deleted:
        assert deleted.status == 204
