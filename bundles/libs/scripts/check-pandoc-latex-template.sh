#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for tool in pandoc pandoc-crossref rg rsvg-convert xelatex pdfinfo; do
  command -v "$tool" >/dev/null || {
    echo "missing required LaTeX template contract tool: $tool" >&2
    exit 1
  }
done

pandoc -D latex > "$work/default.latex"
for hook in \
  '$passoptions.latex()$' \
  '$fonts.latex()$' \
  '$common.latex()$' \
  '$for(header-includes)$' \
  '$body$' \
  '$for(include-after)$'; do
  rg -Fq "$hook" "$work/default.latex" || {
    echo "Pandoc default LaTeX template is missing required hook: $hook" >&2
    exit 1
  }
done

printf '%s\n' \
  '---' \
  'title: Template Contract Probe' \
  'subtitle: Built-in Pandoc template with OmniDoc theme overlay' \
  'author:' \
  '  - Alice Example' \
  '  - Bob Example' \
  'lang: en-US' \
  'documentclass: book' \
  'classoption: openany' \
  '---' \
  '' \
  '# Writer features' \
  '' \
  'Inline `code` must keep its writer support after external filters.' \
  '' \
  '```console' \
  '$ omnidoc build --to pdf' \
  '{"messages":[{"role":"user","content":"extract contract clauses and return a schema with currency, payment_amount, payment_method, payment_due_days, and penalty_rate without inventing missing values"}]}' \
  '```' \
  '' \
  '| Feature | Contract |' \
  '|---|---|' \
  '| Template | Pandoc built-in |' \
  '| Theme | Header plus style package |' \
  '' \
  '![SVG conversion contract](probe.svg){width=35%}' \
  '' \
  '> Theme typography and boxes remain external to the upstream template.' \
  '' \
  'Emoji remains visible in PDF output: 🌟 ❤️ 👩‍💻.' \
  > "$work/probe.md"

printf '%s\n' \
  '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="120">' \
  '  <rect width="320" height="120" rx="18" fill="#176b87"/>' \
  '  <text x="160" y="72" text-anchor="middle" fill="white" font-size="28">OmniDoc</text>' \
  '</svg>' \
  > "$work/probe.svg"

TEXMFHOME="$root/texmf//:" pandoc "$work/probe.md" \
  --standalone \
  --resource-path="$work" \
  --syntax-highlighting=idiomatic \
  --variable=listings=true \
  --filter=pandoc-crossref \
  --lua-filter="$root/pandoc/data/filters/latex-patch.lua" \
  --lua-filter="$root/pandoc/data/filters/emoji.lua" \
  --include-in-header="$root/pandoc/headers/emoji.tex" \
  --include-in-header="$root/pandoc/headers/engineering-book.tex" \
  --pdf-engine=xelatex \
  -o "$work/probe.pdf"

pdfinfo "$work/probe.pdf" | rg -q '^Pages:[[:space:]]+[1-9][0-9]*$'

TEXMFHOME="$root/texmf//:" pandoc "$root/tests/blocks-showcase.md" \
  --standalone \
  --lua-filter="$root/pandoc/data/filters/admonition.lua" \
  --include-in-header="$root/pandoc/headers/engineering-book.tex" \
  --pdf-engine=xelatex \
  -o "$work/blocks-showcase.pdf"
pdfinfo "$work/blocks-showcase.pdf" | rg -q '^Pages:[[:space:]]+[1-9][0-9]*$'

TEXMFHOME="$root/texmf//:" pandoc "$work/probe.md" \
  --standalone \
  --lua-filter="$root/pandoc/data/filters/emoji.lua" \
  --include-in-header="$root/pandoc/headers/emoji.tex" \
  -t latex \
  -o "$work/probe.tex"
rg -Fq '\omnidocEmoji{1f31f}{1f31f}' "$work/probe.tex"
! rg -Fq '🌟' "$work/probe.tex"

echo "Pandoc built-in LaTeX template contract passed"
