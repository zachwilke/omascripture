#!/usr/bin/env python3
"""Exercise the quick installer with local release fixtures and isolated paths."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

repo = Path(__file__).resolve().parent.parent
asset = 'omascripture-linux-x86_64.tar.gz'
with tempfile.TemporaryDirectory(prefix='omascripture-installer-test-') as temporary:
    root = Path(temporary)
    release = root / 'release'
    release.mkdir()
    tools = root / 'tools'
    tools.mkdir()
    target = root / 'user with spaces % and $'
    for name, source in {
        'curl': '''#!/usr/bin/env python3
import os, pathlib, shutil, sys
if os.environ.get('MOCK_DOWNLOAD_FAIL'): sys.exit(22)
url = next(a for a in sys.argv[1:] if a.startswith('https://'))
shutil.copyfile(pathlib.Path(os.environ['MOCK_RELEASE']) / url.rsplit('/',1)[1], sys.argv[sys.argv.index('-o')+1])
''',
        'cargo': '#!/bin/sh\necho "Installer must not invoke Cargo" >&2\nexit 99\n',
        'uname': '#!/bin/sh\nif [ "$1" = -s ]; then echo Linux; else echo "${MOCK_ARCH:-x86_64}"; fi\n',
    }.items():
        path = tools / name
        path.write_text(source)
        path.chmod(0o755)
    binary = b'#!/bin/sh\nif [ "$1" = --version ]; then echo "omascripture fixture"; else touch "$MOCK_LAUNCH_MARKER"; fi\n'
    def package(payload=binary):
        with tarfile.open(release / asset, 'w:gz') as archive:
            for name in ['install.sh', 'uninstall.sh', 'assets/omascripture.svg']:
                archive.add(repo / name, arcname='omascripture/' + name)
            member = tarfile.TarInfo('omascripture/omascripture')
            member.size = len(payload)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(payload))
        digest = hashlib.sha256((release / asset).read_bytes()).hexdigest()
        (release / (asset + '.sha256')).write_text(f'{digest}  {asset}\n')
    env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
        OMASCRIPTURE_INSTALL_ROOT=str(target), MOCK_RELEASE=str(release),
        MOCK_LAUNCH_MARKER=str(root / 'launched'))
    def run(success=True, **overrides):
        result = subprocess.run(['bash', str(repo / 'setup.sh'), '--no-launch'],
            env=dict(env, **overrides), text=True, capture_output=True)
        assert (result.returncode == 0) == success, result.stdout + result.stderr
        return result
    package()
    run()
    installed = target / '.local/bin/omascripture'
    assert installed.read_bytes() == binary
    desktop = (target / '.local/share/applications/io.github.zachwilke.OmaScripture.desktop').read_text()
    assert 'Terminal=false' in desktop and 'Exec="' in desktop and '%%' in desktop
    assert (target / '.local/share/icons/hicolor/scalable/apps/omascripture.svg').exists()
    assert not (root / 'launched').exists()
    study = target / '.local/share/omascripture/study.json'
    study.write_text('{"translation":"kjv","notes":{"43:3:16":"Keep this note"}}')
    original = study.read_bytes()
    run()
    assert study.read_bytes() == original
    (release / (asset + '.sha256')).write_text('0' * 64 + '  ' + asset + '\n')
    assert 'verification failed' in run(False).stderr
    assert installed.read_bytes() == binary
    (release / (asset + '.sha256')).write_text('0' * 64 + '  ../unexpected\n')
    assert 'checksum file is invalid' in run(False).stderr
    run(False, MOCK_DOWNLOAD_FAIL='1')
    run(False, MOCK_ARCH='aarch64')
    package(b'#!/bin/sh\nexit 1\n')
    run(False)
    assert installed.read_bytes() == binary
    assert study.read_bytes() == original
    subprocess.run(['bash', str(target / '.local/share/omascripture/uninstall.sh')],
        env=env, check=True, capture_output=True)
    assert not installed.exists()
    assert study.read_bytes() == original
    print('Installer checks passed: fresh install, spaces, reinstall, checksum rejection, download failure, architecture, incompatible executable, no-launch, and data-preserving uninstall.')
