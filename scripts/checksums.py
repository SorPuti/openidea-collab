#!/usr/bin/env python3
"""Write SHA256SUMS for the files in a release artifact directory."""
import hashlib
from pathlib import Path
import sys

folder = Path(sys.argv[1])
files = sorted(p for p in folder.iterdir() if p.is_file() and p.name != "SHA256SUMS")
(folder / "SHA256SUMS").write_text("".join(
    f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n" for p in files
))
