#!/usr/bin/env python3
import os
import pathlib
import re
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
CONTRACT = ROOT / "release" / "omnidoc-libs.toml"


def load(path: pathlib.Path) -> dict:
    with path.open("rb") as stream:
        return tomllib.load(stream)


def version(value: str) -> tuple[int, int, int]:
    match = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", value)
    if not match:
        raise ValueError(f"invalid semantic version: {value}")
    return tuple(int(component) for component in match.groups())


def matches(requirement: str, candidate: str) -> bool:
    actual = version(candidate)
    for clause in requirement.split(","):
        clause = clause.strip()
        match = re.fullmatch(r"(>=|<=|>|<|=)?\s*(v?\d+\.\d+\.\d+)", clause)
        if not match:
            raise ValueError(f"unsupported version requirement: {requirement}")
        operator = match.group(1) or "="
        expected = version(match.group(2))
        accepted = {
            ">=": actual >= expected,
            "<=": actual <= expected,
            ">": actual > expected,
            "<": actual < expected,
            "=": actual == expected,
        }[operator]
        if not accepted:
            return False
    return True


def main() -> int:
    library_root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ROOT.parent / "omnidoc-libs")
    try:
        contract = load(CONTRACT)
        cargo = load(ROOT / "Cargo.toml")
        manifest = load(library_root / "manifest.toml")
        if contract.get("contract_version") != 1:
            raise ValueError("unsupported release contract version")

        omnidoc_version = cargo["package"]["version"]
        if contract.get("omnidoc_version") != omnidoc_version:
            raise ValueError(
                f"contract OmniDoc version {contract.get('omnidoc_version')} does not match Cargo version {omnidoc_version}"
            )
        if not matches(manifest["compatible_omnidoc"], omnidoc_version):
            raise ValueError(
                f"omnidoc-libs {manifest['version']} is incompatible with OmniDoc {omnidoc_version}"
            )

        library = contract["library"]
        library_version = library["version"]
        if library_version != manifest["version"]:
            raise ValueError(
                f"contract library version {library_version} does not match manifest version {manifest['version']}"
            )
        expected_revision = f"v{library_version}"
        if library["revision"] != expected_revision:
            raise ValueError(f"library revision must be {expected_revision}")
        workflow_revision = os.environ.get("OMNIDOC_LIBS_RELEASE_REF")
        if workflow_revision and workflow_revision != library["revision"]:
            raise ValueError(
                f"workflow library revision {workflow_revision} does not match contract revision {library['revision']}"
            )
        if library["checksum_algorithm"] != manifest["checksum_algorithm"]:
            raise ValueError("release and library checksum algorithms differ")

        archive_name = f"omnidoc-libs-v{library_version}.tar.gz"
        release_base = f"{library['repository']}/releases/download/{expected_revision}"
        expected_archive = f"{release_base}/{archive_name}"
        if library["archive_url"] != expected_archive:
            raise ValueError(f"archive URL must be {expected_archive}")
        if library["checksum_url"] != f"{expected_archive}.sha256":
            raise ValueError(f"checksum URL must be {expected_archive}.sha256")

        print(
            f"verified OmniDoc {omnidoc_version} release contract with omnidoc-libs {library_version}"
        )
        return 0
    except (KeyError, OSError, TypeError, ValueError) as error:
        print(f"library release contract verification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
