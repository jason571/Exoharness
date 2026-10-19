# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Atomic private experiment evidence, independent of execution adapters."""
import contextlib
import fcntl
import json
import os
from pathlib import Path
import tempfile


def write_private(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, mode=0o700, exist_ok=True)
    if path.is_symlink():
        raise ValueError('Refusing a symlink evidence path')
    fd, temporary = tempfile.mkstemp(prefix='.evaluation-', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as stream:
            json.dump(value, stream, ensure_ascii=False, indent=2, allow_nan=False)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


@contextlib.contextmanager
def exclusive(root):
    root = Path(root)
    if root.is_symlink():
        raise ValueError('Evolution workspace must not be a symlink')
    root.mkdir(parents=True, mode=0o700, exist_ok=True)
    if root.stat().st_mode & 0o077:
        raise ValueError('Evolution workspace must be private')
    path = root / '.lock'
    fd = os.open(path, os.O_CREAT | os.O_RDWR | getattr(os, 'O_NOFOLLOW', 0), 0o600)
    with os.fdopen(fd, 'r+') as stream:
        fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield
