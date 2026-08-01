# dugong

[![Crates.io](https://img.shields.io/crates/v/dugong.svg)](https://crates.io/crates/dugong) [![Documentation](https://docs.rs/dugong/badge.svg)](https://docs.rs/dugong) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](LICENSE-MIT)

Dagre-compatible graph layout algorithms in Rust (port of `dagrejs/dagre`).

> **Implementation dependency:** Mermaid applications should depend on [`merman`](https://crates.io/crates/merman). `dugong` is published to support Merman's release dependency graph; its low-level graph and pipeline APIs are not a separately supported product surface.

`merman-render` uses this crate for diagram families that follow Dagre-style ranking, ordering, positioning, compound graph, and self-edge behavior. The port favors deterministic iteration and source-backed compatibility over a general graph-layout abstraction.

The current upstream baseline is [`@dagrejs/dagre` v2.0.2](https://github.com/dagrejs/dagre/tree/v2.0.2), pinned to commit `ba986662394f8f3ed608717194e5958f3386ce01`. Repository maintainers track the authoritative checkout in [`tools/upstreams/REPOS.lock.json`](https://github.com/Latias94/merman/blob/main/tools/upstreams/REPOS.lock.json).

Benchmarks cover the complete `layout` pipeline and the `network_simplex` and `feasible_tree` ranker internals.

Licensed under either Apache-2.0 or MIT at your option. Dagre-derived material is recorded in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
