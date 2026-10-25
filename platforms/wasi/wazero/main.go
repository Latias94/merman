// Real wazero smoke host for the independent merman-wasi scalar ABI v1.
package main

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"encoding/xml"
	"flag"
	"fmt"
	"image/color"
	"image/png"
	"io"
	"math"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"runtime/debug"
	"sort"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

type object = map[string]any

type protocol struct {
	MemoryLimit       uint32            `json:"memory_limit_bytes"`
	Timeout           int               `json:"timeout_seconds"`
	ImportModule      string            `json:"import_module"`
	Imports           []string          `json:"allowed_imports"`
	Exports           map[string]int    `json:"function_exports"`
	DependencyExports map[string][2]int `json:"optional_dependency_exports"`
	Capabilities      []string          `json:"required_capabilities"`
	Operations        []string          `json:"required_operations"`
	Checks            []string          `json:"checks"`
}

type fixtures struct {
	FontFiles   []string          `json:"font_files"`
	Options     object            `json:"engine_options"`
	PNGOptions  object            `json:"png_options"`
	Constrained object            `json:"constrained_options"`
	Sources     map[string]string `json:"sources"`
}

func require(ok bool, format string, args ...any) {
	if !ok {
		panic(fmt.Sprintf(format, args...))
	}
}
func must(err error) {
	if err != nil {
		panic(err)
	}
}
func read(path string) []byte   { data, err := os.ReadFile(path); must(err); return data }
func digest(data []byte) string { sum := sha256.Sum256(data); return hex.EncodeToString(sum[:]) }
func encode(value any) []byte   { data, err := json.Marshal(value); must(err); return data }
func decode(data []byte) object { var value object; must(json.Unmarshal(data, &value)); return value }
func obj(value any) object      { return value.(map[string]any) }
func num(value any) float64     { return value.(float64) }
func stringsOf(value any) []string {
	var out []string
	for _, item := range value.([]any) {
		out = append(out, item.(string))
	}
	return out
}
func contains(ids []string, id string) bool {
	for _, value := range ids {
		if value == id {
			return true
		}
	}
	return false
}
func write(path string, data []byte) {
	must(os.MkdirAll(filepath.Dir(path), 0755))
	must(os.WriteFile(path, data, 0644))
}
func writeJSON(path string, value any) {
	data, err := json.MarshalIndent(value, "", "  ")
	must(err)
	write(path, append(data, '\n'))
}
func overlay(base, extra object) object {
	result := object{}
	for key, value := range base {
		result[key] = value
	}
	for key, value := range extra {
		result[key] = value
	}
	return result
}

type result struct {
	Handle, Pointer, Status, Engine uint32
	Descriptor, Data, JSON          []byte
	Media                           string
}

func descriptor(data []byte) [10]uint32 {
	require(len(data) == 40, "descriptor must contain exactly 40 bytes")
	var fields [10]uint32
	for i := range fields {
		fields[i] = binary.LittleEndian.Uint32(data[4*i : 4*i+4])
	}
	require(fields[0] == 1 && fields[1] == 40 && fields[2] <= 1, "unsupported result descriptor")
	for i := 4; i < 10; i += 2 {
		require((fields[i] == 0) == (fields[i+1] == 0), "noncanonical result segment")
		require(fields[i+1] <= 32<<20 && uint64(fields[i])+uint64(fields[i+1]) <= 1<<32,
			"result segment exceeds ABI ceiling or overflows wasm32")
	}
	return fields
}

type host struct {
	ctx     context.Context
	runtime wazero.Runtime
	module  api.Module
	audit   object
	shims   map[string]int
}

func auditModule(compiled wazero.CompiledModule, policy protocol) object {
	imports, exports := []string{}, []string{}
	require(len(compiled.ImportedMemories()) == 0, "imported memory is forbidden")
	for _, fn := range compiled.ImportedFunctions() {
		module, name, _ := fn.Import()
		require(module == policy.ImportModule && contains(policy.Imports, name), "forbidden actual import: %s.%s", module, name)
		imports = append(imports, module+"."+name)
	}
	functions := compiled.ExportedFunctions()
	dependencyExports := []string{}
	for name := range functions {
		_, ok := policy.Exports[name]
		if !ok {
			shape, admitted := policy.DependencyExports[name]
			require(admitted, "unexpected function export: %s", name)
			fn := functions[name]
			require(len(fn.ParamTypes()) == shape[0] && len(fn.ResultTypes()) == shape[1], "wrong dependency helper arity: %s", name)
			for _, kind := range append(append([]api.ValueType{}, fn.ParamTypes()...), fn.ResultTypes()...) {
				require(kind == api.ValueTypeI32, "non-i32 dependency helper: %s", name)
			}
			dependencyExports = append(dependencyExports, name)
		}
		exports = append(exports, name)
	}
	for name, count := range policy.Exports {
		fn, ok := functions[name]
		require(ok, "missing function export: %s", name)
		require(len(fn.ParamTypes()) == count, "wrong scalar parameter count: %s", name)
		for _, kind := range fn.ParamTypes() {
			require(kind == api.ValueTypeI32, "non-i32 parameter: %s", name)
		}
		if name == "_initialize" {
			require(len(fn.ResultTypes()) == 0, "_initialize must return void")
		} else {
			require(reflect.DeepEqual(fn.ResultTypes(), []api.ValueType{api.ValueTypeI32}), "wrong scalar result: %s", name)
		}
	}
	memories := compiled.ExportedMemories()
	memory, ok := memories["memory"]
	require(ok && len(memories) == 1, "requires one exported memory")
	require(memory.Min() <= policy.MemoryLimit/65536, "initial memory exceeds host policy")
	exports = append(exports, "memory")
	sort.Strings(exports)
	sort.Strings(imports)
	sort.Strings(dependencyExports)
	return object{"imports": imports, "exports": exports, "preopened_directories": []string{},
		"dependency_exports": dependencyExports,
		"memory_limit_bytes": policy.MemoryLimit, "timeout_seconds": policy.Timeout}
}

func newHost(ctx context.Context, data []byte, policy protocol) *host {
	r := wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig().
		WithMemoryLimitPages(policy.MemoryLimit/65536).WithCloseOnContextDone(true))
	h := &host{ctx: ctx, runtime: r, shims: map[string]int{"random_get": 0, "clock_time_get": 0, "sched_yield": 0}}
	ready := false
	defer func() {
		if !ready {
			_ = r.Close(context.Background())
		}
	}()
	compiled, err := r.CompileModule(ctx, data)
	must(err)
	h.audit = auditModule(compiled, policy)
	builder := r.NewHostModuleBuilder(policy.ImportModule)
	wasi_snapshot_preview1.NewFunctionExporter().ExportFunctions(builder)
	builder.NewFunctionBuilder().WithFunc(func(_ context.Context, m api.Module, ptr, length uint32) uint32 {
		h.shims["random_get"]++
		buffer, ok := m.Memory().Read(ptr, length)
		if !ok {
			return 21
		}
		clear(buffer)
		return 0
	}).Export("random_get")
	builder.NewFunctionBuilder().WithFunc(func(_ context.Context, m api.Module, id uint32, precision uint64, ptr uint32) uint32 {
		h.shims["clock_time_get"]++
		if !m.Memory().WriteUint64Le(ptr, 0) {
			return 21
		}
		return 0
	}).Export("clock_time_get")
	builder.NewFunctionBuilder().WithFunc(func() uint32 { h.shims["sched_yield"]++; return 0 }).Export("sched_yield")
	_, err = builder.Instantiate(ctx)
	must(err)
	// No WithFSConfig, WithEnv, WithRandSource, or inherited stdio is installed.
	h.module, err = r.InstantiateModule(ctx, compiled, wazero.NewModuleConfig().WithStartFunctions())
	must(err)
	h.call("_initialize")
	require(h.call("abi_version") == 1, "ABI version is not 1")
	ready = true
	return h
}

func (h *host) call(name string, args ...uint32) uint32 {
	if name != "_initialize" {
		name = "merman_wasi_" + name
	}
	values := make([]uint64, len(args))
	for i, value := range args {
		values[i] = uint64(value)
	}
	out, err := h.module.ExportedFunction(name).Call(h.ctx, values...)
	must(err)
	if len(out) == 0 {
		return 0
	}
	return uint32(out[0])
}
func (h *host) release(name string, handle uint32) {
	require(h.call(name, handle) == 0, "%s failed", name)
}
func (h *host) read(ptr, length uint32) []byte {
	data, ok := h.module.Memory().Read(ptr, length)
	require(ok, "out-of-bounds module segment")
	return append([]byte{}, data...)
}
func (h *host) alloc(data []byte) uint32 {
	if len(data) == 0 {
		return 0
	}
	ptr := h.call("alloc", uint32(len(data)))
	require(ptr != 0, "input allocation failed")
	require(h.module.Memory().Write(ptr, data), "input write out of bounds")
	return ptr
}
func (h *host) result(handle uint32) result {
	require(handle != 0, "producer could not publish an owned result")
	ptr := h.call("result_info", handle)
	require(ptr != 0, "result has no descriptor")
	data := h.read(ptr, 40)
	fields := descriptor(data)
	media, sideJSON := h.read(fields[4], fields[5]), h.read(fields[8], fields[9])
	require(utf8.Valid(media) && utf8.Valid(sideJSON), "result media/JSON is not UTF-8")
	return result{Handle: handle, Pointer: ptr, Status: fields[2], Engine: fields[3], Descriptor: data,
		Media: string(media), Data: h.read(fields[6], fields[7]), JSON: sideJSON}
}
func (h *host) newEngine(fonts []byte, ranges []object, options object) uint32 {
	config := object{"version": 1, "fonts": ranges, "bundled_fonts": false}
	if options != nil {
		config["options"] = options
	}
	data := encode(config)
	fontPtr, configPtr := h.alloc(fonts), h.alloc(data)
	value := h.result(h.call("engine_new", configPtr, uint32(len(data)), fontPtr, uint32(len(fonts))))
	// Destroy borrowed bytes before releasing them; all subsequent rendering uses copied fonts.
	require(h.module.Memory().Write(fontPtr, make([]byte, len(fonts))), "font input overwrite failed")
	require(h.module.Memory().Write(configPtr, make([]byte, len(data))), "config input overwrite failed")
	h.release("free", fontPtr)
	h.release("free", configPtr)
	success(value, 0, "application/json")
	require(value.Engine != 0 && len(value.Data) == 0 && reflect.DeepEqual(decode(value.JSON), object{"version": float64(1), "ok": true}), "constructor payload mismatch")
	require(h.call("result_free", value.Engine) == 0xfffffffe, "engine/result kinds alias")
	h.release("result_free", value.Handle)
	require(h.call("result_info", value.Handle) == 0, "freed result remains accessible")
	return value.Engine
}
func (h *host) execute(engine uint32, operation, source string, options object) result {
	request := object{"version": 1, "operation_id": operation}
	if options != nil {
		request["options"] = options
	}
	data, src := encode(request), []byte(source)
	reqPtr, srcPtr := h.alloc(data), h.alloc(src)
	handle := h.call("execute", engine, reqPtr, uint32(len(data)), srcPtr, uint32(len(src)))
	h.release("free", reqPtr)
	if srcPtr != 0 {
		h.release("free", srcPtr)
	}
	return h.result(handle)
}
func success(value result, engine uint32, media string) {
	require(value.Status == 0, "unexpected owned error: %s", value.JSON)
	if engine != 0 {
		require(value.Engine == engine, "result engine identity mismatch")
	}
	require(value.Media == media, "result media mismatch: %s", value.Media)
}
func ownedError(h *host, value result, code, capability string) object {
	require(value.Status == 1 && value.Engine == 0 && len(value.Data) == 0 && value.Media == "application/json", "expected owned ordinary error")
	payload := decode(value.JSON)
	require(payload["version"] == float64(1) && payload["ok"] == false && payload["kind"] != "" && payload["message"] != "", "invalid core error schema")
	_ = num(payload["code"])
	if code != "" {
		require(payload["code_name"] == code, "unexpected error: %s", value.JSON)
	}
	if capability != "" {
		require(payload["kind"] == "missing-capability" && payload["capability_id"] == capability, "expected missing %s: %s", capability, value.JSON)
	}
	h.release("result_free", value.Handle)
	return payload
}

func validateCatalog(catalog object, flavor string, policy protocol) {
	require(catalog["schema_version"] == float64(1) && catalog["transport_api_version"] == float64(1), "catalog schema/API mismatch")
	caps := obj(catalog["capabilities"])
	for _, key := range []string{"capability_ids", "operation_ids", "output_ids", "system_adapter_ids"} {
		ids := stringsOf(caps[key])
		require(sort.StringsAreSorted(ids), "unsorted catalog IDs: %s", key)
		for i := 1; i < len(ids); i++ {
			require(ids[i-1] != ids[i], "duplicate catalog ID: %s", key)
		}
	}
	ids, ops := stringsOf(caps["capability_ids"]), stringsOf(caps["operation_ids"])
	for _, id := range policy.Capabilities {
		require(contains(ids, id), "missing SDK capability: %s", id)
	}
	for _, id := range policy.Operations {
		require(contains(ops, id), "missing smoke operation: %s", id)
	}
	require(contains(ids, "math") == (flavor != "standard"), "math flavor/catalog mismatch")
	require(contains(ids, "layout-elk") == (flavor == "full"), "ELK flavor/catalog mismatch")
	require(len(stringsOf(caps["system_adapter_ids"])) == 0, "ambient system adapters forbidden")
	require(contains(stringsOf(catalog["constructor_service_ids"]), "native-font-context"), "missing font constructor service")
	providers := stringsOf(obj(caps["text_measurement"])["provider_ids"])
	require(contains(providers, "native-fonts") && !contains(providers, "host-callback"), "unexpected measurement providers")
}

func validateSVG(data []byte, metadata object, mathematical bool) object {
	require(metadata["version"] == float64(1) && metadata["operation_id"] == "svg" && metadata["media_type"] == "image/svg+xml" && num(metadata["byte_length"]) == float64(len(data)), "SVG metadata mismatch")
	require(metadata["runtime_policy"] == "deterministic", "non-deterministic SVG")
	decoder := xml.NewDecoder(bytes.NewReader(data))
	var view []float64
	seen := false
	for {
		token, err := decoder.Token()
		if err == io.EOF {
			break
		}
		must(err)
		if element, ok := token.(xml.StartElement); ok && !seen {
			seen = true
			require(element.Name.Local == "svg" && element.Name.Space == "http://www.w3.org/2000/svg", "not an SVG root")
			for _, attr := range element.Attr {
				if attr.Name.Local == "viewBox" {
					for _, part := range strings.Fields(strings.ReplaceAll(attr.Value, ",", " ")) {
						var value float64
						_, err := fmt.Sscan(part, &value)
						must(err)
						view = append(view, value)
					}
				}
			}
		}
	}
	require(len(view) == 4 && view[2] > 0 && view[3] > 0, "invalid SVG viewport")
	viewport := obj(metadata["svg_viewport"])
	for i, key := range []string{"min_x", "min_y", "width", "height"} {
		value := num(viewport[key])
		require(!math.IsNaN(value) && !math.IsInf(value, 0) && math.Abs(value-view[i]) <= .001, "SVG viewport metadata mismatch: %s", key)
	}
	require(!bytes.Contains(data, []byte("@font-face")) && !bytes.Contains(data, []byte("data:font")), "unexpected embedded font")
	if mathematical {
		require(bytes.Contains(data, []byte("merman-prepared-math-native")) && bytes.Contains(data, []byte("<path")), "math SVG has no native formula projection")
		require(!bytes.Contains(data, []byte(`data-merman-math-native="unavailable"`)), "math SVG reports unavailable rendering")
	} else {
		require(bytes.Contains(data, []byte("Memory fonts")) && bytes.Contains(data, []byte("Retained bytes")), "missing fixture labels")
	}
	return viewport
}
func validatePNG(data []byte, metadata, viewport object) object {
	require(metadata["version"] == float64(1) && metadata["operation_id"] == "png" && metadata["media_type"] == "image/png" && num(metadata["byte_length"]) == float64(len(data)), "PNG metadata mismatch")
	require(metadata["runtime_policy"] == "deterministic", "non-deterministic PNG")
	image, err := png.Decode(bytes.NewReader(data))
	must(err)
	bounds := image.Bounds()
	width, height := bounds.Dx(), bounds.Dy()
	var pixels []byte
	visible, varied := false, false
	var first color.NRGBA
	for y := bounds.Min.Y; y < bounds.Max.Y; y++ {
		for x := bounds.Min.X; x < bounds.Max.X; x++ {
			pixel := color.NRGBAModel.Convert(image.At(x, y)).(color.NRGBA)
			if x == bounds.Min.X && y == bounds.Min.Y {
				first = pixel
			}
			visible = visible || pixel.A != 0
			varied = varied || pixel != first
			pixels = append(pixels, pixel.R, pixel.G, pixel.B, pixel.A)
		}
	}
	require(visible && varied, "PNG is transparent or flat")
	plan := obj(metadata["output_plan"])
	require(plan["kind"] == "raster" && plan["limited"] == false && num(plan["requested_scale"]) == 2 && num(plan["effective_scale"]) == 2, "PNG is limited or not 2x")
	require(num(plan["width_px"]) == float64(width) && num(plan["height_px"]) == float64(height), "PNG dimension metadata mismatch")
	require(float64(width) == math.Ceil(num(viewport["width"]))*2 && float64(height) == math.Ceil(num(viewport["height"]))*2, "PNG/SVG scale mismatch")
	return object{"width": width, "height": height, "rgba8_sha256": digest(pixels), "sha256": digest(data), "byte_length": len(data)}
}
func saveRender(output, name, operation string, value result, viewport object) object {
	metadata := decode(value.JSON)
	write(filepath.Join(output, name+"."+operation), value.Data)
	writeJSON(filepath.Join(output, name+"."+operation+".metadata.json"), metadata)
	if operation == "svg" {
		view := validateSVG(value.Data, metadata, name == "math")
		return object{"sha256": digest(value.Data), "byte_length": len(value.Data), "svg_viewport": view}
	}
	return validatePNG(value.Data, metadata, viewport)
}

func run(module, flavor, catalogOut, output, directory string) object {
	var policy protocol
	must(json.Unmarshal(read(filepath.Join(directory, "protocol.json")), &policy))
	fixtureBytes := read(filepath.Join(directory, "fixtures.json"))
	var fixture fixtures
	must(json.Unmarshal(fixtureBytes, &fixture))
	root := filepath.Dir(filepath.Dir(directory))
	var fonts []byte
	ranges, fontEvidence := []object{}, []object{}
	for _, path := range fixture.FontFiles {
		data := read(filepath.Join(root, path))
		ranges = append(ranges, object{"offset": len(fonts), "length": len(data)})
		fontEvidence = append(fontEvidence, object{"path": path, "sha256": digest(data), "byte_length": len(data)})
		fonts = append(fonts, data...)
	}
	moduleBytes := read(module)
	ctx, cancel := context.WithTimeout(context.Background(), time.Duration(policy.Timeout)*time.Second)
	defer cancel()
	h := newHost(ctx, moduleBytes, policy)
	defer h.runtime.Close(context.Background())
	artifacts, errors := object{}, object{}
	report := object{"schema_version": 1, "abi_version": 1, "descriptor_bytes": 40, "ok": true, "runtime": "wazero", "runtime_version": wazeroVersion(), "flavor": flavor,
		"module_sha256": digest(moduleBytes), "fixtures_sha256": digest(fixtureBytes), "fonts": fontEvidence,
		"audit": h.audit, "artifacts": artifacts, "errors": errors}
	catalogResult := h.result(h.call("runtime_catalog"))
	success(catalogResult, 0, "application/json")
	require(catalogResult.Engine == 0 && len(catalogResult.JSON) == 0, "catalog engine/metadata must be empty")
	write(catalogOut, catalogResult.Data)
	report["catalog_sha256"] = digest(catalogResult.Data)
	catalog := decode(catalogResult.Data)
	validateCatalog(catalog, flavor, policy)
	if flavor == "standard" {
		require(len(h.audit["dependency_exports"].([]string)) == 0, "standard unexpectedly links math dependency helpers")
	}
	engine := h.newEngine(fonts, ranges, fixture.Options)
	for _, name := range stringsOf(catalog["metadata_ids"]) {
		value := h.execute(engine, name, "", nil)
		success(value, engine, "application/json")
		require(len(value.JSON) == 0 && json.Valid(value.Data), "invalid named metadata payload")
		h.release("result_free", value.Handle)
	}
	report["metadata_ids_called"] = catalog["metadata_ids"]
	errors["parse"] = ownedError(h, h.execute(engine, "svg", fixture.Sources["parse_error"], nil), "MERMAN_PARSE_ERROR", "")
	svg := h.execute(engine, "svg", fixture.Sources["basic"], nil)
	success(svg, engine, "image/svg+xml")
	artifacts["basic.svg"] = saveRender(output, "basic", "svg", svg, nil)
	png := h.execute(engine, "png", fixture.Sources["basic"], fixture.PNGOptions)
	success(png, engine, "image/png")
	artifacts["basic.png"] = saveRender(output, "basic", "png", png, obj(obj(artifacts["basic.svg"])["svg_viewport"]))
	require(reflect.DeepEqual(h.result(svg.Handle), svg), "PNG execution mutated independent SVG result")
	elk := h.execute(engine, "svg", fixture.Sources["elk"], nil)
	success(elk, engine, "image/svg+xml")
	saveRender(output, "elk", "svg", elk, nil)
	if flavor != "full" {
		dagre := h.execute(engine, "svg", fixture.Sources["dagre"], nil)
		success(dagre, engine, "image/svg+xml")
		require(bytes.Equal(elk.Data, dagre.Data), "absent ELK did not use byte-equivalent Dagre family fallback")
		h.release("result_free", dagre.Handle)
	}
	h.release("result_free", elk.Handle)
	plan := h.execute(engine, "svg-plan-json", fixture.Sources["elk"], nil)
	success(plan, engine, "application/json")
	value := decode(plan.Data)
	require(value["ready"] == true && contains(stringsOf(value["required_capability_ids"]), "layout-elk") == (flavor == "full") && len(stringsOf(value["missing_capability_ids"])) == 0, "ELK plan disagrees with actual family fallback/capabilities")
	report["elk_family_fallback"] = flavor != "full"
	writeJSON(filepath.Join(output, "elk.plan.json"), value)
	h.release("result_free", plan.Handle)
	mathEngine := h.newEngine(fonts, ranges, nil)
	if flavor != "standard" {
		defaultMath := h.execute(mathEngine, "svg", fixture.Sources["math"], nil)
		success(defaultMath, mathEngine, "image/svg+xml")
		h.release("result_free", defaultMath.Handle)
	}
	for _, operation := range []string{"svg", "png"} {
		opts := fixture.Options
		if operation == "png" {
			opts = overlay(opts, fixture.PNGOptions)
		}
		value := h.execute(mathEngine, operation, fixture.Sources["math"], opts)
		if flavor == "standard" {
			errors["math."+operation] = ownedError(h, value, "", "math")
		} else {
			media := "image/svg+xml"
			var viewport object
			if operation == "png" {
				media = "image/png"
				viewport = obj(obj(artifacts["math.svg"])["svg_viewport"])
			}
			success(value, mathEngine, media)
			artifacts["math."+operation] = saveRender(output, "math", operation, value, viewport)
			h.release("result_free", value.Handle)
		}
	}
	if flavor != "standard" {
		for _, operation := range []string{"svg", "png"} {
			opts := overlay(fixture.Options, fixture.Constrained)
			if operation == "png" {
				opts = overlay(opts, fixture.PNGOptions)
			}
			value := h.execute(mathEngine, operation, fixture.Sources["math"], opts)
			media, key := "image/svg+xml", "sha256"
			var viewport object
			if operation == "png" {
				media, key = "image/png", "rgba8_sha256"
				viewport = obj(obj(artifacts["constrained.request.svg"])["svg_viewport"])
			}
			success(value, mathEngine, media)
			artifact := saveRender(filepath.Join(output, "constrained.request"), "math", operation, value, viewport)
			require(artifact[key] == obj(artifacts["math."+operation])[key], "legacy constrained request changed formula output")
			artifacts["constrained.request."+operation] = artifact
			h.release("result_free", value.Handle)
		}
		for _, operation := range []string{"svg", "png"} {
			opts, media, key := object{"svg": fixture.Options["svg"]}, "image/svg+xml", "sha256"
			var viewport object
			if operation == "png" {
				opts, media, key = overlay(opts, fixture.PNGOptions), "image/png", "rgba8_sha256"
				viewport = obj(obj(artifacts["recovered.svg"])["svg_viewport"])
			}
			recovered := h.execute(mathEngine, operation, fixture.Sources["math"], opts)
			success(recovered, mathEngine, media)
			artifact := saveRender(filepath.Join(output, "recovered"), "math", operation, recovered, viewport)
			require(artifact[key] == obj(artifacts["math."+operation])[key], "engine reuse changed formula output after constrained request")
			artifacts["recovered."+operation] = artifact
			h.release("result_free", recovered.Handle)
		}
		constrained := h.newEngine(fonts, ranges, overlay(fixture.Options, fixture.Constrained))
		for _, operation := range []string{"svg", "png"} {
			opts, media, key := object{}, "image/svg+xml", "sha256"
			var viewport object
			if operation == "png" {
				opts, media, key = fixture.PNGOptions, "image/png", "rgba8_sha256"
				viewport = obj(obj(artifacts["constrained.constructor.svg"])["svg_viewport"])
			}
			value := h.execute(constrained, operation, fixture.Sources["math"], opts)
			success(value, constrained, media)
			artifact := saveRender(filepath.Join(output, "constrained.constructor"), "math", operation, value, viewport)
			require(artifact[key] == obj(artifacts["math."+operation])[key], "legacy constrained constructor changed formula output")
			artifacts["constrained.constructor."+operation] = artifact
			h.release("result_free", value.Handle)
		}
		h.release("engine_close", constrained)
	}
	h.release("engine_close", mathEngine)
	h.release("engine_close", engine)
	require(reflect.DeepEqual(h.result(svg.Handle), svg), "engine close invalidated retained SVG")
	require(reflect.DeepEqual(h.result(png.Handle), png), "engine close invalidated retained PNG")
	require(reflect.DeepEqual(h.result(catalogResult.Handle), catalogResult), "catalog did not survive engine close")
	h.release("result_free", png.Handle)
	require(reflect.DeepEqual(h.result(svg.Handle), svg), "freeing PNG invalidated independent SVG")
	h.release("result_free", svg.Handle)
	h.release("result_free", catalogResult.Handle)
	require(h.call("result_info", svg.Handle) == 0 && h.call("engine_close", engine) == 0xfffffffe, "released handles remain valid")
	report["checks"] = policy.Checks
	report["constrained_math_status"] = "passed"
	if flavor == "standard" {
		report["constrained_math_status"] = "not-applicable-without-math"
	}
	report["deterministic_import_calls"] = h.shims
	writeJSON(filepath.Join(output, "smoke.json"), report)
	return report
}

func wazeroVersion() string {
	info, ok := debug.ReadBuildInfo()
	if ok {
		for _, dep := range info.Deps {
			if dep.Path == "github.com/tetratelabs/wazero" {
				return dep.Version
			}
		}
	}
	return "unverified"
}
func realMain() (code int) {
	module := flag.String("module", "", "path to the actual reactor")
	flavor := flag.String("flavor", "", "standard, math or full")
	catalogOut := flag.String("catalog-out", "", "raw catalog output path")
	output := flag.String("output-dir", "", "host-only output directory")
	flag.Parse()
	defer func() {
		if err := recover(); err != nil {
			report := object{"schema_version": 1, "ok": false, "runtime": "wazero", "error": fmt.Sprint(err)}
			if *output != "" {
				data, _ := json.MarshalIndent(report, "", "  ")
				_ = os.MkdirAll(*output, 0755)
				_ = os.WriteFile(filepath.Join(*output, "smoke.json"), data, 0644)
			}
			fmt.Println(string(encode(report)))
			code = 1
		}
	}()
	require(*module != "" && *catalogOut != "" && *output != "" && contains([]string{"standard", "math", "full"}, *flavor), "required: --module --flavor standard|math|full --catalog-out --output-dir")
	_, file, _, ok := runtime.Caller(0)
	require(ok, "cannot locate shared fixtures")
	directory := filepath.Dir(filepath.Dir(file))
	fmt.Println(string(encode(run(*module, *flavor, *catalogOut, *output, directory))))
	return 0
}
func main() { os.Exit(realMain()) }
