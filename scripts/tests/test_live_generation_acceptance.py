"""Acceptance harness input contracts; no model, media or account calls."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from live_generation_acceptance import acceptance_cases


class GenerationCaseTests(unittest.TestCase):
    def test_video_uses_the_exact_production_prompt_without_workflow_hints(self):
        cases = acceptance_cases(requested=["video"])
        self.assertEqual(cases["video"]["prompt"], "为开源的ExoHarness生成15S宣传视频，开源地址，https://github.com/jason571/Exoharness\n视频最后必须出现地址链接")

    def test_mv_requires_existing_audio_before_any_submission(self):
        with self.assertRaisesRegex(ValueError, "existing audio"):
            acceptance_cases(requested=["mv"])

    def test_mv_upload_preserves_source_and_does_not_request_songwriting(self):
        with tempfile.TemporaryDirectory() as directory:
            audio = Path(directory) / "existing.wav"
            audio.write_bytes(b"owned existing audio input")
            before = hashlib.sha256(audio.read_bytes()).hexdigest()
            cases = acceptance_cases(requested=["mv"], mv_audio=audio)
            self.assertEqual(cases["mv"]["attachment_paths"], [{"type": "audio", "path": str(audio.resolve())}])
            self.assertIn("保持原音轨", cases["mv"]["prompt"])
            self.assertIn("不生成歌曲、配乐或歌词", cases["mv"]["prompt"])
            self.assertEqual(hashlib.sha256(audio.read_bytes()).hexdigest(), before)

    def test_non_mv_cases_do_not_require_audio_and_music_has_explicit_lyrics(self):
        cases = acceptance_cases(requested=["music", "ppt", "web", "video"])
        self.assertIn("[Verse]", cases["music"]["prompt"])
        self.assertIn("不需要自动写词", cases["music"]["prompt"])
        self.assertNotIn("attachment_paths", cases["ppt"])

    def test_private_followup_cases_keep_exact_prompt_and_session(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cases.json"
            expected = {"web-resume": {"kind": "web", "prompt": "继续发布已有网页", "session_id": "owned-session"}}
            path.write_text(json.dumps(expected))
            self.assertEqual(acceptance_cases(path, ["web-resume"]), expected)

    def test_unknown_selection_and_unsafe_case_names_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "Unknown"):
            acceptance_cases(requested=["absent"])
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cases.json"
            path.write_text(json.dumps({"../auth": {"kind": "web", "prompt": "page"}}))
            with self.assertRaisesRegex(ValueError, "Invalid"):
                acceptance_cases(path)


if __name__ == "__main__":
    unittest.main()
