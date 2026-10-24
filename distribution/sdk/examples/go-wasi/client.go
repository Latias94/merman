// Package wasiexample is a small serial consumer of merman-wasi ABI 1.
package wasiexample

import (
	"context"
	"encoding/binary"
	"encoding/json"
	"errors"
	"fmt"
	"unicode/utf8"

	"github.com/tetratelabs/wazero"
	"github.com/tetratelabs/wazero/api"
	"github.com/tetratelabs/wazero/imports/wasi_snapshot_preview1"
)

const maxInput = 32 << 20

// OwnedError is the core's ordinary error payload, not an ABI status code.
// Raw preserves the complete JSON, including future optional details.
type OwnedError struct {
	Version      uint32          `json:"version"`
	OK           bool            `json:"ok"`
	Code         int32           `json:"code"`
	CodeName     string          `json:"code_name"`
	Kind         string          `json:"kind"`
	CapabilityID *string         `json:"capability_id"`
	Message      string          `json:"message"`
	Details      json.RawMessage `json:"details,omitempty"`
	Raw          json.RawMessage `json:"-"`
}

func (e *OwnedError) Error() string { return e.CodeName + ": " + e.Message }

// Result owns host-side copies; it remains valid after another call or Close.
// Catalog and named metadata put JSON in Data and leave Metadata empty.
type Result struct {
	MediaType string
	Data      []byte
	Metadata  json.RawMessage
	engine    uint32
}

// Client owns one runtime, reactor and engine. All calls, including Close,
// MUST be sequential; this type is not goroutine-safe. After a guest trap or
// in-flight cancellation, replace the entire Client by calling New again.
type Client struct {
	runtime  wazero.Runtime
	module   api.Module
	engine   uint32
	unusable bool
}

// New initializes the reactor once and copies fonts into its memory-only service.
// Nil options explicitly select resources.profile=interactive. Nonempty options
// are forwarded to core unchanged, including duplicate fields. Caller fonts
// precede bundled fonts; whether bundles exist is an artifact capability.
func New(ctx context.Context, moduleBytes, optionsJSON []byte, fontBytes [][]byte) (client *Client, err error) {
	if err = ctx.Err(); err != nil {
		return nil, err
	}
	type fontRange struct {
		Offset uint32 `json:"offset"`
		Length uint32 `json:"length"`
	}
	ranges := make([]fontRange, 0, len(fontBytes))
	var fonts []byte
	for _, font := range fontBytes {
		if len(font) == 0 || len(font) > maxInput-len(fonts) {
			return nil, errors.New("font inputs must be nonempty and total at most 32 MiB")
		}
		ranges = append(ranges, fontRange{uint32(len(fonts)), uint32(len(font))})
		fonts = append(fonts, font...)
	}
	if len(optionsJSON) == 0 {
		optionsJSON = []byte(`{"resources":{"profile":"interactive"}}`)
	}
	config, err := json.Marshal(struct {
		Version uint32          `json:"version"`
		Options json.RawMessage `json:"options"`
		Fonts   []fontRange     `json:"fonts"`
	}{1, optionsJSON, ranges})
	if err != nil {
		return nil, fmt.Errorf("constructor options: %w", err)
	}
	c := &Client{runtime: wazero.NewRuntimeWithConfig(ctx, wazero.NewRuntimeConfig().
		WithMemoryLimitPages((512<<20)/65536).WithCloseOnContextDone(true))}
	defer func() {
		if err != nil {
			err = errors.Join(err, c.runtime.Close(context.Background()))
		}
	}()
	if _, err = wasi_snapshot_preview1.Instantiate(ctx, c.runtime); err != nil {
		return nil, err
	}
	c.module, err = c.runtime.InstantiateWithConfig(ctx, moduleBytes, wazero.NewModuleConfig().WithStartFunctions())
	if err != nil {
		return nil, fmt.Errorf("instantiate reactor: %w", err)
	}
	if _, err = c.call(ctx, "_initialize"); err != nil {
		return nil, err
	}
	version, err := c.call(ctx, "merman_wasi_abi_version")
	if err != nil {
		return nil, err
	}
	if version != 1 {
		return nil, fmt.Errorf("unsupported WASI ABI %d (want 1)", version)
	}
	if c.module.Memory() == nil {
		return nil, errors.New("reactor has no memory")
	}
	result, err := c.inputs(ctx, [][]byte{config, fonts}, func(ptr []uint32) (Result, error) {
		handle, err := c.call(ctx, "merman_wasi_engine_new", uint64(ptr[0]), uint64(len(config)), uint64(ptr[1]), uint64(len(fonts)))
		if err != nil {
			return Result{}, err
		}
		return c.result(ctx, handle)
	})
	if err != nil {
		return nil, err
	}
	if result.engine == 0 {
		return nil, errors.New("constructor returned no engine")
	}
	c.engine = result.engine
	return c, nil
}

// Catalog returns the runtime catalog as raw JSON Data.
func (c *Client) Catalog(ctx context.Context) (Result, error) {
	if err := c.ready(ctx); err != nil {
		return Result{}, err
	}
	handle, err := c.call(ctx, "merman_wasi_runtime_catalog")
	if err != nil {
		return Result{}, err
	}
	return c.result(ctx, handle)
}

// Execute passes operation IDs (including unknown IDs) and options to core.
// Source and returned Data are raw bytes, never base64. An *OwnedError permits
// another request; runtime call errors make this Client unusable.
func (c *Client) Execute(ctx context.Context, operation string, source, optionsJSON []byte) (Result, error) {
	if err := c.ready(ctx); err != nil {
		return Result{}, err
	}
	request, err := json.Marshal(struct {
		Version   uint32          `json:"version"`
		Operation string          `json:"operation_id"`
		Options   json.RawMessage `json:"options,omitempty"`
	}{1, operation, optionsJSON})
	if err != nil {
		return Result{}, fmt.Errorf("request options: %w", err)
	}
	return c.inputs(ctx, [][]byte{request, source}, func(ptr []uint32) (Result, error) {
		handle, err := c.call(ctx, "merman_wasi_execute", uint64(c.engine), uint64(ptr[0]), uint64(len(request)), uint64(ptr[1]), uint64(len(source)))
		if err != nil {
			return Result{}, err
		}
		return c.result(ctx, handle)
	})
}

func (c *Client) ready(ctx context.Context) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if c.unusable {
		return errors.New("client closed or unusable; create a replacement with New")
	}
	return nil
}

func (c *Client) call(ctx context.Context, name string, args ...uint64) (uint32, error) {
	if err := c.ready(ctx); err != nil {
		return 0, err
	}
	fn := c.module.ExportedFunction(name)
	if fn == nil {
		return 0, fmt.Errorf("missing ABI export %s", name)
	}
	values, err := fn.Call(ctx, args...)
	if err != nil {
		c.unusable = true
		return 0, errors.Join(fmt.Errorf("%s: instance unusable; replace Client: %w", name, err), ctx.Err(), c.runtime.Close(context.Background()))
	}
	if name == "_initialize" {
		return 0, nil
	}
	if len(values) != 1 {
		return 0, fmt.Errorf("%s: expected one scalar result", name)
	}
	return uint32(values[0]), nil
}

func (c *Client) release(name string, handle uint32) error {
	if c.unusable || handle == 0 {
		return nil
	}
	// Cleanup uses a live context even if the request expired between guest calls.
	status, err := c.call(context.Background(), name, uint64(handle))
	if err != nil {
		return err
	}
	if status != 0 {
		return fmt.Errorf("%s: release status %d", name, int32(status))
	}
	return nil
}

func (c *Client) inputs(ctx context.Context, data [][]byte, run func([]uint32) (Result, error)) (result Result, err error) {
	ptr := make([]uint32, len(data))
	defer func() {
		for i := len(ptr) - 1; i >= 0; i-- {
			err = errors.Join(err, c.release("merman_wasi_free", ptr[i]))
		}
	}()
	for i, bytes := range data {
		if len(bytes) == 0 {
			continue
		}
		if len(bytes) > maxInput {
			return Result{}, errors.New("input exceeds 32 MiB ABI limit")
		}
		ptr[i], err = c.call(ctx, "merman_wasi_alloc", uint64(len(bytes)))
		if err != nil {
			return Result{}, err
		}
		if ptr[i] == 0 {
			return Result{}, errors.New("guest input allocation failed")
		}
		if !c.module.Memory().Write(ptr[i], bytes) {
			return Result{}, errors.New("input write out of bounds")
		}
	}
	return run(ptr)
}

func (c *Client) segment(ptr, length uint32) ([]byte, error) {
	if (ptr == 0) != (length == 0) {
		return nil, errors.New("noncanonical result segment")
	}
	if length == 0 {
		return nil, nil
	}
	if length > maxInput || uint64(ptr)+uint64(length) > 1<<32 {
		return nil, errors.New("result segment exceeds ABI bounds")
	}
	data, ok := c.module.Memory().Read(ptr, length)
	if !ok {
		return nil, errors.New("result segment outside guest memory")
	}
	return append([]byte(nil), data...), nil
}

func (c *Client) result(ctx context.Context, handle uint32) (result Result, err error) {
	if handle == 0 {
		return result, errors.New("guest could not publish an owned result")
	}
	defer func() { err = errors.Join(err, c.release("merman_wasi_result_free", handle)) }()
	ptr, err := c.call(ctx, "merman_wasi_result_info", uint64(handle))
	if err != nil {
		return result, err
	}
	if ptr == 0 || uint64(ptr)+40 > 1<<32 {
		return result, errors.New("invalid result descriptor offset")
	}
	descriptor, ok := c.module.Memory().Read(ptr, 40)
	if !ok {
		return result, errors.New("40-byte descriptor outside guest memory")
	}
	var fields [10]uint32
	for i := range fields {
		fields[i] = binary.LittleEndian.Uint32(descriptor[4*i : 4*i+4])
	}
	if fields[0] != 1 || fields[1] != 40 || fields[2] > 1 {
		return result, errors.New("unsupported result descriptor")
	}
	media, err := c.segment(fields[4], fields[5])
	if err != nil {
		return result, err
	}
	result.Data, err = c.segment(fields[6], fields[7])
	if err != nil {
		return result, err
	}
	result.Metadata, err = c.segment(fields[8], fields[9])
	if err != nil {
		return result, err
	}
	if !utf8.Valid(media) || !utf8.Valid(result.Metadata) {
		return result, errors.New("result media or JSON is not UTF-8")
	}
	result.MediaType, result.engine = string(media), fields[3]
	if fields[2] == 1 {
		if result.engine != 0 || len(result.Data) != 0 || result.MediaType != "application/json" {
			return result, errors.New("invalid owned-error descriptor")
		}
		payload := &OwnedError{Raw: append(json.RawMessage(nil), result.Metadata...)}
		if err := json.Unmarshal(result.Metadata, payload); err != nil {
			return result, fmt.Errorf("decode owned error: %w", err)
		}
		return result, payload
	}
	return result, nil
}

// Close releases the engine then the runtime. It is idempotent and serial.
// Cleanup is deliberately independent of an expired caller context. After a
// trap no guest release functions are invoked; closing the runtime reclaims all.
func (c *Client) Close(ctx context.Context) error {
	if c.unusable {
		return c.runtime.Close(context.Background())
	}
	err := c.release("merman_wasi_engine_close", c.engine)
	c.unusable = true
	return errors.Join(err, c.runtime.Close(context.Background()))
}
