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
include_depfile="$work/book/.omnidoc-cache/include-files.d"
include_code_depfile="$work/book/.omnidoc-cache/include-code-files.d"
test -s "$html"
test -s "$epub"
test -s "$report"
test -s "$lock"
test -s "$include_depfile"
test -s "$include_code_depfile"

rg -q 'class="omni-display-math"' "$html"
rg -q 'display="inline"' "$html"
rg -q 'display="block"' "$html"
rg -q 'nested include-code works' "$html"
rg -q 'id="本章小结-1"|id="本章小结-2"' "$html"
rg -q 'omnidoc-base-css' "$lock"
rg -q 'lua-filter:display-math.lua' "$lock"
rg -q 'theme-manifest:engineering-book' "$lock"
rg -q 'chapters/chapter-one.md' "$include_depfile"
rg -q 'chapters/nested/details.md' "$include_depfile"
rg -q 'assets/example.rs' "$include_code_depfile"
jq -e '
  .reports
  | length == 2
    and all(.artifact_digest | startswith("blake3:"))
    and all(.cache_details | index("forced_by_user"))
' "$report" >/dev/null
jq -e '
  .reports
  | map(select(.output == "epub"))[0].compatibility
  | .profile == "readium"
    and .profile_version == 1
    and .valid == true
    and (.reader_matrix | length >= 3)
    and all(.checks[]; .passed == true)
' "$report" >/dev/null
python3 - "$lock" <<'PY'
import pathlib
import sys
import tomllib

lock = tomllib.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
if lock.get("lock_version") != 4:
    raise SystemExit("expected lock schema v4")
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
"$bin" build "$work/book" --to html --report
jq -e '
  .reports[0]
  | .skipped == true
    and .cache_reason == "input_digest_match"
    and (.cache_details | length == 0)
' "$report" >/dev/null
python3 - "$epub" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
content = path.read_bytes()
if b"application/epub+zip" not in content:
    raise SystemExit("cannot locate EPUB mimetype content")
path.write_bytes(content.replace(b"application/epub+zip", b"application/epub-bad", 1))
PY
"$bin" build "$work/book" --to epub --report
jq -e '
  .reports[0]
  | .skipped == false
    and .cache_reason == "artifact_compatibility_failed"
    and (.cache_details | index("artifact_compatibility_failed"))
    and .compatibility.profile == "readium"
    and .compatibility.valid == true
' "$report" >/dev/null

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

printf '\nDepfile invalidation probe.\n' >> "$work/book/chapters/nested/details.md"
"$bin" build "$work/book" --to html --report
jq -e '
  .reports[0]
  | .skipped == false
    and .cache_reason == "input_digest_changed"
    and (.cache_details | index("dependency_changed:chapters/nested/details.md"))
' "$report" >/dev/null

printf '\n/* cache invalidation probe */\n' >> "$work/data/omnidoc/pandoc/css/omnidoc-base.css"
"$bin" build "$work/book" --to html --report
jq -e '
  .reports[0]
  | .skipped == false
    and .cache_reason == "input_digest_changed"
    and any(.cache_details[]; startswith("resource_changed:omnidoc-libs:omnidoc-base-css"))
' "$report" >/dev/null

printf '\n# theme contract invalidation probe\n' >> "$work/data/omnidoc/themes/engineering-book.toml"
"$bin" build "$work/book" --to html --report
jq -e '
  .reports[0]
  | .skipped == false
    and .cache_reason == "input_digest_changed"
    and any(.cache_details[]; startswith("resource_changed:omnidoc-libs:theme-manifest"))
' "$report" >/dev/null

if command -v epubcheck >/dev/null; then
  epubcheck "$epub"
else
  echo "epubcheck not installed; structural EPUB checks passed"
fi

echo "Golden Book checks passed"
