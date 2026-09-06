#!/usr/bin/env python3
import json
from pathlib import Path

path = Path('projects/manifest.json')
data = json.loads(path.read_text())
projects = data['projects']
matches = [p for p in projects if p.get('name') == 'mini-container-runtime']
if len(matches) != 1:
    raise SystemExit(f'expected one mini-container-runtime entry, got {len(matches)}')
project = matches[0]
if project.get('status') != 'hold':
    raise SystemExit(f'unexpected prior status: {project.get("status")!r}')
if project.get('observed_main_sha') != 'b660e8d14aebf181e29ad844c18f7133ad0334ea':
    raise SystemExit('manifest source checkpoint drifted before READY transition')
project.update({
    'status': 'ready-for-import',
    'observed_main_sha': '3b96aca6d23289147fe1f21132a4503edaf19a06',
    'source_ci_run_id': 34037281275,
    'source_ci_conclusion': 'success',
    'blocker': None,
    'repository_hygiene_notes': 'Frozen source contains Go module metadata, MIT license, README/Makefile, Go source, tests, compose fixture and read-only CI. No .gitmodules declaration or obvious committed top-level build/cache payload was observed at the frozen tree.',
    'attribution_notes': 'Configured full-history commit-message searches for Co-Authored-By, Generated-By, Assisted-By, Signed-off-by, Anthropic and OpenAI returned zero matches before import.',
    'source_equivalent_ci': [
        'go vet ./...',
        'go test ./...',
    ],
    'integration_notes': 'Source is frozen and eligible for history-preserving import. This remains a Linux-host subsystem based on namespaces/cgroups/process primitives; no userspace-tcpip-stack integration is claimed without a bounded executable namespace/network contract.',
})
path.write_text(json.dumps(data, indent=2) + '\n')
