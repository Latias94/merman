# ADR-0008: Async and Runtime Neutrality

## Status

Accepted

## Context

`merman` will be used in diverse environments: CLI tools, servers, desktop apps, WebAssembly, and
libraries embedded in other systems. Choosing a specific async runtime (Tokio/async-std/etc.) in
the core crate can create unnecessary integration friction.

Some future functionality may be async (e.g. optional loading of external diagram packs, large
resource loading, or async-friendly APIs for embedding).

## Decision

- `merman-core` must not depend on a specific async runtime.
- Public APIs may be `async` when it improves composability, but must be implementable using only
  `core`/`std` + `futures` traits where needed.
- Provide a sync convenience layer where it improves ergonomics (e.g. a `parse_sync` helper in a
  higher-level crate), but keep `merman-core` focused on the runtime-neutral contract.

## Consequences

- Downstream crates can pick their preferred runtime without adapter glue.
- `merman-core` remains suitable for WASM and embedded contexts.

