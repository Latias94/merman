package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"os"
	"time"

	wasiexample "merman.local/wasi-example"
)

type fontPaths []string

func (p *fontPaths) String() string         { return fmt.Sprint([]string(*p)) }
func (p *fontPaths) Set(value string) error { *p = append(*p, value); return nil }

func run() (err error) {
	module := flag.String("module", "", "reactor .wasm path")
	source := flag.String("source", "", "Mermaid source file")
	operation := flag.String("operation", "svg", "svg or png")
	options := flag.String("options", "", "request options JSON file (optional)")
	output := flag.String("output", "", "raw output file")
	metadata := flag.String("metadata", "", "metadata JSON output file (optional)")
	catalog := flag.String("catalog", "", "catalog JSON output file (optional)")
	var paths fontPaths
	flag.Var(&paths, "font", "caller font file (repeatable, optional)")
	flag.Parse()
	if *module == "" || *source == "" || *output == "" || flag.NArg() != 0 || (*operation != "svg" && *operation != "png") {
		return errors.New("required: --module FILE --source FILE --operation svg|png --output FILE")
	}
	wasm, err := os.ReadFile(*module)
	if err != nil {
		return err
	}
	src, err := os.ReadFile(*source)
	if err != nil {
		return err
	}
	var opts []byte
	if *options != "" {
		opts, err = os.ReadFile(*options)
		if err != nil {
			return err
		}
	}
	fonts := make([][]byte, 0, len(paths))
	for _, path := range paths {
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		fonts = append(fonts, data)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 60*time.Second)
	defer cancel()
	client, err := wasiexample.New(ctx, wasm, []byte(`{"resources":{"profile":"interactive"}}`), fonts)
	if err != nil {
		return err
	}
	defer func() { err = errors.Join(err, client.Close(context.Background())) }()
	if *catalog != "" {
		result, err := client.Catalog(ctx)
		if err != nil {
			return err
		}
		if err := os.WriteFile(*catalog, result.Data, 0644); err != nil {
			return err
		}
	}
	result, err := client.Execute(ctx, *operation, src, opts)
	if err != nil {
		return err
	}
	if err := os.WriteFile(*output, result.Data, 0644); err != nil {
		return err
	}
	if *metadata != "" {
		if err := os.WriteFile(*metadata, result.Metadata, 0644); err != nil {
			return err
		}
	}
	fmt.Fprintf(os.Stderr, "%s: %d bytes (%s)\n", *output, len(result.Data), result.MediaType)
	return nil
}

func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, "render:", err)
		os.Exit(1)
	}
}
