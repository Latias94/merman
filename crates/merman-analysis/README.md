# merman-analysis

`merman-analysis` owns the diagnostics-first contract for Merman lint, validation, Markdown
scanning, binding payloads, and future LSP adapters.

The crate intentionally starts below FFI, UniFFI, WASM, CLI, and render wrappers. It provides
stable JSON payload types and source-position mapping helpers before those public surfaces migrate
from coarse validation to `analyze_json`.

See `docs/adr/0070-diagnostics-first-analysis-contract.md` for the accepted architecture decision.

