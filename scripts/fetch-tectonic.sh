#!/usr/bin/env bash
set -euo pipefail

target="${1:?usage: scripts/fetch-tectonic.sh TARGET OUTPUT_DIR}"
output_dir="${2:?usage: scripts/fetch-tectonic.sh TARGET OUTPUT_DIR}"
version="0.16.9"
tag="tectonic%40${version}"

case "$target" in
  x86_64-unknown-linux-gnu)
    asset="tectonic-${version}-x86_64-unknown-linux-gnu.tar.gz"
    checksum="f3c825128095dc3399ea11c08c18035b33050a216930c295c79e8eb11bd21de4"
    executable="tectonic"
    ;;
  x86_64-apple-darwin)
    asset="tectonic-${version}-x86_64-apple-darwin.tar.gz"
    checksum="79d8839fa3594bfea9b2bf2ac0a0455bcc4d0de956a5e5c403107e9a72f79e86"
    executable="tectonic"
    ;;
  aarch64-apple-darwin)
    asset="tectonic-${version}-aarch64-apple-darwin.tar.gz"
    checksum="edb67c61aba768289f6da441c9e6f523cfaff4f8b2a5708523ef29c543f8e88e"
    executable="tectonic"
    ;;
  x86_64-pc-windows-msvc)
    asset="tectonic-${version}-x86_64-pc-windows-msvc.zip"
    checksum="131a24604785a9600989a3d91225f597df52ac06f00aeffe86fd529f99ee5cdd"
    executable="tectonic.exe"
    ;;
  *)
    echo "unsupported Tectonic target: $target" >&2
    exit 1
    ;;
esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
archive="$work/$asset"
if [[ -n "${OMNIDOC_TECTONIC_ARCHIVE:-}" ]]; then
  cp "$OMNIDOC_TECTONIC_ARCHIVE" "$archive"
else
  url="https://github.com/tectonic-typesetting/tectonic/releases/download/${tag}/${asset}"
  curl --fail --location --retry 3 --output "$archive" "$url"
fi

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$archive" | awk '{print $1}')"
else
  actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
fi
if [[ "$actual" != "$checksum" ]]; then
  echo "Tectonic checksum mismatch for $asset" >&2
  echo "expected: $checksum" >&2
  echo "actual:   $actual" >&2
  exit 1
fi

mkdir -p "$work/unpack"
case "$asset" in
  *.tar.gz)
    tar -xzf "$archive" -C "$work/unpack"
    ;;
  *.zip)
    if command -v unzip >/dev/null 2>&1; then
      unzip -q "$archive" -d "$work/unpack"
    else
      7z x -y "$archive" "-o$work/unpack" >/dev/null
    fi
    ;;
esac

source_binary="$(find "$work/unpack" -type f -name "$executable" -print -quit)"
if [[ -z "$source_binary" ]]; then
  echo "Tectonic executable not found in $asset" >&2
  exit 1
fi

mkdir -p "$output_dir"
cp "$source_binary" "$output_dir/$executable"
if [[ "$executable" != *.exe ]]; then
  chmod 0755 "$output_dir/$executable"
fi
"$output_dir/$executable" --version
