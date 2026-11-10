#!/usr/bin/env python3
"""Register this experimental plugin using the OpenIDEA development-link format."""
import json
import os
from pathlib import Path
import tempfile
import tomllib
import sys

PLUGIN_ID = 'sorputi.openidea-collab'

def manifest_id(path):
    with (path/'plugin.toml').open('rb') as source:
        value = tomllib.load(source).get('id')
    if not isinstance(value, str):
        raise ValueError(f'Invalid manifest: {path}')
    return value

def main():
    if len(sys.argv) != 2: raise ValueError('Expected plugin directory')
    target = Path(sys.argv[1]).resolve(strict=True)
    if manifest_id(target) != PLUGIN_ID: raise ValueError('Unexpected plugin ID')
    configured = os.environ.get('OPENIDEA_PLUGINS_DIR')
    data = os.environ.get('XDG_DATA_HOME')
    if configured:
        plugins = Path(configured)
    else:
        base = Path(data) if data and Path(data).is_absolute() else Path.home()/'.local/share'
        plugins = base/'ai.l0ng.openidea/plugins'
    if not plugins.is_absolute(): raise ValueError('OpenIDEA plugins directory must be absolute')
    plugins.mkdir(parents=True, exist_ok=True)
    if (plugins/PLUGIN_ID).exists():
        raise ValueError('An installed Collab package already exists; remove or update it in OpenIDEA first')
    path = plugins/'links.json'
    if path.is_symlink(): raise ValueError('links.json must not be a symlink')
    before = path.read_bytes() if path.exists() else None
    registry = json.loads(before) if before is not None else {'paths': []}
    if not isinstance(registry, dict) or not isinstance(registry.get('paths'), list) or not all(isinstance(v, str) for v in registry['paths']):
        raise ValueError('Invalid OpenIDEA links.json')
    matches = []
    for index, name in enumerate(registry['paths']):
        existing = Path(name)
        # An uninspectable link cannot safely be ruled out as another Collab copy.
        if not existing.is_dir(): raise ValueError(f'Cannot inspect existing linked plugin: {existing}')
        if manifest_id(existing) == PLUGIN_ID: matches.append(index)
    if len(matches) > 1: raise ValueError('Multiple Collab links exist; resolve them in OpenIDEA first')
    previous = registry['paths'][matches[0]] if matches else None
    if matches: registry['paths'][matches[0]] = str(target)
    else: registry['paths'].append(str(target))
    data = (json.dumps(registry, indent=2, ensure_ascii=False)+'\n').encode()
    with tempfile.NamedTemporaryFile(dir=plugins, prefix='.collab-link-', delete=False) as file:
        temp = Path(file.name)
        try:
            file.write(data); file.flush(); os.fsync(file.fileno())
            current = path.read_bytes() if path.exists() else None
            if current != before: raise RuntimeError('OpenIDEA links changed during installation; retry')
            os.replace(temp, path)
        finally:
            temp.unlink(missing_ok=True)
    print(f'Development link registered: {target}')
    if previous: print(f'Previous link preserved on disk: {previous}')
    print('Open Settings → Plugins to load the link and approve the helper.')

if __name__ == '__main__':
    try: main()
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f'Cannot register plugin: {error}', file=sys.stderr)
        sys.exit(1)
