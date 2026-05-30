import merman


def main() -> None:
    engine = merman.MermanEngine()
    source = "flowchart TD\nA[Hello] --> B[World]"

    svg = engine.render_svg(source, None)
    if "<svg" not in svg or "Hello" not in svg or "World" not in svg:
        raise RuntimeError("SVG smoke failed")

    semantic_json = engine.parse_json(source, None)
    if "flowchart-v2" not in semantic_json:
        raise RuntimeError("semantic JSON smoke failed")

    layout_json = engine.layout_json(source, None)
    if "layout" not in layout_json:
        raise RuntimeError("layout JSON smoke failed")

    print("merman Python UniFFI smoke passed")


if __name__ == "__main__":
    main()
