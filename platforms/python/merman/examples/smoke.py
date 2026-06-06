import merman


def main() -> None:
    engine = merman.MermanEngine()
    if engine.abi_version() != 1:
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
    if "sequence" not in engine.ascii_supported_diagrams():
        raise RuntimeError("ASCII supported diagrams smoke failed")
    if "default" not in engine.supported_themes():
        raise RuntimeError("themes smoke failed")

    print("merman Python UniFFI smoke passed")


if __name__ == "__main__":
    main()
