import json
from dataclasses import dataclass

import merman

def text_measurement_result(operation, width: float, height: float):
    operation_type = merman.MermanTextMeasurementOperation
    values = dict(
        result_kind=merman.MermanTextMeasurementResultKind.METRICS,
        width=0.0,
        height=0.0,
        length=0.0,
        line_count=0,
        bbox_left=None,
        bbox_right=None,
        raw_width=None,
    )
    if operation in {
        operation_type.MEASURE,
        operation_type.WRAPPED,
        operation_type.MERMAID_CALCULATE_TEXT_DIMENSIONS,
    }:
        values.update(width=width, height=height, line_count=1)
    elif operation in {
        operation_type.COMPUTED_LENGTH,
        operation_type.SIMPLE_B_BOX_WIDTH,
        operation_type.RAW_B_BOX_WIDTH,
        operation_type.BOUNDING_CLIENT_RECT_WIDTH,
        operation_type.TSPAN_B_BOX_WIDTH,
        operation_type.WRAP_PROBE_B_BOX_WIDTH,
        operation_type.CANVAS_MEASURE_TEXT_WIDTH,
    }:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
            length=width,
        )
    elif operation in {
        operation_type.TSPAN_B_BOX_HEIGHT,
        operation_type.SIMPLE_B_BOX_HEIGHT,
        operation_type.RAW_B_BOX_HEIGHT,
    }:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
            length=height,
        )
    elif operation in {
        operation_type.CREATE_TEXT_B_BOX_Y_OFFSET,
        operation_type.CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET,
    }:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
            length=(
                -2.0
                if operation == operation_type.CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET
                else -1.0
            ),
        )
    elif operation in {
        operation_type.B_BOX_X,
        operation_type.B_BOX_X_WITH_ASCII_OVERHANG,
        operation_type.TITLE_B_BOX_X,
    }:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.HORIZONTAL_EXTENTS,
            bbox_left=width / 2.0,
            bbox_right=width / 2.0,
        )
    elif operation == operation_type.WRAPPED_WITH_RAW_WIDTH:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.WRAPPED_WITH_RAW_WIDTH,
            width=width,
            height=height,
            line_count=1,
            raw_width=width,
        )
    else:
        return None
    return merman.MermanTextMeasureResult(**values)


def assert_text_measurement_contract() -> None:
    operation_type = merman.MermanTextMeasurementOperation
    expected_operation_codes = {
        entry[0] for entry in merman.TEXT_MEASUREMENT_OPERATIONS
    }
    operation_codes = {operation.value for operation in operation_type}
    if operation_codes != expected_operation_codes:
        raise RuntimeError(f"unexpected text measurement operation codes: {operation_codes}")
    expected_result_kind_codes = {
        entry[0] for entry in merman.TEXT_MEASUREMENT_RESULT_KINDS
    }
    result_kind_codes = {
        kind.value for kind in merman.MermanTextMeasurementResultKind
    }
    if result_kind_codes != expected_result_kind_codes:
        raise RuntimeError(
            f"unexpected text measurement result-kind codes: {result_kind_codes}"
        )

    dimensions = text_measurement_result(
        merman.MermanTextMeasurementOperation.MERMAID_CALCULATE_TEXT_DIMENSIONS,
        42.0,
        24.0,
    )
    if dimensions.result_kind != merman.MermanTextMeasurementResultKind.METRICS:
        raise RuntimeError("MermaidCalculateTextDimensions must return metrics")

    canvas_width = text_measurement_result(
        merman.MermanTextMeasurementOperation.CANVAS_MEASURE_TEXT_WIDTH,
        42.0,
        24.0,
    )
    if canvas_width.result_kind != merman.MermanTextMeasurementResultKind.LENGTH:
        raise RuntimeError("CanvasMeasureTextWidth must return length")

    raw_bbox_height = text_measurement_result(
        merman.MermanTextMeasurementOperation.RAW_B_BOX_HEIGHT,
        42.0,
        24.0,
    )
    if (
        raw_bbox_height.result_kind
        != merman.MermanTextMeasurementResultKind.LENGTH
        or raw_bbox_height.length != 24.0
    ):
        raise RuntimeError("RawBBoxHeight must return the raw bbox height as length")

    y_offset = text_measurement_result(
        merman.MermanTextMeasurementOperation.CREATE_TEXT_B_BOX_Y_OFFSET,
        42.0,
        24.0,
    )
    if y_offset.length >= 0.0:
        raise RuntimeError("CreateTextBBoxYOffset must preserve signed lengths")

    middle_y_offset = text_measurement_result(
        merman.MermanTextMeasurementOperation.CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET,
        42.0,
        24.0,
    )
    if (
        middle_y_offset.result_kind
        != merman.MermanTextMeasurementResultKind.LENGTH
        or middle_y_offset.length >= 0.0
    ):
        raise RuntimeError("CreateTextMiddleBBoxYOffset must preserve signed lengths")


def main() -> None:
    assert_text_measurement_contract()

    merman.require_text_measurement_protocol_version(
        merman.TEXT_MEASUREMENT_PROTOCOL_VERSION
    )
    try:
        merman.require_text_measurement_protocol_version(
            merman.TEXT_MEASUREMENT_PROTOCOL_VERSION + 1
        )
    except merman.TextMeasurementProtocolVersionMismatch as error:
        if (
            error.expected != merman.TEXT_MEASUREMENT_PROTOCOL_VERSION
            or error.actual != merman.TEXT_MEASUREMENT_PROTOCOL_VERSION + 1
        ):
            raise
    else:
        raise RuntimeError("expected mismatched text-measurement protocol to be rejected")

    engine = merman.MermanEngine()
    if not engine.package_version():
        raise RuntimeError("empty package version")
    if engine.binding_api_version() != 3:
        raise RuntimeError("unexpected UniFFI binding API version")
    runtime_catalog = merman.get_runtime_catalog(engine)
    runtime_capabilities = runtime_catalog.get("capabilities")
    if (
        runtime_catalog.get("schema_version") != 1
        or not isinstance(runtime_capabilities, dict)
        or "system_adapter_ids" not in runtime_capabilities
        or "operation_ids" not in runtime_capabilities
        or not set(runtime_capabilities.get("output_ids", [])).issubset(
            runtime_capabilities.get("operation_ids", [])
        )
    ):
        raise RuntimeError("runtime catalog schema smoke failed")

    source = "---\ntitle: Host measurement phases\n---\nflowchart TD\nA[Hello] --> B[World]"

    resource_options = (
        merman.ResourceOptionsBuilder()
        .profile(merman.ResourceProfile.CONSTRAINED)
        .limit(merman.ResourceOverrideId.MAX_SOURCE_BYTES, 4096)
        .build()
        .to_options_json()
    )
    if json.loads(resource_options) != {
        "resources": {
            "limits": {"max_source_bytes": 4096},
            "profile": "constrained",
        },
        "version": 2,
    }:
        raise RuntimeError("resource options export smoke failed")

    svg = engine.render_svg(source, resource_options)
    if "<svg" not in svg or "Hello" not in svg or "World" not in svg:
        raise RuntimeError("SVG smoke failed")

    generic_options = json.loads(resource_options)
    generic_options["runtime_policy"] = "native"
    generic_semantic = engine.execute(
        merman.MermanOperationRequest(
            operation_id="semantic-json",
            source=source,
            uri=None,
            options_json=json.dumps(generic_options),
        )
    )
    if (
        generic_semantic.operation_id != "semantic-json"
        or generic_semantic.media_type != "application/json"
        or b"flowchart-v2" not in generic_semantic.data
        or json.loads(generic_semantic.metadata_json).get("runtime_policy")
        != "native"
    ):
        raise RuntimeError("generic operation smoke failed")

    try:
        engine.execute(
            merman.MermanOperationRequest(
                operation_id="not-an-operation",
                source=source,
                uri=None,
                options_json=resource_options,
            )
        )
    except merman.MermanError.Binding as error:
        if (
            error.kind != merman.MermanErrorKind.UNKNOWN_OPERATION
            or error.capability_id is not None
        ):
            raise
    else:
        raise RuntimeError("unknown operation did not preserve its typed binding error")

    tiny_source_options = json.dumps(
        {
            "version": 2,
            "resources": {
                "profile": "constrained",
                "limits": {"max_source_bytes": 8},
            },
        }
    )
    try:
        engine.render_svg(source, tiny_source_options)
    except merman.MermanError.Binding as error:
        if (
            error.code_name != "MERMAN_RESOURCE_LIMIT_EXCEEDED"
            or error.resource is None
            or error.resource.limit_id != "max_source_bytes"
            or error.resource.phase != "source"
            or error.resource.actual <= error.resource.max
            or error.resource.profile != "constrained"
        ):
            raise
    else:
        raise RuntimeError("resource failure did not preserve structured details")

    png = engine.render_png(source, None)
    if not png.startswith(b"\x89PNG\r\n\x1a\n"):
        raise RuntimeError("PNG smoke failed")
    jpeg = engine.render_jpeg(source, None)
    if not jpeg.startswith(b"\xff\xd8\xff"):
        raise RuntimeError("JPEG smoke failed")
    pdf = engine.render_pdf(source, None)
    if not pdf.startswith(b"%PDF-"):
        raise RuntimeError("PDF smoke failed")

    ascii_text = engine.render_ascii(source, None)
    if "Hello" not in ascii_text or "World" not in ascii_text:
        raise RuntimeError("ASCII smoke failed")

    semantic_json = engine.parse_json(source, None)
    if "flowchart-v2" not in semantic_json:
        raise RuntimeError("semantic JSON smoke failed")

    layout_json = engine.layout_json(source, None)
    if "layout" not in layout_json:
        raise RuntimeError("layout JSON smoke failed")

    document_source = "# Example\n\n```mermaid\n" + source + "\n```\n"
    document_json = json.loads(
        engine.analyze_document_json(
            document_source,
            None,
            "file:///tmp/example.md",
        )
    )
    if document_json["source"]["kind"] != "markdown" or not document_json["valid"]:
        raise RuntimeError("document analysis smoke failed")
    document_facts_json = json.loads(
        engine.analyze_document_facts_json(
            document_source,
            None,
            "file:///tmp/example.md",
        )
    )
    if (
        document_facts_json["version"] != 1
        or document_facts_json["source"]["kind"] != "markdown"
        or document_facts_json["diagrams"][0]["source_id"] != "mermaid-fence-1"
    ):
        raise RuntimeError("document facts smoke failed")

    validation = engine.validate(source, None)
    if not validation.valid or validation.code_name != "MERMAN_OK":
        raise RuntimeError("validation smoke failed")

    invalid = engine.validate("", None)
    if invalid.valid or invalid.code_name != "MERMAN_NO_DIAGRAM":
        raise RuntimeError("invalid validation smoke failed")

    if "flowchart" not in engine.supported_diagrams():
        raise RuntimeError("supported diagrams smoke failed")
    ascii_capabilities = engine.ascii_capabilities()
    if not any(
        item.diagram_type == "sequence" and item.support_level == "full"
        for item in ascii_capabilities
    ):
        raise RuntimeError("ASCII full capability smoke failed")
    if not any(
        item.diagram_type == "gantt"
        and item.support_level == "summary"
        and not item.summary_fallback
        for item in ascii_capabilities
    ):
        raise RuntimeError("ASCII summary capability smoke failed")
    if not any(
        item.diagram_type == "class"
        and item.support_level == "partial"
        and item.summary_fallback
        for item in ascii_capabilities
    ):
        raise RuntimeError("ASCII fallback capability smoke failed")
    if "default" not in engine.supported_themes():
        raise RuntimeError("themes smoke failed")
    if "one-dark" not in engine.supported_host_theme_presets():
        raise RuntimeError("host theme presets smoke failed")
    if not any(
        item.diagram_type == "flowchart"
        and item.logical_family_kind == "flowchart"
        and item.metadata_id == "flowchart"
        and item.render_model_kind == "flowchart"
        and item.has_detector
        and item.has_semantic_parser
        and item.has_editor_parser
        and item.has_combined_parser
        and item.has_render_parser
        and not item.has_header
        and item.config_namespace == "flowchart"
        for item in engine.diagram_family_capabilities()
    ):
        raise RuntimeError("diagram family capabilities smoke failed")
    if not hasattr(merman, "MermanLintRuleCatalogEntry"):
        raise RuntimeError("lint rule catalog entry export smoke failed")
    lint_rules = engine.lint_rule_catalog()
    if not lint_rules or not all(
        isinstance(rule, merman.MermanLintRuleCatalogEntry) for rule in lint_rules
    ):
        raise RuntimeError("lint rule catalog type smoke failed")
    if not any(
        rule.id == "merman.authoring.flowchart.explicit_direction"
        and rule.origin == "merman_authoring"
        for rule in lint_rules
    ):
        raise RuntimeError("lint rule catalog content smoke failed")
    configurable_rules = engine.configurable_lint_rule_catalog()
    if not configurable_rules or not all(
        isinstance(rule, merman.MermanLintRuleCatalogEntry) for rule in configurable_rules
    ):
        raise RuntimeError("configurable lint rule catalog type smoke failed")
    if not any(
        rule.id == "merman.authoring.flowchart.explicit_direction"
        and rule.configurable
        for rule in configurable_rules
    ):
        raise RuntimeError("configurable lint rule catalog content smoke failed")
    if not all(rule.configurable for rule in configurable_rules):
        raise RuntimeError("configurable lint rule catalog smoke failed")

    @dataclass
    class Measurer(merman.MermanTextMeasurer):
        calls: int = 0
        phases: set = None
        operations: set = None

        def __post_init__(self):
            self.phases = set()
            self.operations = set()

        def measure(self, request):
            self.calls += 1
            self.phases.add(request.phase)
            self.operations.add(request.operation)
            width = max(len(request.text) * 8.0, 1.0)
            height = max(request.line_height, 1.0)
            return text_measurement_result(request.operation, width, height)

    measurer = Measurer()
    reusable = engine.reusable_engine_with_text_measurer(None, measurer)
    if "Hello" not in reusable.render_svg(source, None):
        raise RuntimeError("reusable engine smoke failed")
    if measurer.calls == 0:
        raise RuntimeError("text measurer callback smoke failed")
    phase_names = {phase.name for phase in measurer.phases}
    if not {"WRAP", "SVG_B_BOX"}.issubset(phase_names):
        raise RuntimeError(f"expected named measurement phases, got {phase_names}")
    operation_names = {operation.name for operation in measurer.operations}
    if "WRAPPED" not in operation_names:
        raise RuntimeError(f"expected concrete measurement operations, got {operation_names}")

    reusable = engine.reusable_engine(
        '{"svg":{"diagram_id":"python-baseline","pipeline":"readable"}}'
    )
    reusable_result = reusable.execute(
        merman.MermanOperationRequest(
            operation_id="svg",
            source=source,
            uri=None,
            options_json='{"svg":{"diagram_id":"python-request"}}',
        )
    )
    reusable_svg = reusable_result.data.decode()
    if (
        'id="python-request"' not in reusable_svg
        or "data-merman-foreignobject" not in reusable_svg
    ):
        raise RuntimeError("reusable request options did not merge over the engine baseline")
    baseline_svg = reusable.render_svg(source, None)
    if 'id="python-baseline"' not in baseline_svg:
        raise RuntimeError("reusable request options mutated the engine baseline")
    try:
        reusable.execute(
            merman.MermanOperationRequest(
                operation_id="semantic-json",
                source=source,
                uri=None,
                options_json='{"runtime_policy":"native"}',
            )
        )
    except merman.MermanError.Binding as error:
        if (
            error.code_name != "MERMAN_OPTIONS_JSON_ERROR"
            or "cannot set runtime_policy" not in error.message
        ):
            raise
    else:
        raise RuntimeError("reusable request changed the constructor-owned runtime policy")

    reusable_document_json = json.loads(
        reusable.analyze_document_json(document_source, None, "file:///tmp/example.md")
    )
    if reusable_document_json["source"]["kind"] != "markdown":
        raise RuntimeError("reusable document analysis smoke failed")
    reusable_document_facts_json = json.loads(
        reusable.analyze_document_facts_json(
            document_source,
            None,
            "file:///tmp/example.md",
        )
    )
    if (
        reusable_document_facts_json["version"] != 1
        or reusable_document_facts_json["source"]["kind"] != "markdown"
    ):
        raise RuntimeError("reusable document facts smoke failed")
    if hasattr(reusable, "set_text_measurer") or hasattr(
        reusable, "clear_text_measurer"
    ):
        raise RuntimeError("reusable callbacks must be immutable after construction")

    class FailingMeasurer(merman.MermanTextMeasurer):
        def measure(self, request):
            raise RuntimeError("host measurer failed")

    failing = engine.reusable_engine_with_text_measurer(None, FailingMeasurer())
    if "Hello" not in failing.render_svg(source, None):
        raise RuntimeError("failing text measurer did not use vendored fallback")

    try:
        engine.render_svg(source, "{")
    except merman.MermanError.Binding as error:
        if (
            error.code != 3
            or error.code_name != "MERMAN_OPTIONS_JSON_ERROR"
            or "invalid options_json" not in error.message
        ):
            raise
    else:
        raise RuntimeError("invalid options_json did not raise MermanError.Binding")

    print("merman Python UniFFI smoke passed")


if __name__ == "__main__":
    main()
