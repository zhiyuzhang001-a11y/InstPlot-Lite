#!/usr/bin/env python3
"""Prepare a signed Windows update bundle for manual upload to OSS."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
from datetime import date
from pathlib import Path


PUBLIC_KEY_HEX = "e00891871d272e894bb2a6b30c77713d71a7af301141d7f923e4a86a5f8ee822"
OSS_ROOT = "https://instplot-release.oss-cn-beijing.aliyuncs.com/instplot-lite"


def package_version(cargo_toml: Path) -> str:
    match = re.search(
        r'^version\s*=\s*"([^"]+)"', cargo_toml.read_text(encoding="utf-8"), re.MULTILINE
    )
    if not match:
        raise SystemExit(f"Cannot read package version from {cargo_toml}")
    return match.group(1)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_private_key(private_key: Path) -> None:
    public_der = subprocess.check_output(
        [
            "openssl",
            "pkey",
            "-in",
            str(private_key),
            "-pubout",
            "-outform",
            "DER",
        ]
    )
    if len(public_der) != 44 or public_der[-32:].hex() != PUBLIC_KEY_HEX:
        raise SystemExit(
            "The selected private key does not match the public key embedded in InstPlot Lite."
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("installer", type=Path)
    parser.add_argument("--private-key", required=True, type=Path)
    parser.add_argument("--notes", required=True)
    parser.add_argument("--published-at", default=date.today().isoformat())
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    repository = Path(__file__).resolve().parents[2]
    version = package_version(repository / "Cargo.toml")
    expected_name = f"InstPlot-Lite-{version}-windows-x64-setup.exe"
    installer = args.installer.resolve()
    private_key = args.private_key.resolve()
    if not installer.is_file():
        raise SystemExit(f"Installer not found: {installer}")
    if installer.name != expected_name:
        raise SystemExit(
            f"Installer must be named {expected_name}, got {installer.name}"
        )
    if not private_key.is_file():
        raise SystemExit(f"Private key not found: {private_key}")
    verify_private_key(private_key)

    output = (args.output or repository / "target" / "oss-upload").resolve()
    staged_root = output / "instplot-lite"
    if staged_root.exists():
        shutil.rmtree(staged_root)
    release_dir = output / "instplot-lite" / "releases" / version
    stable_dir = output / "instplot-lite" / "stable"
    signatures_dir = stable_dir / "signatures"
    release_dir.mkdir(parents=True, exist_ok=True)
    stable_dir.mkdir(parents=True, exist_ok=True)
    signatures_dir.mkdir(parents=True, exist_ok=True)
    copied_installer = release_dir / expected_name
    shutil.copy2(installer, copied_installer)

    manifest = {
        "schema": 1,
        "version": version,
        "published_at": args.published_at,
        "notes": args.notes,
        "installer_url": f"{OSS_ROOT}/releases/{version}/{expected_name}",
        "signature_url": f"{OSS_ROOT}/stable/signatures/{version}.sig",
        "sha256": sha256(copied_installer),
        "size_bytes": copied_installer.stat().st_size,
    }
    manifest_path = stable_dir / "latest.json"
    signature_path = signatures_dir / f"{version}.sig"
    manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    subprocess.run(
        [
            "openssl",
            "pkeyutl",
            "-sign",
            "-rawin",
            "-inkey",
            str(private_key),
            "-in",
            str(manifest_path),
            "-out",
            str(signature_path),
        ],
        check=True,
    )
    if signature_path.stat().st_size != 64:
        raise SystemExit("OpenSSL produced an invalid Ed25519 signature length.")

    print(f"Prepared OSS release {version} in {output}")
    print(f"1. Upload first: {copied_installer}")
    print(f"2. Upload next:  {signature_path}")
    print(f"3. Upload last:  {manifest_path}")


if __name__ == "__main__":
    main()
