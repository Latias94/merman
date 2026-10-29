"""Shared smoke assertions, fixtures and native/WASI pixel evidence."""

import hashlib
import io
import json
import math
from pathlib import Path
import struct
import xml.etree.ElementTree as ET

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
PROTOCOL = json.loads((HERE / "protocol.json").read_text())
FIXTURE_BYTES = (HERE / "fixtures.json").read_bytes()
FIXTURES = json.loads(FIXTURE_BYTES)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode()


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def font_inputs():
    blob, ranges, evidence = bytearray(), [], []
    for name in FIXTURES["font_files"]:
        data = (ROOT / name).read_bytes()
        ranges.append({"offset": len(blob), "length": len(data)})
        evidence.append({"path": name, "sha256": sha(data), "byte_length": len(data)})
        blob.extend(data)
    return bytes(blob), ranges, evidence


def decode_descriptor(data):
    require(len(data) == 40, "descriptor must contain exactly 40 bytes")
    fields = struct.unpack("<10I", data)
    require(fields[:2] == (1, 40), "unsupported descriptor version/size")
    require(fields[2] in (0, 1), "unknown descriptor status")
    for ptr, size in zip(fields[4::2], fields[5::2]):
        require((ptr == 0) == (size == 0), "noncanonical result segment")
        require(size <= 32 * 1024 * 1024, "result segment exceeds ABI ceiling")
        require(ptr + size <= 2**32, "result segment overflows wasm32")
    return fields


def validate_catalog(catalog, flavor, transport_version=1):
    require(catalog["schema_version"] == 1, "catalog schema mismatch")
    require(catalog["transport_api_version"] == transport_version,
            "catalog transport API mismatch")
    caps = catalog["capabilities"]
    for key in ("capability_ids", "operation_ids", "output_ids", "system_adapter_ids"):
        ids = caps[key]
        require(ids == sorted(set(ids)), "catalog IDs must be sorted and unique: " + key)
    ids = caps["capability_ids"]
    require(set(PROTOCOL["required_capabilities"]) <= set(ids), "missing SDK capability")
    require(set(PROTOCOL["required_operations"]) <= set(caps["operation_ids"]),
            "missing smoke operation")
    require(("math" in ids) == (flavor != "standard"), "math flavor/catalog mismatch")
    require(("layout-elk" in ids) == (flavor == "full"), "ELK flavor/catalog mismatch")
    require(caps["system_adapter_ids"] == [], "ambient system adapters are forbidden")
    require("native-font-context" in catalog["constructor_service_ids"],
            "missing immutable font constructor service")
    providers = caps["text_measurement"]["provider_ids"]
    require("native-fonts" in providers and (transport_version != 1 or "host-callback" not in providers),
            "unexpected text measurement provider")


def validate_error(payload, code=None, capability=None):
    require(payload["version"] == 1 and payload["ok"] is False, "invalid core error schema")
    require(isinstance(payload["code"], int) and payload["kind"] and payload["message"],
            "incomplete core error payload")
    if code:
        require(payload["code_name"] == code, "unexpected error: " + json.dumps(payload))
    if capability:
        require(payload["kind"] == "missing-capability" and
                payload["capability_id"] == capability,
                "expected missing capability: " + capability + ": " + json.dumps(payload))
    return payload


def validate_native_error(payload, status_name=None, capability=None):
    # Native C owns status/status_name; WASI preserves core code/code_name instead.
    require(payload["version"] == 1 and payload["ok"] is False, "invalid native error schema")
    require(isinstance(payload["status"], int) and payload["kind"] and payload["message"],
            "incomplete native error payload")
    if status_name:
        require(payload["status_name"] == status_name, "unexpected native error: " + json.dumps(payload))
    if capability:
        require(payload["kind"] == "missing-capability" and payload["capability_id"] == capability,
                "expected native missing capability: " + capability + ": " + json.dumps(payload))
    return payload


def validate_svg(data, metadata, mathematical=False):
    require(metadata["version"] == 1 and metadata["operation_id"] == "svg", "SVG metadata mismatch")
    require(metadata["media_type"] == "image/svg+xml" and
            metadata["byte_length"] == len(data), "SVG payload metadata mismatch")
    require(metadata["runtime_policy"] == "deterministic", "non-deterministic SVG")
    root = ET.fromstring(data)
    require(root.tag == "{http://www.w3.org/2000/svg}svg", "not an SVG root")
    viewbox = [float(part) for part in root.attrib["viewBox"].replace(",", " ").split()]
    require(len(viewbox) == 4 and all(math.isfinite(n) for n in viewbox), "invalid viewBox")
    viewport = metadata["svg_viewport"]
    for key, actual in zip(("min_x", "min_y", "width", "height"), viewbox):
        require(math.isfinite(viewport[key]) and abs(viewport[key] - actual) <= 0.001,
                "SVG viewport metadata mismatch: " + key)
    require(viewbox[2] > 0 and viewbox[3] > 0, "empty SVG viewport")
    require(b"@font-face" not in data and b"data:font" not in data, "unexpected embedded font")
    if mathematical:
        # Check the emitted inline (maintenance) or one-hop use (main) formula instance.
        # This is structural evidence; the matching PNG must separately prove visible ink.
        svg = "{http://www.w3.org/2000/svg}"
        definitions = {element for defs in root.iter(svg + "defs") for element in defs.iter()}
        instances = [element for element in root.iter() if element not in definitions and
                     "merman-prepared-math-native" in element.get("class", "").split()]
        # The maintenance resvg-safe lowering may reference the whole label wrapper,
        # with the math marker inside that definition rather than on the use itself.
        for use in root.iter(svg + "use"):
            if use in definitions or use in instances:
                continue
            href = use.get("href", use.get("{http://www.w3.org/1999/xlink}href", ""))
            targets = [element for element in definitions if href.startswith("#") and
                       element.get("id") == href[1:]]
            for target in targets:
                instances.extend(element for element in target.iter() if
                                 "merman-prepared-math-native" in element.get("class", "").split())
        require(len(instances) == 1, "math SVG requires one native formula instance")
        formula = instances[0]
        if formula.tag == svg + "use":
            href = formula.get("href", formula.get("{http://www.w3.org/1999/xlink}href", ""))
            targets = [element for element in definitions if href.startswith("#") and
                       element.get("id") == href[1:]]
            require(len(targets) == 1, "math SVG formula reference is unresolved")
            formula = targets[0]
        require(any(path.get("d", "").strip() for path in formula.iter(svg + "path")),
                "math SVG has no native formula glyph paths")
        require(b' data-merman-math-native="unavailable"' not in data,
                "math SVG reports unavailable rendering")
    else:
        require(b"Memory fonts" in data and b"Retained bytes" in data, "missing fixture labels")
    return viewport


def png_pixels(data):
    from PIL import Image

    with Image.open(io.BytesIO(data)) as image:
        require(image.format == "PNG", "not a PNG")
        image.load()
        rgba = image.convert("RGBA")
        require(rgba.getchannel("A").getbbox() is not None, "PNG is fully transparent")
        require(any(low != high for low, high in rgba.getextrema()), "PNG is a flat image")
        return rgba.width, rgba.height, rgba.tobytes()


def validate_png(data, metadata, viewport, mathematical=False):
    require(metadata["version"] == 1 and metadata["operation_id"] == "png", "PNG metadata mismatch")
    require(metadata["media_type"] == "image/png" and metadata["byte_length"] == len(data),
            "PNG payload metadata mismatch")
    require(metadata["runtime_policy"] == "deterministic", "non-deterministic PNG")
    width, height, pixels = png_pixels(data)
    plan = metadata["output_plan"]
    require(plan["kind"] == "raster" and plan["limited"] is False, "PNG unexpectedly limited")
    require((plan["width_px"], plan["height_px"]) == (width, height), "PNG dimensions mismatch")
    require(plan["requested_scale"] == plan["effective_scale"] == 2, "PNG is not 2x")
    # The exporter rounds the base geometry up before applying device scale.
    require(width == math.ceil(viewport["width"]) * 2 and
            height == math.ceil(viewport["height"]) * 2, "PNG/SVG scale mismatch")
    result = {"width": width, "height": height, "rgba8_sha256": sha(pixels),
              "sha256": sha(data), "byte_length": len(data)}
    if mathematical:
        # The smoke fixture is one centered red x^2 on a white node with black borders.
        # A central region excludes the frame; solid red ink excludes neutral node paint.
        # Require a small area and extent, rather than accepting one stray colored pixel.
        ink_pixels = 0
        left, top, right, bottom = width, height, -1, -1
        for y in range(height // 4, 3 * height // 4):
            for x in range(width // 4, 3 * width // 4):
                offset = (y * width + x) * 4
                red, green, blue, alpha = pixels[offset:offset + 4]
                if red >= 200 and green <= 64 and blue <= 64 and alpha >= 200:
                    ink_pixels += 1
                    left, top = min(left, x), min(top, y)
                    right, bottom = max(right, x), max(bottom, y)
        require(ink_pixels >= 16 and right - left >= 4 and bottom - top >= 4,
                "math PNG has no visible formula ink in the fixture region")
        result["math_ink_pixels"] = ink_pixels
    return result


def save_render(output, name, operation, data, metadata, viewport=None):
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    (output / (name + "." + operation)).write_bytes(data)
    write_json(output / (name + "." + operation + ".metadata.json"), metadata)
    if operation == "svg":
        viewport = validate_svg(data, metadata, name == "math")
        return {"sha256": sha(data), "byte_length": len(data), "svg_viewport": viewport}
    return validate_png(data, metadata, viewport, mathematical=name == "math")


def compare_pixels(first, second):
    a, b = png_pixels(Path(first).read_bytes()), png_pixels(Path(second).read_bytes())
    require(a[:2] == b[:2], "PNG dimensions differ: " + str((a[:2], b[:2])))
    require(a[2] == b[2], "PNG RGBA8 pixels differ: " + str((first, second)))
    return {"width": a[0], "height": a[1], "rgba8_sha256": sha(a[2])}
