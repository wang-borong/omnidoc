#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$root/tests/fixtures/golden-book"
libs="${OMNIDOC_LIBS:-$root/../omnidoc-libs}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for tool in cargo pandoc pandoc-crossref xelatex pdfinfo pdffonts pdftotext jq rg fc-match python3; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
test -d "$libs/pandoc" || { echo "invalid OMNIDOC_LIBS: $libs" >&2; exit 1; }

for family in "Noto Serif CJK SC" "Noto Sans CJK SC" "Noto Sans Mono CJK SC"; do
  matched="$(fc-match --format '%{family}\n' "$family" | head -n 1)"
  rg -qi 'Noto.*CJK' <<< "$matched" || {
    echo "missing required font: $family (matched: $matched)" >&2
    exit 1
  }
done

cargo build --manifest-path "$root/Cargo.toml" --locked
cp -a "$fixture" "$work/book"
python3 - "$work/book/.omnidoc.toml" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
content = path.read_text(encoding="utf-8")
path.write_text(
    content.replace('outputs = ["html", "epub"]', 'outputs = ["pdf"]'),
    encoding="utf-8",
)
PY
mkdir -p "$work/data" "$work/config" "$work/home"
cp -a "$libs" "$work/data/omnidoc"

export XDG_DATA_HOME="$work/data"
export XDG_CONFIG_HOME="$work/config"
export HOME="$work/home"

bin="$root/target/debug/omnidoc"
pdf="$work/book/build/golden-book.pdf"
report="$work/book/build/omnidoc-report.json"
lock="$work/book/omnidoc.lock"
include_depfile="$work/book/.omnidoc-cache/include-files.d"
include_code_depfile="$work/book/.omnidoc-cache/include-code-files.d"

"$bin" build "$work/book" --to pdf --force --report --write-lock

test -s "$pdf"
test -s "$report"
test -s "$lock"
test -s "$include_depfile"
test -s "$include_code_depfile"

pages="$(pdfinfo "$pdf" | awk '/^Pages:/ { print $2 }')"
test "${pages:-0}" -ge 3 || { echo "Golden PDF has fewer than 3 pages" >&2; exit 1; }

pdffonts "$pdf" > "$work/fonts.txt"
awk 'NR > 2 && NF && ($(NF-4) != "yes" || $(NF-3) != "yes") { print; failed = 1 } END { exit failed }' "$work/fonts.txt"
rg -qi 'NotoSerifCJK' "$work/fonts.txt"
rg -qi 'NotoSansCJK' "$work/fonts.txt"

pdftotext "$pdf" "$work/content.txt"
rg -q '第一章：递归包含' "$work/content.txt"
rg -q '第二章：结构化内容' "$work/content.txt"
rg -q '块级公式应居中' "$work/content.txt"

jq -e '
  .reports[0]
  | .output == "pdf"
    and .skipped == false
    and (.artifact_digest | startswith("blake3:"))
    and (.toolchain.latex_engine | startswith("XeTeX "))
' "$report" >/dev/null

python3 - "$lock" <<'PY'
import pathlib
import sys
import tomllib

lock = tomllib.loads(pathlib.Path(sys.argv[1]).read_text(encoding="utf-8"))
target = lock.get("targets", {}).get("pdf")
if target is None:
    raise SystemExit("missing PDF lock target")
if not target.get("input_digest", "").startswith("blake3:"):
    raise SystemExit("missing PDF input digest")
dependencies = set(target.get("dependencies", []))
for expected in {
    "assets/cover.pdf",
    "assets/diagram.pdf",
    "chapters/chapter-one.md",
    "chapters/nested/details.md",
    "assets/example.rs",
}:
    if expected not in dependencies:
        raise SystemExit(f"missing PDF dependency: {expected}")
resources = {resource["logical_name"] for resource in target.get("resources", [])}
for expected in {
    "theme-manifest:engineering-book",
    "theme-latex-header:pandoc/headers/engineering-book.tex",
    "theme-latex-package:texmf/tex/common/omni-engineering-book.sty",
}:
    if expected not in resources:
        raise SystemExit(f"missing PDF resource: {expected}")
PY

rg -q 'chapters/chapter-one.md' "$include_depfile"
rg -q 'chapters/nested/details.md' "$include_depfile"
rg -q 'assets/example.rs' "$include_code_depfile"
"$bin" lock --check "$work/book"

"$bin" build "$work/book" --to pdf --report
jq -e '.reports[0].skipped == true and .reports[0].cache_reason == "input_digest_match"' "$report" >/dev/null

printf '\n%% cache invalidation probe\n' >> "$work/data/omnidoc/texmf/tex/common/omni-engineering-book.sty"
"$bin" build "$work/book" --to pdf --report
jq -e '.reports[0].skipped == false and .reports[0].cache_reason == "input_digest_changed"' "$report" >/dev/null

cp "$work/book/assets/cover.pdf" "$work/book/assets/diagram.pdf"
"$bin" build "$work/book" --to pdf --report
jq -e '.reports[0].skipped == false and .reports[0].cache_reason == "input_digest_changed"' "$report" >/dev/null

echo "Golden PDF checks passed"
