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
    engine = merman.MermanEngine(None, services)
    svg = engine.render_svg(SOURCE, None)
    require("<svg" in svg and "Hello" in svg, "SVG smoke failed")
    require('data-icon="python-smoke"' in svg, "icon service smoke failed")
    require(measurer.calls > 0, "host text measurer was not called")
    engine.close()

    print("merman Python UniFFI smoke passed")


if __name__ == "__main__":
    main()
