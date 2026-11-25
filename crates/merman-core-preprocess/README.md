# merman-core-preprocess

The source-edit engine used by `merman-core`. It applies ordered UTF-8 edits with cooperative
cancellation and maps ranges in the rewritten text back to the original byte offsets.

Family parsers share its controlled token cursor, line-ending scanner, frontmatter fences, and
Langium string and declaration scanners.
`merman-core` owns directive and YAML evaluation, diagnostics, and editor facts.
