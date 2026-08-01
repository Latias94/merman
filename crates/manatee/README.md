# manatee

[![Crates.io](https://img.shields.io/crates/v/manatee.svg)](https://crates.io/crates/manatee) [![Documentation](https://docs.rs/manatee/badge.svg)](https://docs.rs/manatee) [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-59636e.svg)](LICENSE-MIT)

Headless compound graph layout algorithms in Rust.

> **Implementation dependency:** Mermaid applications should enable `layout-cytoscape` on the [`merman`](https://crates.io/crates/merman) facade. Depending on `manatee` directly bypasses Merman's diagram-specific graph conversion and rendering contracts.

The crate contains the COSE-Bilkent and FCoSE algorithms used by Mermaid's Cytoscape-backed layouts. Randomized initialization is always controlled by an explicit seed or `FcoseRandomPolicy`; layout never consults ambient process randomness.

Source baselines include Cytoscape `v3.34.0`, `cytoscape-fcose` `v2.2.0`, `cytoscape-cose-bilkent` `v4.1.0`, and their pinned `layout-base` and `cose-base` dependencies. Exact revisions are authoritative in [`tools/upstreams/REPOS.lock.json`](https://github.com/Latias94/merman/blob/main/tools/upstreams/REPOS.lock.json).

Licensed under either Apache-2.0 or MIT at your option. Ported source attribution is collected in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
