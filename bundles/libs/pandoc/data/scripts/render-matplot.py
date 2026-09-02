#!/usr/bin/env python3
"""Render a trusted Matplotlib snippet to SVG, PDF, or PNG."""

from __future__ import annotations

import argparse
import os
import tempfile
from pathlib import Path


SUPPORTED_FORMATS = {"svg", "pdf", "png"}


def output_metadata(output_format: str) -> dict[str, object]:
    if output_format == "svg":
        return {"Creator": "OmniDoc matplot", "Date": None}
    if output_format == "pdf":
        return {
            "Creator": "OmniDoc matplot",
            "CreationDate": None,
            "ModDate": None,
        }
    return {"Software": "OmniDoc matplot"}


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Render trusted Python plotting code with Matplotlib"
    )
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()

    output_format = args.destination.suffix.lower().lstrip(".")
    if output_format not in SUPPORTED_FORMATS:
        supported = ", ".join(sorted(SUPPORTED_FORMATS))
        raise ValueError(f"matplot output must be one of: {supported}")

    config_dir = Path(tempfile.gettempdir()) / "omnidoc-matplotlib"
    config_dir.mkdir(parents=True, exist_ok=True)
    os.environ.setdefault("MPLCONFIGDIR", str(config_dir))

    try:
        import matplotlib

        matplotlib.use("Agg")
        matplotlib.rcParams.update(
            {
                "font.family": "sans-serif",
                "font.sans-serif": ["Noto Sans CJK SC", "DejaVu Sans"],
                "axes.unicode_minus": False,
                "svg.hashsalt": "omnidoc-matplot",
            }
        )
        import matplotlib.pyplot as plt
        import numpy as np
        from matplotlib.figure import Figure
    except ModuleNotFoundError as error:
        raise RuntimeError(
            "matplot requires the Python packages 'numpy' and 'matplotlib'"
        ) from error

    source = args.source.read_text(encoding="utf-8")
    initial_figure, initial_axis = plt.subplots(figsize=(7.2, 4.2))
    namespace = {
        "__name__": "__matplot__",
        "__file__": str(args.source),
        "np": np,
        "numpy": np,
        "plt": plt,
        "fig": initial_figure,
        "figure": initial_figure,
        "ax": initial_axis,
        "axis": initial_axis,
    }

    try:
        exec(compile(source, str(args.source), "exec"), namespace)

        fig_value = namespace.get("fig")
        figure_value = namespace.get("figure")
        current_figure = plt.gcf()
        if isinstance(fig_value, Figure) and fig_value is not initial_figure:
            rendered_figure = fig_value
        elif isinstance(figure_value, Figure) and figure_value is not initial_figure:
            rendered_figure = figure_value
        elif current_figure is not initial_figure:
            rendered_figure = current_figure
        else:
            rendered_figure = initial_figure

        if not rendered_figure.axes:
            raise RuntimeError("matplot code did not create any axes")

        args.destination.parent.mkdir(parents=True, exist_ok=True)
        rendered_figure.savefig(
            args.destination,
            format=output_format,
            dpi=180,
            transparent=True,
            bbox_inches="tight",
            pad_inches=0.08,
            metadata=output_metadata(output_format),
        )
    finally:
        plt.close("all")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
