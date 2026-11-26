# merman-core-preprocess

The source-edit engine used by `merman-core`. It applies ordered UTF-8 edits with cooperative
cancellation and maps ranges in the rewritten text back to the original byte offsets.

It owns the shared token cursor, line-ending scanner, frontmatter and directive fences, and
Langium string and declaration scanners. `merman-core` owns directive config admission, YAML
evaluation, diagnostics, and editor facts.
