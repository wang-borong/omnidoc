#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for tool in pandoc rg; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done

pandoc "$root/tests/filter-smoke.md" \
  --mathml \
  --lua-filter="$root/pandoc/data/filters/display-math.lua" \
  --lua-filter="$root/pandoc/data/filters/include-files.lua" \
  --lua-filter="$root/pandoc/data/filters/emoji.lua" \
  --css="$root/pandoc/css/omnidoc-base.css" \
  --standalone --embed-resources --toc -t html5 -o "$work/smoke.html"

rg -q 'display="inline"' "$work/smoke.html"
rg -q 'class="omni-display-math"' "$work/smoke.html"
rg -q 'display="block"' "$work/smoke.html"
rg -q 'href="#summary-1"' "$work/smoke.html"
rg -q '\.omni-display-math' "$work/smoke.html"
rg -q '🌟' "$work/smoke.html"

block_css=(
  "$root/pandoc/css/omnidoc-base.css"
  "$root/pandoc/css/modules/engineering-tokens.css"
  "$root/pandoc/css/engineering-book.css"
  "$root/pandoc/css/modules/semantic-blocks.css"
  "$root/pandoc/css/modules/code.css"
  "$root/pandoc/css/modules/tables.css"
  "$root/pandoc/css/modules/math.css"
  "$root/pandoc/css/modules/figures.css"
)
block_css_args=()
for css in "${block_css[@]}"; do
  block_css_args+=(--css="$css")
done

pandoc "$root/tests/blocks-showcase.md" \
  --lua-filter="$root/pandoc/data/filters/admonition.lua" \
  "${block_css_args[@]}" \
  --standalone --embed-resources -t html5 -o "$work/blocks-showcase.html"
for kind in note tip important warning error question answer example exercise solution; do
  rg -q "class=\"admonition $kind\"" "$work/blocks-showcase.html"
  rg -q "data-kind=\"$kind\"" "$work/blocks-showcase.html"
done

pandoc "$root/tests/blocks-showcase.md" \
  --lua-filter="$root/pandoc/data/filters/admonition.lua" \
  "${block_css_args[@]}" \
  --standalone -t epub3 -o "$work/blocks-showcase.epub"
test -s "$work/blocks-showcase.epub"
if command -v unzip >/dev/null; then
  unzip -tqq "$work/blocks-showcase.epub"
fi

cat >"$work/fake-omnidoc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
source="$3"
format="svg"
output="."
shift 3
while (($#)); do
  case "$1" in
    --format) format="$2"; shift 2 ;;
    --output) output="$2"; shift 2 ;;
    --force) shift ;;
    *) shift ;;
  esac
done
stem="$(basename "${source%.*}")"
test "$format" = svg
label="${BITFIELD_LABEL:-bitfield smoke}"
cat >"$output/$stem.svg" <<SVG
<svg xmlns="http://www.w3.org/2000/svg" width="120" height="24" viewBox="0 0 120 24">
  <text x="4" y="17">$label</text>
</svg>
SVG
EOF
chmod +x "$work/fake-omnidoc"

cat >"$work/bitfield.md" <<'EOF'
```{.bitfield #fig-smoke caption="Bitfield smoke"}
{"entries":[{"name":"VALUE","bits":7},{"name":"READY","bits":1}]}
```
EOF

(
  cd "$work"
  OMNIDOC_BIN="$work/fake-omnidoc" pandoc bitfield.md \
    --lua-filter="$root/pandoc/data/filters/diagram-generator.lua" \
    --standalone --embed-resources -t html5 -o bitfield.html
)
rg -q 'data:image/svg\+xml;base64' "$work/bitfield.html"
rg -q 'Bitfield smoke' "$work/bitfield.html"
rg -q 'bitfield smoke' "$work/figures/fig-smoke.svg"

# A stable figure identifier must not preserve stale rendered bytes when the
# fenced diagram source or renderer output changes.
(
  cd "$work"
  BITFIELD_LABEL="bitfield updated" OMNIDOC_BIN="$work/fake-omnidoc" \
    pandoc bitfield.md \
      --lua-filter="$root/pandoc/data/filters/diagram-generator.lua" \
      --standalone --embed-resources -t html5 -o bitfield-updated.html
)
rg -q 'bitfield updated' "$work/figures/fig-smoke.svg"

# Exercise the native circuit and spiceplot blocks when their optional Python
# dependencies and ngspice are available. The package smoke test remains
# usable on minimal release hosts, while fully provisioned CI validates the
# complete rendering path and dependency tracking contract.
if command -v ngspice >/dev/null && \
   MPLCONFIGDIR="$work/matplotlib" python3 -c 'import matplotlib, numpy, schemdraw' >/dev/null 2>&1; then
  cat >"$work/divider.py" <<'EOF'
d += elm.SourceV().up().label("5 V")
d += elm.Resistor().right().label("1 kΩ")
d += elm.Diode().down().label("D")
d += elm.Line().left()
EOF

  cat >"$work/divider.cir" <<'EOF'
OmniDoc spiceplot smoke test
V1 in 0 5
R1 in out 1k
D1 out 0 DTEST
.model DTEST D(Is=1e-15)
.end
EOF

  cat >"$work/divider.json" <<'EOF'
{
  "netlist": "divider.cir",
  "analysis": "dc V1 0 5 0.1",
  "traces": [{"expr": "v(out)", "label": "V(out)"}],
  "xlabel": "Input voltage (V)",
  "ylabel": "Output voltage (V)"
}
EOF

  cat >"$work/native-diagrams.md" <<'EOF'
```{.circuit #fig-circuit-smoke include-code="divider.py" caption="Circuit smoke" width="65%"}
```

```{.spiceplot #fig-spiceplot-smoke include-code="divider.json" caption="Spiceplot smoke" width="80%"}
```
EOF

  (
    cd "$work"
    MPLCONFIGDIR="$work/matplotlib" pandoc native-diagrams.md \
      --metadata omnidoc-depfile-include-code-files="$work/include-code.d" \
      --metadata omnidoc-depfile-diagram-generator="$work/diagram-generator.d" \
      --lua-filter="$root/pandoc/data/filters/include-code-files.lua" \
      --lua-filter="$root/pandoc/data/filters/diagram-generator.lua" \
      --standalone --embed-resources -t html5 -o native-diagrams.html
  )
  rg -q 'Circuit smoke' "$work/native-diagrams.html"
  rg -q 'Spiceplot smoke' "$work/native-diagrams.html"
  test -s "$work/figures/fig-circuit-smoke.svg"
  test -s "$work/figures/fig-spiceplot-smoke.svg"
  rg -q "$work/divider.py" "$work/include-code.d"
  rg -q "$work/divider.json" "$work/include-code.d"
  rg -q "$work/divider.cir" "$work/diagram-generator.d"
fi

# OmniDoc-managed theme headers must be appended to, rather than replace, a
# project's own header-includes metadata.
cat >"$work/managed-header.tex" <<'EOF'
\newcommand{\OmniManagedHeaderMarker}{managed}
EOF
cat >"$work/header-smoke.md" <<'EOF'
---
header-includes:
  - \newcommand{\ProjectHeaderMarker}{project}
---

Header smoke.
EOF
pandoc "$work/header-smoke.md" \
  --metadata omnidoc-default-latex-header="$work/managed-header.tex" \
  --lua-filter="$root/pandoc/data/filters/latex-headers.lua" \
  --standalone -t latex -o "$work/header-smoke.tex"
rg -Fq '\newcommand{\ProjectHeaderMarker}{project}' "$work/header-smoke.tex"
rg -Fq '\newcommand{\OmniManagedHeaderMarker}{managed}' "$work/header-smoke.tex"

"$root/scripts/check-pandoc-latex-template.sh"

echo "omnidoc-libs filter smoke test passed"
