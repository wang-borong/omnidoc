#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$root/tests/fixtures/golden-book"
libs="${OMNIDOC_LIBS:-$root/bundles/libs}"
pdf_engine="${OMNIDOC_PDF_ENGINE:-xelatex}"
work="$(mktemp -d)"
if [[ -n "${OMNIDOC_KEEP_WORK:-}" ]]; then
  echo "Golden PDF work directory: $work"
else
  trap 'rm -rf "$work"' EXIT
fi

required_tools=(pandoc pandoc-crossref pdfinfo pdffonts pdftotext pdftoppm jq rg fc-match python3)
if [[ -z "${OMNIDOC_BIN:-}" ]]; then
  required_tools+=(cargo)
fi
for tool in "${required_tools[@]}"; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
test -d "$libs/pandoc" || { echo "invalid OMNIDOC_LIBS: $libs" >&2; exit 1; }

case "$pdf_engine" in
  xelatex)
    command -v xelatex >/dev/null || { echo "missing required tool: xelatex" >&2; exit 1; }
    engine_program="$(command -v xelatex)"
    ;;
  tectonic)
    if [[ -n "${OMNIDOC_TECTONIC_BIN:-}" ]]; then
      engine_program="$OMNIDOC_TECTONIC_BIN"
      test -x "$engine_program" || { echo "invalid OMNIDOC_TECTONIC_BIN: $engine_program" >&2; exit 1; }
    else
      command -v tectonic >/dev/null || { echo "missing required tool: tectonic" >&2; exit 1; }
      engine_program="$(command -v tectonic)"
    fi
    ;;
  *)
    echo "unsupported OMNIDOC_PDF_ENGINE: $pdf_engine" >&2
    exit 1
    ;;
esac

for family in "Noto Serif CJK SC" "Noto Sans CJK SC" "Noto Sans Mono CJK SC"; do
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
cp -a "$fixture" "$work/book"
python3 - "$work/book/.omnidoc.toml" "$engine_program" "$work/texmf" "$pdf_engine" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
content = path.read_text(encoding="utf-8")
content = content.replace('outputs = ["html", "epub"]', 'outputs = ["pdf"]')
content = content.replace(
    'epub = ["--toc", "--toc-depth=3"]',
    'epub = ["--toc", "--toc-depth=3"]\n'
    'pdf = ["--include-in-header=fls-probe.tex"]',
)
content += f"\n[tools]\nlatex_engine = {json.dumps(sys.argv[2])}\n"
if sys.argv[4] == "tectonic":
    content += f"\n[tectonic]\nsearch_paths = [{json.dumps(sys.argv[3])}]\n"
path.write_text(content, encoding="utf-8")
PY
mkdir -p "$work/data" "$work/config" "$work/home"
mkdir -p "$work/texmf/tex/latex/omnidoc-fls-probe"
printf '\\usepackage{omnidoc-fls-probe}\n' > "$work/book/fls-probe.tex"
printf '\\ProvidesPackage{omnidoc-fls-probe}[2026/07/16 OmniDoc FLS probe]\n' \
  > "$work/texmf/tex/latex/omnidoc-fls-probe/omnidoc-fls-probe.sty"
cp -a "$libs" "$work/data/omnidoc"

export XDG_DATA_HOME="$work/data"
export XDG_CONFIG_HOME="$work/config"
export HOME="$work/home"
export TEXINPUTS="$work/texmf//:"

# Read-only commands intentionally do not create global configuration. Create
# the isolated defaults explicitly before customizing TEXINPUTS below.
"$bin" config init --author "OmniDoc CI"

pdf="$work/book/build/golden-book.pdf"
report="$work/book/build/omnidoc-report.json"
lock="$work/book/omnidoc.lock"
include_depfile="$work/book/.omnidoc-cache/include-files.d"
include_code_depfile="$work/book/.omnidoc-cache/include-code-files.d"
latex_input_depfile="$work/book/.omnidoc-cache/latex-inputs.d"

"$bin" doctor --strict "$work/book"
if [[ "$pdf_engine" == "tectonic" ]]; then
  "$bin" theme validate engineering-book --check-fonts --json > "$work/theme.json"
  jq -e '
    .[0]
    | .valid == true
      and .font_check_performed == true
      and (.missing_fonts | length == 0)
      and .latex_check_performed == false
  ' "$work/theme.json" >/dev/null
else
  "$bin" theme validate engineering-book --check-fonts --check-latex --json > "$work/theme.json"
  jq -e '
    .[0]
    | .valid == true
      and .font_check_performed == true
      and (.missing_fonts | length == 0)
      and .latex_check_performed == true
      and (.missing_latex_packages | length == 0)
  ' "$work/theme.json" >/dev/null
fi

python3 - "$work/config/omnidoc.toml" "$work/texmf" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
content = path.read_text(encoding="utf-8")
path.write_text(
    content.replace('texinputs = "./tex//:"', f'texinputs = "{sys.argv[2]}//:"'),
    encoding="utf-8",
)
PY

"$bin" build "$work/book" --to pdf --force --report --write-lock

test -s "$pdf"
test -s "$report"
test -s "$lock"
test -s "$include_depfile"
test -s "$include_code_depfile"
test -s "$latex_input_depfile"

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

visual_dir="${OMNIDOC_PDF_VISUAL_DIR:-$work/visual}"
mkdir -p "$visual_dir"
if [[ -n "${OMNIDOC_PDF_VISUAL_DIR:-}" ]]; then
  cp "$pdf" "$visual_dir/golden-book.pdf"
  cp "$work/fonts.txt" "$visual_dir/fonts.txt"
  cp "$work/content.txt" "$visual_dir/content.txt"
fi
visual_mode="${OMNIDOC_PDF_VISUAL_MODE:-check}"
python3 "$root/scripts/pdf-visual-contract.py" \
  "$visual_mode" \
  "$pdf" \
  "$fixture/pdf-visual-contract.json" \
  --output-dir "$visual_dir"

if [[ "$pdf_engine" == "tectonic" ]]; then
  jq -e '
    .reports[0]
    | .output == "pdf"
      and .skipped == false
      and (.cache_details | index("forced_by_user"))
      and (.artifact_digest | startswith("blake3:"))
      and .toolchain.latex_engine == "Tectonic 0.16.9"
      and .toolchain.latex_engine_kind == "tectonic"
      and .toolchain.latex_engine_origin == "configured"
      and .toolchain.tectonic_bundle == "default-web-bundle"
      and (.toolchain | has("tex_kpathsea") | not)
      and (.toolchain["font:Noto Serif CJK SC"] | contains("digest=blake3:"))
      and (.toolchain["font:Noto Sans CJK SC"] | contains("digest=blake3:"))
      and (.toolchain["font:Noto Sans Mono CJK SC"] | contains("digest=blake3:"))
      and ([.resources[].logical_name | select(startswith("latex-fls-input:"))] | length > 3)
  ' "$report" >/dev/null
else
  jq -e '
    .reports[0]
    | .output == "pdf"
      and .skipped == false
      and (.cache_details | index("forced_by_user"))
      and (.artifact_digest | startswith("blake3:"))
      and (.toolchain.latex_engine | startswith("XeTeX "))
      and .toolchain.latex_engine_kind == "xelatex"
      and (.toolchain["font:Noto Serif CJK SC"] | contains("digest=blake3:"))
      and (.toolchain["font:Noto Sans CJK SC"] | contains("digest=blake3:"))
      and (.toolchain["font:Noto Sans Mono CJK SC"] | contains("digest=blake3:"))
      and (.toolchain["latex-package:fontspec"] | contains("digest=blake3:"))
      and (.toolchain["latex-package:xeCJK"] | contains("digest=blake3:"))
      and (.toolchain.tex_kpathsea | startswith("kpathsea version "))
      and ([.resources[].logical_name | select(startswith("latex-fls-input:"))] | length > 20)
  ' "$report" >/dev/null
fi

python3 - "$lock" "$pdf_engine" <<'PY'
import pathlib
import sys
import tomllib

lock_path = pathlib.Path(sys.argv[1])
lock_text = lock_path.read_text(encoding="utf-8")
if str(lock_path.parent.parent) in lock_text:
    raise SystemExit("lock contains a machine-specific temporary path")
lock = tomllib.loads(lock_text)
if lock.get("lock_version") != 4:
    raise SystemExit("expected lock schema v4")
target = lock.get("targets", {}).get("pdf")
if target is None:
    raise SystemExit("missing PDF lock target")
if not target.get("input_digest", "").startswith("blake3:"):
    raise SystemExit("missing PDF input digest")
toolchain = lock.get("toolchain", {})
engine = sys.argv[2]
for family in {
    "Noto Serif CJK SC",
    "Noto Sans CJK SC",
    "Noto Sans Mono CJK SC",
}:
    identity = toolchain.get(f"font:{family}", "")
    if "digest=blake3:" not in identity:
        raise SystemExit(f"missing locked font identity: {family}")
if engine == "tectonic":
    if toolchain.get("latex_engine") != "Tectonic 0.16.9":
        raise SystemExit("missing locked Tectonic identity")
    if toolchain.get("tectonic_bundle") != "default-web-bundle":
        raise SystemExit("missing locked Tectonic bundle policy")
    if "tex_kpathsea" in toolchain:
        raise SystemExit("Tectonic lock unexpectedly depends on kpsewhich")
else:
    for package in {"fontspec", "xeCJK", "tcolorbox", "tikz"}:
        identity = toolchain.get(f"latex-package:{package}", "")
        if "digest=blake3:" not in identity:
            raise SystemExit(f"missing locked LaTeX package identity: {package}")
if not toolchain.get("pandoc", "").startswith("pandoc "):
    raise SystemExit("missing Pandoc toolchain identity for the built-in LaTeX template")
dependencies = set(target.get("dependencies", []))
for expected in {
    "assets/cover.pdf",
    "assets/diagram.pdf",
    "chapters/chapter-one.md",
    "chapters/nested/details.md",
    "assets/example.rs",
    "fls-probe.tex",
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
if not any(resource.startswith("latex-fls-input:") for resource in resources):
    raise SystemExit("missing .fls-derived LaTeX resources")
expected_engine_resources = {"omnidoc-fls-probe.sty"}
if engine != "tectonic":
    expected_engine_resources.add("fontspec.sty")
for expected in expected_engine_resources:
    if not any(resource.endswith(expected) for resource in resources):
        raise SystemExit(f"missing .fls-derived resource: {expected}")
PY

rg -q 'chapters/chapter-one.md' "$include_depfile"
rg -q 'chapters/nested/details.md' "$include_depfile"
rg -q 'assets/example.rs' "$include_code_depfile"
rg -q 'omnidoc-fls-probe.sty' "$latex_input_depfile"
if [[ "$pdf_engine" != "tectonic" ]]; then
  rg -q 'fontspec.sty' "$latex_input_depfile"
fi
"$bin" lock --check "$work/book"

"$bin" build "$work/book" --to pdf --report
jq -e '
  .reports[0]
  | .skipped == true
    and .cache_reason == "input_digest_match"
    and (.cache_details | length == 0)
' "$report" >/dev/null

printf '\n%% configured Pandoc header invalidation probe\n' >> "$work/book/fls-probe.tex"
"$bin" build "$work/book" --to pdf --report
jq -e '
  .reports[0]
  | .skipped == false
    and .cache_reason == "input_digest_changed"
    and (.cache_details | index("dependency_changed:fls-probe.tex"))
' "$report" >/dev/null

printf '\n%% indirect TeX dependency invalidation probe\n' \
  >> "$work/texmf/tex/latex/omnidoc-fls-probe/omnidoc-fls-probe.sty"
"$bin" build "$work/book" --to pdf --report
jq -e '
  .reports[0]
  | .skipped == false
    and .cache_reason == "input_digest_changed"
    and any(.cache_details[]; startswith("resource_changed:") and endswith("omnidoc-fls-probe.sty#1"))
' "$report" >/dev/null

printf '\n%% cache invalidation probe\n' >> "$work/data/omnidoc/texmf/tex/common/omni-engineering-book.sty"
"$bin" build "$work/book" --to pdf --report
jq -e '.reports[0].skipped == false and .reports[0].cache_reason == "input_digest_changed"' "$report" >/dev/null

cp "$work/book/assets/cover.pdf" "$work/book/assets/diagram.pdf"
"$bin" build "$work/book" --to pdf --report
jq -e '
  .reports[0]
  | .skipped == false
    and .cache_reason == "input_digest_changed"
    and (.cache_details | index("dependency_changed:assets/diagram.pdf"))
' "$report" >/dev/null

echo "Golden PDF checks passed"
