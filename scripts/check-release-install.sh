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

# No engine override is configured here: this proves that an extracted release
# discovers its sibling engines/tectonic binary automatically.
"$bin" doctor --strict --output pdf "$root/tests/fixtures/golden-book"

OMNIDOC_BIN="$bin" \
OMNIDOC_LIBS="$XDG_DATA_HOME/omnidoc" \
  "$root/scripts/check-golden-book.sh"

OMNIDOC_BIN="$bin" \
OMNIDOC_LIBS="$XDG_DATA_HOME/omnidoc" \
OMNIDOC_PDF_ENGINE=tectonic \
OMNIDOC_TECTONIC_BIN="$(dirname "$bin")/engines/tectonic" \
  "$root/scripts/check-golden-pdf.sh"

OMNIDOC_BIN="$bin" \
OMNIDOC_LIBS="$XDG_DATA_HOME/omnidoc" \
OMNIDOC_TECTONIC_BIN="$(dirname "$bin")/engines/tectonic" \
  "$root/scripts/check-tectonic-latex.sh"

echo "Release install and Markdown/native-LaTeX smoke tests passed"
