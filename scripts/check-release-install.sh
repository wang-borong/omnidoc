#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="${OMNIDOC_BIN:?OMNIDOC_BIN must point to an extracted release binary}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

export HOME="$work/home"
export XDG_CONFIG_HOME="$work/config"
export XDG_DATA_HOME="$work/data"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME"

"$bin" libs --install
"$bin" libs --verify --json > "$work/library.json"
jq -e '
  .manifest_valid == true
    and .integrity_verified == true
    and .omnidoc_compatible == true
    and .pandoc_compatible == true
' "$work/library.json" >/dev/null

"$bin" libs --update
"$bin" libs --verify

OMNIDOC_BIN="$bin" \
OMNIDOC_LIBS="$XDG_DATA_HOME/omnidoc" \
  "$root/scripts/check-golden-book.sh"

echo "Release install and Golden Book smoke test passed"
