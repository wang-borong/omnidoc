#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$root/tests/fixtures/golden-latex-cjk"
libs="${OMNIDOC_LIBS:-$root/bundles/libs}"
work="$(mktemp -d)"
if [[ -n "${OMNIDOC_KEEP_WORK:-}" ]]; then
  echo "Native Tectonic LaTeX work directory: $work"
else
  trap 'rm -rf "$work"' EXIT
fi

required_tools=(pdffonts pdftotext jq rg fc-match python3)
if [[ -z "${OMNIDOC_BIN:-}" ]]; then
  required_tools+=(cargo)
fi
for tool in "${required_tools[@]}"; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
test -d "$libs/texmf" || { echo "invalid OMNIDOC_LIBS: $libs" >&2; exit 1; }

if [[ -n "${OMNIDOC_TECTONIC_BIN:-}" ]]; then
  engine="$OMNIDOC_TECTONIC_BIN"
  test -x "$engine" || { echo "invalid OMNIDOC_TECTONIC_BIN: $engine" >&2; exit 1; }
else
  command -v tectonic >/dev/null || { echo "missing required tool: tectonic" >&2; exit 1; }
  engine="$(command -v tectonic)"
fi

for family in "Noto Serif CJK SC" "Noto Sans CJK SC"; do
  matched="$(fc-match --format '%{family}\n' "$family" | head -n 1)"
  rg -qi 'Noto.*CJK' <<< "$matched" || {
    echo "missing required font: $family (matched: $matched)" >&2
    exit 1
  }
done

if [[ -n "${OMNIDOC_BIN:-}" ]]; then
  bin="$OMNIDOC_BIN"
  test -x "$bin" || { echo "invalid OMNIDOC_BIN: $bin" >&2; exit 1; }
else
  cargo build --manifest-path "$root/Cargo.toml" --locked
  bin="$root/target/debug/omnidoc"
fi

cp -a "$fixture" "$work/project"
python3 - "$work/project/.omnidoc.toml" "$engine" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
content = path.read_text(encoding="utf-8")
content = content.replace('latex_engine = "tectonic"', f"latex_engine = {json.dumps(sys.argv[2])}")
path.write_text(content, encoding="utf-8")
PY

mkdir -p "$work/data" "$work/config" "$work/home"
cp -a "$libs" "$work/data/omnidoc"

export XDG_DATA_HOME="$work/data"
export XDG_CONFIG_HOME="$work/config"
export HOME="$work/home"

project="$work/project"
pdf="$project/build/golden-latex-cjk.pdf"
report="$project/build/omnidoc-report.json"
lock="$project/omnidoc.lock"
depfile="$project/.omnidoc-cache/latex-inputs.d"

"$bin" doctor --strict "$project"
"$bin" build "$project" --force --report --write-lock

test -s "$pdf"
test -s "$report"
test -s "$lock"
test -s "$depfile"

pdffonts "$pdf" > "$work/fonts.txt"
awk 'NR > 2 && NF && ($(NF-4) != "yes" || $(NF-3) != "yes") { print; failed = 1 } END { exit failed }' "$work/fonts.txt"
rg -qi 'NotoSerifCJK' "$work/fonts.txt"
rg -qi 'NotoSansCJK' "$work/fonts.txt"

pdftotext "$pdf" "$work/content.txt"
rg -q 'Native LaTeX 原生构建' "$work/content.txt"
rg -q 'Mathematics 数学公式' "$work/content.txt"
rg -q 'Bibliography 参考文献' "$work/content.txt"
rg -q 'Knuth' "$work/content.txt"
rg -q 'Literate Programming' "$work/content.txt"
rg -q '原生 LaTeX 构建已加载项目内递归搜索路径中的宏包' "$work/content.txt"

rg -q 'chapters/intro.tex' "$depfile"
rg -q 'biblio/references.bib' "$depfile"
rg -q 'omnidoc-native-probe.sty' "$depfile"

jq -e '
  .reports[0]
  | .output == "pdf"
    and .skipped == false
    and (.cache_details | index("forced_by_user"))
    and .toolchain.latex_engine == "Tectonic 0.16.9"
    and .toolchain.latex_engine_kind == "tectonic"
    and .toolchain.latex_engine_origin == "configured"
    and (.toolchain | has("tex_kpathsea") | not)
    and ([.resources[].logical_name | select(startswith("latex-fls-input:"))] | length > 0)
' "$report" >/dev/null

"$bin" lock --check "$project"
"$bin" build "$project" --report
jq -e '
  .reports[0]
  | .skipped == true
    and .cache_reason == "input_digest_match"
    and (.cache_details | length == 0)
' "$report" >/dev/null

printf '\n%% native Tectonic dependency invalidation probe\n' \
  >> "$project/tex/local/omnidoc-native-probe.sty"
"$bin" build "$project" --report
jq -e '
  .reports[0]
  | .skipped == false
    and .cache_reason == "input_digest_changed"
    and any(
      .cache_details[];
      (startswith("dependency_changed:") or startswith("resource_changed:"))
        and contains("omnidoc-native-probe.sty")
    )
' "$report" >/dev/null

echo "Native Tectonic LaTeX checks passed"
