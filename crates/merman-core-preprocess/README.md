# merman-core-preprocess

The source-edit engine used by `merman-core`. It applies ordered UTF-8 edits with cooperative
cancellation and maps ranges in the rewritten text back to the original byte offsets.

It owns the shared token cursor, line-ending scanner, frontmatter and directive fences, and
Langium string and declaration scanners. `merman-core` owns directive config admission, YAML
evaluation, diagnostics, and editor facts.

This crate exposes a workspace-internal service provider interface (SPI). It is published in
lockstep with Merman and expects an exact version match with the Merman crates that use it.

Use `merman-core` for the public parsing API.
