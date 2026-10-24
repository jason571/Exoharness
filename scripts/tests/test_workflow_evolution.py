"""Reviewed legacy training evidence is read by Rust, never executed as code."""
import copy
import json
import shutil
import hashlib
from rust_eval_test_support import NativeEvaluationTest, FIXTURES


class NativeWorkflowTests(NativeEvaluationTest):
    def test_structured_host_rejections_keep_their_failure_class(self):
        def digest(value):
            return hashlib.sha256(json.dumps(value,sort_keys=True,ensure_ascii=False,separators=(',',':'),allow_nan=False).encode()).hexdigest()
        for flag,expected in (('_host_input_rejection','input_invalid'),('_host_authorization_denied','capability_denied')):
            with self.subTest(flag=flag):
                case=self.workflow();root=self.evidence();tp=root/'sample/trace-0.json';op=root/'sample/outcome.json';mp=root/'manifest.json'
                trace=json.loads(tp.read_text());trace['run']['status']='failed';trace['invocations'][0]['outcome_status']='rejected';trace['invocations'][0]['result'][flag]={'code':'rejected'}
                outcome=json.loads(op.read_text());outcome['state']='failed';outcome['turn_states']=['failed']
                manifest=json.loads(mp.read_text());sample=manifest['samples'][0];sample['delivery_passed']=False;sample['trace_sha256']=[digest(trace)];sample['outcome_sha256']=digest(outcome)
                for path,value in ((tp,trace),(op,outcome),(mp,manifest)):
                    path.write_text(json.dumps(value));path.chmod(0o600)
                result=self.validate('evolution','trajectories',case,extra=('--training-evidence',root));self.assertIn(expected,result['rows'][0]['failure_classes']);self.assertFalse(result['rows'][0]['verified_delivery']);shutil.rmtree(root)
    def evidence(self):
        root = self.root / 'evidence'; shutil.copytree(FIXTURES / 'training-evidence', root)
        for p in (root, *root.rglob('*')):
            p.chmod(0o700 if p.is_dir() else 0o600)
        return root

    def test_legacy_trajectory_identity_and_private_payload_omission(self):
        case = self.workflow(); root = self.evidence()
        result = self.validate('evolution', 'trajectories', case, extra=('--training-evidence', root))
        self.assertEqual(result['identity'], case['expected_trajectory_identity'])
        self.assertEqual(result['rows'], case['expected_trajectory_rows'])
        self.assertNotIn('PRIVATE SOURCE', json.dumps(result))

    def test_observed_tools_and_private_data_boundaries(self):
        case = self.workflow(); root = self.evidence()
        self.validate('evolution', 'candidate', case, extra=('--training-evidence', root))
        for key, value in (('tool', 'unobserved_tool'), ('action', 'Use Bearer PRIVATE'), ('expect', 'https://private.invalid')):
            with self.subTest(key=key):
                invalid = copy.deepcopy(case); invalid['candidate']['workflows'][0]['steps'][0][key] = value
                self.validate('evolution', 'candidate', invalid, success=False, extra=('--training-evidence', root))

    def test_changed_trace_and_owner_are_rejected(self):
        case = self.workflow(); root = self.evidence(); trace = root / 'sample/trace-0.json'
        value = json.loads(trace.read_text()); value['prompt'] = 'Changed prompt'
        trace.write_text(json.dumps(value)); trace.chmod(0o600)
        self.validate('evolution', 'trajectories', case, success=False, extra=('--training-evidence', root))
        shutil.rmtree(root); root = self.evidence(); case['policy']['scope'] = {'kind': 'user', 'user_id': 'other-owner'}
        self.validate('evolution', 'trajectories', case, success=False, extra=('--training-evidence', root))

    def test_symlink_evidence_is_refused(self):
        case = self.workflow(); root = self.evidence(); trace = root / 'sample/trace-0.json'
        saved = self.root / 'trace.json'; shutil.copy2(trace, saved); trace.unlink(); trace.symlink_to(saved)
        self.validate('evolution', 'trajectories', case, success=False, extra=('--training-evidence', root))
