import merman


SOURCE = 'flowchart TD\nA@{ icon: "smoke:rocket", label: "Hello" } --> B[World]'


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
    require(api.binding_api_version() == 3, "unexpected UniFFI binding API version")

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
    svg = engine.render_svg(SOURCE, None)
    require("<svg" in svg and "Hello" in svg, "SVG smoke failed")
    require('data-icon="python-smoke"' in svg, "icon service smoke failed")
    require(measurer.calls > 0, "host text measurer was not called")

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

    engine.close()
    print("merman Python UniFFI smoke passed")


if __name__ == "__main__":
    main()
