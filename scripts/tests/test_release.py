"""Exercise release provenance guards against an isolated Git repository."""
import importlib.util
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release_metadata", ROOT / "scripts/verify-release.py")
metadata = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(metadata)


class ReleaseGuards(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.original_root = metadata.ROOT
        metadata.ROOT = self.root
        self.addCleanup(setattr, metadata, "ROOT", self.original_root)
        for path in ["LICENSE", "openidea-collab/plugin.toml", "openidea-collab/Cargo.toml",
                     "openidea-collab/Cargo.lock", "collab-server/Cargo.lock",
                     "openidea-collab/LICENSE", "openidea-collab/NOTICE", "openidea-collab/README.md"]:
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, target)
        self.git("init", "-q")
        self.git("config", "user.name", "Release Guard Test")
        self.git("config", "user.email", "test@example.invalid")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture")
        self.version = metadata.verify()["version"]
        self.tag = f"v{self.version}"
        self.git("tag", self.tag)

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.root), *args], check=True, capture_output=True)

    def test_exact_clean_tag_is_accepted(self):
        self.assertEqual(metadata.verify(self.tag)["id"], "sorputi.openidea-collab")

    def test_dirty_checkout_and_untracked_source_are_refused(self):
        target = self.root / "untracked.txt"
        target.write_text("must not enter the package")
        with self.assertRaisesRegex(ValueError, "clean"):
            metadata.verify(self.tag)
        target.unlink()
        (self.root / "openidea-collab/README.md").write_text("changed")
        with self.assertRaisesRegex(ValueError, "clean"):
            metadata.verify(self.tag)

    def test_different_head_is_refused(self):
        self.git("commit", "--allow-empty", "-qm", "after tag")
        with self.assertRaisesRegex(ValueError, "HEAD differs"):
            metadata.verify(self.tag)

    def test_wrong_tag_and_mismatched_versions_are_refused(self):
        with self.assertRaisesRegex(ValueError, "tag must match"):
            metadata.verify("v999.0.0")
        cargo = self.root / "openidea-collab/Cargo.toml"
        cargo.write_text(cargo.read_text().replace(f'version = "{self.version}"', 'version = "999.0.0"', 1))
        with self.assertRaisesRegex(ValueError, "versions disagree"):
            metadata.verify()

    def test_missing_or_different_license_is_refused(self):
        license_file = self.root / "openidea-collab/LICENSE"
        license_file.write_text("different license")
        with self.assertRaisesRegex(ValueError, "license differs"):
            metadata.verify()
        license_file.unlink()
        with self.assertRaises(FileNotFoundError):
            metadata.verify()


if __name__ == "__main__":
    unittest.main()
