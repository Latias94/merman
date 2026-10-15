"""Compose unchanged render previews into the PR's before/after panels."""

import argparse
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def compose(root: Path, font_path: str) -> None:
    report = json.loads((root / "comparison.json").read_text(encoding="utf-8"))
    fonts = {
        "title": ImageFont.truetype(font_path, 32),
        "heading": ImageFont.truetype(font_path, 24),
        "caption": ImageFont.truetype(font_path, 20),
        "note": ImageFont.truetype(font_path, 17),
    }
    width, margin, gap = 1800, 28, 28
    panel_width = (width - margin * 2 - gap) // 2
    for comparison in report["comparisons"]:
        rows = []
        for row in comparison["rows"]:
            images = []
            for side in ("before", "after"):
                source = Image.open(root / side / f"{row['stem']}.png").convert("RGBA")
                if row.get("crop"):
                    source = source.crop(row["crop"])
                images.append(source)
            scale = min(1.0, panel_width / max(image.width for image in images))
            previews = [
                image.resize(
                    (round(image.width * scale), round(image.height * scale)),
                    Image.Resampling.LANCZOS,
                )
                for image in images
            ]
            rows.append((row, previews, max(image.height for image in previews)))
        height = 132 + sum(
            42 + (28 if row.get("evidence") else 0) + row_height + 28
            for row, _, row_height in rows
        ) + 52
        canvas = Image.new("RGB", (width, height), "#f1f4f8")
        draw = ImageDraw.Draw(canvas)
        draw.text((margin, 20), comparison["title"], font=fonts["title"], fill="#17212f")
        for index, side in enumerate(("before", "after")):
            x = margin + index * (panel_width + gap)
            revision = report[side]["revision"][:9]
            draw.text(
                (x, 75), f"{side.title()}  {revision}", font=fonts["heading"], fill="#17212f"
            )
        y = 132
        for row, images, row_height in rows:
            draw.text((margin, y), row["caption"], font=fonts["caption"], fill="#334155")
            y += 42
            if row.get("evidence"):
                draw.text((margin, y - 10), row["evidence"], font=fonts["note"], fill="#475569")
                y += 28
            for index, image in enumerate(images):
                x = margin + index * (panel_width + gap)
                draw.rounded_rectangle(
                    (x, y, x + panel_width, y + row_height), radius=6, fill="#e1e7ef"
                )
                canvas.paste(image, (x + (panel_width - image.width) // 2, y), image)
            y += row_height + 28
        draw.text(
            (margin, y + 8),
            "Same source, options and preview scale per pair. Detail crops are recorded; full outputs are retained.",
            font=fonts["note"],
            fill="#475569",
        )
        canvas.save(root / f"issue-{comparison['issue']}.png", optimize=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--font", required=True, help="Installed TrueType font for panel headings")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args()
    compose(args.root, args.font)
