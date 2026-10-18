#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Build a private candidate projection fixture from saved evidence; never run tools.

Candidate checkpoints mirror the host adapter using the corrected producer
contract. New Skill instructions are explicitly a candidate revision, not a
mutation of the historical Run. This compares context bytes, not paid tokens.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re


def build(trace, index, parts, skill_dir):
    text = (skill_dir / 'SKILL.md').read_text()
    spec = json.loads((skill_dir / 'context-stages.json').read_text())
    tool_set = {}
    for request in index:
        for part in request['manifest']['parts']:
            if part['kind'] == 'tool':
                tool = parts[part['sha256']]['content']
                tool_set[tool['name']] = tool

    def resolve(raw):
        matches = [name for name in tool_set if name == raw or re.fullmatch('mcp_' + re.escape(raw) + r'_[a-f0-9]{16}', name)]
        assert len(matches) == 1, raw
        return matches[0]

    headings = list(re.finditer(r'^## .+$', text, re.M))
    stages = []
    workflow = set()
    for stage in spec['stages']:
        names = sorted(resolve(name) for name in stage['tools'])
        workflow.update(names)
        ranges = []
        for heading in stage['omit_sections']:
            matches = [(i, match) for i, match in enumerate(headings) if match.group() == heading]
            assert len(matches) == 1
            i, match = matches[0]
            end = headings[i + 1].start() if i + 1 < len(headings) else len(text)
            ranges.append([len(text[:match.start()].encode()), len(text[:end].encode())])
        stages.append({'tools': names, 'omit_ranges': sorted(ranges)})
    entries = []
    output = []
    for number, manifest in enumerate(index, 1):
        grouped = {kind: [] for kind in ['configuration', 'message', 'tool']}
        for part in sorted(manifest['manifest']['parts'], key=lambda p: (p['kind'], p['index'])):
            grouped[part['kind']].append(parts[part['sha256']]['content'])
        request = {**grouped['configuration'][0], 'messages': grouped['message'], 'tools': grouped['tool']}
        if number == 1:
            entries.append({'User': request['messages'][1]['text']})
        current = copy.deepcopy(entries)
        output.append({'entries': current, 'request': request})
        for invocation in trace['invocations']:
            match = re.search(r':model-(\d+)-tool-', invocation['id'])
            if not match or int(match.group(1)) != number:
                continue
            result = copy.deepcopy(invocation['result'])
            if invocation['tool_name'] == 'read_skill':
                result['content'] = text
                result['revision'] += 1
                result['_host_skill_context'] = {
                    'version': 1, 'id': result['id'], 'revision': result['revision'],
                    'content_sha256': hashlib.sha256(text.encode()).hexdigest(),
                    'entry_tools': sorted(resolve(n) for n in spec['entry_tools']),
                    'workflow_tools': sorted(workflow), 'stages': stages,
                }
            data = result.get('structuredContent', result)
            meta = data.get('_meta', {}).get('exoharness', {})
            directive = meta.get('workflow_continuation')
            if directive and directive.get('required_tool') == 'write_music_video_plan_overview':
                directive['context_scope'] = 'checkpoint'
                directive['retire_provider_history'] = False
            entries.append({'ToolExchange': {'invocation_id': invocation['id'],
                'tool': invocation['tool_name'], 'arguments': invocation['arguments'], 'result': result}})
            if directive and directive.get('context_scope') == 'checkpoint':
                names = [] if directive['status'] == 'complete' else [resolve(directive['required_tool'])]
                preserved = {path: data[path[1:]] for path in meta['context_preserve']}
                summary = {'isError': False, 'declared_checkpoint': meta['context_checkpoint'],
                    'preserved_values': preserved, '_host_workflow': {'version': 1,
                    'capability': invocation['capability_id'], 'revision': invocation['capability_revision'],
                    'directive': directive, 'tools': names,
                    'provider_tools': sorted(n for n in tool_set if n.startswith('mcp_'))}}
                if directive['status'] == 'complete':
                    summary['delivery'] = {k: v for k, v in data.items() if k != '_meta'}
                entries.append({'Checkpoint': {'invocation_id': invocation['id'],
                    'reference': 'urn:exoharness:invocation:' + invocation['id'], 'summary': summary}})
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['trace', 'index', 'parts', 'skill_dir', 'output']:
        parser.add_argument('--' + name.replace('_', '-'), type=Path, required=True)
    args = parser.parse_args()
    load = lambda path: json.loads(path.read_text())
    fixture = build(load(args.trace), load(args.index), load(args.parts), args.skill_dir)
    args.output.write_text(json.dumps(fixture, ensure_ascii=False))
    args.output.chmod(0o600)
    print(json.dumps({'candidate_requests': len(fixture), 'no_external_execution': True}))
