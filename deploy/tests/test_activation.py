import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "activate-relay.sh"


class ActivationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)
        self.releases = self.root / "releases"
        self.releases.mkdir()
        self.binary = self.root / "collab-server"
        self.previous = self.releases / "previous"
        self.previous.write_bytes(b"previous")
        self.previous.chmod(0o755)
        self.binary.symlink_to(self.previous)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.marker = self.root / "restart-count"
        systemctl = self.bin / "systemctl"
        systemctl.write_text('#!/bin/bash\nif [[ $1 == restart ]]; then echo x >> "$TEST_RESTARTS"; fi\nexit 0\n')
        systemctl.chmod(0o755)
        curl = self.bin / "curl"
        curl.write_text('#!/bin/bash\n[[ -z ${FAIL_NEW:-} || $(wc -l < "$TEST_RESTARTS") -gt 1 ]]\n')
        curl.chmod(0o755)
        sleep = self.bin / "sleep"
        sleep.write_text('#!/bin/bash\nexit 0\n')
        sleep.chmod(0o755)
        script = SCRIPT.read_text().replace('PATH=/usr/sbin:/usr/bin:/sbin:/bin', f'PATH={self.bin}:/usr/sbin:/usr/bin:/sbin:/bin')
        script = script.replace('base=/opt/openidea-collab/releases', f'base={self.releases}')
        script = script.replace('binary=/usr/local/bin/collab-server', f'binary={self.binary}')
        script = script.replace('/run/lock/openidea-collab-deploy.lock', str(self.root / 'lock'))
        self.script = self.root / "activate"
        self.script.write_text(script)
        self.env = dict(os.environ, TEST_RESTARTS=str(self.marker))
        self.payload = b'\x7fELFtest executable'
        self.commit = 'a' * 40

    def activate(self, payload=None, checksum=None, **env):
        payload = self.payload if payload is None else payload
        checksum = checksum or hashlib.sha256(payload).hexdigest()
        return subprocess.run(['bash', str(self.script), self.commit, checksum], input=payload,
                              capture_output=True, env=dict(self.env, **env), timeout=10)

    def test_healthy_release_activated(self):
        self.assertEqual(self.activate().returncode, 0)
        self.assertEqual(self.binary.resolve(), self.releases / self.commit)

    def test_failed_release_restores_previous(self):
        self.assertNotEqual(self.activate(FAIL_NEW='1').returncode, 0)
        self.assertEqual(self.binary.resolve(), self.previous)
        self.assertEqual(len(self.marker.read_text().splitlines()), 2)

    def test_bad_checksum_leaves_service_alone(self):
        self.assertNotEqual(self.activate(checksum='0' * 64).returncode, 0)
        self.assertEqual(self.binary.resolve(), self.previous)
        self.assertFalse(self.marker.exists())

    def test_existing_commit_cannot_be_overwritten(self):
        (self.releases / self.commit).write_bytes(b'different')
        self.assertNotEqual(self.activate().returncode, 0)
        self.assertEqual(self.binary.resolve(), self.previous)
        self.assertFalse(self.marker.exists())
