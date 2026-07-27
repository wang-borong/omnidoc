#!/usr/bin/env python3
"""Small, dependency-free documentation lint rules for OmniDoc."""

from __future__ import annotations

import os
import pathlib
import re


SKIPPED_DIRS = {
    ".git",
    ".omnidoc-cache",
    "build",
    "dist",
    "node_modules",
    "target",
}
MARKER = re.compile(r"\b(TODO|FIXME)\b", re.IGNORECASE)
HEADING = re.compile(r"^#\s+\S")
FENCE = re.compile(r"^\s*(```+|~~~+)")


def markdown_files(root: pathlib.Path):
    for path in sorted(root.rglob("*")):
        if not path.is_file() or path.suffix.lower() not in {".md", ".markdown"}:
            continue
        if any(part in SKIPPED_DIRS for part in path.relative_to(root).parts):
            continue
        yield path


def inspect(path: pathlib.Path, root: pathlib.Path) -> None:
    relative = path.relative_to(root).as_posix()
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    in_fence = False
    fence_marker = ""
    has_heading = False
    has_prose = False

    for line_number, line in enumerate(lines, 1):
        fence = FENCE.match(line)
        if fence:
            marker = fence.group(1)[0]
            if not in_fence:
                in_fence = True
                fence_marker = marker
            elif marker == fence_marker:
                in_fence = False
            continue
        if in_fence:
            continue
        if HEADING.match(line):
            has_heading = True
        stripped = line.strip()
        if stripped and not stripped.startswith(("---", "...", "#", "<!--")):
            has_prose = True
        for match in MARKER.finditer(line):
            print(
                f"warning:{relative}:{line_number}:{match.start() + 1}:"
                f"remove or resolve {match.group(1).upper()} before publishing"
            )

    if has_prose and not has_heading:
        print(f"warning:{relative}:1:1:add a level-one heading to the document")


def main() -> None:
    root = pathlib.Path(os.environ["OMNIDOC_PROJECT_DIR"]).resolve()
    for path in markdown_files(root):
        inspect(path, root)


if __name__ == "__main__":
    main()
