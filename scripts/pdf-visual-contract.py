#!/usr/bin/env python3
"""Capture or verify a page-aware visual contract for a PDF document."""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile


def read_pgm(path: pathlib.Path) -> tuple[int, int, bytes]:
    data = path.read_bytes()
    position = 0

    def token() -> bytes:
        nonlocal position
        while position < len(data):
            if data[position] == ord("#"):
                position = data.find(b"\n", position)
                if position < 0:
                    raise ValueError(f"unterminated PGM comment in {path}")
            elif chr(data[position]).isspace():
                position += 1
            else:
                break
        start = position
        while position < len(data) and not chr(data[position]).isspace():
            position += 1
        return data[start:position]

    if token() != b"P5":
        raise ValueError(f"unsupported PGM format in {path}")
    width = int(token())
    height = int(token())
    maximum = int(token())
    if maximum != 255:
        raise ValueError(f"unsupported PGM maximum {maximum} in {path}")
    if data[position : position + 2] == b"\r\n":
        position += 2
    elif position < len(data) and chr(data[position]).isspace():
        position += 1
    else:
        raise ValueError(f"missing PGM header delimiter in {path}")
    pixels = data[position:]
    if len(pixels) != width * height:
        raise ValueError(f"invalid PGM payload length in {path}")
    return width, height, pixels


def average_region(
    pixels: bytes,
    width: int,
    height: int,
    x0: int,
    y0: int,
    x1: int,
    y1: int,
) -> int:
    total = 0
    count = 0
    for y in range(y0, y1):
        start = y * width + x0
        end = y * width + x1
        total += sum(pixels[start:end])
        count += end - start
    return total // max(count, 1)


def difference_hash(pixels: bytes, width: int, height: int) -> str:
    samples: list[list[int]] = []
    for row in range(8):
        y0 = row * height // 8
        y1 = max(y0 + 1, (row + 1) * height // 8)
        values = []
        for column in range(9):
            x0 = column * width // 9
            x1 = max(x0 + 1, (column + 1) * width // 9)
            values.append(average_region(pixels, width, height, x0, y0, x1, y1))
        samples.append(values)
    value = 0
    for row in samples:
        for left, right in zip(row, row[1:]):
            value = (value << 1) | int(left > right)
    return f"{value:016x}"


def page_metrics(path: pathlib.Path, threshold: int) -> dict[str, object]:
    width, height, pixels = read_pgm(path)
    left, top, right, bottom = width, height, -1, -1
    ink = 0
    for index, value in enumerate(pixels):
        if value >= threshold:
            continue
        ink += 1
        x = index % width
        y = index // width
        left = min(left, x)
        top = min(top, y)
        right = max(right, x)
        bottom = max(bottom, y)
    bbox = None if right < 0 else [left, top, right, bottom]
    return {
        "width": width,
        "height": height,
        "bbox": bbox,
        "ink_ratio": round(ink / (width * height), 6),
        "dhash": difference_hash(pixels, width, height),
    }


def render_pdf(pdf: pathlib.Path, directory: pathlib.Path, dpi: int) -> list[pathlib.Path]:
    executable = shutil.which("pdftoppm")
    if executable is None:
        raise RuntimeError("pdftoppm is required for PDF visual validation")
    directory.mkdir(parents=True, exist_ok=True)
    for stale in directory.glob("page-*.pgm"):
        stale.unlink()
    report = directory / "visual-report.json"
    if report.exists():
        report.unlink()
    prefix = directory / "page"
    subprocess.run(
        [executable, "-r", str(dpi), "-gray", str(pdf), str(prefix)],
        check=True,
    )
    pages = sorted(
        directory.glob("page-*.pgm"),
        key=lambda path: int(path.stem.rsplit("-", 1)[1]),
    )
    if not pages:
        raise RuntimeError("pdftoppm did not render any pages")
    return pages


def hamming_distance(left: str, right: str) -> int:
    return (int(left, 16) ^ int(right, 16)).bit_count()


def compare_page(index: int, actual: dict, expected: dict, tolerances: dict) -> list[str]:
    errors: list[str] = []
    dimension = int(tolerances["dimension_pixels"])
    for key in ("width", "height"):
        if abs(int(actual[key]) - int(expected[key])) > dimension:
            errors.append(
                f"page {index} {key} changed: expected {expected[key]}, got {actual[key]}"
            )
    actual_bbox = actual["bbox"]
    expected_bbox = expected["bbox"]
    if (actual_bbox is None) != (expected_bbox is None):
        errors.append(f"page {index} blank/nonblank state changed")
    elif actual_bbox is not None and expected_bbox is not None:
        bbox_tolerance = int(tolerances["bbox_pixels"])
        labels = ("left", "top", "right", "bottom")
        for label, actual_value, expected_value in zip(labels, actual_bbox, expected_bbox):
            if abs(int(actual_value) - int(expected_value)) > bbox_tolerance:
                errors.append(
                    f"page {index} bbox {label} changed: expected {expected_value}, got {actual_value}"
                )
    ratio_delta = abs(float(actual["ink_ratio"]) - float(expected["ink_ratio"]))
    if ratio_delta > float(tolerances["ink_ratio"]):
        errors.append(
            f"page {index} ink ratio changed: expected {expected['ink_ratio']}, got {actual['ink_ratio']}"
        )
    distance = hamming_distance(str(actual["dhash"]), str(expected["dhash"]))
    if distance > int(tolerances["dhash_bits"]):
        errors.append(
            f"page {index} perceptual hash changed by {distance} bits "
            f"(limit {tolerances['dhash_bits']})"
        )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("capture", "check"))
    parser.add_argument("pdf", type=pathlib.Path)
    parser.add_argument("contract", type=pathlib.Path)
    parser.add_argument("--output-dir", type=pathlib.Path)
    args = parser.parse_args()

    if not args.pdf.is_file():
        parser.error(f"PDF does not exist: {args.pdf}")

    contract = None
    if args.mode == "check":
        contract = json.loads(args.contract.read_text(encoding="utf-8"))
        if contract.get("contract_version") != 1:
            raise ValueError("unsupported PDF visual contract version")
    dpi = int(contract.get("dpi", 96) if contract else 96)
    threshold = int(contract.get("ink_threshold", 245) if contract else 245)

    temporary = None
    if args.output_dir is None:
        temporary = tempfile.TemporaryDirectory(prefix="omnidoc-pdf-visual-")
        render_dir = pathlib.Path(temporary.name)
    else:
        render_dir = args.output_dir
    pages = render_pdf(args.pdf, render_dir, dpi)
    metrics = [page_metrics(page, threshold) for page in pages]

    if args.mode == "capture":
        captured = {
            "contract_version": 1,
            "dpi": dpi,
            "ink_threshold": threshold,
            "tolerances": {
                "dimension_pixels": 2,
                "bbox_pixels": 24,
                "ink_ratio": 0.025,
                "dhash_bits": 18,
            },
            "pages": metrics,
        }
        args.contract.parent.mkdir(parents=True, exist_ok=True)
        args.contract.write_text(
            json.dumps(captured, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        print(f"captured PDF visual contract for {len(metrics)} pages: {args.contract}")
        return 0

    expected_pages = contract["pages"]
    errors = []
    if len(metrics) != len(expected_pages):
        errors.append(f"page count changed: expected {len(expected_pages)}, got {len(metrics)}")
    for index, (actual, expected) in enumerate(zip(metrics, expected_pages), start=1):
        errors.extend(compare_page(index, actual, expected, contract["tolerances"]))
    report = {
        "valid": not errors,
        "pdf": str(args.pdf),
        "pdf_sha256": hashlib.sha256(args.pdf.read_bytes()).hexdigest(),
        "contract": str(args.contract),
        "pages": metrics,
        "errors": errors,
    }
    report_path = render_dir / "visual-report.json"
    report_path.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    if errors:
        for error in errors:
            print(f"PDF visual regression: {error}", file=sys.stderr)
        print(f"visual report: {report_path}", file=sys.stderr)
        return 1
    print(f"PDF visual contract passed for {len(metrics)} pages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
