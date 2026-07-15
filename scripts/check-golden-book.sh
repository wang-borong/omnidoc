#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$root/tests/fixtures/golden-book"
libs="${OMNIDOC_LIBS:-$root/../omnidoc-libs}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for tool in cargo pandoc pandoc-crossref unzip zipinfo jq python3; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
test -d "$libs/pandoc" || { echo "invalid OMNIDOC_LIBS: $libs" >&2; exit 1; }

cargo build --manifest-path "$root/Cargo.toml" --locked
cp -a "$fixture" "$work/book"
mkdir -p "$work/data" "$work/config" "$work/home"
cp -a "$libs" "$work/data/omnidoc"

export XDG_DATA_HOME="$work/data"
export XDG_CONFIG_HOME="$work/config"
export HOME="$work/home"

bin="$root/target/debug/omnidoc"
"$bin" build "$work/book" --all --force --report --write-lock

html="$work/book/build/golden-book.html"
epub="$work/book/build/golden-book.epub"
report="$work/book/build/omnidoc-report.json"
lock="$work/book/omnidoc.lock"
test -s "$html"
test -s "$epub"
test -s "$report"
test -s "$lock"

rg -q 'class="omni-display-math"' "$html"
rg -q 'display="inline"' "$html"
rg -q 'display="block"' "$html"
rg -q 'id="本章小结-1"|id="本章小结-2"' "$html"
rg -q 'omnidoc-base-css' "$lock"
rg -q 'lua-filter:display-math.lua' "$lock"
jq -e '.reports | length == 2 and all(.artifact_digest | startswith("blake3:"))' "$report" >/dev/null
python3 - "$lock" <<'PY'
import pathlib
import sys
import tomllib

lock = tomllib.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
if lock.get("lock_version") != 3:
    raise SystemExit("expected lock schema v3")
library = lock.get("library", {})
if library.get("version") != "1.0.0":
    raise SystemExit(f"unexpected omnidoc-libs version: {library.get('version')}")
if not library.get("manifest_digest", "").startswith("blake3:"):
    raise SystemExit("missing omnidoc-libs manifest digest")
if not library.get("checksums_digest", "").startswith("blake3:"):
    raise SystemExit("missing omnidoc-libs checksum digest")
targets = lock.get("targets", {})
if set(targets) != {"html", "epub"}:
    raise SystemExit(f"unexpected lock targets: {sorted(targets)}")
for name, target in targets.items():
    if not target.get("input_digest", "").startswith("blake3:"):
        raise SystemExit(f"missing digest for {name}")
PY
"$bin" lock --check "$work/book"

unzip -tq "$epub" >/dev/null
test "$(zipinfo -1 "$epub" | rg -c '\.svg$')" -ge 2
zipinfo -1 "$epub" | rg -q '\.css$'

while IFS= read -r member; do
  unzip -p "$epub" "$member"
done < <(zipinfo -1 "$epub" | rg '\.(xhtml|html)$') > "$work/epub-content.html"
while IFS= read -r member; do
  unzip -p "$epub" "$member"
done < <(zipinfo -1 "$epub" | rg '\.css$') > "$work/epub-style.css"

rg -q 'class="omni-display-math"' "$work/epub-content.html"
rg -q '\.omni-display-math' "$work/epub-style.css"
python3 - "$work/epub-content.html" <<'PY'
import pathlib
import re
import sys

content = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
visible = re.sub(r"<annotation\b[^>]*>.*?</annotation>", "", content, flags=re.S)
if r"\int_0^1" in visible:
    raise SystemExit("raw TeX leaked outside MathML annotation")
PY

printf '\n/* cache invalidation probe */\n' >> "$work/data/omnidoc/pandoc/css/omnidoc-base.css"
"$bin" build "$work/book" --to html --report
jq -e '.reports[0].skipped == false and .reports[0].cache_reason == "input_digest_changed"' "$report" >/dev/null

if command -v epubcheck >/dev/null; then
  epubcheck "$epub"
else
  echo "epubcheck not installed; structural EPUB checks passed"
fi

echo "Golden Book checks passed"
