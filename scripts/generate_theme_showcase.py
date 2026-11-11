#!/usr/bin/env python3
"""Compose changelog cards from the maintained Playground SVG previews."""

from __future__ import annotations

import argparse
from pathlib import Path
import re
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
PREVIEWS = ROOT / "playground/src/themes/previews"
OUTPUT = ROOT / "docs/rendering/theme-gallery/showcase"
SVG = "http://www.w3.org/2000/svg"
ET.register_namespace("", SVG)
ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")

DESIGNS = (
    ("riso", "Riso", "Two inks · misregistered edges", "#F6F0E4"),
    ("pop-art", "Pop Art", "Poster colors · hard ink offsets", "#FFF9E8"),
    ("carbon-copy", "Carbon Copy", "Ruled paper · violet impressions", "#F2EDDF"),
    ("paper-cut", "Paper Cut", "Layered stock · lifted annotations", "#FBF8F0"),
)


def element(parent: ET.Element, tag: str, **attributes: str) -> ET.Element:
    return ET.SubElement(parent, f"{{{SVG}}}{tag}", attributes)


def text(parent: ET.Element, value: str, x: int, y: int, size: int, **attributes: str) -> None:
    element(parent, "text", x=str(x), y=str(y), **{"font-size": str(size), **attributes}).text = value


def preview(theme: str, family: str, x: int, y: int, width: int, height: int) -> ET.Element:
    diagram = ET.parse(PREVIEWS / f"{theme}-{family}.svg").getroot()
    ids = {node.attrib["id"] for node in diagram.iter() if "id" in node.attrib}
    prefix = f"{theme}-{family}"
    alternatives = "|".join(
        re.escape(value) for value in sorted(ids, key=lambda value: (-len(value), value))
    )
    references = re.compile(r"#(" + alternatives + r")(?![\w-])")
    for node in diagram.iter():
        for key, value in list(node.attrib.items()):
            if key == "id":
                node.set(key, f"{prefix}-{value}")
            else:
                node.set(key, references.sub(lambda match: f"#{prefix}-{match[1]}", value))
        if node.tag == f"{{{SVG}}}style" and node.text:
            node.text = references.sub(lambda match: f"#{prefix}-{match[1]}", node.text)
    diagram.attrib.pop("style", None)
    diagram.set("x", str(x))
    diagram.set("y", str(y))
    diagram.set("width", str(width))
    diagram.set("height", str(height))
    diagram.set("preserveAspectRatio", "xMidYMid meet")
    return diagram


def card(theme: str, name: str, description: str, canvas: str) -> bytes:
    root = ET.Element(
        f"{{{SVG}}}svg",
        {
            "viewBox": "0 0 600 460", "width": "600", "height": "460",
            "role": "img", "aria-labelledby": "title description",
            "font-family": '"Segoe UI", Arial, sans-serif',
        },
    )
    element(root, "title", id="title").text = f"{name} theme"
    element(root, "desc", id="description").text = (
        f"{description}. The same review workflow, author/editor sequence and work-stage pie "
        "appear in every card. Flowchart on the left; Sequence and Pie on the right."
    )
    defs = element(root, "defs")
    clip = element(defs, "clipPath", id="card")
    element(clip, "rect", width="600", height="460", rx="14")
    body = element(root, "g", **{"clip-path": "url(#card)"})
    element(body, "rect", width="600", height="460", fill="#0D1713")
    text(body, name, 22, 36, 27, fill="#F0F3F0", **{"font-weight": "650"})
    text(body, description, 22, 61, 15, fill="#A6B0A7")
    element(body, "rect", x="0", y="80", width="600", height="340", fill=canvas)
    element(body, "path", d="M300 80V420M300 250H600", stroke="#D1CEC5", **{"stroke-width": "1"})
    panels = (
        ("flowchart", "Flowchart", 16, 118, 268, 286, 16, 102),
        ("sequence", "Sequence", 314, 118, 272, 122, 314, 102),
        ("pie", "Pie", 314, 286, 272, 124, 314, 272),
    )
    for family, label, x, y, width, height, label_x, label_y in panels:
        body.append(preview(theme, family, x, y, width, height))
        text(body, label, label_x, label_y, 13, fill="#596259", **{"font-weight": "600"})
    text(body, "Flowchart · Sequence · Pie", 22, 445, 14, fill="#A6B0A7")
    text(body, "Light", 558, 445, 14, fill="#A6B0A7", **{"text-anchor": "end"})
    element(root, "rect", x="0.5", y="0.5", width="599", height="459", rx="14", fill="none", stroke="#39433C")
    return ET.tostring(root, encoding="utf-8", xml_declaration=True) + b"\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check saved cards without writing")
    args = parser.parse_args()
    stale = False
    for design in DESIGNS:
        path = OUTPUT / f"{design[0]}.svg"
        content = card(*design)
        if args.check:
            if not path.is_file() or path.read_bytes() != content:
                print(f"Stale theme card: {path.relative_to(ROOT)}")
                stale = True
        else:
            OUTPUT.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
            print(f"Wrote {path.relative_to(ROOT)}")
    return int(stale)


if __name__ == "__main__":
    raise SystemExit(main())
