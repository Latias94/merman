# Merman Render Assets

This directory contains renderer support files whose loading and provenance are owned by `merman-render`.

- `zenuml/*.svg` contains source-backed ZenUML symbols embedded into the Rust library with `include_str!`; [`zenuml/README.md`](zenuml/README.md) records their exact upstream version, hashes, and license.

New assets must state whether they are loaded at runtime or embedded at compile time, remain within this crate's ownership boundary, and carry the required upstream provenance and license material.
