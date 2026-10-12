#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Archive a verified native release allowlist; never include runtime data."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True, help='Release tag, for example v0.1.1')
    parser.add_argument('--release', type=Path, default=ROOT/'release')
    parser.add_argument('--output', type=Path, default=ROOT/'release/github')
    args = parser.parse_args()
    if not re.fullmatch(r'v\d+\.\d+\.\d+(?:-[a-z0-9.]+)?', args.version):
        parser.error('invalid version')
    release = args.release.resolve()
    manifest = json.loads((release/'build-manifest.json').read_text())
    if manifest['target'] != 'x86_64-unknown-linux-gnu':
        parser.error('this archive currently supports only Linux x86_64 GNU')
    allowed_docs = {'README.txt','README.zh-CN.txt','LICENSE','COMMERCIAL-LICENSE.md'}
    paths = []
    for entry in manifest['files']:
        name = PurePosixPath(entry['file'])
        if name.is_absolute() or '..' in name.parts or '__pycache__' in name.parts:
            parser.error('unsafe inventory path')
        if not (str(name) in allowed_docs or name.parts[0] in ('bin','web','scripts')):
            parser.error('inventory contains a file outside the package allowlist')
        source = release/str(name)
        if source.is_symlink() or not source.resolve().is_relative_to(release) or digest(source) != entry['sha256']:
            parser.error(f'inventory verification failed: {name}')
        paths.append(str(name))
    if len(paths) != len(set(paths)) or not allowed_docs.issubset(paths) or 'bin/exoharness' not in paths:
        parser.error('duplicate entries or required package files missing')
    symbols = subprocess.check_output(['objdump','-T',str(release/'bin/exoharness')],text=True)
    versions = re.findall(r'GLIBC_(\d+(?:\.\d+)+)', symbols)
    minimum = max(versions, key=lambda value: tuple(map(int,value.split('.'))))
    commit = subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    package = f'exoharness-{args.version}-linux-x86_64-gnu'
    info = {'tag':args.version,'source_commit':commit,'target':manifest['target'],
            'minimum_glibc':minimum,'license':'AGPL-3.0-only',
            'binary_sha256':digest(release/'bin/exoharness'),
            'external_dependencies':['Python 3.11+','PostgreSQL','Docker Engine for the bundled database launcher and OCI execution'],
            'includes_private_data':False}
    args.output.mkdir(parents=True,exist_ok=True)
    archive = args.output/(package+'.tar.gz')
    with tarfile.open(archive,'w:gz') as bundle:
        for name in sorted(paths+['build-manifest.json']):
            source = release/name
            header = bundle.gettarinfo(str(source),arcname=package+'/'+name)
            header.uid=header.gid=0;header.uname=header.gname='';header.mtime=0
            header.mode=0o755 if name.startswith('bin/') or name.endswith('.sh') else 0o644
            with source.open('rb') as stream:bundle.addfile(header,stream)
        content=(json.dumps(info,indent=2)+'\n').encode()
        header=tarfile.TarInfo(package+'/package-info.json');header.size=len(content);header.mode=0o644
        bundle.addfile(header,io.BytesIO(content))
    (args.output/'SHA256SUMS').write_text(f'{digest(archive)}  {archive.name}\n')
    print(json.dumps({'archive':str(archive),'files':len(paths)+2,'minimum_glibc':minimum,'sha256':digest(archive)}))

if __name__=='__main__':main()
