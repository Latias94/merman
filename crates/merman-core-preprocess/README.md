# merman-core-preprocess

The source-edit engine used by `merman-core`. It applies ordered UTF-8 edits with cooperative
cancellation and maps ranges in the rewritten text back to the original byte offsets.

The parser continues to own Mermaid directives, frontmatter, diagnostics, and editor facts.
