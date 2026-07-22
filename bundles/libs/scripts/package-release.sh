#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
output="${1:-$root/dist}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for tool in python3 tar gzip sha256sum; do
  command -v "$tool" >/dev/null || {
    echo "missing required tool: $tool" >&2
    exit 1
  }
done

python3 "$root/scripts/verify_manifest.py"
version="$(python3 - "$root/manifest.toml" <<'PY'
import pathlib
import sys
import tomllib

with pathlib.Path(sys.argv[1]).open("rb") as stream:
    print(tomllib.load(stream)["version"])
PY
)"
tag="${OMNIDOC_RELEASE_TAG:-}"
if [[ -n "$tag" && "$tag" != "v$version" ]]; then
  echo "release tag $tag does not match manifest version v$version" >&2
  exit 1
fi

base="omnidoc-libs-v$version"
stage="$work/$base"
mkdir -p "$stage" "$output"
cp "$root/manifest.toml" "$root/checksums.sha256" "$root/README.md" "$stage/"

while IFS= read -r payload_root; do
  cp -a "$root/$payload_root" "$stage/$payload_root"
done < <(python3 - "$root/manifest.toml" <<'PY'
import pathlib
import sys
import tomllib

with pathlib.Path(sys.argv[1]).open("rb") as stream:
    for root in tomllib.load(stream)["payload_roots"]:
        print(root)
PY
)

archive="$output/$base.tar.gz"
checksum="$archive.sha256"
rm -f "$archive" "$checksum"
tar \
  --sort=name \
  --mtime='@0' \
  --owner=0 \
  --group=0 \
  --numeric-owner \
  --format=gnu \
  -C "$work" \
  -cf "$work/$base.tar" \
  "$base"
gzip -n -c "$work/$base.tar" > "$archive"
(cd "$output" && sha256sum "$base.tar.gz" > "$base.tar.gz.sha256")

mkdir -p "$work/verify"
tar -xzf "$archive" -C "$work/verify"
(
  cd "$work/verify/$base"
  test -s manifest.toml
  test -s checksums.sha256
  test -s README.md
  sha256sum --check checksums.sha256 >/dev/null
)

echo "created $archive"
echo "created $checksum"
