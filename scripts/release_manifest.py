#!/usr/bin/env python3
"""Generate metadata only; signatures must be produced and verified separately."""
import argparse
import base64
import hashlib
import json
import re
from pathlib import Path


def build_manifest(version: str, target: str, artifact: Path, signature: Path) -> dict:
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", version):
        raise ValueError("Use an explicit SemVer release version without a leading v")
    if target not in {"darwin-aarch64", "darwin-x86_64", "darwin-universal"}:
        raise ValueError("Unsupported macOS release target")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+\.dmg", artifact.name):
        raise ValueError("Release asset must be a simple DMG filename")
    digest = hashlib.sha256()
    with artifact.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    signed = signature.read_text(encoding="utf-8").strip()
    if not signed:
        raise ValueError("Missing artifact signature")
    base64.b64decode(signed, validate=True)
    return {
        "schemaVersion": 1,
        "version": version,
        "minimumMacOS": "12.0",
        "parentApprovalRequired": True,
        "artifacts": {target: {
            "url": f"https://github.com/josebarrueta/tinkercrew/releases/download/v{version}/{artifact.name}",
            "sha256": digest.hexdigest(),
            "signature": signed,
        }},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--signature", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    manifest = build_manifest(args.version, args.target, args.artifact, args.signature)
    args.output.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
