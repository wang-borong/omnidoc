#!/usr/bin/env python3
"""Generate deterministic DOCX/PPTX reference files for bundled themes."""

from __future__ import annotations

import argparse
import pathlib
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
REFERENCE_DIR = ROOT / "pandoc" / "data" / "reference-docs"
ENGINEERING_SLIDES = REFERENCE_DIR / "engineering-slides.pptx"

W_NS = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
A_NS = "http://schemas.openxmlformats.org/drawingml/2006/main"
CP_NS = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
DC_NS = "http://purl.org/dc/elements/1.1/"
DCTERMS_NS = "http://purl.org/dc/terms/"

ET.register_namespace("w", W_NS)
ET.register_namespace("a", A_NS)
ET.register_namespace("cp", CP_NS)
ET.register_namespace("dc", DC_NS)
ET.register_namespace("dcterms", DCTERMS_NS)

DOCX_PROFILES = {
    "engineering-book": {
        "name": "OmniDoc Engineering Book",
        "colors": ["102A43", "F7F3EA", "244B5A", "E8E1D5", "176B87", "E9C46A", "2A9D8F", "7A5AA6", "2B5C8A", "C45B5B"],
        "major_latin": "Aptos Display",
        "minor_latin": "Aptos",
        "east_asia": "Noto Sans CJK SC",
        "body_size": "22",
        "body_after": "140",
        "title_align": "center",
        "heading_color": "176B87",
        "heading_border": "2A9D8F",
        "page_size": (11906, 16838),
        "margin": 1276,
    },
    "corporate-docs": {
        "name": "OmniDoc Corporate Docs",
        "colors": ["182230", "FFFFFF", "12325B", "EAF0FB", "155EEF", "0E9384", "DC6803", "7F56D9", "1570EF", "D92D20"],
        "major_latin": "Aptos Display",
        "minor_latin": "Aptos",
        "east_asia": "Noto Sans CJK SC",
        "body_size": "22",
        "body_after": "120",
        "title_align": "left",
        "heading_color": "155EEF",
        "heading_border": "155EEF",
        "page_size": (11906, 16838),
        "margin": 1440,
    },
    "classic-book": {
        "name": "OmniDoc Classic Book",
        "colors": ["322A25", "FFFDF8", "3F3027", "F6F0E6", "77543D", "A67C3D", "68775B", "8B6F86", "596C80", "A35D4F"],
        "major_latin": "Georgia",
        "minor_latin": "Georgia",
        "east_asia": "Noto Serif CJK SC",
        "body_size": "24",
        "body_after": "100",
        "title_align": "center",
        "heading_color": "77543D",
        "heading_border": None,
        "page_size": (8391, 11906),
        "margin": 1134,
    },
    "clean-document": {
        "name": "OmniDoc Clean Document",
        "colors": ["202124", "FFFFFF", "1F2937", "F3F4F6", "374151", "6B7280", "4B5563", "64748B", "315B8A", "9F3A38"],
        "major_latin": "Aptos Display",
        "minor_latin": "Aptos",
        "east_asia": "Noto Serif CJK SC",
        "body_size": "22",
        "body_after": "120",
        "title_align": "left",
        "heading_color": "374151",
        "heading_border": "D9DCE1",
        "page_size": (11906, 16838),
        "margin": 1440,
    },
}

SLIDE_PROFILE = {
    "name": "OmniDoc Modern Slides",
    "colors": ["172033", "F7F9FC", "253858", "E9EEF7", "3451B2", "00A6A6", "F59E0B", "7C3AED", "2563EB", "E11D48"],
    "major_latin": "Aptos Display",
    "minor_latin": "Aptos",
    "east_asia": "Noto Sans CJK SC",
}


def qname(namespace: str, local: str) -> str:
    return f"{{{namespace}}}{local}"


def read_archive(path: pathlib.Path) -> dict[str, bytes]:
    with zipfile.ZipFile(path) as archive:
        return {name: archive.read(name) for name in archive.namelist()}


def write_archive(path: pathlib.Path, entries: dict[str, bytes]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    with zipfile.ZipFile(temporary, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=(2000, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            archive.writestr(info, entries[name])
    temporary.replace(path)


def replace_color(parent: ET.Element, tag: str, value: str) -> None:
    node = parent.find(f"a:{tag}", {"a": A_NS})
    if node is None:
        raise RuntimeError(f"theme color {tag} is missing")
    for child in list(node):
        node.remove(child)
    ET.SubElement(node, qname(A_NS, "srgbClr"), {"val": value})


def customize_drawing_theme(data: bytes, profile: dict) -> bytes:
    root = ET.fromstring(data)
    root.set("name", profile["name"])
    scheme = root.find(".//a:clrScheme", {"a": A_NS})
    if scheme is None:
        raise RuntimeError("drawing theme has no color scheme")
    scheme.set("name", profile["name"])
    tags = ("dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6")
    for tag, value in zip(tags, profile["colors"], strict=True):
        replace_color(scheme, tag, value)
    replace_color(scheme, "hlink", profile["colors"][4])
    replace_color(scheme, "folHlink", profile["colors"][7])

    for font_kind, latin in (("majorFont", profile["major_latin"]), ("minorFont", profile["minor_latin"])):
        font = root.find(f".//a:{font_kind}", {"a": A_NS})
        if font is None:
            continue
        for child_name in ("latin", "cs"):
            child = font.find(f"a:{child_name}", {"a": A_NS})
            if child is not None:
                child.set("typeface", latin)
        east = font.find("a:ea", {"a": A_NS})
        if east is not None:
            east.set("typeface", profile["east_asia"])
        for script in font.findall("a:font", {"a": A_NS}):
            if script.get("script") in {"Hans", "Hant"}:
                script.set("typeface", profile["east_asia"])
    return ET.tostring(root, encoding="utf-8", xml_declaration=True)


def ensure_child(parent: ET.Element, name: str) -> ET.Element:
    child = parent.find(f"w:{name}", {"w": W_NS})
    if child is None:
        child = ET.SubElement(parent, qname(W_NS, name))
    return child


def word_style(root: ET.Element, style_id: str) -> ET.Element | None:
    return root.find(
        f".//w:style[@w:styleId='{style_id}']",
        {"w": W_NS},
    )


def set_word_color(run_properties: ET.Element, color: str) -> None:
    node = ensure_child(run_properties, "color")
    node.set(qname(W_NS, "val"), color)
    for attribute in ("themeColor", "themeShade", "themeTint"):
        node.attrib.pop(qname(W_NS, attribute), None)


def customize_word_styles(data: bytes, profile: dict) -> bytes:
    root = ET.fromstring(data)
    default_run = root.find(".//w:docDefaults/w:rPrDefault/w:rPr", {"w": W_NS})
    default_paragraph = root.find(".//w:docDefaults/w:pPrDefault/w:pPr", {"w": W_NS})
    if default_run is not None:
        ensure_child(default_run, "sz").set(qname(W_NS, "val"), profile["body_size"])
        ensure_child(default_run, "szCs").set(qname(W_NS, "val"), profile["body_size"])
    if default_paragraph is not None:
        ensure_child(default_paragraph, "spacing").set(
            qname(W_NS, "after"), profile["body_after"]
        )

    title = word_style(root, "Title")
    if title is not None:
        properties = ensure_child(title, "pPr")
        ensure_child(properties, "jc").set(qname(W_NS, "val"), profile["title_align"])
        run = ensure_child(title, "rPr")
        set_word_color(run, profile["heading_color"])
        ensure_child(run, "b")

    heading_sizes = {"Heading1": "38", "Heading2": "30", "Heading3": "26", "Heading4": "24"}
    for style_id, size in heading_sizes.items():
        style = word_style(root, style_id)
        if style is None:
            continue
        run = ensure_child(style, "rPr")
        set_word_color(run, profile["heading_color"])
        ensure_child(run, "sz").set(qname(W_NS, "val"), size)
        ensure_child(run, "szCs").set(qname(W_NS, "val"), size)
        if style_id == "Heading1" and profile["heading_border"]:
            paragraph = ensure_child(style, "pPr")
            borders = ensure_child(paragraph, "pBdr")
            bottom = ensure_child(borders, "bottom")
            bottom.set(qname(W_NS, "val"), "single")
            bottom.set(qname(W_NS, "sz"), "12")
            bottom.set(qname(W_NS, "space"), "4")
            bottom.set(qname(W_NS, "color"), profile["heading_border"])
    return ET.tostring(root, encoding="utf-8", xml_declaration=True)


def customize_word_document(data: bytes, profile: dict) -> bytes:
    root = ET.fromstring(data)
    width, height = profile["page_size"]
    margin = str(profile["margin"])
    for section in root.findall(".//w:sectPr", {"w": W_NS}):
        page_size = ensure_child(section, "pgSz")
        page_size.set(qname(W_NS, "w"), str(width))
        page_size.set(qname(W_NS, "h"), str(height))
        page_size.attrib.pop(qname(W_NS, "orient"), None)
        margins = ensure_child(section, "pgMar")
        for side in ("top", "right", "bottom", "left"):
            margins.set(qname(W_NS, side), margin)
        margins.set(qname(W_NS, "header"), "720")
        margins.set(qname(W_NS, "footer"), "720")
        margins.set(qname(W_NS, "gutter"), "0")
    return ET.tostring(root, encoding="utf-8", xml_declaration=True)


def normalize_core_properties(data: bytes, title: str) -> bytes:
    root = ET.fromstring(data)
    title_node = root.find(f"{{{DC_NS}}}title")
    if title_node is not None:
        title_node.text = title
    for name in ("created", "modified"):
        node = root.find(f"{{{DCTERMS_NS}}}{name}")
        if node is not None:
            node.text = "2000-01-01T00:00:00Z"
    return ET.tostring(root, encoding="utf-8", xml_declaration=True)


def generate_docx(pandoc: str, temporary: pathlib.Path) -> None:
    base = temporary / "reference.docx"
    subprocess.run(
        [pandoc, "-o", str(base), "--print-default-data-file", "reference.docx"],
        check=True,
    )
    source = read_archive(base)
    for key, profile in DOCX_PROFILES.items():
        entries = dict(source)
        entries["word/theme/theme1.xml"] = customize_drawing_theme(
            entries["word/theme/theme1.xml"], profile
        )
        entries["word/styles.xml"] = customize_word_styles(
            entries["word/styles.xml"], profile
        )
        entries["word/document.xml"] = customize_word_document(
            entries["word/document.xml"], profile
        )
        entries["docProps/core.xml"] = normalize_core_properties(
            entries["docProps/core.xml"], profile["name"]
        )
        write_archive(REFERENCE_DIR / f"{key}.docx", entries)


def generate_pptx() -> None:
    if not ENGINEERING_SLIDES.is_file():
        raise RuntimeError(f"missing PPTX base: {ENGINEERING_SLIDES}")
    entries = read_archive(ENGINEERING_SLIDES)
    for theme_path in ("ppt/theme/theme1.xml", "ppt/theme/theme2.xml"):
        if theme_path in entries:
            entries[theme_path] = customize_drawing_theme(entries[theme_path], SLIDE_PROFILE)
    if "docProps/core.xml" in entries:
        entries["docProps/core.xml"] = normalize_core_properties(
            entries["docProps/core.xml"], SLIDE_PROFILE["name"]
        )
    write_archive(REFERENCE_DIR / "modern-slides.pptx", entries)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pandoc", default=shutil.which("pandoc") or "pandoc")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="omnidoc-theme-docs-") as directory:
        generate_docx(args.pandoc, pathlib.Path(directory))
    generate_pptx()
    for name in sorted(DOCX_PROFILES):
        print(REFERENCE_DIR / f"{name}.docx")
    print(REFERENCE_DIR / "modern-slides.pptx")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
