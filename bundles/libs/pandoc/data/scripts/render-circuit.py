#!/usr/bin/env python3
"""Render a trusted Schemdraw circuit source file to SVG, PDF, or PNG."""

from __future__ import annotations

import argparse
import os
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()

    os.environ.setdefault("MPLCONFIGDIR", "/tmp/omnidoc-matplotlib")
    import schemdraw
    from schemdraw import elements as elm

    drawing = schemdraw.Drawing(show=False)
    drawing.config(unit=3.0, fontsize=12, font="Noto Sans CJK SC", color="#172033", lw=1.4)
    namespace = {
        "__file__": str(args.source),
        "__name__": "__omnidoc_circuit__",
        "schemdraw": schemdraw,
        "elm": elm,
        "d": drawing,
    }
    source = args.source.read_text(encoding="utf-8")
    exec(compile(source, str(args.source), "exec"), namespace)
    result = namespace.get("drawing", namespace.get("d", drawing))
    if not hasattr(result, "save"):
        raise TypeError("circuit source must leave a Schemdraw Drawing in 'd' or 'drawing'")

    args.destination.parent.mkdir(parents=True, exist_ok=True)
    result.save(str(args.destination), transparent=True)
    if not args.destination.is_file() or args.destination.stat().st_size == 0:
        raise RuntimeError(f"circuit renderer did not create {args.destination}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
