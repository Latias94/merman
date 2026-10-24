"""Bounded, serial wasmtime consumer of the merman-wasi ABI 1 reactor."""

from contextlib import contextmanager
import json
import struct

import wasmtime as wt

MAX_BYTES = 32 * 1024 * 1024
MAX_ENVELOPE = 1024 * 1024
MEMORY_BYTES = 512 * 1024 * 1024
FUEL = 5_000_000_000


class ProtocolError(RuntimeError):
    """The module violated the ABI or could not publish an owned result."""


class GuestError(RuntimeError):
    """An ordinary owned error; the client remains usable. See payload."""

    def __init__(self, payload):
        self.payload = payload
        super().__init__(f"{payload['code_name']}: {payload['message']}")


def require(condition, message):
    if not condition:
        raise ProtocolError(message)


def envelope(value):
    data = json.dumps(value, ensure_ascii=False, allow_nan=False).encode("utf-8")
    if len(data) > MAX_ENVELOPE:
        raise ValueError("JSON envelope exceeds the ABI 1 MiB limit")
    return data


class Client:
    """One instance and engine. Use serially, never concurrently or reentrantly."""

    def __init__(self, module: bytes, options: dict = None, fonts: list = None, *, fuel: int = FUEL):
        self._closed = self._unusable = False
        self._handle = 0
        self._runtime = self._module = self._store = None
        self._instance = self._exports = self._memory = None
        if not isinstance(module, bytes):
            raise TypeError("module must be bytes")
        if options is not None and not isinstance(options, dict):
            raise TypeError("options must be a dict or None")
        if type(fuel) is not int or not 0 < fuel < 2**64:
            raise ValueError("fuel must be a positive u64 instruction budget")
        self._fuel = fuel
        chunks, ranges, size = [], [], 0
        for font in fonts or []:
            if not isinstance(font, bytes) or not font:
                raise ValueError("each font must be nonempty bytes")
            if size + len(font) > MAX_BYTES:
                raise ValueError("font inputs exceed the ABI 32 MiB limit")
            ranges.append({"offset": size, "length": len(font)})
            chunks.append(font)
            size += len(font)
        opts = dict(options or {})
        opts.setdefault("resources", {"profile": "interactive"})
        config_bytes = envelope({"version": 1, "options": opts,
                                 "fonts": ranges, "bundled_fonts": True})
        try:
            with wt.Config() as config:
                config.consume_fuel = True
                config.wasm_threads = False
                config.parallel_compilation = False
                self._runtime = wt.Engine(config)
            self._module = wt.Module(self._runtime, module)
            self._store = wt.Store(self._runtime)
            self._store.set_limits(memory_size=MEMORY_BYTES, instances=1,
                                   memories=1, tables=10)
            self._store.set_fuel(self._fuel)
            wasi = wt.WasiConfig()
            self._store.set_wasi(wasi)
            with wt.Linker(self._runtime) as linker:
                linker.define_wasi()
                self._instance = linker.instantiate(self._store, self._module)
            self._exports = self._instance.exports(self._store)
            self._memory = self._exports["memory"]
            memory_type = self._memory.type(self._store)
            require(not memory_type.is_64 and not memory_type.is_shared,
                    "requires nonshared wasm32 memory")
            self._call("_initialize")
            require(self._call("abi_version") == 1, "requires ABI 1")
            with self._inputs(config_bytes, b"".join(chunks)) as pointers:
                handle = self._call("engine_new", pointers[0], len(config_bytes),
                                    pointers[1], size)
                engine, media, data, metadata = self._result(handle)
                self._handle = engine
                require(engine != 0 and media == "application/json" and not data
                        and metadata == {"version": 1, "ok": True},
                        "invalid constructor success")
        except BaseException:
            self.close()
            raise

    def __enter__(self):
        self._ready()
        return self

    def __exit__(self, exc_type, exc, traceback):
        self.close()

    def _ready(self):
        if self._closed or self._unusable:
            raise RuntimeError("client is closed or unusable; create a new client")

    def _call(self, name, *args):
        self._ready()
        self._store.set_fuel(self._fuel)
        export = name if name == "_initialize" else "merman_wasi_" + name
        signed = [n if n < 2**31 else n - 2**32 for n in args]
        try:
            value = self._exports[export](self._store, *signed)
        except (wt.Trap, wt.WasmtimeError):
            self._unusable = True
            raise
        return 0 if value is None else value & 0xFFFFFFFF

    def _release(self, name, handle):
        status = self._call(name, handle)
        if status:
            self._unusable = True
            raise ProtocolError(f"{name} failed: {status - 2**32}")

    def _read(self, pointer, length):
        require((pointer == 0) == (length == 0), "noncanonical pointer/length")
        require(length <= MAX_BYTES and pointer + length <= 2**32,
                "segment exceeds ABI byte/address limit")
        require(pointer + length <= self._memory.data_len(self._store),
                "segment exceeds linear memory")
        return bytes(self._memory.read(self._store, pointer, pointer + length))

    @contextmanager
    def _inputs(self, *buffers):
        pointers = []
        try:
            for data in buffers:
                if len(data) > MAX_BYTES:
                    raise ValueError("input exceeds the ABI 32 MiB limit")
                pointer = self._call("alloc", len(data)) if data else 0
                require(pointer != 0 or not data, "input allocation failed")
                pointers.append(pointer)
                if data:
                    require(pointer + len(data) <= min(2**32, self._memory.data_len(self._store)),
                            "input allocation exceeds memory")
                    self._memory.write(self._store, data, pointer)
            yield pointers
        finally:
            for pointer in reversed(pointers):
                if pointer and not (self._closed or self._unusable):
                    self._release("free", pointer)

    def _result(self, handle):
        require(handle != 0, "producer could not publish an owned result")
        try:
            pointer = self._call("result_info", handle)
            fields = struct.unpack("<10I", self._read(pointer, 40))
            require(fields[:2] == (1, 40) and fields[2] in (0, 1),
                    "invalid ABI 1 descriptor")
            require(40 + sum(fields[5::2]) <= MAX_BYTES, "result exceeds ABI limit")
            media, data, raw_json = [self._read(fields[i], fields[i + 1])
                                     for i in (4, 6, 8)]
            media = media.decode("utf-8")
            metadata = json.loads(raw_json) if raw_json else None
            if fields[2] == 1:
                require(fields[3] == 0 and not data and media == "application/json"
                        and isinstance(metadata, dict) and metadata.get("version") == 1
                        and metadata.get("ok") is False and "code_name" in metadata
                        and "message" in metadata, "invalid ordinary error payload")
                raise GuestError(metadata)
            return fields[3], media, data, metadata
        finally:
            if not (self._closed or self._unusable):
                self._release("result_free", handle)

    def catalog(self):
        engine, media, data, metadata = self._result(self._call("runtime_catalog"))
        require(engine == 0 and media == "application/json" and metadata is None,
                "invalid catalog result")
        return json.loads(data)

    def execute(self, operation: str, source: bytes, options: dict = None):
        self._ready()
        if not isinstance(operation, str) or not isinstance(source, bytes):
            raise TypeError("operation must be str and source must be bytes")
        if options is not None and not isinstance(options, dict):
            raise TypeError("options must be a dict or None")
        request = {"version": 1, "operation_id": operation}
        if options is not None:
            request["options"] = options
        request_bytes = envelope(request)
        with self._inputs(request_bytes, source) as pointers:
            handle = self._call("execute", self._handle, pointers[0], len(request_bytes),
                                pointers[1], len(source))
            engine, media, data, metadata = self._result(handle)
            require(engine == self._handle, "execute engine identity mismatch")
            return data, metadata

    def close(self):
        if self._closed:
            return
        try:
            if self._handle and not self._unusable:
                self._release("engine_close", self._handle)
        finally:
            self._closed = True
            self._handle = 0
            self._instance = self._exports = self._memory = None
            try:
                if self._store is not None:
                    self._store.close()
            finally:
                try:
                    if self._module is not None:
                        self._module.close()
                finally:
                    if self._runtime is not None:
                        self._runtime.close()
