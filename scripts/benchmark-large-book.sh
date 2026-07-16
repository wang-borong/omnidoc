#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
libs="${OMNIDOC_LIBS:-$root/../omnidoc-libs}"
output="${OMNIDOC_BENCHMARK_DIR:-$root/_cicd-intermediates/benchmark}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

for tool in cargo pandoc pandoc-crossref jq; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
test -d "$libs/pandoc" || { echo "invalid OMNIDOC_LIBS: $libs" >&2; exit 1; }

mkdir -p "$work/book/chapters" "$work/data" "$work/config" "$work/home" "$output"
cp -a "$libs" "$work/data/omnidoc"
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"
export XDG_DATA_HOME="$work/data"
export XDG_CONFIG_HOME="$work/config"
export HOME="$work/home"
export CARGO_HOME="$cargo_home"
export RUSTUP_HOME="$rustup_home"

printf '%s\n' \
  '[project]' \
  'entry = "main.md"' \
  'from = "markdown"' \
  'to = "html"' \
  'target = "large-book"' \
  '' \
  '[build]' \
  'outdir = "build"' \
  'outputs = ["html"]' \
  '' \
  '[theme]' \
  'name = "engineering-book"' \
  'version = "1"' \
  'compatibility = "readium"' \
  > "$work/book/.omnidoc.toml"

printf '%s\n\n' '---' 'title: Large Book Benchmark' 'lang: zh-CN' '---' > "$work/book/main.md"
for chapter in $(seq 1 100); do
  file="chapters/chapter-$chapter.md"
  printf '# 第 %s 章 Benchmark Chapter %s\n\n' "$chapter" "$chapter" > "$work/book/$file"
  for section in $(seq 1 10); do
    printf '## 第 %s 节 Section %s\n\n' "$section" "$section" >> "$work/book/$file"
    printf '这是用于测量大型书籍构建性能的中英文混排正文。Cache dependency tracking must remain deterministic.\n\n' >> "$work/book/$file"
    printf '公式 $E = mc^2$，以及块级公式：\n\n$$\n\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}\n$$\n\n' >> "$work/book/$file"
  done
  printf '```{.include format="markdown"}\n%s\n```\n\n' "$file" >> "$work/book/main.md"
done

cargo build --manifest-path "$root/Cargo.toml" --locked --release
bin="$root/target/release/omnidoc"
"$bin" build "$work/book" --to html --force --report --write-lock
cp "$work/book/build/omnidoc-report.json" "$output/cold.json"
"$bin" build "$work/book" --to html --report
cp "$work/book/build/omnidoc-report.json" "$output/cached.json"

jq -n \
  --slurpfile cold "$output/cold.json" \
  --slurpfile cached "$output/cached.json" \
  '{
    chapters: 100,
    sections_per_chapter: 10,
    cold_duration_ms: $cold[0].reports[0].duration_ms,
    cached_duration_ms: $cached[0].reports[0].duration_ms,
    cached_skipped: $cached[0].reports[0].skipped,
    input_digest: $cached[0].reports[0].input_digest,
    artifact_digest: $cached[0].reports[0].artifact_digest
  }' > "$output/summary.json"
jq -e '.cached_skipped == true' "$output/summary.json" >/dev/null
jq . "$output/summary.json"
