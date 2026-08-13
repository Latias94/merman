# ADR-0008: Async and Runtime Neutrality

## Status

Accepted

## Context

`merman` will be used in diverse environments: CLI tools, servers, desktop apps, WebAssembly, and
libraries embedded in other systems. Choosing a specific async runtime (Tokio/async-std/etc.) in
the core crate can create unnecessary integration friction.

Some future functionality may be async (e.g. optional loading of external diagram packs, large
resource loading, or async-friendly APIs for embedding).

Rendering itself is deliberately synchronous and cooperative. The canonical `Renderer` facade
does not expose CPU-bound `async fn` wrappers that merely move the same blocking work into a
future. Hosts that need scheduling should place the synchronous operation on their own blocking
worker or process boundary and retain the operation's `OperationControl` handle. Cancellation
and deadlines are observed at parser, layout, emission, postprocessing, and export checkpoints;
they cannot interrupt an opaque third-party call already in progress. Hard interruption requires
worker/process isolation.

## Decision

- `merman-core` must not depend on a specific async runtime.
- Public APIs may be `async` when it improves composability, but must be implementable using only
  `core`/`std` + `futures` traits where needed.
- Provide a sync convenience layer where it improves ergonomics (e.g. a `parse_sync` helper in a
  higher-level crate), but keep `merman-core` focused on the runtime-neutral contract.

## Consequences

- Downstream crates can pick their preferred runtime without adapter glue.
- `merman-core` remains suitable for WASM and embedded contexts.
