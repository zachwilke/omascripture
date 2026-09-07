#!/usr/bin/env python3
"""Measure a native reading window without touching the user's study file.
Usage: python scripts/profile-idle.py [path/to/omascripture]
Requires Linux /proc, a graphical session, and an installed KJV Bible.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

binary = Path(sys.argv[1] if len(sys.argv) > 1 else 'target/release/omascripture').resolve()
source = Path(os.environ.get('OMASCRIPTURE_DATA', Path.home() / '.local/share/omascripture'))
with tempfile.TemporaryDirectory(prefix='omascripture-profile-') as directory:
    root = Path(directory)
    for name in ['translations', 'resources']:
        (root / name).symlink_to(source / name)
    if (source / 'catalog.json').exists():
        shutil.copyfile(source / 'catalog.json', root / 'catalog.json')
    (root / 'study.json').write_text(json.dumps({'translation': 'kjv', 'last': {'book': 43, 'chapter': 3, 'verse': 16}}))
    env = dict(os.environ, OMASCRIPTURE_DATA=str(root), OMASCRIPTURE_PROVIDERS=str(root / 'providers.json'))
    with (root / 'app.log').open('w+') as log:
        app = subprocess.Popen([str(binary), '--gui'], env=env, stdout=log, stderr=log)
        try:
            def sample():
                if app.poll() is not None:
                    log.seek(0)
                    raise RuntimeError(log.read() or f'App exited: {app.returncode}')
                fields = Path(f'/proc/{app.pid}/stat').read_text().rsplit(')', 1)[1].split()
                ticks = int(fields[11]) + int(fields[12])
                memory = {}
                for line in Path(f'/proc/{app.pid}/smaps_rollup').read_text().splitlines():
                    parts = line.split()
                    if parts[0] in ('Rss:', 'Pss:', 'Private_Clean:', 'Private_Dirty:'):
                        memory[parts[0][:-1]] = int(parts[1])
                return ticks, memory
            time.sleep(5)
            before, _ = sample()
            start = time.monotonic()
            time.sleep(12)
            after, memory = sample()
            elapsed = time.monotonic() - start
            print(json.dumps({'binary': str(binary), 'idle_seconds': round(elapsed, 2),
                'cpu_percent_one_core': round(100 * (after-before) / os.sysconf('SC_CLK_TCK') / elapsed, 3),
                'memory_kib': memory}, indent=2))
        finally:
            if app.poll() is None:
                app.terminate()
                try:
                    app.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    app.kill()
                    app.wait()
