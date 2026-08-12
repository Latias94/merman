# Contracts

This directory owns machine-readable contracts that are projected into multiple runtime and tool
surfaces.

- `abi/` owns the native ABI and host text-measurement protocol descriptors.
- `editor-language/` owns editor token declarations and the checked token-equivalence evidence.

Edit the descriptors here and use their documented `xtask` generators. Generated Rust, C,
TypeScript, Kotlin, Dart, Python, and editor files remain in the surface that consumes them.
