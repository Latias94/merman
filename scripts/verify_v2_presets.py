#!/usr/bin/env python3
"""
Merman Theme v2 Preset & Contract Verifier.

Zero external dependencies (Python 3.11+ stdlib only).
Validates every ThemeDocumentV2 preset JSON against:
  1. Structural contract (schema_version == 2, no mermaid.variables backdoors)
  2. OKLCH seed derivation for 17 semantic color roles (independent Rust parity reference)
  3. Hard WCAG 2.1 contrast gates:
       - text.primary vs canvas / surface.node / surface.container >= 4.5:1
       - text.muted vs canvas / surface.node >= 4.5:1
       - text.title vs canvas / surface.container >= 4.5:1
       - text.callout vs surface.callout >= 4.5:1
       - text.on_emphasis vs surface.emphasis >= 4.5:1
       - data.ink[i] vs canvas >= 3.0:1 (WCAG 2.1 Non-Text Contrast 1.4.11)
       - Shared area fills and Pie slices at pie_opacity vs their paired text >= 4.5:1
  4. Font stack portability gates:
       - Never places bare `system-ui` first (breaks resvg serif fallback)
       - Never places macOS-exclusive faces first without cross-platform alternatives
       - Always ends with a generic CSS family (`sans-serif`, `serif`, or `monospace`)
  5. Perceptual uniqueness gate (OKLab Delta-E across all preset appearances):
       - Flags near-duplicate preset pairs (prevents the v1 `reading` vs `prism` 1.7-RGB clone bug)
"""

from __future__ import annotations

import argparse
import json
import math
import pathlib
import sys
from typing import Any

GENERIC_FAMILIES = {"sans-serif", "serif", "monospace"}
MACOS_ONLY_FIRST = {"Avenir", "Avenir Next", "Avenir Next Condensed", "Menlo", "Helvetica Neue", "SF Pro", "SF Mono"}

REQUIRED_TOP_KEYS = {"schema_version", "id", "name", "description", "category", "appearances"}


def hex_to_rgb(h: str) -> tuple[float, float, float]:
    h = h.lstrip("#")
    if len(h) == 8:
        h = h[:6]
    return (int(h[0:2], 16) / 255.0, int(h[2:4], 16) / 255.0, int(h[4:6], 16) / 255.0)


def rgb_to_hex(rgb: tuple[float, float, float]) -> str:
    r, g, b = (max(0, min(255, round(c * 255.0))) for c in rgb)
    return f"#{r:02X}{g:02X}{b:02X}"


def srgb_to_linear(c: float) -> float:
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def linear_to_srgb(c: float) -> float:
    c = max(0.0, min(1.0, c))
    return 12.92 * c if c <= 0.0031308 else 1.055 * (c ** (1.0 / 2.4)) - 0.055


def relative_luminance(hex_color: str) -> float:
    r, g, b = (srgb_to_linear(c) for c in hex_to_rgb(hex_color))
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast_ratio(c1: str, c2: str) -> float:
    l1, l2 = relative_luminance(c1), relative_luminance(c2)
    hi, lo = max(l1, l2), min(l1, l2)
    return (hi + 0.05) / (lo + 0.05)


def alpha_composite(fg: str, bg: str, alpha: float) -> str:
    fr, fg_, fb = hex_to_rgb(fg)
    br, bg_, bb = hex_to_rgb(bg)
    return rgb_to_hex((
        fr * alpha + br * (1.0 - alpha),
        fg_ * alpha + bg_ * (1.0 - alpha),
        fb * alpha + bb * (1.0 - alpha),
    ))


# Exact Björn Ottosson OKLab / OKLCH conversions
def rgb_to_oklab(rgb: tuple[float, float, float]) -> tuple[float, float, float]:
    r, g, b = (srgb_to_linear(c) for c in rgb)
    l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l_, m_, s_ = math.cbrt(l), math.cbrt(m), math.cbrt(s)
    L = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_
    a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_
    b_ = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_
    return (L, a, b_)


def oklab_to_rgb(lab: tuple[float, float, float]) -> tuple[float, float, float]:
    L, a, b = lab
    l_ = L + 0.3963377774 * a + 0.2158037573 * b
    m_ = L - 0.1055613458 * a - 0.0638541728 * b
    s_ = L - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_ ** 3, m_ ** 3, s_ ** 3
    r = +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s
    g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s
    b_ = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
    return (linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b_))


def oklab_to_oklch(lab: tuple[float, float, float]) -> tuple[float, float, float]:
    L, a, b = lab
    C = math.hypot(a, b)
    H = math.degrees(math.atan2(b, a)) % 360.0
    return (L, C, H)


def oklch_to_oklab(lch: tuple[float, float, float]) -> tuple[float, float, float]:
    L, C, H = lch
    rad = math.radians(H)
    return (L, C * math.cos(rad), C * math.sin(rad))


def hex_to_oklch(h: str) -> tuple[float, float, float]:
    return oklab_to_oklch(rgb_to_oklab(hex_to_rgb(h)))


def oklch_to_hex(L: float, C: float, H: float) -> str:
    # Gamut-safe chroma bisection
    L = max(0.0, min(1.0, L))
    lo, hi = 0.0, max(0.0, C)
    best = oklab_to_rgb(oklch_to_oklab((L, 0.0, H)))
    for _ in range(18):
        mid = (lo + hi) * 0.5
        L_, a, b = oklch_to_oklab((L, mid, H))
        l_ = L_ + 0.3963377774 * a + 0.2158037573 * b
        m_ = L_ - 0.1055613458 * a - 0.0638541728 * b
        s_ = L_ - 0.0894841775 * a - 1.2914855480 * b
        l, m, s = l_ ** 3, m_ ** 3, s_ ** 3
        r = +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s
        g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s
        b_ = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
        if -1e-4 <= r <= 1.0001 and -1e-4 <= g <= 1.0001 and -1e-4 <= b_ <= 1.0001:
            best = (linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b_))
            lo = mid
        else:
            hi = mid
    return rgb_to_hex(best)


def mix_oklab(c1: str, c2: str, t: float) -> str:
    L1, a1, b1 = rgb_to_oklab(hex_to_rgb(c1))
    L2, a2, b2 = rgb_to_oklab(hex_to_rgb(c2))
    return rgb_to_hex(oklab_to_rgb((
        L1 + (L2 - L1) * t,
        a1 + (a2 - a1) * t,
        b1 + (b2 - b1) * t,
    )))


def best_contrast_ink(bg: str, dark_ink: str, light_ink: str) -> str:
    return dark_ink if contrast_ratio(bg, dark_ink) >= contrast_ratio(bg, light_ink) else light_ink


def derive_roles(mode: str, layer: dict[str, Any]) -> dict[str, str]:
    """Derive the 17 semantic color roles from the OKLCH seed."""
    seed = layer.get("seed", {})
    overrides = layer.get("roles", {})
    is_dark = mode == "dark"

    canvas = seed["canvas"]
    ink = seed["ink"]
    accent = seed["accent"]

    cL, cC, cH = hex_to_oklch(canvas)
    aL, aC, aH = hex_to_oklch(accent)

    if "surface" in seed:
        surface_node = seed["surface"]
    else:
        surface_node = oklch_to_hex(cL + 0.055 if is_dark else min(0.995, cL + 0.025), cC * 0.85, cH)

    surface_container = oklch_to_hex(
        cL + 0.03 if is_dark else max(0.0, cL - 0.032),
        cC * 1.05,
        cH,
    )
    surface_callout = seed.get(
        "callout",
        oklch_to_hex(0.30 if is_dark else 0.94, 0.045, 82.0),
    )
    surface_emphasis = mix_oklab(surface_node, accent, 0.22 if is_dark else 0.12)

    stroke_border = mix_oklab(surface_node, ink, 0.28 if is_dark else 0.24)
    stroke_connector = mix_oklab(canvas, ink, 0.62 if is_dark else 0.56)
    stroke_guide = mix_oklab(canvas, ink, 0.16 if is_dark else 0.14)

    text_primary = ink
    text_muted = mix_oklab(canvas, ink, 0.72 if is_dark else 0.68)

    # Contrast-safe callout & emphasis text
    dark_ref = "#111827" if not is_dark else canvas
    light_ref = "#F8FAFC" if is_dark else surface_node
    text_callout = best_contrast_ink(surface_callout, ink if not is_dark else dark_ref, ink if is_dark else light_ref)
    text_on_emphasis = best_contrast_ink(surface_emphasis, ink if not is_dark else dark_ref, ink if is_dark else light_ref)

    roles = {
        "canvas": canvas,
        "surface.node": surface_node,
        "surface.container": surface_container,
        "surface.callout": surface_callout,
        "surface.emphasis": surface_emphasis,
        "stroke.border": stroke_border,
        "stroke.connector": stroke_connector,
        "stroke.guide": stroke_guide,
        "text.primary": text_primary,
        "text.muted": text_muted,
        "text.title": ink,
        "text.callout": text_callout,
        "text.on_emphasis": text_on_emphasis,
        "status.info": seed.get("info", accent),
        "status.success": seed.get("success", "#4ADE80" if is_dark else "#15803D"),
        "status.warn": seed.get("warn", "#FBBF24" if is_dark else "#B45309"),
        "status.danger": seed.get("danger", "#F87171" if is_dark else "#B91C1C"),
    }

    for k, v in overrides.items():
        if isinstance(v, str) and v.startswith("#"):
            roles[k] = v
    if "text.title" not in overrides:
        roles["text.title"] = accent if contrast_ratio(accent, roles["canvas"]) >= 4.5 else roles["text.primary"]
    return roles


def oklab_distance(c1: str, c2: str) -> float:
    L1, a1, b1 = rgb_to_oklab(hex_to_rgb(c1))
    L2, a2, b2 = rgb_to_oklab(hex_to_rgb(c2))
    return math.sqrt((L1 - L2) ** 2 + (a1 - a2) ** 2 + (b1 - b2) ** 2)


def verify_preset(path: pathlib.Path) -> tuple[list[str], list[dict[str, Any]]]:
    errors: list[str] = []
    signatures: list[dict[str, Any]] = []

    raw = path.read_text(encoding="utf-8")
    doc = json.loads(raw)

    if doc.get("schema_version") != 2:
        errors.append(f"{path.name}: schema_version must be 2")
    if "mermaid" in doc:
        errors.append(f"{path.name}: forbidden legacy `mermaid` compatibility backdoor found")
    missing = REQUIRED_TOP_KEYS - set(doc.keys())
    if missing:
        errors.append(f"{path.name}: missing required keys {sorted(missing)}")

    # Font portability check
    typo = doc.get("typography", {})
    for slot in ("sans", "mono", "serif"):
        stack = typo.get(slot)
        if stack is None:
            continue
        if len(stack) < 2:
            errors.append(f"{path.name}: typography.{slot} needs at least 2 entries")
            continue
        if stack[0] == "system-ui":
            errors.append(f"{path.name}: typography.{slot} puts `system-ui` first (breaks resvg fallback)")
        if stack[0] in MACOS_ONLY_FIRST:
            errors.append(f"{path.name}: typography.{slot} puts macOS-only `{stack[0]}` first")
        if stack[-1] not in GENERIC_FAMILIES:
            errors.append(f"{path.name}: typography.{slot} must end with a generic family")

    pie_opacity = doc.get("form", {}).get("chart", {}).get("pie_opacity", 0.92)
    appearances = doc.get("appearances", {})
    if len(appearances) > 1 and doc.get("default_appearance") not in appearances:
        errors.append(f"{path.name}: multi-appearance theme must declare valid default_appearance")

    for mode, layer in appearances.items():
        roles = derive_roles(mode, layer)
        tag = f"{doc['id']}:{mode}"

        # Hard WCAG text contrast checks (>= 4.5:1)
        text_pairs = [
            ("text.primary", "canvas", 4.5),
            ("text.primary", "surface.node", 4.5),
            ("text.primary", "surface.container", 4.5),
            ("text.muted", "canvas", 4.5),
            ("text.muted", "surface.node", 4.5),
            ("text.title", "canvas", 4.5),
            ("text.title", "surface.container", 4.5),
            ("text.callout", "surface.callout", 4.5),
            ("text.on_emphasis", "surface.emphasis", 4.5),
        ]
        for fg_k, bg_k, min_cr in text_pairs:
            cr = contrast_ratio(roles[fg_k], roles[bg_k])
            if cr < min_cr:
                errors.append(
                    f"{tag}: WCAG FAIL {fg_k}({roles[fg_k]}) on {bg_k}({roles[bg_k]}) = {cr:.2f}:1 < {min_cr}:1"
                )

        # Data palette checks
        data = layer.get("data", {})
        inks = data.get("ink", [])
        fills = data.get("fill", [])
        fill_ink_spec = data.get("fill_ink", "auto")

        min_series_cr = 99.0
        for idx, c in enumerate(inks):
            cr = contrast_ratio(c, roles["canvas"])
            min_series_cr = min(min_series_cr, cr)
            if cr < 3.0:
                errors.append(
                    f"{tag}: DATA INK FAIL data.ink[{idx}]({c}) on canvas({roles['canvas']}) = {cr:.2f}:1 < 3.0:1"
                )

        fill_inks = []
        pie_fill_inks = []
        min_fill_text_cr = 99.0
        for opacity, palette in [(1.0, fill_inks), (pie_opacity, pie_fill_inks)]:
            for idx, f in enumerate(fills):
                comp = alpha_composite(f, roles["canvas"], opacity)
                if fill_ink_spec == "auto":
                    chosen = best_contrast_ink(comp, "#0F172A", "#F8FAFC")
                    if contrast_ratio(chosen, comp) < 4.5:
                        chosen = best_contrast_ink(comp, "#000000", "#FFFFFF")
                else:
                    chosen = fill_ink_spec
                palette.append(chosen)
                cr = contrast_ratio(chosen, comp)
                min_fill_text_cr = min(min_fill_text_cr, cr)
                if cr < 4.5:
                    errors.append(
                        f"{tag}: DATA FILL TEXT FAIL data.fill[{idx}]({f} @ {opacity} -> {comp}) with {fill_ink_spec} = {cr:.2f}:1 < 4.5:1"
                    )

        signatures.append({
            "tag": tag,
            "mode": mode,
            "canvas": roles["canvas"],
            "node": roles["surface.node"],
            "ink": roles["text.primary"],
            "accent": layer["seed"]["accent"],
            "min_text_cr": min(
                contrast_ratio(roles[fg], roles[bg]) for fg, bg, _ in text_pairs
            ),
            "min_series_cr": min_series_cr if inks else 0.0,
            "min_fill_text_cr": min_fill_text_cr if fills else 0.0,
            "bytes": len(raw.encode("utf-8")),
            "lines": raw.count("\n") + 1,
            "colors": {
                "roles": roles, "ink": inks, "fill": fills,
                "fill_ink": fill_inks, "pie_fill_ink": pie_fill_inks,
            },
        })

    return errors, signatures


def format_colors(colors: dict[str, dict[str, Any]]) -> str:
    entries = []
    for tag, palette in sorted(colors.items()):
        roles = json.dumps(palette["roles"], indent=2).replace("\n", "\n    ")
        entries.append(
            f'  {json.dumps(tag)}: {{\n'
            f'    "roles": {roles},\n'
            f'    "ink": {json.dumps(palette["ink"])},\n'
            f'    "fill": {json.dumps(palette["fill"])},\n'
            f'    "fill_ink": {json.dumps(palette["fill_ink"])},\n'
            f'    "pie_fill_ink": {json.dumps(palette["pie_fill_ink"])}\n'
            '  }'
        )
    return "{\n" + ",\n".join(entries) + "\n}\n"


def main() -> int:
    repo_root = pathlib.Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description="Verify Theme v2 presets and their reference colors.")
    parser.add_argument(
        "preset_dir", nargs="?", type=pathlib.Path,
        default=repo_root / "crates/merman-theme-contract/assets/presets",
        help="directory containing *.theme.json files (defaults to the built-in presets)",
    )
    color_options = parser.add_mutually_exclusive_group()
    color_options.add_argument(
        "--write-colors", type=pathlib.Path, metavar="PATH", help="write the golden colors JSON",
    )
    color_options.add_argument(
        "--check-colors", type=pathlib.Path, metavar="PATH", help="fail if the golden colors JSON differs",
    )
    args = parser.parse_args()

    preset_dir = args.preset_dir
    files = sorted(preset_dir.glob("*.theme.json"))
    if not files:
        print(f"No presets found in {preset_dir}", file=sys.stderr)
        return 1

    all_errors: list[str] = []
    all_sigs: list[dict[str, Any]] = []

    for f in files:
        errs, sigs = verify_preset(f)
        all_errors.extend(errs)
        all_sigs.extend(sigs)

    # Perceptual uniqueness check (average OKLab distance over canvas/node/accent/ink)
    min_pair = (999.0, "", "")
    for i in range(len(all_sigs)):
        for j in range(i + 1, len(all_sigs)):
            a, b = all_sigs[i], all_sigs[j]
            if a["mode"] != b["mode"]:
                continue
            d = (
                oklab_distance(a["canvas"], b["canvas"])
                + oklab_distance(a["node"], b["node"])
                + oklab_distance(a["accent"], b["accent"])
                + oklab_distance(a["ink"], b["ink"])
            ) / 4.0
            if d < min_pair[0]:
                min_pair = (d, a["tag"], b["tag"])
            if d < 0.035:
                all_errors.append(
                    f"REDUNDANCY FAIL: {a['tag']} vs {b['tag']} avg OKLab delta = {d:.4f} < 0.035"
                )

    print(f"Verified {len(files)} presets ({len(all_sigs)} concrete appearances):")
    print(f"{'Preset:Mode':<22} {'Lines':>5} {'Bytes':>6} {'MinText':>8} {'MinLine':>8} {'MinFill':>8}")
    print("-" * 65)
    for s in all_sigs:
        print(
            f"{s['tag']:<22} {s['lines']:>5} {s['bytes']:>6} "
            f"{s['min_text_cr']:>7.2f}:1 {s['min_series_cr']:>7.2f}:1 {s['min_fill_text_cr']:>7.2f}:1"
        )
    print("-" * 65)
    print(f"Closest same-mode pair: {min_pair[1]} <-> {min_pair[2]} (avg OKLab delta = {min_pair[0]:.4f})")

    if all_errors:
        print("\nFAILURES:")
        for e in all_errors:
            print(f"  - {e}")
        return 1

    colors = {s["tag"]: s["colors"] for s in all_sigs}
    if args.write_colors:
        args.write_colors.write_text(format_colors(colors), encoding="utf-8", newline="\n")
    elif args.check_colors:
        try:
            expected = json.loads(args.check_colors.read_text(encoding="utf-8"))
        except (OSError, ValueError) as error:
            print(f"Could not read preset colors from {args.check_colors} ({error})", file=sys.stderr)
            return 1
        if expected != colors:
            print(
                f"Preset colors differ from {args.check_colors}. Regenerate with --write-colors.",
                file=sys.stderr,
            )
            return 1

    print("\nALL GATES PASSED (0 errors).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
