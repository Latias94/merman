package main

import (
	"bytes"
	"context"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"image"
	"image/color"
	"image/png"
	"os"
	"strings"
	"testing"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
)

func expectPanic(t *testing.T, operation func()) {
	t.Helper()
	defer func() {
		if recover() == nil {
			t.Error("invalid ABI data was accepted")
		}
	}()
	operation()
}

func TestDescriptorLittleEndianAndValidation(t *testing.T) {
	fields := [10]uint32{1, 40, 0, 0x12345678, 100, 3, 103, 5, 0, 0}
	encodeFields := func(value [10]uint32) []byte {
		data := make([]byte, 40)
		for i, field := range value {
			binary.LittleEndian.PutUint32(data[i*4:], field)
		}
		return data
	}
	if descriptor(encodeFields(fields)) != fields {
		t.Fatal("not ten little-endian u32 fields")
	}
	for _, entry := range [][2]uint32{{0, 2}, {1, 36}, {2, 3}, {7, 33 << 20}} {
		bad := fields
		bad[entry[0]] = entry[1]
		expectPanic(t, func() { descriptor(encodeFields(bad)) })
	}
	expectPanic(t, func() { descriptor(make([]byte, 39)) })
	bad := fields
	bad[4], bad[5] = 0xfffffffe, 4
	expectPanic(t, func() { descriptor(encodeFields(bad)) })
	bad = fields
	bad[4] = 0
	expectPanic(t, func() { descriptor(encodeFields(bad)) })
}

func TestPNGUsesUnpremultipliedRGBA8AndMetadata(t *testing.T) {
	image := image.NewNRGBA(image.Rect(0, 0, 4, 2))
	for i := 0; i < len(image.Pix); i += 4 {
		copy(image.Pix[i:], []byte{255, 0, 0, 255})
	}
	image.SetNRGBA(1, 1, color.NRGBA{0, 255, 0, 128})
	var output bytes.Buffer
	if err := png.Encode(&output, image); err != nil {
		t.Fatal(err)
	}
	metadata := object{"version": float64(1), "operation_id": "png", "media_type": "image/png",
		"byte_length": float64(output.Len()), "runtime_policy": "deterministic",
		"output_plan": object{"kind": "raster", "limited": false, "requested_scale": float64(2),
			"effective_scale": float64(2), "width_px": float64(4), "height_px": float64(2)}}
	viewport := object{"width": float64(1.5), "height": float64(1)}
	result := validatePNG(output.Bytes(), metadata, viewport)
	if result["rgba8_sha256"] != digest(image.Pix) {
		t.Fatal("decoded pixels were premultiplied or reordered")
	}
	obj(metadata["output_plan"])["effective_scale"] = float64(1)
	expectPanic(t, func() { validatePNG(output.Bytes(), metadata, viewport) })
}

func dependencyProbePolicy(t *testing.T) protocol {
	t.Helper()
	data, err := os.ReadFile("../protocol.json")
	if err != nil {
		t.Fatal(err)
	}
	var policy protocol
	if err := json.Unmarshal(data, &policy); err != nil {
		t.Fatal(err)
	}
	// This tiny synthetic reactor has no SDK implementation. Only its ABI/version
	// initialization and a direct reexport of the import are needed by these tests.
	policy.Exports = map[string]int{"_initialize": 0, "merman_wasi_abi_version": 0, "merman_wasi_probe": 6}
	return policy
}

func dependencyProbeModule(t *testing.T, params, results []byte) []byte {
	t.Helper()
	// (module (import "wasi_snapshot_preview1" "path_readlink" (func $probe ...))
	//   (memory (export "memory") 1) (func (export "_initialize"))
	//   (func (export "merman_wasi_abi_version") (result i32) i32.const 1)
	//   (export "merman_wasi_probe" (func $probe)))
	// Only the small import signature varies; all section sizes fit one byte.
	types := append([]byte{3, 0x60, byte(len(params))}, params...)
	types = append(types, byte(len(results)))
	types = append(types, results...)
	types = append(types, 0x60, 0, 0, 0x60, 0, 1, api.ValueTypeI32)
	data := []byte{0, 0x61, 0x73, 0x6d, 1, 0, 0, 0, 1, byte(len(types))}
	data = append(data, types...)
	fixed, err := hex.DecodeString(
		"02280116776173695f736e617073686f745f70726576696577310d706174685f726561646c696e6b0000" +
			"03030201020503010001" +
			"074604066d656d6f727902000b5f696e697469616c697a650001176d65726d616e5f776173695f6162695f76657273696f6e0002116d65726d616e5f776173695f70726f62650000" +
			"0a090202000b040041010b")
	if err != nil {
		t.Fatal(err)
	}
	return append(data, fixed...)
}

func TestDependencyImportAuditIsFlavorScopedAndExactlyTyped(t *testing.T) {
	ctx := context.Background()
	r := wazero.NewRuntime(ctx)
	defer r.Close(ctx)
	policy := dependencyProbePolicy(t)
	if contains(policy.Imports, "path_readlink") || policy.DeniedDependencyImports["path_readlink"] != [2]int{6, 1} {
		t.Fatal("path_readlink must have a separate six-i32 to i32 denial policy")
	}
	params, results := bytes.Repeat([]byte{api.ValueTypeI32}, 6), []byte{api.ValueTypeI32}
	check := func(data []byte, flavor, rejection string) {
		t.Helper()
		compiled, err := r.CompileModule(ctx, data)
		if err != nil {
			t.Fatal(err)
		}
		defer compiled.Close(ctx)
		if rejection != "" {
			defer func() {
				if failure := recover(); failure == nil || !strings.Contains(fmt.Sprint(failure), rejection) {
					t.Errorf("expected %q rejection, got %v", rejection, failure)
				}
			}()
			auditModule(compiled, policy, flavor)
			return
		}
		audit := auditModule(compiled, policy, flavor)
		if actual := audit["denied_dependency_imports"].([]string); len(actual) != 1 || actual[0] != "wasi_snapshot_preview1.path_readlink" {
			t.Fatalf("missing actual denied import: %v", actual)
		}
	}
	data := dependencyProbeModule(t, params, results)
	for _, flavor := range []string{"math", "full"} {
		check(data, flavor, "")
	}
	check(data, "standard", "forbidden for flavor standard")
	for _, shape := range []struct{ params, results []byte }{
		{params[:5], results},
		{bytes.Repeat([]byte{api.ValueTypeI32}, 7), results},
		{append([]byte{api.ValueTypeI64}, params[1:]...), results},
		{append([]byte{api.ValueTypeF32}, params[1:]...), results},
		{params, []byte{api.ValueTypeI64}},
		{params, nil},
	} {
		check(dependencyProbeModule(t, shape.params, shape.results), "math", "wrong denied dependency import signature")
	}
	check(bytes.Replace(data, []byte("path_readlink"), []byte("path_readlinX"), 1), "math", "forbidden actual import")
	check(bytes.Replace(data, []byte("wasi_snapshot_preview1"), []byte("wasi_snapshot_preview2"), 1), "math", "forbidden actual import")
}

func TestDeniedPathReadlinkReturns76WithoutTouchingMemory(t *testing.T) {
	ctx := context.Background()
	data := dependencyProbeModule(t, bytes.Repeat([]byte{api.ValueTypeI32}, 6), []byte{api.ValueTypeI32})
	h := newHost(ctx, data, dependencyProbePolicy(t), "math")
	defer h.runtime.Close(ctx)
	sentinel := bytes.Repeat([]byte{0xa5}, int(h.module.Memory().Size()))
	if !h.module.Memory().Write(0, sentinel) {
		t.Fatal("cannot initialize synthetic memory")
	}
	for _, args := range [][6]uint32{{3, 16, 8, 128, 16, 256}, {0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff}} {
		if errno := h.call("probe", args[:]...); errno != 76 {
			t.Fatalf("denial returned %d instead of preview1 NOTCAPABLE=76", errno)
		}
		if !bytes.Equal(h.read(0, uint32(len(sentinel))), sentinel) {
			t.Fatal("denied import modified guest memory")
		}
	}
	if h.deniedDependencyCalls["path_readlink"] != 2 {
		t.Fatalf("wrong actual denial count: %v", h.deniedDependencyCalls)
	}
}
