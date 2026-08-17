# roughr-merman

[![Crates.io](https://img.shields.io/crates/v/roughr-merman.svg)](https://crates.io/crates/roughr-merman) [![Documentation](https://docs.rs/roughr-merman/badge.svg)](https://docs.rs/roughr-merman) [![License: MIT](https://img.shields.io/badge/license-MIT-yellow.svg)](LICENSE)

`roughr-merman` is Merman's fork of the Rust Rough.js port. It generates the rough drawing operations used by Mermaid-compatible SVG rendering.

> **Implementation dependency:** applications rendering Mermaid should depend on [`merman`](https://crates.io/crates/merman) and enable `svg`. This crate is published so the Merman release graph can resolve, not as a separately supported drawing product or a drop-in replacement for upstream `roughr`.

## Why Merman Maintains A Fork

Mermaid's normal and hand-drawn SVG paths depend on exact Rough.js behavior. Small differences in seed coercion, cloned stroke streams, or default options change generated path operations and make parity unstable across platforms.

The fork therefore treats randomness as an explicit operation-owned contract:

- `RoughJsSeed` preserves the JavaScript `Number` until Rough.js would coerce it through `Math.imul`, including the preceding `seed + 1` used by cloned curve strokes.
- `RoughMathRandom` is a cloneable, caller-owned shared stream for branches where upstream Rough.js reads global `Math.random()`.
- `RoughRandomness` combines both sources, and `OptionsBuilder::build()` rejects options that do not supply it.
- Shape generation never reads ambient host randomness.

This lets one Merman render operation replay Rough.js decisions deterministically without silently changing upstream numeric semantics.

## Version Compatibility

`roughr-merman` follows its own SemVer line because it is a maintained fork of rough-rs whose API and Rough.js parity work do not move in lockstep with Merman releases. Merman selects a compatible roughr minor line through an ordinary Cargo requirement; breaking roughr changes start a new minor line before 1.0.

Version `0.12.3` restores the `OptionsBuilder::seed` and `svgtypes` 0.11 call surface required by Merman 0.7 while retaining the explicit randomness and `svgtypes` 0.16 surface used by current Merman releases. The default `legacy-compat` feature carries the old path type and host-random fallback; current Merman disables default features and therefore keeps the smaller explicit-randomness dependency closure. New direct callers should use `OptionsBuilder::randomness`; the legacy entry points exist so already-published Merman versions remain buildable.

## Scope

The crate produces operation sets for lines, curves, arcs, polygons, ellipses, and SVG paths. It does not choose a canvas, raster backend, or UI framework, and this repository does not ship the old upstream Piet gallery adapter.

APIs and defaults may change when required by the pinned Mermaid and Rough.js behavior. Consumers that intentionally use the low-level generator should follow the exact `roughr-merman` version selected by the matching Merman release.

## Source And License

The implementation originated in [`orhanbalci/rough-rs`](https://github.com/orhanbalci/rough-rs), itself a Rust port of [`rough-stuff/rough`](https://github.com/rough-stuff/rough). Merman's parity changes are maintained in this repository.

Licensed under the MIT License. See [LICENSE](LICENSE).
