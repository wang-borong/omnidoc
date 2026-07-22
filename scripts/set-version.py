#!/usr/bin/env python3
import argparse
import os
import pathlib
import re
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
VERSION_PATTERN = re.compile(r"\d+\.\d+\.\d+")
REPOSITORY = "https://github.com/wang-borong/omnidoc"


def atomic_write(path: pathlib.Path, content: str) -> None:
    mode = path.stat().st_mode
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=path.parent, delete=False
    ) as stream:
        stream.write(content)
        temporary = pathlib.Path(stream.name)
    os.chmod(temporary, mode)
    os.replace(temporary, path)


def replace_once(path: pathlib.Path, pattern: str, replacement: str) -> None:
    content = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, content, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit(f"cannot locate version field in {path.relative_to(ROOT)}")
    atomic_write(path, updated)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Set the unified OmniDoc and library bundle version"
    )
    parser.add_argument("version", help="semantic version without a v prefix")
    args = parser.parse_args()
    version = args.version
    if VERSION_PATTERN.fullmatch(version) is None:
        parser.error("version must use MAJOR.MINOR.PATCH")

    replace_once(
        ROOT / "Cargo.toml",
        r'^(version = ")[^"]+("\s*)$',
        rf"\g<1>{version}\g<2>",
    )
    replace_once(
        ROOT / "Cargo.lock",
        r'(\[\[package\]\]\nname = "omnidoc"\nversion = ")[^"]+("\s*)',
        rf"\g<1>{version}\g<2>",
    )
    manifest = ROOT / "bundles" / "libs" / "manifest.toml"
    replace_once(manifest, r'^version = "[^"]+"$', f'version = "{version}"')
    replace_once(
        manifest,
        r'^compatible_omnidoc = "[^"]+"$',
        f'compatible_omnidoc = "={version}"',
    )

    tag = f"v{version}"
    archive = f"omnidoc-libs-v{version}.tar.gz"
    release_base = f"{REPOSITORY}/releases/download/{tag}"
    contract = f'''contract_version = 1
omnidoc_version = "{version}"

[library]
version = "{version}"
revision = "{tag}"
repository = "{REPOSITORY}"
archive_url = "{release_base}/{archive}"
checksum_algorithm = "sha256"
checksum_url = "{release_base}/{archive}.sha256"
'''
    atomic_write(ROOT / "release" / "omnidoc-libs.toml", contract)
    print(f"set OmniDoc and library bundle version to {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
