#!/usr/bin/env python3
"""Review or switch the Collab development link; never edits plugin contents."""
import argparse
from pathlib import Path
import shlex
import subprocess
import sys
import tomllib

PLUGIN_ID = 'sorputi.openidea-collab'

def run(*args, capture=False):
    return subprocess.run(['openidea-plugin', *map(str, args)], check=True, text=True,
                          capture_output=capture)

def current():
    output = run('links', capture=True).stdout
    found = []
    for line in output.splitlines():
        name, separator, path = line.partition('\t')
        if separator and name == PLUGIN_ID:
            found.append(Path(path))
    if len(found) > 1:
        raise RuntimeError('multiple Collab links exist; resolve them manually before switching')
    return found[0] if found else None

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('target', type=Path, help='built plugin directory')
    parser.add_argument('--apply', action='store_true', help='perform the switch; default is review only')
    args = parser.parse_args()
    target = args.target.resolve(strict=True)
    if not target.is_dir() or '\n' in str(target) or '\t' in str(target):
        raise RuntimeError('target must be a directory with a plain path')
    with (target/'plugin.toml').open('rb') as file:
        if tomllib.load(file).get('id') != PLUGIN_ID:
            raise RuntimeError('target is not the Collab plugin')
    run('check', target)
    previous = current()
    print(f'Current: {previous or "(none)"}\nTarget:  {target}', flush=True)
    if previous == target:
        print('Already linked; no changes.')
        return
    if not args.apply:
        print('Review only; add --apply to switch.')
        return
    if previous is not None and not previous.is_dir():
        raise RuntimeError('previous link is missing; cannot guarantee rollback')
    if current() != previous:
        raise RuntimeError('links changed during validation; no switch performed')
    removed = False
    try:
        if previous is not None:
            run('unlink', previous)
            removed = True
        run('link', target)
        if current() != target:
            raise RuntimeError('new link could not be verified')
    except (Exception, KeyboardInterrupt) as error:
        print(f'Switch failed: {error}', file=sys.stderr)
        try:
            linked = current()
            if linked == target:
                run('unlink', target)
                linked = current()
            if removed and previous is not None and linked is None:
                run('link', previous)
            if removed and current() != previous:
                raise RuntimeError('previous link is not restored')
            print('Previous link restored.', file=sys.stderr)
        except Exception as rollback:
            print(f'Rollback failed: {rollback}. Restore manually: openidea-plugin link {shlex.quote(str(previous))}', file=sys.stderr)
        raise SystemExit(1)
    print('Switch verified. Approve the helper in OpenIDEA and reload the plugin.')
    if previous is not None:
        print(f'Rollback: python3 {shlex.quote(str(Path(__file__).resolve()))} {shlex.quote(str(previous))} --apply')

if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'Error: {error}', file=sys.stderr)
        raise SystemExit(1)
