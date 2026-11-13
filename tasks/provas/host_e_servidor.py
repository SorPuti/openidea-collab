#!/usr/bin/env python3
"""Provas isoladas; requer dist/plugin.wasm já construído e openidea-plugin no PATH."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]

def run(*args, **kwargs):
    print('+', ' '.join(map(str, args)), flush=True)
    subprocess.run(args, check=True, **kwargs)

with tempfile.TemporaryDirectory(prefix='collab-provas-') as tmp:
    tmp = Path(tmp)
    package = tmp / 'package'
    (package / 'dist').mkdir(parents=True)
    (package / 'web').mkdir()
    shutil.copy(ROOT / 'openidea-collab/dist/plugin.wasm', package / 'dist/plugin.wasm')
    manifest = (ROOT / 'openidea-collab/plugin.toml').read_text()
    manifest = manifest.replace('required = ["project.read", "project.write", "events", "storage", "editor.read"]', 'required = ["project.read", "project.write", "events", "storage", "editor.read"]\noptional = ["net:collab.example.org"]')
    manifest += '\n[[contributes.customEditors]]\nid = "collab-editor"\ntitle = "Collaborative Editor"\npatterns = ["*.collabtest"]\npriority = "option"\nweb = "web/index.html"\n'
    (package / 'plugin.toml').write_text(manifest)
    (package / 'web/index.html').write_text('<!doctype html><title>probe</title>')
    run('openidea-plugin', 'check', str(package))

    server = tmp / 'server'
    shutil.copytree(ROOT / 'collab-server', server, ignore=shutil.ignore_patterns('target'))
    route = server / 'src/routes.rs'
    contents = route.read_text()
    old = 'Router::new()\n        .route("/rooms"'
    assert contents.count(old) == 1
    route.write_text(contents.replace(old, 'Router::new()\n        .route("/healthz", get(|| async { StatusCode::NO_CONTENT }))\n        .route("/rooms"'))
    main = server / 'src/main.rs'
    contents = main.read_text()
    old = 'let addr = SocketAddr::from(([0, 0, 0, 0], port));'
    assert contents.count(old) == 1
    main.write_text(contents.replace(old, 'let addr = SocketAddr::from(([127, 0, 0, 1], port));'))
    (server / 'tests/health_snippet.rs').write_text('''use std::{collections::HashMap, sync::{Arc, Mutex}};\nuse collab_server::routes::router;\n#[tokio::test]\nasync fn health_is_read_only() {\n let state = Arc::new(Mutex::new(HashMap::new()));\n let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();\n let address = listener.local_addr().unwrap();\n let handle = tokio::spawn(async move { axum::serve(listener, router(state)).await.unwrap() });\n let response = reqwest::get(format!("http://{address}/healthz")).await.unwrap();\n assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);\n handle.abort();\n}\n''')
    env = os.environ.copy()
    env['CARGO_TARGET_DIR'] = '/tmp/openidea-collab-proof-target'
    run('cargo', 'test', '--manifest-path', str(server / 'Cargo.toml'), '--locked', '--offline', '--test', 'health_snippet', env=env)
