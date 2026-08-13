import merman


SOURCE = 'flowchart TD\nA@{ icon: "smoke:rocket", label: "Hello" } --> B[World]'
BASIC_SOURCE = "flowchart TD\nA[Hello] --> B[World]"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


class Measurer(merman.MermanTextMeasurer):
    def __init__(self) -> None:
        self.calls = 0

    def measure(self, request):
        self.calls += 1
        return None


def main() -> None:
    api = merman.Merman()
    require(api.binding_api_version() == 4, "unexpected UniFFI binding API version")

    registry = merman.MermanIconRegistry.from_packs(
        [
            merman.MermanIconPack(
                json=(
                    '{"icons":{"rocket":{"body":'
                    '"<path data-icon=\\"python-smoke\\" d=\\"M0 0H16V16H0z\\"/>"}}}'
                ),
                registration_name="smoke",
            )
        ]
    )
    measurer = Measurer()
    services = (
        merman.MermanEngineServices()
        .with_icon_registry(registry)
        .with_text_measurer(measurer)
    )
    engine = merman.MermanEngine(
        '{"resources":{"profile":"constrained"}}', services
    )
    catalog = merman.get_runtime_catalog(api)
    capabilities = set(catalog["capabilities"]["capability_ids"])
    require(
        {
            "analysis",
            "ascii",
            "layout-cytoscape",
            "layout-elk",
            "svg",
        }.issubset(capabilities),
        "default native capabilities are incomplete",
    )
    require(
        not {
            "jpeg",
            "math",
            "pdf",
            "png",
            "system-clock",
            "system-random",
            "system-timezone",
        }.intersection(capabilities),
        "default native artifact includes specialist capabilities",
    )
    svg = engine.render_svg(SOURCE, None)
    require("<svg" in svg and "Hello" in svg, "SVG smoke failed")
    require('data-icon="python-smoke"' in svg, "icon service smoke failed")
    require(measurer.calls > 0, "host text measurer was not called")
    require("Hello" in engine.render_ascii(BASIC_SOURCE, None), "ASCII smoke failed")
    require(engine.analyze_json(BASIC_SOURCE, None), "analysis smoke failed")

    for capability_id, operation in (
        ("png", lambda: engine.render_png(SOURCE, None)),
        ("jpeg", lambda: engine.render_jpeg(SOURCE, None)),
        ("pdf", lambda: engine.render_pdf(SOURCE, None)),
        (
            "math",
            lambda: engine.render_svg(
                'flowchart TD\nA["$$x^2$$"] --> B', None
            ),
        ),
    ):
        try:
            operation()
        except merman.MermanError.Binding as error:
            require(
                error.kind == merman.MermanErrorKind.MISSING_CAPABILITY
                and error.capability_id == capability_id,
                f"{capability_id} failure lost its missing-capability contract",
            )
        else:
            raise RuntimeError(
                f"default native artifact unexpectedly supports {capability_id}"
            )

    try:
        engine.render_svg(
            SOURCE,
            '{"version":2,"resources":{"profile":"constrained","limits":{"max_source_bytes":8}}}',
        )
    except merman.MermanError.Binding as error:
        require(
            error.code_name == "MERMAN_RESOURCE_LIMIT_EXCEEDED"
            and error.resource is not None
            and error.resource.cause == "ceiling"
            and error.resource.limit_id == "max_source_bytes"
            and error.resource.phase == "source"
            and error.resource.actual > error.resource.max
            and error.resource.profile == "constrained",
            "resource failure lost its structured details",
        )
    else:
        raise RuntimeError("resource failure did not return a binding error")

    deadline = merman.MermanOperationControl(timeout_ms=0)
    try:
        api.execute(
            merman.MermanOperationRequest(
                operation_id="svg",
                source=BASIC_SOURCE,
                uri=None,
                options_json=None,
                control=deadline,
            )
        )
    except merman.MermanError.Binding as error:
        require(
            error.code_name == "MERMAN_CANCELLED"
            and error.cancellation is not None
            and error.cancellation.reason == "deadline_exceeded"
            and error.cancellation.phase == "admission",
            "deadline failure lost its structured cancellation details",
        )
    else:
        raise RuntimeError("expired operation deadline did not cancel the request")

    engine.close()
    print("merman Python UniFFI smoke passed")


if __name__ == "__main__":
    main()
