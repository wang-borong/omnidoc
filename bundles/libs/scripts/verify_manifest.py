#!/usr/bin/env python3
import argparse
import hashlib
import pathlib
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
MANIFEST_PATH = ROOT / "manifest.toml"


def contract_path(relative: str) -> pathlib.Path:
    path = pathlib.PurePosixPath(relative)
    if (
        not relative
        or path.is_absolute()
        or any(part in {"", ".", ".."} for part in path.parts)
        or "\\" in relative
    ):
        raise ValueError(f"unsafe manifest path: {relative}")
    return ROOT.joinpath(*path.parts)


def load_manifest() -> dict:
    with MANIFEST_PATH.open("rb") as stream:
        manifest = tomllib.load(stream)
    if manifest.get("manifest_version") != 1:
        raise ValueError("unsupported manifest_version")
    if manifest.get("checksum_algorithm") != "sha256":
        raise ValueError("unsupported checksum_algorithm")
    return manifest


def payload_files(manifest: dict) -> list[pathlib.Path]:
    files: list[pathlib.Path] = []
    for root_name in manifest["payload_roots"]:
        root = contract_path(root_name)
        if root.is_symlink():
            raise ValueError(f"symbolic link is not allowed: {root_name}")
        if root.is_file():
            files.append(root)
        elif root.is_dir():
            for path in root.rglob("*"):
                relative = path.relative_to(ROOT).as_posix()
                if path.is_symlink():
                    raise ValueError(f"symbolic link is not allowed: {relative}")
                if path.is_file():
                    files.append(path)
        else:
            raise ValueError(f"payload root does not exist: {root_name}")
    return sorted(files, key=lambda path: path.relative_to(ROOT).as_posix())


def digest(path: pathlib.Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def generated_checksums(manifest: dict) -> dict[str, str]:
    return {
        path.relative_to(ROOT).as_posix(): digest(path)
        for path in payload_files(manifest)
    }


def read_checksums(path: pathlib.Path) -> dict[str, str]:
    checksums: dict[str, str] = {}
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            checksum, relative = line.split("  ", 1)
        except ValueError as error:
            raise ValueError(f"invalid checksum line {line_number}") from error
        if len(checksum) != 64 or any(char not in "0123456789abcdef" for char in checksum):
            raise ValueError(f"invalid SHA-256 on line {line_number}")
        contract_path(relative)
        if relative in checksums:
            raise ValueError(f"duplicate checksum path: {relative}")
        checksums[relative] = checksum
    return checksums


def write_checksums(path: pathlib.Path, checksums: dict[str, str]) -> None:
    content = "".join(
        f"{checksum}  {relative}\n"
        for relative, checksum in sorted(checksums.items())
    )
    path.write_text(content, encoding="utf-8")


def verify_required_resources(manifest: dict) -> list[str]:
    return [
        relative
        for relative in manifest["required_resources"]
        if not contract_path(relative).is_file()
        or contract_path(relative).is_symlink()
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate or verify omnidoc-libs checksums")
    parser.add_argument("--write", action="store_true", help="rewrite the checksum file")
    args = parser.parse_args()

    try:
        manifest = load_manifest()
        checksum_path = contract_path(manifest["checksum_file"])
        if checksum_path.is_symlink():
            raise ValueError("checksum_file must not be a symbolic link")
        expected = generated_checksums(manifest)
        missing_required = verify_required_resources(manifest)
        if missing_required:
            raise ValueError("missing required resources: " + ", ".join(missing_required))
        if args.write:
            write_checksums(checksum_path, expected)
            print(f"wrote {len(expected)} checksums to {checksum_path.name}")
            return 0
        actual = read_checksums(checksum_path)
        missing = sorted(set(expected) - set(actual))
        extra = sorted(set(actual) - set(expected))
        changed = sorted(
            path for path in set(expected) & set(actual) if expected[path] != actual[path]
        )
        if missing or extra or changed:
            if missing:
                print("missing checksum entries: " + ", ".join(missing), file=sys.stderr)
            if extra:
                print("extra checksum entries: " + ", ".join(extra), file=sys.stderr)
            if changed:
                print("checksum mismatches: " + ", ".join(changed), file=sys.stderr)
            return 1
        print(f"verified {len(expected)} payload checksums")
        return 0
    except (OSError, KeyError, TypeError, ValueError) as error:
        print(f"manifest verification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
