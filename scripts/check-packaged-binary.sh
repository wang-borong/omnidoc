#!/usr/bin/env bash
set -euo pipefail

archive="${1:?usage: scripts/check-packaged-binary.sh ARCHIVE}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

case "$archive" in
  *.zip)
    7z x -y "$archive" "-o$work" >/dev/null
    ;;
  *.tar.gz)
    tar -xzf "$archive" -C "$work"
    ;;
  *)
    echo "unsupported package archive: $archive" >&2
    exit 1
    ;;
esac

bin="$(find "$work" -type f \( -name omnidoc -o -name omnidoc.exe \) -print -quit)"
test -n "$bin" || { echo "packaged OmniDoc binary not found" >&2; exit 1; }
if [[ "$bin" != *.exe ]]; then
  chmod +x "$bin"
fi

"$bin" --version
"$bin" doctor --help >/dev/null
"$bin" fmt --help >/dev/null
"$bin" libs --help >/dev/null
test -s "$(dirname "$bin")/omnidoc-libs.toml"
test -s "$(dirname "$bin")/CHANGELOG.md"
test -s "$(dirname "$bin")/THIRD_PARTY_LICENSES.md"
test -s "$(dirname "$bin")/docs/decisions/0001-tectonic-engine-policy.md"

engine="$(dirname "$bin")/engines/tectonic"
if [[ "$bin" == *.exe ]]; then
  engine="${engine}.exe"
fi
test -x "$engine" || { echo "packaged Tectonic engine not found" >&2; exit 1; }
test "$("$engine" --version)" = "Tectonic 0.16.9"

if [[ "$(uname -s)" == "Linux" ]]; then
  for executable in "$bin" "$engine"; do
    dependencies="$(ldd "$executable" 2>&1 || true)"
    if [[ "$dependencies" == *"not found"* ]]; then
      echo "unresolved packaged runtime dependency for $executable" >&2
      echo "$dependencies" >&2
      exit 1
    fi
  done
fi

echo "Packaged binary smoke test passed: $archive"
