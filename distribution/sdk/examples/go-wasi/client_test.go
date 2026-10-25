package wasiexample

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"image/color"
	"image/png"
	"math"
	"os"
	"testing"
	"time"

	"github.com/tetratelabs/wazero"
)

const testSource = "---\nconfig:\n  fontFamily: Cousine\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA[Memory fonts] --> B[Retained bytes]\n"

func TestInFlightCancellationDiscardsRuntime(t *testing.T) {
	ctx := context.Background()
	runtime := wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig().WithCloseOnContextDone(true))
	t.Cleanup(func() { _ = runtime.Close(ctx) })
	// A tiny real Wasm loop tests cancellation without depending on render duration.
	module, err := runtime.Instantiate(ctx, []byte{
		0, 97, 115, 109, 1, 0, 0, 0,
		1, 5, 1, 96, 0, 1, 127,
		3, 2, 1, 0,
		7, 8, 1, 4, 115, 112, 105, 110, 0, 0,
		10, 10, 1, 8, 0, 3, 64, 12, 0, 11, 0, 11,
	})
	if err != nil {
		t.Fatal(err)
	}
	client := &Client{runtime: runtime, module: module}
	limited, cancel := context.WithTimeout(ctx, 20*time.Millisecond)
	defer cancel()
	if _, err := client.call(limited, "spin"); !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("in-flight deadline: %v", err)
	}
	if !client.unusable || !module.IsClosed() {
		t.Fatal("interrupted instance was retained")
	}
	if _, err := client.Execute(ctx, "svg", []byte(testSource), nil); err == nil {
		t.Fatal("interrupted client admitted another operation")
	}
	if err := client.Close(ctx); err != nil {
		t.Fatal(err)
	}
}

func testClient(t *testing.T, fonts [][]byte) (*Client, context.Context) {
	t.Helper()
	path := os.Getenv("MERMAN_WASI_MODULE")
	if path == "" {
		t.Skip("set MERMAN_WASI_MODULE to an actual ABI 1 reactor")
	}
	module, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 90*time.Second)
	t.Cleanup(cancel)
	client, err := New(ctx, module, nil, fonts)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := client.Close(context.Background()); err != nil {
			t.Error(err)
		}
	})
	return client, ctx
}

func TestSVGPNG(t *testing.T) {
	client, ctx := testClient(t, nil)
	catalog, err := client.Catalog(ctx)
	if err != nil || !json.Valid(catalog.Data) || len(catalog.Metadata) != 0 {
		t.Fatalf("catalog: %v", err)
	}
	svg, err := client.Execute(ctx, "svg", []byte(testSource), []byte(`{"svg":{"pipeline":"resvg-safe","diagram_id":"go-example"}}`))
	if err != nil {
		t.Fatal(err)
	}
	if svg.MediaType != "image/svg+xml" || !bytes.Contains(svg.Data, []byte("<svg")) || !bytes.Contains(svg.Data, []byte("Memory fonts")) {
		t.Fatal("invalid SVG output")
	}
	saved := append([]byte(nil), svg.Data...)
	var svgMeta struct {
		Viewport struct{ Width, Height float64 } `json:"svg_viewport"`
	}
	if err := json.Unmarshal(svg.Metadata, &svgMeta); err != nil {
		t.Fatal(err)
	}
	raster, err := client.Execute(ctx, "png", []byte(testSource), []byte(`{"svg":{"pipeline":"resvg-safe","diagram_id":"go-example","root_background_color":"transparent"},"raster":{"scale":2,"matte":"#ffffff"}}`))
	if err != nil {
		t.Fatal(err)
	}
	image, err := png.Decode(bytes.NewReader(raster.Data))
	if err != nil {
		t.Fatal(err)
	}
	var meta struct {
		Plan struct {
			Width     int     `json:"width_px"`
			Height    int     `json:"height_px"`
			Requested float64 `json:"requested_scale"`
			Effective float64 `json:"effective_scale"`
		} `json:"output_plan"`
	}
	if err := json.Unmarshal(raster.Metadata, &meta); err != nil {
		t.Fatal(err)
	}
	bounds := image.Bounds()
	if raster.MediaType != "image/png" || meta.Plan.Requested != 2 || meta.Plan.Effective != 2 || meta.Plan.Width != bounds.Dx() || meta.Plan.Height != bounds.Dy() {
		t.Fatal("PNG plan or scale mismatch")
	}
	if svgMeta.Viewport.Width <= 0 || svgMeta.Viewport.Height <= 0 || bounds.Dx() != int(math.Ceil(svgMeta.Viewport.Width))*2 || bounds.Dy() != int(math.Ceil(svgMeta.Viewport.Height))*2 {
		t.Fatal("SVG viewport / PNG dimensions mismatch")
	}
	if got := color.NRGBAModel.Convert(image.At(bounds.Min.X, bounds.Min.Y)).(color.NRGBA); got != (color.NRGBA{255, 255, 255, 255}) {
		t.Fatalf("white matte corner: %v", got)
	}
	if !bytes.Equal(svg.Data, saved) {
		t.Fatal("later render mutated retained output")
	}
}

func TestParseErrorRecovery(t *testing.T) {
	client, ctx := testClient(t, nil)
	_, err := client.Execute(ctx, "svg", []byte("flowchart ???"), nil)
	var owned *OwnedError
	if !errors.As(err, &owned) || owned.Version != 1 || owned.OK || owned.CodeName != "MERMAN_PARSE_ERROR" || owned.Message == "" || !json.Valid(owned.Raw) {
		t.Fatalf("expected typed parse error: %v", err)
	}
	if _, err := client.Execute(ctx, "svg", []byte(testSource), nil); err != nil {
		t.Fatalf("ordinary error poisoned engine: %v", err)
	}
}

func TestCallerFont(t *testing.T) {
	if os.Getenv("MERMAN_WASI_MODULE") == "" {
		t.Skip("set MERMAN_WASI_MODULE to an actual ABI 1 reactor")
	}
	path := os.Getenv("MERMAN_WASI_FONT")
	if path == "" {
		t.Skip("set MERMAN_WASI_FONT to a font file covering the fixture")
	}
	font, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	client, ctx := testClient(t, [][]byte{font})
	if _, err := client.Execute(ctx, "svg", []byte(testSource), nil); err != nil {
		t.Fatal(err)
	}
}

func TestCanceledCallDoesNotPoisonUntouchedInstance(t *testing.T) {
	client, ctx := testClient(t, nil)
	canceled, cancel := context.WithCancel(ctx)
	cancel()
	if _, err := client.Execute(canceled, "svg", []byte(testSource), nil); !errors.Is(err, context.Canceled) {
		t.Fatalf("canceled Execute: %v", err)
	}
	if _, err := client.Catalog(canceled); !errors.Is(err, context.Canceled) {
		t.Fatalf("canceled Catalog: %v", err)
	}
	if _, err := client.Execute(ctx, "svg", []byte(testSource), nil); err != nil {
		t.Fatalf("untouched instance poisoned: %v", err)
	}
}
