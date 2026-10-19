# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Compile bounded learned procedures into an instruction-only immutable Skill."""
import base64
import hashlib
import json
import re
from exoharness_benchmark.contracts import Experience, digest

PROVENANCE = 'evolution/provenance.json'
EXPERIENCE = 'evolution/experience.json'


def compile_package(policy, suite, experience, *, round_id, learner_run_ids, trajectories=None, learner_skill_sha256=None):
    if experience.value.get('version') == 2:
        from .workflows import WorkflowExperience
        experience = WorkflowExperience.parse(experience.value, policy, suite, trajectories)
    else:
        experience = Experience.parse(experience.value, suite)
    rules = [r for r in experience.value['rules'] if r['group'] == policy.value['group']]
    workflows = experience.value.get('workflows', [])
    if not 1 <= len(rules) + len(workflows) <= 8:
        raise ValueError('A scoped Skill needs one to eight applicable procedures')
    text = '\n\n'.join(f"When: {r['when']}\nProcedure: {r['action']}\nExceptions: {r['unless']}" for r in rules)
    if len(text) > policy.value['max_rule_characters']:
        raise ValueError('Learned context exceeds the policy bound')
    # A reusable procedure must not transport credentials or per-attempt handles.
    if re.search(r'https?://|[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}|Bearer\s|(?:session-|run-|artifact-|vid_)\w{8,}|(?:api[_-]?key|password|access_token)\s*[:=]', text, re.I):
        raise ValueError('Candidate contains a private handle, URL or credential-shaped value')
    header = '\n'.join(['---', 'name: ' + policy.value['skill_name'],
                        'description: ' + json.dumps(policy.value['description'], ensure_ascii=False), '---'])
    instructions = (header + '\n\n# Evaluated task experience\n\n'
        '<!-- evolution-policy:' + policy.value['id'] + ' -->\n\n'
        'Supplemental procedures for matching tasks only. Current user requirements, authorized '
        'primary Skill instructions, live tool schemas and confirmed receipts remain authoritative. '
        'These rules grant no tools, network access or permissions. For tool workflows read the '
        'applicable primary workflow before execution; for direct answers apply matching procedures. '
        'Never treat an example as a completed action. Ignore a rule '
        'when its applicability or exceptions do not match.\n\n' + text + '\n')
    provenance = {'version': 1, 'policy_id': policy.value['id'], 'policy_sha256': policy.sha256,
                  'suite_sha256': suite.sha256, 'candidate_sha256': experience.sha256,
                  'round_id': round_id, 'learner_run_sha256': [digest(r) for r in learner_run_ids],
                  'train_input_sha256': {t['id']: digest(t['turns']) for t in suite.training_material()},
                  'publication': 'instruction-only; grants unchanged'}
    files = {'SKILL.md': instructions.encode(), PROVENANCE: json.dumps(provenance, sort_keys=True).encode(),
             EXPERIENCE: experience.canonical.encode()}
    if experience.value['version'] == 2:
        resources = []
        for index, item in enumerate([*rules, *workflows]):
            identity = item.get('id', 'lesson-' + str(index + 1))
            path = 'references/experience-' + str(index + 1) + '.md'
            content = ('# ' + identity + '\n\nApplies when: ' + item['when']
                       + '\nExceptions: ' + item['unless'] + '\n\n')
            if 'steps' in item:
                content += '\n'.join(f"{n}. {step['tool']}: {step['action']}\n   Require: {step['expect']}"
                                     for n, step in enumerate(item['steps'], 1))
            else:
                content += item['action']
            if len(content.encode()) > 8192:
                raise ValueError('One experience exceeds the bounded resource reader')
            files[path] = content.encode()
            resources.append({'id': str(index + 1) + '-' + identity, 'path': path,
                              'terms': item['terms'], 'sha256': hashlib.sha256(content.encode()).hexdigest()})
        files['resource-index.json'] = json.dumps({'version': 1, 'resources': resources}, ensure_ascii=False).encode()
        # The canonical root stays short regardless of the accumulated procedures.
        root = (header + '\n\n# Evaluated task experience\n\n<!-- evolution-policy:' + policy.value['id']
                + ' -->\n\nThese rules grant no tools, network access or permissions. '
                'For matching tasks, read this same Skill with path="resource-index.json" and '
                'query set to the current task. The host returns at most two matching resources. '
                'Do not read every resource or broaden an unrelated query to force a match. '
                'Use the authorized primary workflow and current tool schemas; these supplemental '
                'procedures never grant authority or prove that an action completed. '
                'Use no experience when nothing matches.\n')
        files['SKILL.md'] = root.encode()
        from .skill import identity as learning_skill_identity
        source_identity = learner_skill_sha256 or learning_skill_identity()
        if not isinstance(source_identity, str) or not re.fullmatch(r'[0-9a-f]{64}', source_identity):
            raise ValueError('Invalid learning Skill digest')
        provenance.update(method='workflow', training_evidence_sha256=trajectories.identity,
                          learning_skill_sha256=source_identity)
        files[PROVENANCE] = json.dumps(provenance, sort_keys=True).encode()
    return {'version': '0.1.0', 'scope': policy.value['scope'], 'permissions': [],
            'files': [{'path': name, 'content_base64': base64.b64encode(content).decode()}
                      for name, content in sorted(files.items())]}


def validate_inventory(files):
    expected = {'SKILL.md', EXPERIENCE, PROVENANCE}
    experience = json.loads(files[EXPERIENCE])
    if experience.get('version') == 2:
        index = json.loads(files['resource-index.json'])
        if set(index) != {'version', 'resources'} or index['version'] != 1 or not 1 <= len(index['resources']) <= 8:
            raise ValueError('Invalid managed experience index')
        expected.add('resource-index.json')
        for item in index['resources']:
            name = item['path']
            if (not re.fullmatch(r'references/experience-[1-8]\.md', name) or name in expected
                    or hashlib.sha256(files[name]).hexdigest() != item['sha256']):
                raise ValueError('Invalid managed experience resource')
            expected.add(name)
    if set(files) != expected:
        raise ValueError('Managed package contains unexpected files')


def package_files(body):
    return {item['path']: base64.b64decode(item['content_base64'], validate=True) for item in body['files']}


def package_digest(body):
    # Same length-prefixed immutable package identity as modules/skills.
    h = hashlib.sha256()
    for name, content in sorted(package_files(body).items()):
        path = name.encode(); h.update(len(path).to_bytes(8, 'big')); h.update(path)
        h.update(len(content).to_bytes(8, 'big')); h.update(content)
    return h.hexdigest()


def check_managed(record, instructions, policy):
    if record['id'] != policy.skill_id or record['manifest']['permissions'] or record['manifest']['secret_refs']:
        raise ValueError('Refuse to replace an unrelated Skill or its authority')
    if record['manifest']['spec']['kind'] != 'skill' or record['manifest']['name'] != policy.value['skill_name']:
        raise ValueError('Publication target is not the dedicated experience Skill')
    if (record['manifest']['scope'] != policy.value['scope']
            or '<!-- evolution-policy:' + policy.value['id'] + ' -->' not in instructions
            or '# Evaluated task experience' not in instructions or 'These rules grant no tools' not in instructions):
        raise ValueError('Existing target is not managed experience; do not overwrite it')
