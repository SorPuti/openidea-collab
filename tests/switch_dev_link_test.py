#!/usr/bin/env python3
"""Exercise the real CLI against an isolated links directory, including rollback."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
script = root/'scripts/switch-dev-link.py'
cli = shutil.which('openidea-plugin')
assert cli
source = root/'openidea-collab'
with tempfile.TemporaryDirectory(prefix='collab-link-test-') as tmp:
    tmp = Path(tmp)
    env = dict(os.environ, OPENIDEA_PLUGINS_DIR=str(tmp/'links'))
    packages = []
    for name in ('old version', 'new version'):
        dest = tmp/name
        (dest/'dist').mkdir(parents=True)
        shutil.copy(source/'plugin.toml', dest/'plugin.toml')
        shutil.copy(source/'dist/plugin.wasm', dest/'dist/plugin.wasm')
        shutil.copytree(source/'web', dest/'web')
        packages.append(dest)
    old, new = packages
    def call(*args, check=True, environment=env):
        return subprocess.run(args, env=environment, check=check, text=True, capture_output=True)
    def links():
        return call(cli, 'links').stdout
    call(cli, 'link', str(old))
    call('python3', str(script), str(new))
    assert str(old) in links() and str(new) not in links()
    call('python3', str(script), str(new), '--apply')
    assert str(new) in links() and str(old) not in links()
    call('python3', str(script), str(old), '--apply')
    assert str(old) in links() and str(new) not in links()

    wrapper_dir = tmp/'commands'
    wrapper_dir.mkdir()
    wrapper = wrapper_dir/'openidea-plugin'
    wrapper.write_text('#!/usr/bin/env python3\nimport os,sys\n'
        f'cli={cli!r}\nblocked={str(new)!r}\n'
        'if len(sys.argv)>2 and sys.argv[1]=="link" and sys.argv[2]==blocked: sys.exit(42)\n'
        'os.execv(cli,[cli,*sys.argv[1:]])\n')
    wrapper.chmod(0o755)
    broken_env = dict(env, PATH=str(wrapper_dir)+os.pathsep+env.get('PATH', ''))
    failure = call('python3', str(script), str(new), '--apply', check=False, environment=broken_env)
    assert failure.returncode != 0 and 'Previous link restored' in failure.stderr
    assert str(old) in links() and str(new) not in links()

    invalid = tmp/'invalid'
    invalid.mkdir()
    (invalid/'plugin.toml').write_text('id = "different.plugin"\n')
    assert call('python3', str(script), str(invalid), '--apply', check=False).returncode != 0
    assert str(old) in links() and str(new) not in links()
    call(cli, 'link', str(new))
    duplicate = call('python3', str(script), str(old), '--apply', check=False)
    assert duplicate.returncode != 0 and 'multiple Collab links' in duplicate.stderr
    assert str(old) in links() and str(new) in links()
print('PASS: isolated dry-run, switch, rollback after failure, invalid package and duplicate links')
