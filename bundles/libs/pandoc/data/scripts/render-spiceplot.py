#!/usr/bin/env python3
"""Run an ngspice analysis described by JSON and render its traces."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path


def trace_definition(value: object) -> tuple[str, str]:
    if isinstance(value, str):
        return value, value
    if isinstance(value, dict) and isinstance(value.get("expr"), str):
        return value["expr"], str(value.get("label", value["expr"]))
    raise ValueError("each trace must be a string or an object containing 'expr'")


def wrapper_netlist(netlist: str, analysis: str, data_path: Path, expressions: list[str]) -> str:
    lines = netlist.splitlines()
    while lines and not lines[-1].strip():
        lines.pop()
    if lines and lines[-1].strip().lower() == ".end":
        lines.pop()
    escaped = str(data_path).replace("\\", "/")
    lines.extend([
        ".control",
        "set wr_singlescale",
        analysis,
        f"wrdata {escaped} {' '.join(expressions)}",
        "quit",
        ".endc",
        ".end",
    ])
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("spec", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--ngspice", default=os.environ.get("NGSPICE", "ngspice"))
    args = parser.parse_args()

    spec = json.loads(args.spec.read_text(encoding="utf-8"))
    netlist_value = spec.get("netlist")
    analysis = spec.get("analysis")
    traces_value = spec.get("traces")
    if not isinstance(netlist_value, str) or not netlist_value:
        raise ValueError("spiceplot requires a non-empty 'netlist' path")
    if not isinstance(analysis, str) or not analysis.strip():
        raise ValueError("spiceplot requires an ngspice 'analysis' command")
    if not isinstance(traces_value, list) or not traces_value:
        raise ValueError("spiceplot requires a non-empty 'traces' array")

    traces = [trace_definition(value) for value in traces_value]
    netlist_path = Path(netlist_value)
    if not netlist_path.is_absolute():
        netlist_path = Path.cwd() / netlist_path
    netlist = netlist_path.read_text(encoding="utf-8")

    with tempfile.TemporaryDirectory(prefix="omnidoc-spiceplot-") as temporary:
        temporary_path = Path(temporary)
        data_path = temporary_path / "traces.txt"
        wrapper_path = temporary_path / "analysis.cir"
        log_path = temporary_path / "ngspice.log"
        wrapper_path.write_text(
            wrapper_netlist(netlist, analysis, data_path, [expr for expr, _ in traces]),
            encoding="utf-8",
        )
        completed = subprocess.run(
            [args.ngspice, "-b", "-o", str(log_path), str(wrapper_path)],
            text=True,
            capture_output=True,
            check=False,
        )
        if completed.returncode != 0 or not data_path.is_file():
            log = log_path.read_text(encoding="utf-8", errors="replace") if log_path.exists() else ""
            raise RuntimeError(
                f"ngspice analysis failed for {netlist_path}\n{completed.stderr}\n{log}".strip()
            )

        os.environ.setdefault("MPLCONFIGDIR", "/tmp/omnidoc-matplotlib")
        import matplotlib

        matplotlib.rcParams["font.family"] = "sans-serif"
        matplotlib.rcParams["font.sans-serif"] = ["Noto Sans CJK SC", "DejaVu Sans"]
        matplotlib.rcParams["axes.unicode_minus"] = False
        import matplotlib.pyplot as plt
        import numpy as np

        data = np.loadtxt(data_path, ndmin=2)
        if data.shape[1] != len(traces) + 1:
            raise RuntimeError(
                f"ngspice returned {data.shape[1] - 1} traces, expected {len(traces)}"
            )
        x = data[:, 0] * float(spec.get("x_multiplier", 1.0))
        figure, axis = plt.subplots(figsize=tuple(spec.get("figsize", [7.2, 4.0])))
        plot = axis.semilogx if spec.get("xscale") == "log" else axis.plot
        for index, (_, label) in enumerate(traces, start=1):
            plot(x, data[:, index], label=label, linewidth=1.8)
        axis.set_xlabel(str(spec.get("xlabel", "x")))
        axis.set_ylabel(str(spec.get("ylabel", "y")))
        if spec.get("title"):
            axis.set_title(str(spec["title"]))
        if spec.get("yscale") == "log":
            axis.set_yscale("log")
        axis.grid(True, which="both", alpha=0.28)
        if len(traces) > 1 or spec.get("legend", True):
            axis.legend(frameon=False)
        figure.tight_layout()
        args.destination.parent.mkdir(parents=True, exist_ok=True)
        figure.savefig(args.destination, transparent=True, bbox_inches="tight")
        plt.close(figure)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
