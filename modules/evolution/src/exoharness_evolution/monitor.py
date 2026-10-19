# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Bounded rolling observation of opted-in owners and an exact Skill revision."""
from urllib.parse import urlencode
from exoharness_benchmark.evidence import write_private
from exoharness_benchmark.metrics import usage


def observe(publisher, policy, ledger, *, page_limit=2):
    publication = ledger.state['publications'][-1] if ledger.state['publications'] else None
    owners = set(policy.value['monitor_user_ids'])
    if not publication or not owners:
        return {'status': 'not_configured', 'samples': 0}
    revision = publication['revision']
    observations = ledger.state['monitor'].setdefault(str(revision), {})
    # Inspect recent pages only. Monitoring is a rolling guard, not an unbounded
    # replay or an unbiased benchmark. Keep at most 1,000 numeric observations.
    for page in range(min(page_limit, 10)):
        sessions = publisher.client.api('/v1/trace/sessions?' + urlencode({'limit': 50, 'offset': page * 50}))
        for session in sessions:
            if session.get('owner_user_id') not in owners:
                continue
            runs = publisher.client.api('/v1/trace/runs?' + urlencode({'session_id': session['id'], 'limit': 50}))
            for run in runs:
                if run['id'] in observations or run.get('status') not in ('succeeded', 'failed', 'recoverable'):
                    continue
                trace = publisher.client.api('/v1/trace/runs/' + run['id'] + '?event_limit=1')
                read = any(i.get('tool_name') == 'read_skill'
                    and (i.get('arguments') or {}).get('id') == policy.skill_id
                    and (i.get('result') or {}).get('revision') == revision
                    and not (i.get('result') or {}).get('isError') for i in trace['invocations'])
                if read:
                    measured = usage(trace['model_calls'])
                    observations[run['id']] = {'succeeded': run['status'] == 'succeeded',
                        'tokens': None if measured['unknown_model_calls'] else measured['total_tokens'],
                        'created_at': run.get('created_at') if type(run.get('created_at')) is int else 0}
        if len(sessions) < 50:
            break
    # Preserve the newest observations, and never retain prompts or tool payloads.
    if len(observations) > 1000:
        newest = sorted(observations.items(), key=lambda item: (item[1].get('created_at', 0), item[0]), reverse=True)[:1000]
        observations.clear(); observations.update(newest)
    values = list(observations.values()); failed = sum(not v['succeeded'] for v in values)
    known = [v['tokens'] for v in values if type(v['tokens']) is int]
    enough = len(values) >= policy.value['monitor_min_samples']
    regression = enough and (failed / len(values) > policy.value['monitor_failure_rate']
        or len(known) != len(values) or sum(known) / len(known) > policy.value['monitor_max_tokens'])
    result = {'status': 'regression' if regression else 'observing', 'samples': len(values),
              'failures': failed, 'mean_tokens': sum(known) / len(known) if known else None,
              'skill_revision': revision, 'suspended': False, 'sampling': 'bounded_recent_window'}
    ledger.save()
    if regression and policy.value['auto_suspend']:
        intents = ledger.state.setdefault('suspensions', {})
        intent = intents.setdefault(str(revision), {'revision': revision, 'phase': 'planned'})
        record = publisher.record()
        if record and record['source_digest'] != publication['identity']['source_digest']:
            raise ValueError('Published package changed; do not suspend an unrelated package')
        if record and record['revision'] == revision and record['enabled']:
            intent['phase'] = 'submitting'; ledger.save()
            record = publisher.suspend(revision, publication['identity']['source_digest'])
        elif record and record['enabled']:
            raise ValueError('Published Skill changed; do not suspend an unrelated revision')
        if record and not record['enabled']:
            # Suspension increments the registry revision once. A lost reply can
            # be reconciled only with this intent and the exact source package.
            if record['source_digest'] != publication['identity']['source_digest']:
                raise ValueError('Disabled Skill is a different package')
            intent.update(phase='confirmed', disabled_revision=record['revision']); ledger.save()
            result['suspended'] = True
    write_private(ledger.root / 'monitor.json', result)
    return result
