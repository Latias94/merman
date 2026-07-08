from dataclasses import dataclass

import merman


def main() -> None:
    engine = merman.MermanEngine()
    if engine.abi_version() != 2:
        raise RuntimeError(f"unexpected ABI version: {engine.abi_version()}")
    if not engine.package_version():
        raise RuntimeError("empty package version")

    source = "flowchart TD\nA[Hello] --> B[World]"

    svg = engine.render_svg(source, None)
    if "<svg" not in svg or "Hello" not in svg or "World" not in svg:
        raise RuntimeError("SVG smoke failed")

    ascii_text = engine.render_ascii(source, None)
    if "Hello" not in ascii_text or "World" not in ascii_text:
        raise RuntimeError("ASCII smoke failed")

    semantic_json = engine.parse_json(source, None)
    if "flowchart-v2" not in semantic_json:
        raise RuntimeError("semantic JSON smoke failed")

    layout_json = engine.layout_json(source, None)
    if "layout" not in layout_json:
        raise RuntimeError("layout JSON smoke failed")

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
    if not any(item.diagram_type == "flowchart" for item in engine.diagram_family_capabilities()):
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

        def measure(self, request):
            self.calls += 1
            return merman.MermanTextMeasureResult(
                width=max(len(request.text) * 8.0, 1.0),
                height=max(request.line_height, 1.0),
                line_count=1,
            )

    measurer = Measurer()
    reusable = engine.reusable_engine_with_text_measurer(None, measurer)
    if "Hello" not in reusable.render_svg(source):
        raise RuntimeError("reusable engine smoke failed")
    if measurer.calls == 0:
        raise RuntimeError("text measurer callback smoke failed")

    setter_measurer = Measurer()
    reusable = engine.reusable_engine(None)
    reusable.set_text_measurer(setter_measurer)
    if "Hello" not in reusable.render_svg(source):
        raise RuntimeError("set text measurer smoke failed")
    calls_after_set = setter_measurer.calls
    if calls_after_set == 0:
        raise RuntimeError("set text measurer callback smoke failed")
    reusable.clear_text_measurer()
    if "Hello" not in reusable.render_svg(source):
        raise RuntimeError("clear text measurer smoke failed")
    if setter_measurer.calls != calls_after_set:
        raise RuntimeError("clear text measurer did not reset callback")

    class FailingMeasurer(merman.MermanTextMeasurer):
        def measure(self, request):
            raise RuntimeError("host measurer failed")

    failing = engine.reusable_engine_with_text_measurer(None, FailingMeasurer())
    try:
        failing.render_svg(source)
    except merman.MermanError.Binding as error:
        if "host measurer failed" not in error.message:
            raise RuntimeError(
                f"unexpected callback error message: {error.message}"
            ) from error
    else:
        raise RuntimeError("failing text measurer did not surface callback error")
    failing.set_text_measurer(Measurer())
    if "Hello" not in failing.render_svg(source):
        raise RuntimeError("text measurer recovery smoke failed")

    print("merman Python UniFFI smoke passed")


if __name__ == "__main__":
    main()
