# dugong-graphlib

[![Crates.io](https://img.shields.io/crates/v/dugong-graphlib.svg)](https://crates.io/crates/dugong-graphlib) [![Documentation](https://docs.rs/dugong-graphlib/badge.svg)](https://docs.rs/dugong-graphlib) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](LICENSE-MIT)

Graph container APIs used by `dugong` (port of `@dagrejs/graphlib`).

> **Implementation dependency:** applications should use [`merman`](https://crates.io/crates/merman) for Mermaid rendering. This crate is the Graphlib compatibility layer behind `dugong`, not an independently versioned general graph library.

Its scope is deliberately small: directed and undirected storage, compound parent/child graphs, named multigraph edges, JSON-compatible graph shapes, and the helper algorithms exposed through `dugong_graphlib::alg`.

Node and compound-child enumeration intentionally follows Graphlib's JavaScript object semantics:
canonical array-index string ids enumerate first in numeric order, then ordinary string ids enumerate
by creation order. Edge keys retain Graphlib's insertion-order behavior.

The current upstream baseline is [`@dagrejs/graphlib` v2.2.4](https://github.com/dagrejs/graphlib/tree/v2.2.4), pinned to commit `380d5efa1f4ab0904539f046bdba583d14ac2add`. Repository maintainers track the authoritative checkout in [`tools/upstreams/REPOS.lock.json`](https://github.com/Latias94/merman/blob/main/tools/upstreams/REPOS.lock.json).

Licensed under either Apache-2.0 or MIT at your option. Graphlib-derived material is recorded in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
