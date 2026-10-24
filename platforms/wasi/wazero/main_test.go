package main

import (
	"bytes"
	"encoding/binary"
	"image"
	"image/color"
	"image/png"
	"testing"
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
