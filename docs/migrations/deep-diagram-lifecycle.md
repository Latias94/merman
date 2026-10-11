# Deep diagram lifecycle migration

This source change updates Rust ownership and public model types. Mermaid-compatible JSON keeps
its existing field names, nesting, and ordering. Upgrade coupled Merman crates together.

Block's nested compatibility views now observe later style, class, type, and label updates through
the same canonical record, matching pinned Mermaid. The former Rust subtree copies incorrectly
froze these fields when a parent completed. Repeated composite declarations keep their first
children and columns, as Mermaid does. These are source-backed semantic corrections; model
budget counters retain their established accounting rules without exporting the former typed wire.

## Keep semantic JSON managed

`ParsedDiagram.model`, `DiagramParseOutcome.model`, semantic artifact projections, and layout JSON
now use `merman_core::ManagedSemanticJson` instead of an owned `serde_json::Value`. Ordinary clone,
equality, debug formatting, and drop traverse containers iteratively. The owner accepts an existing
value without changing its JSON shape.

```rust
use merman_core::{Engine, ParseOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parsed = Engine::new()
        .parse_diagram_sync("flowchart TD\nA --> B\n", ParseOptions::strict())?
        .expect("a diagram");
    let copy = parsed.model.clone();
    assert_eq!(copy, parsed.model);
    let mut bytes = Vec::new();
    copy.write_json(&mut bytes)?;
    Ok(())
}
```

Use `as_value()` or immutable dereferencing for inspection. Calling `clone`, generic serde,
or another recursive operation on that raw borrow uses `serde_json::Value` behavior.
`into_unmanaged_value()` explicitly transfers responsibility for clone, serialization, and
destruction to the caller. To copy a raw value into managed ownership, use
`ManagedSemanticJson::from(&value)`; this copy is iterative.

Custom registry models use `CustomJsonRenderModel::new(model_name, value)` with either a raw or
managed value.
`value()` borrows the raw payload, `json()` borrows its managed owner, and `into_json()` returns
the owner. Replace the former `into_value()` with `into_unmanaged_value()` only when the raw
ownership transfer is intentional.

## Export deep JSON with the maintained writer

Generic `Serialize` on managed JSON supports at most 128 nested JSON containers. It checks the
actual emitted container shape, including empty arrays and objects, before invoking the serializer.
Deeper values return a serde error. This boundary is independent of Mermaid source admission,
typed hierarchy depth, and model resource policies. `ModelComplexity::from_serializable` treats a
serializer that refuses this boundary as saturated complexity so resource checks reject it without
panicking; call `ModelComplexity::from_json(managed.as_value())` when an exact deep JSON count is
needed.

Use `write_json` for compact output or `write_json_pretty` for serde-compatible indentation at
greater depths. Their controlled counterparts return
`OperationControlResult<serde_json::Result<()>>`: the outer error is cancellation or deadline
expiry, and the inner error is serialization or writer I/O. Both observe the supplied control
during traversal. The caller owns any bytes written before an error.

A cancellation or deadline already observed in the supplied operation scope retains its recorded
phase and takes precedence over a later writer failure. A writer failure before that observation
remains an inner I/O error, including when cancellation has only been requested.

The CLI `parse` command, including `--meta` and `--pretty`, layout output, and the semantic binding
transport use the maintained writer. JSON transport fields and schema versions are unchanged.
Deserialization continues to follow the selected deserializer's own input limits; the managed
owner does not provide an unlimited-depth JSON reader.

## Traverse canonical records

State, Block, Treemap, and Ishikawa retain their hierarchy as flat records with document or child
indices. A record clone no longer duplicates all descendants. Render adapters use these records
directly; nested compatibility JSON is built when requested.

| Model | Rust API migration |
| --- | --- |
| State | `StateDiagramRenderModel.document` owns a `StateDocument`. A state's `doc` is `Option<StateDocumentId>`; use `model.document.get(id)` to borrow its statements. `root()`, `len()`, and `is_empty()` expose document metadata. IDs belong to their owning model. |
| Block | `blocks_flat` stores each logical block once. `BlockNodeRenderModel.children` holds record indices; use `model.block(index)` and `model.root()`. |
| Treemap | `model.root` remains the synthetic root; its `children` and each node's `children` index `model.nodes`. |
| Ishikawa | `model.root` is `Option<usize>` indexing `model.nodes`; node `children` contains indices into the same vector. |

The four owning diagram models no longer implement `Serialize` or `Deserialize`. Their former
typed-wire import and export paths, including State and Block `to_json` and Treemap and Ishikawa
`to_compat_json`, are removed. Build or inspect the canonical records directly. Use an Engine
semantic parse or a semantic artifact's `compatibility_json()` for Mermaid JSON, including its
family-specific metadata and configuration, and keep that projection in managed ownership for
deep export.

State and Block node records do not implement standalone serde because their document or child
indices require an owning model. Standalone Treemap and Ishikawa node serde exposes their canonical
record indices; it does not import or export the former nested typed wire.

Flowchart, ER, C4, Class, and Sequence parser carriers also use flat ownership internally. Their
returned flat public semantic models retain their established shape. Mindmap's typed model remains
flat; its nested `rootNode` JSON is protected by managed ownership during projection.

## Resource and lifecycle boundaries

Existing source/model limits, profile names, explicit overrides, and sticky operation terminals
retain their meanings. Direct Engine parsing and policy-limited facade rendering remain different
entry points. The obsolete renderer-only `typed_model_tree_depth` rejection for Treemap and
Ishikawa is removed because these typed models no longer own recursive trees. Independent SVG
backend capabilities still apply to emitted SVG and native export backends.

Railroad's existing local 256-depth boundary now counts constructed AST nodes, including postfix
wrappers. ZenUML retains its existing accepted 256-level model representation; its semantic and
JSON projection traversals are iterative, and owning-model serde applies the 128-container output
boundary. State document views and legacy Block JSON can repeat descendants; output cost remains
proportional to that requested representation. Flat canonical ownership does not promise that
every parsing, layout, or output operation is linear in source length.

See the [lifecycle evidence](../research/2026-10-10-deep-diagram-lifecycle.md) for isolated host-stack
cases and the [implementation plan](../plans/2026-10-10-2342-refactor-deep-diagram-lifecycle-plan.md)
for the contract and family scope.
