# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Benchmark-only declared domain routes; judges/reset stay on the control side."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import re
import threading
import urllib.error
import urllib.parse
import urllib.request
from .harness import NoRedirect, origin


class DomainGateway:
    def __init__(self, dataset, group, upstream, port=0):
        self.upstream = origin(upstream)
        dataset.path(group, 'test', '001')
        rules = dataset.read(dataset.groups / group / 'env/endpoints.txt').decode()
        self.routes = []
        for line in rules.splitlines():
            match = re.fullmatch(r'(GET|POST)\s+(/[^\s]*)', line.strip())
            if not match:
                continue
            method, path = match.groups()
            if re.search(r'judge|eval|gold|answer|reset|operator', path, re.I):
                continue
            parts = re.split(r'(\{[^{}]+\}|<[^<>]+>)', path.split('?', 1)[0])
            pattern = ''.join('[A-Za-z0-9_.-]+' if p.startswith(('{', '<')) else re.escape(p) for p in parts)
            self.routes.append((method, re.compile(pattern)))
        if not self.routes:
            raise ValueError('No released domain routes')
        self.opener = urllib.request.build_opener(NoRedirect)
        gateway = self
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self): self.forward('GET')
            def do_POST(self): self.forward('POST')
            def log_message(self, *args): pass
            def forward(self, method):
                decoded = urllib.parse.unquote(self.path)
                parsed = urllib.parse.urlsplit(decoded)
                if (not self.path.startswith('/') or parsed.scheme or parsed.netloc or '\\' in decoded
                        or '..' in parsed.path.split('/') or '%' in decoded
                        or not any(method == m and p.fullmatch(parsed.path) for m, p in gateway.routes)):
                    self.send_error(403); return
                try:
                    size = int(self.headers.get('Content-Length', '0'))
                    if not 0 <= size <= 1024 * 1024: raise ValueError('request too large')
                    body = self.rfile.read(size) if method == 'POST' else None
                    req = urllib.request.Request(gateway.upstream + self.path, data=body, method=method,
                                                 headers={'Content-Type': 'application/json'})
                    with gateway.opener.open(req, timeout=20) as response:
                        raw = response.read(8 * 1024 * 1024 + 1)
                        content_type = response.headers.get('Content-Type', 'application/json')
                    if len(raw) > 8 * 1024 * 1024: raise ValueError('response too large')
                    self.send_response(200); self.send_header('Content-Type', content_type)
                    self.send_header('Content-Length', str(len(raw))); self.end_headers(); self.wfile.write(raw)
                except Exception:
                    self.send_error(502, 'Domain environment unavailable')
        self.server = ThreadingHTTPServer(('127.0.0.1', port), Handler)
        self.server.daemon_threads = True
        self.origin = 'http://127.0.0.1:' + str(self.server.server_port)

    def __enter__(self):
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True); self.thread.start()
        return self

    def __exit__(self, *args):
        self.server.shutdown(); self.server.server_close(); self.thread.join(timeout=3)

    def reset_owned_mutable(self):
        # Never exposed as a solver route. The caller must own this disposable
        # environment; do not use a shared production service as an environment.
        request = urllib.request.Request(self.upstream + '/api/operator/reset', data=b'{}',
            headers={'Content-Type': 'application/json'}, method='POST')
        with self.opener.open(request, timeout=20) as response:
            response.read(1024 * 1024)
