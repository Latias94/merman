"""Probe complete Cyberpunk PDF pixels through the CLI; this does not issue qualification."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import platform
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

import PIL
from PIL import Image, ImageChops
import pypdfium2 as pdfium


ROOT = Path(__file__).resolve().parents[2]
SVG = "{http://www.w3.org/2000/svg}"
SCENES = {
    "flowchart": {
        "labels": ["Browse Products", "Item in Stock?", "Add to Cart", "Out of Stock", "Checkout", "Yes", "No"],
        "effects": 13,
        "arrows": 3,
    },
    "sequence": {
        "labels": ["Client", "Client", "API", "API", "Request data", "Return data", "Verify token", "loop", "[Each request]"],
        "effects": 21,
        "arrows": 2,
    },
    "xychart": {
        "labels": ["Request Volume", "Window", "Requests", "A", "B", "C", "D", *map(str, range(11))],
        "effects": 28,
        "arrows": 0,
    },
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cli_run(cli: Path, arguments: list[str]) -> bytes:
    result = subprocess.run([str(cli), *arguments], capture_output=True, timeout=60)
    if result.returncode:
        raise RuntimeError(result.stderr.decode("utf-8", errors="replace"))
    return result.stdout


def rasterize(path: Path) -> Image.Image:
    with pdfium.PdfDocument(path) as document:
        if len(document) != 1:
            raise ValueError(f"{path.name}: expected one PDF page")
        page = document[0]
        bitmap = page.render(scale=96 / 72, fill_color=(255, 255, 255, 255))
        try:
            return bitmap.to_pil().convert("RGB")
        finally:
            bitmap.close()


def raw_pdf(cli: Path, root: ET.Element, stem: Path) -> tuple[Path, Image.Image]:
    svg_path = stem.with_suffix(".svg")
    pdf_path = stem.with_suffix(".pdf")
    ET.ElementTree(root).write(svg_path, encoding="utf-8", xml_declaration=True)
    cli_run(cli, ["render", "--quiet", "--input-kind", "svg", "--format", "pdf", "-o", str(pdf_path), str(svg_path)])
    pixels = rasterize(pdf_path)
    pixels.save(stem.with_suffix(".png"))
    return pdf_path, pixels


def set_style(node: ET.Element, declaration: str) -> None:
    node.set("style", node.get("style", "") + f";{declaration}!important")


def probe(
    cli: Path,
    root: ET.Element,
    node: ET.Element,
    original: Image.Image,
    stem: Path,
    kind: str,
    label: str,
) -> dict:
    attributes = dict(node.attrib)
    try:
        if kind == "arrow":
            node.set("marker-end", "none")
        else:
            set_style(node, "filter:none" if kind == "effect" else "visibility:hidden")
        pdf_path, changed = raw_pdf(cli, root, stem)
    finally:
        node.attrib.clear()
        node.attrib.update(attributes)
    if changed.size != original.size:
        raise ValueError(f"{stem.name}: mutation changed page dimensions")
    difference = ImageChops.difference(original, changed)
    bounds = difference.getbbox()
    if bounds is None:
        raise ValueError(f"{stem.name}: {label} contributes no PDF pixels")
    return {
        "kind": kind,
        "label": label,
        "pdf": pdf_path.name,
        "pdf_sha256": sha256(pdf_path),
        "pixel_sha256": hashlib.sha256(changed.tobytes()).hexdigest(),
        "difference_bounds": bounds,
        "maximum_channel_delta": max(high for _, high in difference.getextrema()),
    }


def check_scene(cli: Path, output: Path, family: str, expected: dict) -> dict:
    source = ROOT / "crates/merman-theme-fixtures/fixtures/public-cyberpunk" / f"{family}.mmd"
    source_digest = sha256(source)
    options = ["--theme-preset", "cyberpunk", "--config-file", str(output / "config.json")]
    for extension in ("svg", "pdf"):
        path = output / f"{family}.{extension}"
        arguments = ["render", "--quiet", "--format", extension, *options]
        if extension == "svg":
            arguments.extend(["--svg-pipeline", "resvg-safe"])
        cli_run(cli, [*arguments, "-o", str(path), str(source)])
    original = rasterize(output / f"{family}.pdf")
    original.save(output / f"{family}.png")
    root = ET.parse(output / f"{family}.svg").getroot()
    if list(root.iter(SVG + "foreignObject")):
        raise ValueError("This probe requires native SVG labels")
    _, replayed = raw_pdf(cli, root, output / f"{family}-replayed")
    if original.size != replayed.size or original.tobytes() != replayed.tobytes():
        raise ValueError(f"{family}: serialized SVG replay changed actual PDF pixels")
    effects = [node for node in root.iter() if "filter" in node.attrib]
    arrows = [node for node in root.iter() if "marker-end" in node.attrib]
    layers = [node for node in root.iter() if "data-merman-theme-canvas-layer" in node.attrib]
    labels = [(node, "".join(node.itertext()).strip()) for node in root.iter(SVG + "text")]
    labels = [(node, label) for node, label in labels if label]
    if sorted(label for _, label in labels) != sorted(expected["labels"]):
        raise ValueError(f"{family}: unexpected visible label inventory")
    if (len(effects), len(arrows), len(layers)) != (expected["effects"], expected["arrows"], 3):
        raise ValueError(f"{family}: unexpected effect, arrow or canvas inventory")
    observations = []
    for kind, terminals in (("effect", effects), ("arrow", arrows), ("canvas", layers)):
        for index, node in enumerate(terminals):
            observations.append(probe(cli, root, node, original, output / f"{family}-{kind}-{index}", kind, str(index)))
    for node in effects:
        set_style(node, "filter:none")
    _, glyphs = raw_pdf(cli, root, output / f"{family}-without-glow")
    for index, (node, label) in enumerate(labels):
        observations.append(probe(cli, root, node, glyphs, output / f"{family}-label-{index}", "label", label))
    if source_digest != sha256(source):
        raise ValueError(f"{family}: source changed during capture")
    return {
        "family": family,
        "source_sha256": source_digest,
        "pdf_sha256": sha256(output / f"{family}.pdf"),
        "size_at_96_dpi": original.size,
        "pixel_sha256": hashlib.sha256(original.tobytes()).hexdigest(),
        "svg_replay_pixels_equal": True,
        "probes": observations,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    cli = args.cli.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    ET.register_namespace("", SVG[1:-1])
    ET.register_namespace("xlink", "http://www.w3.org/1999/xlink")
    (output / "config.json").write_text('{"htmlLabels":false}\n', encoding="utf-8")
    report = {
        "captured_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "workspace_revision": subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip(),
        "script_sha256": sha256(Path(__file__)),
        "cli_sha256": sha256(cli),
        "cli_bytes": cli.stat().st_size,
        "cli_version": cli_run(cli, ["--version"]).decode().strip(),
        "python": platform.python_version(),
        "host": platform.platform(),
        "pypdfium2": str(pdfium.PYPDFIUM_INFO),
        "pdfium": str(pdfium.PDFIUM_INFO),
        "pillow": PIL.__version__,
        "dpi": 96,
        "font_profile": "system",
        "rows": [],
    }
    try:
        (output / "capabilities.json").write_bytes(cli_run(cli, ["capabilities", "--json"]))
        for family, expected in SCENES.items():
            row = check_scene(cli, output, family, expected)
            report["rows"].append(row)
            print(f"{family}: {len(row['probes'])} actual PDF pixel probes passed", flush=True)
        if report["cli_sha256"] != sha256(cli):
            raise ValueError("CLI executable changed during capture")
        report["status"] = "passed"
    except Exception as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
    (output / "results.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
