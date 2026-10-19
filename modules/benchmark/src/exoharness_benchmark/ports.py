# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Execution boundary for real Harness adapters and isolated protocol fixtures."""
from pathlib import Path
from typing import Protocol


class BenchmarkHarness(Protocol):
    run_ceiling: int

    def fingerprint(self) -> str:
        """Return verified executable/model/profile/capability configuration identity."""
        ...

    def execute(self, directory: Path, prompts: list[str], fingerprint: str,
                attachments=()) -> dict:
        """Observe a durable attempt; an ambiguous submission must never be repeated."""
        ...
