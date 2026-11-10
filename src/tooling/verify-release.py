#!/usr/bin/env python3
"""Check source metadata; --tag also enforces an exact, clean release checkout."""
import argparse
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]


def git(*args):
    return subprocess.check_output(["git", "-C", str(ROOT), *args], text=True).strip()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(tag=None):
    plugin = ROOT / "src/plugin"
    manifest = tomllib.loads((plugin / "plugin.toml").read_text())
    cargo = tomllib.loads((plugin / "Cargo.toml").read_text())
    require(manifest["id"] == "sorputi.openidea-collab", "unexpected plugin ID")
    require(manifest["version"] == cargo["package"]["version"], "plugin versions disagree")
    require(manifest["license"] == "MIT", "manifest license must match LICENSE")
    require((ROOT / "LICENSE").is_file(), "project license missing")
    require((ROOT / "docs/plugin.md").is_file(), "plugin README missing")
    require((ROOT / "NOTICE").is_file(), "plugin dependency notice missing")
    require("on-event:file-saved" not in manifest["activation"], "unsafe legacy file upload activation")
    require("net:collab.synthosia.com" in manifest["capabilities"]["required"], "relay grant missing")
    require("project.write" not in manifest["capabilities"]["required"], "remote file writes must remain disabled")
    require((ROOT / "Cargo.lock").is_file(), "workspace lockfile missing")
    if tag:
        require(tag == f"v{manifest['version']}", "tag must match plugin version")
        require(git("rev-parse", "HEAD") == git("rev-parse", f"refs/tags/{tag}^{{commit}}"), "HEAD differs from tag")
        require(not git("status", "--porcelain", "--untracked-files=all"), "release checkout must be clean")
    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag")
    args = parser.parse_args()
    try:
        result = verify(args.tag)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"Release validation failed: {error}\n")
    print(f"{result['id']} {result['version']}: source metadata OK")
