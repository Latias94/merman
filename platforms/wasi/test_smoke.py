"""Focused adapter contracts; production qualification still uses a real reactor."""

import io
import json
from pathlib import Path
import struct
import tempfile
import unittest

from common import (FIXTURES, FIXTURE_BYTES, PROTOCOL, ROOT, compare_pixels, decode_descriptor,
                    font_inputs, png_pixels, save_render, sha, validate_catalog, validate_error, validate_native_error,
                    validate_png, validate_svg)
from wasmtime_smoke import Host, audit_module


def reactor_wat(imports="", initialize="", omitted=(), extra=""):
    functions = []
    for name, count in PROTOCOL["function_exports"].items():
        if name in omitted:
            continue
        params = " (param " + " ".join(["i32"] * count) + ")" if count else ""
        result = "" if name == "_initialize" else " (result i32)"
        body = initialize if name == "_initialize" else "i32.const " + ("1" if name.endswith("abi_version") else "0")
        functions.append('(func (export "' + name + '")' + params + result + " " + body + ")")
    return '(module ' + imports + ' (memory (export "memory") 1) ' + " ".join(functions) + extra + ")"


class DescriptorTests(unittest.TestCase):
    def test_little_endian_ten_scalar_fields(self):
        fields = (1, 40, 0, 0x12345678, 100, 3, 103, 5, 0, 0)
        self.assertEqual(decode_descriptor(struct.pack("<10I", *fields)), fields)
        self.assertEqual(struct.pack("<10I", *fields)[12:16], b"\x78\x56\x34\x12")

    def test_invalid_descriptors_are_rejected(self):
        fields = [1, 40, 0, 1, 0, 0, 0, 0, 0, 0]
        for index, value in ((0, 2), (1, 36), (2, 2), (4, 10), (5, 10), (7, 33 << 20)):
            bad = fields.copy()
            bad[index] = value
            with self.subTest(index=index), self.assertRaises(ValueError):
                decode_descriptor(struct.pack("<10I", *bad))
        with self.assertRaises(ValueError):
            decode_descriptor(bytes(39))
        with self.assertRaises(ValueError):
            decode_descriptor(struct.pack("<10I", 1, 40, 0, 0, 0xFFFFFFFE, 4, 0, 0, 0, 0))

    def test_shared_production_fonts_match_manifest(self):
        blob, ranges, evidence = font_inputs()
        manifest = json.loads((ROOT / Path(FIXTURES["font_files"][0]).parent / "manifest.json").read_bytes())
        by_name = {item["path"]: item["sha256"] for item in manifest["files"]}
        self.assertEqual(sum(item["length"] for item in ranges), len(blob))
        for item in evidence:
            self.assertEqual(item["sha256"], by_name[Path(item["path"]).name])


class RealRuntimeAuditTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import wasmtime as wt
        cls.wt = wt
        cls.engine = wt.Engine()

    @classmethod
    def tearDownClass(cls):
        cls.engine.close()

    def audit(self, text):
        with self.wt.Module(self.engine, text) as module:
            return audit_module(module)

    def test_actual_module_exports_and_memory_are_audited(self):
        result = self.audit(reactor_wat())
        self.assertEqual(result["imports"], [])
        self.assertIn("memory", result["exports"])
        self.assertEqual(result["preopened_directories"], [])

    def test_forbidden_imports_rejected_before_instantiation(self):
        for module, name in (("wasi_snapshot_preview1", "sock_accept"),
                             ("wasi", "thread-spawn"),
                             ("env", "random_get"),
                             ("wasi_snapshot_preview1", "path_create_directory")):
            text = reactor_wat('(import "' + module + '" "' + name + '" (func))')
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.audit(text)

    def test_missing_initialize_wrong_signature_and_extra_export(self):
        for text in (reactor_wat(omitted=("_initialize",)),
                     reactor_wat(extra='(func (export "_start"))'),
                     reactor_wat().replace('(export "merman_wasi_alloc") (param i32)',
                                          '(export "merman_wasi_alloc") (param i64)')):
            with self.subTest(text=text[:80]), self.assertRaises(ValueError):
                self.audit(text)

    def test_pinned_math_dependency_helpers_are_not_sdk_exports(self):
        helper = '(func (export "__wbindgen_malloc") (param i32 i32) (result i32) i32.const 0)'
        result = self.audit(reactor_wat(extra=helper))
        self.assertIn("__wbindgen_malloc", result["dependency_exports"])
        with self.assertRaises(ValueError):
            self.audit(reactor_wat(extra=helper.replace("(param i32 i32)", "(param i64 i32)")))

    def test_actual_host_has_no_preopens_environment_or_ambient_entropy(self):
        imports = '''
          (import "wasi_snapshot_preview1" "random_get" (func $rand (param i32 i32) (result i32)))
          (import "wasi_snapshot_preview1" "clock_time_get" (func $time (param i32 i64 i32) (result i32)))
          (import "wasi_snapshot_preview1" "environ_sizes_get" (func $env (param i32 i32) (result i32)))
          (import "wasi_snapshot_preview1" "fd_prestat_get" (func $prestat (param i32 i32) (result i32)))
        '''
        initialize = '''
          i32.const 16 i32.const 255 i32.const 24 memory.fill
          i32.const 16 i32.const 16 call $rand drop
          i32.const 0 i64.const 1 i32.const 32 call $time drop
          i32.const 40 i32.const 44 call $env drop
          i32.const 60 i32.const 3 i32.const 48 call $prestat i32.store
          i32.const 64 i32.const 0 i32.const -1 call $rand i32.store
        '''
        host = Host(self.wt.wat2wasm(reactor_wat(imports, initialize)))
        try:
            self.assertEqual(host.read(16, 32), bytes(32))
            self.assertNotEqual(struct.unpack("<I", host.read(60, 4))[0], 0)
            self.assertEqual(host.shims["random_get"], 2)
            self.assertEqual(host.shims["clock_time_get"], 1)
            self.assertEqual(struct.unpack("<I", host.read(64, 4))[0], 21)
            self.assertEqual(host.call("abi_version"), 1)
        finally:
            host.close()


class OutputTests(unittest.TestCase):
    def png(self, color="red", compress=6):
        from PIL import Image
        image = Image.new("RGBA", (4, 2), color)
        image.putpixel((1, 1), (0, 255, 0, 128))
        output = io.BytesIO()
        image.save(output, "PNG", compress_level=compress)
        return output.getvalue()

    def metadata(self, data):
        return {"version": 1, "operation_id": "png", "media_type": "image/png",
                "byte_length": len(data), "runtime_policy": "deterministic",
                "output_plan": {"kind": "raster", "limited": False, "requested_scale": 2,
                                "effective_scale": 2, "width_px": 4, "height_px": 2}}

    def test_png_metadata_and_unpremultiplied_pixels(self):
        data = self.png()
        result = validate_png(data, self.metadata(data), {"width": 1.5, "height": 1})
        self.assertEqual((result["width"], result["height"]), (4, 2))
        self.assertEqual(png_pixels(data)[2][20:24], b"\x00\xff\x00\x80")
        broken = self.metadata(data)
        broken["output_plan"]["effective_scale"] = 1
        with self.assertRaises(ValueError):
            validate_png(data, broken, {"width": 1.5, "height": 1})

    def test_png_comparison_is_decoded_not_compressed_bytes(self):
        first, second = self.png(compress=0), self.png(compress=9)
        self.assertNotEqual(first, second)
        with tempfile.TemporaryDirectory() as directory:
            a, b = Path(directory) / "a.png", Path(directory) / "b.png"
            a.write_bytes(first)
            b.write_bytes(second)
            self.assertIn("rgba8_sha256", compare_pixels(a, b))
            b.write_bytes(self.png(color="blue"))
            with self.assertRaises(ValueError):
                compare_pixels(a, b)

    def test_svg_metadata_is_checked_against_real_viewbox(self):
        data = (b'<svg xmlns="http://www.w3.org/2000/svg" viewBox="-1 -2 30 20">'
                b'<text>Memory fonts</text><text>Retained bytes</text></svg>')
        metadata = {"version": 1, "operation_id": "svg", "media_type": "image/svg+xml",
                    "byte_length": len(data), "runtime_policy": "deterministic",
                    "svg_viewport": {"min_x": -1, "min_y": -2, "width": 30, "height": 20}}
        validate_svg(data, metadata)
        metadata["svg_viewport"]["width"] = 31
        with self.assertRaises(ValueError):
            validate_svg(data, metadata)

    def test_missing_math_is_a_structured_capability_not_any_failure(self):
        value = {"version": 1, "ok": False, "code": 3, "code_name": "MERMAN_UNSUPPORTED_OPERATION",
                 "kind": "missing-capability", "capability_id": "math", "message": "math unavailable"}
        validate_error(value, capability="math")
        value["capability_id"] = "layout-elk"
        with self.assertRaises(ValueError):
            validate_error(value, capability="math")

    def test_native_error_names_do_not_alias_core_or_descriptor_status(self):
        error = {"version": 1, "ok": False, "status": 3, "status_name": "unsupported-operation",
                 "kind": "missing-capability", "capability_id": "math", "message": "math unavailable"}
        validate_native_error(error, "unsupported-operation", "math")
        with self.assertRaises(KeyError):
            validate_error(error, capability="math")

    def test_catalog_is_actual_flavor_not_a_file_label(self):
        catalog = {"schema_version": 1, "transport_api_version": 1,
                   "constructor_service_ids": ["native-font-context"],
                   "capabilities": {"capability_ids": sorted(PROTOCOL["required_capabilities"]),
                                    "operation_ids": sorted(PROTOCOL["required_operations"]),
                                    "output_ids": ["png", "svg"], "system_adapter_ids": [],
                                    "text_measurement": {"provider_ids": ["native-fonts"]}}}
        validate_catalog(catalog, "standard")
        with self.assertRaises(ValueError):
            validate_catalog(catalog, "math")


class MathOutputTests(unittest.TestCase):
    viewport = {"min_x": 0, "min_y": 0, "width": 100, "height": 50}
    glyph = '<path d="M40 20h20v10H40Z" fill="red"/>'
    marker = 'class="merman-prepared-math-native"'

    def svg(self, body):
        return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">' +
                body + '<path d="M1 1H99V49H1Z" fill="none" stroke="black"/></svg>').encode()

    def metadata(self, data, operation):
        result = {"version": 1, "operation_id": operation, "media_type": "image/" +
                  ("svg+xml" if operation == "svg" else "png"),
                  "byte_length": len(data), "runtime_policy": "deterministic"}
        if operation == "svg":
            result["svg_viewport"] = self.viewport
        else:
            result["output_plan"] = {
                "kind": "raster", "limited": False, "requested_scale": 2,
                "effective_scale": 2, "width_px": 200, "height_px": 100}
        return result

    def png(self, ink=True, elsewhere=False):
        from PIL import Image, ImageDraw
        image = Image.new("RGBA", (200, 100), "white")
        draw = ImageDraw.Draw(image)
        draw.rectangle((2, 2, 198, 98), outline="black", width=2)
        if ink:
            draw.rectangle((84, 44, 114, 54) if not elsewhere else (4, 4, 24, 14),
                           fill="red")
        output = io.BytesIO()
        image.save(output, "PNG")
        return output.getvalue()

    def test_empty_math_marker_cannot_borrow_a_node_path(self):
        for body in [f'<g {self.marker}/>',
                     f'<defs><g {self.marker}>{self.glyph}</g></defs>',
                     f'<defs><g id="formula">{self.glyph}</g></defs><use {self.marker} href="#missing"/>',
                     f'<defs><g id="formula"/></defs><use {self.marker} href="#formula"/>']:
            with self.subTest(body=body), self.assertRaisesRegex(ValueError, "formula"):
                data = self.svg(body)
                validate_svg(data, self.metadata(data, "svg"), mathematical=True)

    def test_inline_and_referenced_native_formula_shapes_are_supported(self):
        for body in [f'<g {self.marker}>{self.glyph}</g>',
                     f'<defs><g id="formula">{self.glyph}</g></defs><use {self.marker} href="#formula"/>',
                     f'<defs><g id="label"><g {self.marker}>{self.glyph}</g></g></defs><use href="#label"/>']:
            with self.subTest(body=body):
                data = self.svg(body)
                self.assertEqual(validate_svg(data, self.metadata(data, "svg"), mathematical=True),
                                 self.viewport)

    def test_formula_absence_fails_independently_of_runtime_pixel_equality(self):
        data = self.png(ink=False)
        with tempfile.TemporaryDirectory() as directory:
            paths = [Path(directory) / (runtime + ".png")
                     for runtime in ("native", "wasmtime", "wazero")]
            for path in paths:
                path.write_bytes(data)
            for path in paths[1:]:
                compare_pixels(paths[0], path)
            with self.assertRaisesRegex(ValueError, "formula ink"):
                save_render(directory, "math", "png", data, self.metadata(data, "png"),
                            self.viewport)

    def test_formula_color_must_be_inside_the_fixture_region(self):
        for data in [self.png(ink=False), self.png(elsewhere=True)]:
            with self.subTest(size=len(data)), self.assertRaisesRegex(ValueError, "formula ink"):
                validate_png(data, self.metadata(data, "png"), self.viewport, mathematical=True)
        data = self.png()
        result = validate_png(data, self.metadata(data, "png"), self.viewport, mathematical=True)
        self.assertGreaterEqual(result["math_ink_pixels"], 16)

    def test_wasi_coordinator_rechecks_each_host_formula_content(self):
        from types import SimpleNamespace
        from unittest.mock import patch
        import smoke

        for missing_runtime in (None, "wazero", "wasmtime"):
            with self.subTest(missing_runtime=missing_runtime), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                module = root / "reactor.wasm"
                module.write_bytes(b"transport stub")
                args = SimpleNamespace(module=module, output_dir=root / "output", flavor="math",
                                       catalog_out=root / "catalog.json")

                def host_result(command, **kwargs):
                    directory = Path(command[command.index("--output-dir") + 1])
                    Path(command[command.index("--catalog-out") + 1]).write_bytes(b"{}")
                    artifacts = {}
                    for name in ("basic", "math"):
                        body = (f'<g {self.marker}>{self.glyph}</g>' if name == "math" else
                                '<text>Memory fonts</text><text>Retained bytes</text>')
                        svg = self.svg(body)
                        png = self.png(ink=name != "math" or directory.name != missing_runtime)
                        for operation, data in (("svg", svg), ("png", png)):
                            (directory / (name + "." + operation)).write_bytes(data)
                            (directory / (name + "." + operation + ".metadata.json")).write_text(
                                json.dumps(self.metadata(data, operation)))
                        artifacts[name + ".png"] = validate_png(png, self.metadata(png, "png"),
                                                                 self.viewport)
                    report = {"ok": True, "module_sha256": sha(module.read_bytes()),
                              "checks": PROTOCOL["checks"], "fixtures_sha256": sha(FIXTURE_BYTES),
                              "fonts": [], "metadata_ids_called": [], "runtime_version": "stub",
                              "artifacts": artifacts}
                    return SimpleNamespace(returncode=0, stdout=json.dumps(report), stderr="")

                with patch("smoke.subprocess.run", side_effect=host_result):
                    if missing_runtime is None:
                        self.assertTrue(smoke.run(args)["ok"])
                    else:
                        with self.assertRaisesRegex(ValueError, "formula ink"):
                            smoke.run(args)


if __name__ == "__main__":
    unittest.main()
