# merman-core-preprocess

The source-edit engine used by `merman-core`. It applies ordered UTF-8 edits with cooperative
cancellation and maps ranges in the rewritten text back to the original byte offsets.

It owns the shared token cursor, line-ending scanner, Langium string and declaration scanners,
and source decoding for frontmatter and directives. The `yaml` feature admits bounded YAML
materialization and frontmatter fields. The `directive-config` feature decodes bounded JSON5
arguments. `yaml-editor` enables local key ranges and authored order for the selected decoders.
`merman-core` handles detector-dependent configuration, value admission, and remapping
diagnostics and editor facts to the original source.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman-core` for the public parsing API.
