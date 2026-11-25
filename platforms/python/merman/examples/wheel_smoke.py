"""Qualify an installed wheel without a source checkout or build tools."""

from __future__ import annotations

from importlib.metadata import version
import json
import math
from pathlib import Path
import platform
import re
import sys
import xml.etree.ElementTree as ET

import merman


MATH_SOURCE = 'flowchart TD\nA["$$x^2$$"] --> B[Done]'
THEME_SOURCE = "---\nconfig:\n  layout: dagre\n---\nflowchart LR\nA[Review] --> B[Publish]"
SVG_NAMESPACE = "{http://www.w3.org/2000/svg}"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def python_package_version(native_version: str) -> str:
    release, separator, local = native_version.partition("+")
    release = release.replace("-alpha.", "a").replace("-beta.", "b").replace("-rc.", "rc")
    if separator:
        release += "+" + re.sub(r"[-_]+", ".", local).lower()
    return release


def assert_math_svg(svg: str) -> int:
    root = ET.fromstring(svg)
    require(root.tag == SVG_NAMESPACE + "svg", "math output has no SVG root")
    require("$$" not in svg, "math output retained literal formula delimiters")
    formulas = list(root.iter(SVG_NAMESPACE + "svg"))[1:]
    require(bool(formulas), "math output has no inline formula SVG viewport")
    for formula in formulas:
        view_box = [float(value) for value in formula.get("viewBox", "").replace(",", " ").split()]
        require(
            len(view_box) == 4 and all(math.isfinite(value) for value in view_box)
            and view_box[2] > 0 and view_box[3] > 0,
            "math output has no positive finite formula viewport",
        )
        require(
            any(path.get("d", "").strip() for path in formula.iter(SVG_NAMESPACE + "path")),
            "math output has no outlined formula glyphs",
        )
    return len(formulas)


def assert_ascii_output_metadata(api) -> None:
    source = "flowchart LR\nA@{ shape: f-circ }\nB[Target]\n" + "A o--o B\n" * 97
    result = api.render_ascii_result(
        source,
        '{"ascii":{"graph_presentation":"adaptive","layout_profile":"auto",'
        '"max_width":80,"overflow":"fallback"}}',
    )
    plan = result.metadata.output_plan.ascii
    raw_plan = json.loads(result.metadata.output_plan.raw_json)
    require(result.metadata.version == 1, "unexpected operation metadata schema")
    require(plan.schema_version == 4, "unexpected ASCII output schema")
    require(
        plan.primary_width is None
        and plan.primary_height is None
        and plan.layout_profile is None
        and plan.primary_overflowed is None,
        "unavailable primary geometry must remain None",
    )
    for field in ("primary_width", "primary_height", "layout_profile", "primary_overflowed"):
        require(field in raw_plan and raw_plan[field] is None, f"{field} must be explicit null")
    require(
        plan.requested_graph_presentation == "adaptive"
        and plan.requested_layout_profile == "auto"
        and plan.compact_attempted
        and plan.requested_max_width == 80
        and plan.projection == "structured_text"
        and plan.outcome == "fallback"
        and plan.fallback_attempted
        and plan.fallback_reason == "port_capacity_exceeded"
        and plan.lossiness == "presentation_only",
        "adaptive selection evidence lost",
    )
    require(
        plan.emitted_width == raw_plan["emitted_width"]
        and plan.emitted_height == raw_plan["emitted_height"]
        and plan.emitted_overflowed is False
        and raw_plan["emitted_overflowed"] is False
        and "overflowed" not in raw_plan,
        "emitted geometry or overflow metadata lost",
    )
    require("E97:" in bytes(result.data).decode("utf-8"), "adaptive fallback lost original relations")

    empty = api.render_ascii_result("flowchart LR", None)
    empty_plan = empty.metadata.output_plan.ascii
    empty_raw = json.loads(empty.metadata.output_plan.raw_json)
    require(
        empty_plan.schema_version == 4
        and empty_plan.primary_width == 0
        and empty_plan.primary_height == 0
        and empty_plan.layout_profile == "canonical"
        and empty_plan.primary_overflowed is False
        and empty_plan.emitted_overflowed is False
        and empty_plan.requested_graph_presentation == "strict"
        and empty_plan.outcome == "empty"
        and empty_raw["primary_width"] == 0
        and empty_raw["primary_height"] == 0
        and empty_raw["primary_overflowed"] is False,
        "measured empty primary geometry must retain zero and False",
    )
    flowchart = next(
        capability for capability in api.ascii_capabilities()
        if capability.diagram_type == "flowchart"
    )
    require(
        flowchart.graph_presentations == ["strict", "adaptive"],
        "graph presentation capability projection lost",
    )


def _execute_theme_operation(
    client, operation_id: str, source: str, *, options: dict | None = None,
):
    result = client.execute(merman.MermanOperationRequestV4(
        operation_id=operation_id,
        source=source,
        uri=None,
        options_json=json.dumps(options) if options is not None else None,
        control=None,
    ))
    require(result.operation_id == operation_id, f"unexpected result for {operation_id}")
    return result


def _resolve_theme(client, document: dict | str, **options) -> dict:
    request = json.dumps({"document": document, **options})
    return json.loads(bytes(_execute_theme_operation(client, "resolve-theme", request).data))


def assert_theme_v2(api) -> None:
    """Validate authoring and rendering through the installed native library."""
    engine = merman.MermanEngine('{"theme":"studio"}', None)
    try:
        catalog = json.loads(api.theme_catalog_json())
        require(catalog == json.loads(engine.theme_catalog_json()), "theme catalogs differ")
        require(
            catalog["schema_version"] == 2
            and len(catalog["presets"]) == 13
            and len(catalog["mermaid_themes"]) == 11
            and any(row["id"] == "flowchart" and row["capabilities"] > 0 for row in catalog["families"]),
            "unexpected theme v2 catalog",
        )
        studio = next(preset for preset in catalog["presets"] if preset["id"] == "studio")
        require(
            studio["appearances"] == ["light", "dark"]
            and studio["default_appearance"] == "light"
            and len(studio["swatches"]) == 6,
            "Studio lost its appearance and swatch data",
        )

        for name, client in (("one-shot", api), ("reusable", engine)):
            exported = _execute_theme_operation(client, "export-theme-preset", studio["id"])
            text = bytes(exported.data).decode("utf-8")
            document = json.loads(text)
            require(
                exported.media_type == "application/json"
                and document["schema_version"] == 2
                and document["id"] == studio["id"]
                and set(document["appearances"]) == {"light", "dark"},
                f"{name} did not export the editable preset",
            )
            result = _resolve_theme(client, document, appearance="dark")
            require(
                result["ok"] and result["resolved"]["appearance"] == "dark"
                and isinstance(result["notices"], list)
                and result == _resolve_theme(client, text, appearance="dark"),
                f"{name} could not resolve both document forms: {result}",
            )
            for raw, column, path, message in (
                ('{\n  "schema_version": 2,\n  "extends": ]\n}', 14, "extends", "expected value"),
                ('{\n  "schema_version": 2,\n  "schema_version": 2\n}', 18, ".", "duplicate key"),
            ):
                diagnostic = _resolve_theme(client, raw)
                require(
                    diagnostic["ok"] is False and diagnostic["line"] == 3
                    and diagnostic["column"] == column and diagnostic["path"] == path
                    and message in diagnostic["message"],
                    f"{name} lost document-relative diagnostics: {diagnostic}",
                )

            custom = {
                "schema_version": 2, "extends": studio["id"],
                "roles": {"text.primary": "#FFFFFF"},
            }
            fixed = _resolve_theme(client, custom, auto_fix_contrast=True)
            require(
                fixed["ok"] and any(
                    notice["kind"] == "contrast_adjusted" and notice["foreground"] == "text.primary"
                    for notice in fixed["notices"]
                ),
                f"{name} ignored auto_fix_contrast: {fixed}",
            )
            rendered = _execute_theme_operation(
                client, "svg", THEME_SOURCE, options={"theme": custom, "background": "#123456"},
            )
            outcome = json.loads(rendered.metadata.raw_json)["theme_outcome"]
            require(
                outcome["mode"] == "merman_native" and outcome["appearance"] == "light"
                and any(
                    isinstance(notice, dict) and notice["kind"] == "low_contrast"
                    for notice in outcome["notices"]
                ),
                f"{name} lost SVG theme outcome: {outcome}",
            )
            root = ET.fromstring(bytes(rendered.data))
            canvas = root.find(f".//{SVG_NAMESPACE}g[@class='merman-theme-canvas']/{SVG_NAMESPACE}rect")
            require(
                root.tag == SVG_NAMESPACE + "svg" and canvas is not None
                and canvas.get("fill", "").lower() == "#123456",
                f"{name} background did not replace the theme canvas",
            )

        baseline = _execute_theme_operation(engine, "svg", THEME_SOURCE)
        inherited = json.loads(baseline.metadata.raw_json)["theme_outcome"]
        require(
            inherited["preset_id"] == "studio" and inherited["appearance"] == "light",
            "request theme changed the reusable baseline",
        )
        ascii_options = {"ascii": {"color_mode": "truecolor"}}
        plain_palette = api.render_ascii(THEME_SOURCE, json.dumps(ascii_options))
        native_palette = api.render_ascii(THEME_SOURCE, json.dumps({
            **ascii_options, "theme": {"preset": "studio", "appearance": "dark"},
        }))
        require("\x1b[38;2;" in native_palette and native_palette != plain_palette,
                "truecolor ASCII did not use the native theme palette")
    finally:
        engine.close()


def main() -> None:
    require(
        Path(merman.__file__).resolve().is_relative_to(Path(sys.prefix).resolve()),
        "consumer imported merman outside the installed environment",
    )
    api = merman.Merman()
    require(api.binding_api_version_v8() == 8, "unexpected UniFFI binding API version")
    installed_version = version("merman")
    native_version = api.package_version()
    require(installed_version == python_package_version(native_version), "wheel and native library versions differ")
    catalog = merman.get_runtime_catalog(api)
    require(catalog["transport_api_version"] == 8, "unexpected catalog transport API")
    capabilities = set(catalog["capabilities"]["capability_ids"])
    require(
        {"analysis", "ascii", "layout-cytoscape", "layout-elk", "math", "svg"}.issubset(capabilities),
        "installed native capabilities are incomplete",
    )
    require(
        not {"jpeg", "pdf", "png", "system-clock", "system-random", "system-timezone"}.intersection(capabilities),
        "installed native artifact includes specialist capabilities",
    )
    assert_ascii_output_metadata(api)
    assert_theme_v2(api)

    math_svg = api.render_svg(MATH_SOURCE, '{"site_config":{"htmlLabels":true}}')
    formulas = assert_math_svg(math_svg)
    constrained = merman.MermanEngine('{"resources":{"profile":"constrained"}}', None)
    try:
        require("<svg" in constrained.render_svg("flowchart TD\nA --> B", None), "constrained SVG failed")
        try:
            constrained.render_svg(MATH_SOURCE, None)
        except merman.MermanError.Binding as error:
            require(
                error.kind == merman.MermanErrorKind.MISSING_CAPABILITY
                and error.capability_id == "math",
                "constrained math rejection lost its capability identity",
            )
        else:
            raise RuntimeError("constrained policy unexpectedly admitted the native math backend")
    finally:
        constrained.close()

    for capability, render in (("png", api.render_png), ("jpeg", api.render_jpeg), ("pdf", api.render_pdf)):
        try:
            render("flowchart TD\nA --> B", None)
        except merman.MermanError.Binding as error:
            require(
                error.kind == merman.MermanErrorKind.MISSING_CAPABILITY
                and error.capability_id == capability,
                f"{capability} rejection lost its capability identity",
            )
        else:
            raise RuntimeError(f"ordinary SVG wheel unexpectedly includes {capability}")

    print(json.dumps({
        "python": platform.python_version(),
        "machine": platform.machine(),
        "libc": platform.libc_ver(),
        "package_version": installed_version,
        "native_version": native_version,
        "import_path": str(Path(merman.__file__).resolve()),
        "math_svg_bytes": len(math_svg.encode("utf-8")),
        "math_formula_viewports": formulas,
        "ascii_schema4_verified": True,
        "theme_schema2_verified": True,
        "constrained_math_rejected": True,
        "missing_codecs": ["png", "jpeg", "pdf"],
    }, sort_keys=True))


if __name__ == "__main__":
    main()
