"""Real wasmtime host for merman-wasi scalar ABI v1; no guest preopens."""

import argparse
import importlib.metadata
import json
from pathlib import Path
import sys
import threading

from common import (FIXTURES, FIXTURE_BYTES, PROTOCOL, decode_descriptor, font_inputs,
                    json_bytes, require, save_render, sha, validate_catalog,
                    validate_error, write_json)


def audit_module(module):
    import wasmtime as wt

    imports = []
    for item in module.imports:
        require(item.module == PROTOCOL["import_module"] and
                item.name in PROTOCOL["allowed_imports"] and isinstance(item.type, wt.FuncType),
                "forbidden actual import: " + item.module + "." + item.name)
        imports.append(item.module + "." + item.name)
    exports = {item.name: item.type for item in module.exports}
    required = PROTOCOL["function_exports"]
    optional = PROTOCOL["optional_dependency_exports"]
    allowed = set(required) | set(optional) | {"memory"} | set(PROTOCOL["allowed_global_exports"])
    require(set(exports) <= allowed, "unexpected actual exports: " + str(set(exports) - allowed))
    for name, count in required.items():
        kind = exports.get(name)
        require(isinstance(kind, wt.FuncType), "missing function export: " + name)
        require([str(t) for t in kind.params] == ["i32"] * count,
                "export parameters are not scalar ABI v1: " + name)
        expected = [] if name == "_initialize" else ["i32"]
        require([str(t) for t in kind.results] == expected, "wrong export result: " + name)
    dependency_exports = []
    for name, (params, results) in optional.items():
        if name in exports:
            kind = exports[name]
            require(isinstance(kind, wt.FuncType) and
                    [str(t) for t in kind.params] == ["i32"] * params and
                    [str(t) for t in kind.results] == ["i32"] * results,
                    "wrong pinned dependency helper signature: " + name)
            dependency_exports.append(name)
    memory = exports.get("memory")
    require(isinstance(memory, wt.MemoryType) and not memory.is_64 and not memory.is_shared,
            "requires exported nonshared wasm32 memory")
    require(memory.limits.min * 65536 <= PROTOCOL["memory_limit_bytes"], "initial memory too large")
    for name in PROTOCOL["allowed_global_exports"]:
        if name in exports:
            require(isinstance(exports[name], wt.GlobalType) and
                    str(exports[name].content) == "i32" and not exports[name].mutable,
                    "unexpected ABI global type: " + name)
    return {"imports": sorted(imports), "exports": sorted(exports),
            "dependency_exports": sorted(dependency_exports),
            "preopened_directories": [], "memory_limit_bytes": PROTOCOL["memory_limit_bytes"],
            "timeout_seconds": PROTOCOL["timeout_seconds"], "fuel_per_call": PROTOCOL["fuel"]}


class Host:
    def __init__(self, module_bytes):
        import wasmtime as wt

        config = wt.Config()
        config.consume_fuel = True
        config.epoch_interruption = True
        config.wasm_threads = False
        config.parallel_compilation = False
        self.engine = wt.Engine(config)
        self.module = wt.Module(self.engine, module_bytes)
        self.audit = audit_module(self.module)
        self.store = wt.Store(self.engine)
        self.store.set_limits(memory_size=PROTOCOL["memory_limit_bytes"], instances=1,
                              memories=1, tables=10)
        self.store.set_fuel(PROTOCOL["fuel"])
        self.store.set_epoch_deadline(1)
        self.shims = {"random_get": 0, "clock_time_get": 0, "sched_yield": 0}
        self.timer = threading.Timer(PROTOCOL["timeout_seconds"], self.engine.increment_epoch)
        self.timer.daemon = True
        self.timer.start()
        try:
            wasi = wt.WasiConfig()
            # No inherit_* or preopen_dir calls: all host files stay outside the guest.
            self.store.set_wasi(wasi)
            linker = wt.Linker(self.engine)
            linker.define_wasi()
            linker.allow_shadowing = True

            def random_get(caller, pointer, length):
                self.shims["random_get"] += 1
                pointer, length = pointer & 0xFFFFFFFF, length & 0xFFFFFFFF
                memory = caller.get("memory")
                if memory is None or pointer + length > memory.data_len(caller):
                    return 21
                for offset in range(0, length, 65536):
                    memory.write(caller, bytes(min(65536, length - offset)), pointer + offset)
                return 0

            def clock_time_get(caller, clock_id, precision, pointer):
                self.shims["clock_time_get"] += 1
                return self.fixed_write(caller, pointer, bytes(8))

            def sched_yield():
                self.shims["sched_yield"] += 1
                return 0

            for name, params, func, caller in (
                ("random_get", [wt.ValType.i32(), wt.ValType.i32()], random_get, True),
                ("clock_time_get", [wt.ValType.i32(), wt.ValType.i64(), wt.ValType.i32()], clock_time_get, True),
                ("sched_yield", [], sched_yield, False),
            ):
                linker.define_func(PROTOCOL["import_module"], name,
                                   wt.FuncType(params, [wt.ValType.i32()]), func,
                                   access_caller=caller)
            self.instance = linker.instantiate(self.store, self.module)
            self.exports = self.instance.exports(self.store)
            self.memory = self.exports["memory"]
            self.call("_initialize")
            require(self.call("abi_version") == 1, "ABI version is not 1")
        except BaseException:
            self.close()
            raise

    def close(self):
        self.timer.cancel()
        self.store.close()
        self.module.close()
        self.engine.close()

    def fixed_write(self, caller, pointer, data):
        pointer &= 0xFFFFFFFF
        memory = caller.get("memory")
        if memory is None or pointer + len(data) > memory.data_len(caller):
            return 21  # WASI EFAULT, not a host memory exception.
        memory.write(caller, data, pointer)
        return 0

    def call(self, name, *args):
        self.store.set_fuel(PROTOCOL["fuel"])
        name = name if name == "_initialize" else "merman_wasi_" + name
        signed = [a if a < 2**31 else a - 2**32 for a in args]
        value = self.exports[name](self.store, *signed)
        return 0 if value is None else value & 0xFFFFFFFF

    def release(self, kind, token):
        require(self.call(kind, token) == 0, kind + " failed")

    def read(self, offset, length):
        require(offset + length <= self.memory.data_len(self.store), "out-of-bounds module segment")
        # Acquire a fresh memory view after every call; never retain runtime views.
        return bytes(self.memory.read(self.store, offset, offset + length))

    def alloc(self, data):
        if not data:
            return 0
        offset = self.call("alloc", len(data))
        require(offset != 0, "input allocation failed")
        self.memory.write(self.store, data, offset)
        return offset

    def result(self, handle):
        require(handle != 0, "producer could not publish an owned result")
        pointer = self.call("result_info", handle)
        require(pointer != 0, "result has no descriptor")
        descriptor = self.read(pointer, 40)
        fields = decode_descriptor(descriptor)
        segments = [self.read(fields[i], fields[i + 1]) for i in (4, 6, 8)]
        return {"handle": handle, "pointer": pointer, "descriptor": descriptor,
                "status": fields[2], "engine": fields[3], "media": segments[0].decode(),
                "data": segments[1], "json": segments[2]}

    def new_engine(self, options):
        fonts, ranges, _ = font_inputs()
        config = {"version": 1, "fonts": ranges, "bundled_fonts": False}
        if options is not None:
            config["options"] = options
        config_bytes = json_bytes(config)
        font_ptr, config_ptr = self.alloc(fonts), self.alloc(config_bytes)
        try:
            handle = self.call("engine_new", config_ptr, len(config_bytes), font_ptr, len(fonts))
            result = self.result(handle)
            # Overwrite borrowed inputs before freeing them, proving the constructor copied fonts.
            self.memory.write(self.store, bytes(len(fonts)), font_ptr)
            self.memory.write(self.store, bytes(len(config_bytes)), config_ptr)
        finally:
            self.release("free", config_ptr)
            self.release("free", font_ptr)
        require(result["status"] == 0 and result["engine"] != 0,
                "font constructor failed: " + result["json"].decode())
        require(result["media"] == "application/json" and result["data"] == b"" and
                json.loads(result["json"]) == {"version": 1, "ok": True},
                "constructor payload mismatch")
        require(self.call("result_free", result["engine"]) == 0xFFFFFFFE, "engine/result kinds alias")
        self.release("result_free", handle)
        require(self.call("result_info", handle) == 0, "freed result remains accessible")
        return result["engine"]

    def execute(self, engine, operation, source, options=None):
        request = {"version": 1, "operation_id": operation}
        if options is not None:
            request["options"] = options
        request_bytes = json_bytes(request)
        source_bytes = source.encode()
        request_ptr, source_ptr = self.alloc(request_bytes), self.alloc(source_bytes)
        try:
            handle = self.call("execute", engine, request_ptr, len(request_bytes),
                               source_ptr, len(source_bytes))
        finally:
            self.release("free", request_ptr)
            if source_ptr:
                self.release("free", source_ptr)
        return self.result(handle)


def success(result, engine=None, media=None):
    require(result["status"] == 0, "unexpected owned error: " + result["json"].decode())
    if engine is not None:
        require(result["engine"] == engine, "result engine identity mismatch")
    if media:
        require(result["media"] == media, "result media type mismatch")


def owned_error(host, result, code=None, capability=None):
    require(result["status"] == 1 and result["engine"] == 0 and result["data"] == b"" and
            result["media"] == "application/json", "expected owned ordinary error")
    payload = validate_error(json.loads(result["json"]), code, capability)
    host.release("result_free", result["handle"])
    return payload


def run(args):
    module_bytes = Path(args.module).read_bytes()
    output = Path(args.output_dir)
    output.mkdir(parents=True, exist_ok=True)
    host = Host(module_bytes)
    report = {"schema_version": 1, "abi_version": 1, "descriptor_bytes": 40,
              "ok": True, "runtime": "wasmtime",
              "runtime_version": importlib.metadata.version("wasmtime"), "flavor": args.flavor,
              "module_sha256": sha(module_bytes), "fixtures_sha256": sha(FIXTURE_BYTES),
              "fonts": font_inputs()[2], "audit": host.audit, "artifacts": {}, "errors": {}}
    try:
        catalog_result = host.result(host.call("runtime_catalog"))
        success(catalog_result, media="application/json")
        require(catalog_result["engine"] == 0 and catalog_result["json"] == b"", "catalog engine/metadata must be empty")
        catalog = json.loads(catalog_result["data"])
        # Preserve the actual catalog even when a later capability/smoke assertion fails.
        Path(args.catalog_out).parent.mkdir(parents=True, exist_ok=True)
        Path(args.catalog_out).write_bytes(catalog_result["data"])
        report["catalog_sha256"] = sha(catalog_result["data"])
        validate_catalog(catalog, args.flavor)
        if args.flavor == "standard":
            require(not host.audit["dependency_exports"], "standard unexpectedly links math dependency helpers")
        engine = host.new_engine(FIXTURES["engine_options"])
        for name in catalog["metadata_ids"]:
            result = host.execute(engine, name, "")
            success(result, engine, "application/json")
            require(result["json"] == b"", "named metadata must have empty side metadata")
            json.loads(result["data"])
            host.release("result_free", result["handle"])
        report["metadata_ids_called"] = catalog["metadata_ids"]
        report["errors"]["parse"] = owned_error(host, host.execute(
            engine, "svg", FIXTURES["sources"]["parse_error"]), "MERMAN_PARSE_ERROR")
        svg = host.execute(engine, "svg", FIXTURES["sources"]["basic"])
        success(svg, engine, "image/svg+xml")
        svg_meta = json.loads(svg["json"])
        report["artifacts"]["basic.svg"] = save_render(output, "basic", "svg", svg["data"], svg_meta)
        png = host.execute(engine, "png", FIXTURES["sources"]["basic"], FIXTURES["png_options"])
        success(png, engine, "image/png")
        report["artifacts"]["basic.png"] = save_render(
            output, "basic", "png", png["data"], json.loads(png["json"]), svg_meta["svg_viewport"])
        require(host.result(svg["handle"]) == svg, "PNG execution mutated the independent SVG result")

        elk = host.execute(engine, "svg", FIXTURES["sources"]["elk"])
        success(elk, engine, "image/svg+xml")
        save_render(output, "elk", "svg", elk["data"], json.loads(elk["json"]))
        if args.flavor != "full":
            dagre = host.execute(engine, "svg", FIXTURES["sources"]["dagre"])
            success(dagre, engine, "image/svg+xml")
            require(elk["data"] == dagre["data"], "absent ELK did not use byte-equivalent Dagre family fallback")
            host.release("result_free", dagre["handle"])
        host.release("result_free", elk["handle"])
        plan = host.execute(engine, "svg-plan-json", FIXTURES["sources"]["elk"])
        success(plan, engine, "application/json")
        value = json.loads(plan["data"])
        require(value["ready"] is True and value["missing_capability_ids"] == [] and
                ("layout-elk" in value["required_capability_ids"]) == (args.flavor == "full"),
                "ELK plan disagrees with actual family fallback/capabilities")
        report["elk_family_fallback"] = args.flavor != "full"
        write_json(output / "elk.plan.json", value)
        host.release("result_free", plan["handle"])

        # Construct without options to test WASI's default interactive math policy explicitly.
        math_engine = host.new_engine(None)
        if args.flavor != "standard":
            default_math = host.execute(math_engine, "svg", FIXTURES["sources"]["math"])
            success(default_math, math_engine, "image/svg+xml")
            host.release("result_free", default_math["handle"])
        for operation in ("svg", "png"):
            options = dict(FIXTURES["engine_options"])
            if operation == "png":
                options.update(FIXTURES["png_options"])
            result = host.execute(math_engine, operation, FIXTURES["sources"]["math"],
                                  options)
            if args.flavor == "standard":
                report["errors"]["math." + operation] = owned_error(host, result, capability="math")
            else:
                success(result, math_engine, "image/svg+xml" if operation == "svg" else "image/png")
                metadata = json.loads(result["json"])
                report["artifacts"]["math." + operation] = save_render(
                    output, "math", operation, result["data"], metadata,
                    report["artifacts"].get("math.svg", {}).get("svg_viewport"))
                host.release("result_free", result["handle"])
        if args.flavor != "standard":
            for operation in ("svg", "png"):
                opts = dict(FIXTURES["engine_options"])
                opts.update(FIXTURES["constrained_options"])
                if operation == "png":
                    opts.update(FIXTURES["png_options"])
                result = host.execute(math_engine, operation, FIXTURES["sources"]["math"], opts)
                success(result, math_engine, "image/svg+xml" if operation == "svg" else "image/png")
                artifact = save_render(output / "constrained.request", "math", operation,
                                       result["data"], json.loads(result["json"]),
                                       report["artifacts"].get("constrained.request.svg", {}).get("svg_viewport"))
                key = "sha256" if operation == "svg" else "rgba8_sha256"
                require(artifact[key] == report["artifacts"]["math." + operation][key],
                        "legacy constrained request changed formula output")
                report["artifacts"]["constrained.request." + operation] = artifact
                host.release("result_free", result["handle"])
            for operation in ("svg", "png"):
                opts = {"svg": FIXTURES["engine_options"]["svg"]}
                if operation == "png":
                    opts.update(FIXTURES["png_options"])
                recovered = host.execute(math_engine, operation, FIXTURES["sources"]["math"], opts)
                success(recovered, math_engine, "image/svg+xml" if operation == "svg" else "image/png")
                artifact = save_render(
                    output / "recovered", "math", operation, recovered["data"], json.loads(recovered["json"]),
                    report["artifacts"].get("recovered.svg", {}).get("svg_viewport"))
                key = "sha256" if operation == "svg" else "rgba8_sha256"
                require(artifact[key] == report["artifacts"]["math." + operation][key],
                        "engine reuse changed formula output after constrained request")
                report["artifacts"]["recovered." + operation] = artifact
                host.release("result_free", recovered["handle"])
            opts = dict(FIXTURES["engine_options"])
            opts.update(FIXTURES["constrained_options"])
            constrained = host.new_engine(opts)
            for operation in ("svg", "png"):
                opts = {} if operation == "svg" else FIXTURES["png_options"]
                result = host.execute(constrained, operation, FIXTURES["sources"]["math"], opts)
                success(result, constrained, "image/svg+xml" if operation == "svg" else "image/png")
                artifact = save_render(output / "constrained.constructor", "math", operation,
                                       result["data"], json.loads(result["json"]),
                                       report["artifacts"].get("constrained.constructor.svg", {}).get("svg_viewport"))
                key = "sha256" if operation == "svg" else "rgba8_sha256"
                require(artifact[key] == report["artifacts"]["math." + operation][key],
                        "legacy constrained constructor changed formula output")
                report["artifacts"]["constrained.constructor." + operation] = artifact
                host.release("result_free", result["handle"])
            host.release("engine_close", constrained)
        host.release("engine_close", math_engine)
        host.release("engine_close", engine)
        require(host.result(svg["handle"]) == svg, "engine close invalidated retained SVG")
        require(host.result(png["handle"]) == png, "engine close invalidated retained PNG")
        require(host.result(catalog_result["handle"]) == catalog_result,
                "catalog did not survive independent calls and engine close")
        host.release("result_free", png["handle"])
        require(host.result(svg["handle"]) == svg, "freeing PNG invalidated independent SVG")
        host.release("result_free", svg["handle"])
        host.release("result_free", catalog_result["handle"])
        require(host.call("result_info", svg["handle"]) == 0, "freed SVG still accessible")
        require(host.call("engine_close", engine) == 0xFFFFFFFE, "closed engine remains valid")
        report["checks"] = PROTOCOL["checks"]
        report["constrained_math_status"] = "passed" if args.flavor != "standard" else "not-applicable-without-math"
        report["deterministic_import_calls"] = host.shims
        write_json(output / "smoke.json", report)
        return report
    finally:
        host.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--module", required=True)
    parser.add_argument("--flavor", required=True, choices=("standard", "math", "full"))
    parser.add_argument("--catalog-out", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    try:
        report = run(args)
    except Exception as error:
        report = {"schema_version": 1, "ok": False, "runtime": "wasmtime", "error": str(error)}
        write_json(Path(args.output_dir) / "smoke.json", report)
        print(json.dumps(report))
        return 1
    print(json.dumps(report))
    return 0


if __name__ == "__main__":
    sys.exit(main())
