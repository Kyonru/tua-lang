#!/usr/bin/env python3
"""Replay preserved protocol probes against target/release/tua without rewriting baseline evidence."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE = HERE.parent / 'devex-2026-11-06'
WORK = ROOT / 'target/audits/devex-fixed'
EVIDENCE = HERE / 'evidence'
EVIDENCE.mkdir(exist_ok=True)
for source in BASE.rglob('*.js'):
    dest = WORK / source.relative_to(BASE)
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, dest)
shutil.copy2(BASE / 'typing/cases.json', WORK / 'typing/cases.json')
commands = [
    ('typing', ['node', str(WORK / 'typing/run.js')], {}),
    ('completion', ['node', str(WORK / 'completion/probe.js')], {}),
    ('completion-omitted', ['node', str(WORK / 'completion/probe.js')], {'TUA_AUDIT_TRIGGER':'omit','TUA_AUDIT_OUT':str(WORK / 'completion/context-omitted.json')}),
    ('completion-plain', ['node', str(WORK / 'completion/probe.js')], {'TUA_AUDIT_SNIPPETS':'false','TUA_AUDIT_OUT':str(WORK / 'completion/no-snippet-support.json')}),
    ('completion-actions', ['node', str(WORK / 'completion/actions.js')], {}),
    ('completion-apply', ['node', str(WORK / 'completion/apply-edits.js')], {}),
    ('hover', ['node', str(HERE / 'independent-hover-replay.js')], {}),
]
for name in ['lifecycle','unicode','freshness','performance']:
    commands.append((name, ['node', str(WORK / f'workflow/{name}.js')], {}))
for name in ['platformer','arena','puzzle']:
    commands.append((f'game-{name}', ['node', str(HERE / f'games/{name}/probe.js')], {}))
results = []
started = time.time()
for name, command, env in commands:
    result = subprocess.run(command, cwd=ROOT, env={**os.environ, **env}, capture_output=True, text=True, timeout=300)
    (EVIDENCE / f'{name}.log').write_text(result.stdout + result.stderr)
    results.append({'name':name, 'command':command, 'environment':env, 'exit':result.returncode})
    print(name, result.returncode, flush=True)
# Structured protocol output is deliberately copied; original baseline is untouched.
for folder in ['typing','completion','workflow']:
    for source in (WORK / folder).glob('*.json'):
        if source.name == 'cases.json' or source.stat().st_mtime < started:
            continue
        dest = EVIDENCE / folder / source.name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, dest)
manifest = {'binary':str(ROOT / 'target/release/tua'), 'sha256':hashlib.sha256((ROOT / 'target/release/tua').read_bytes()).hexdigest(), 'commands':results, 'note':'Exit 0 means harness execution succeeded. Inspect recorded outcomes; negative language fixtures intentionally fail checking.'}
(EVIDENCE / 'replay-manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
raise SystemExit(int(any(result['exit'] for result in results)))
