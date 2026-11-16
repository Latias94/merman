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
        "constrained_math_rejected": True,
        "missing_codecs": ["png", "jpeg", "pdf"],
    }, sort_keys=True))


if __name__ == "__main__":
    main()
