# Private XML DOM owner

This directory imports roxmltree's read-only XML DOM into the renderer's private
`crate::xml_dom` module. It is not a public crate, feature, or type boundary.

## Source and reason

The base is the published `roxmltree 0.21.1` package, with the official
[`3f5c78dea57662b43a2f41961397b1f5ffe27166` fix](https://github.com/RazrFalcon/roxmltree/commit/3f5c78dea57662b43a2f41961397b1f5ffe27166)
("Parse element content iteratively", 2026-09-22). The files were imported from
that exact upstream commit and compared with the cached crates.io archive on
2026-10-14. The archive SHA-256 matches its Cargo.lock checksum:
`f1964b10c76125c36f8afe190065a4bf9a87bf324842c05701330bba9f1cacbb`.

At import time the fix was unreleased. Registry 0.21.1 recurses once per XML
element level and can abort on accepted deep XHTML labels before the renderer's
existing resource checks. The upstream tokenizer fix replaces that recurrence
with a loop and a checked `u32` depth counter (24 additions, 12 deletions).
Namespace handling, DOM storage, normalization, source ranges, errors, and the
existing bounded entity-reference recursion remain upstream implementations.
No additional markup-depth restriction or larger native stack is introduced.

The fixed snapshot's `lib.rs`, tokenizer tests, and license files are byte-for-byte
identical to the published package. `parse.rs` additionally contains the fix's
`NodesLimitReached` documentation and an upstream entity-resolver spelling fix.
The upstream API tests add the exact 200,000-element regression.

This is copied source so a published renderer contains the repaired parser.
Cargo git/path overrides with a registry version would select the old registry
parser when the renderer is published.

## Original file hashes

These SHA-256 values describe original bytes before module adaptation. Source
URLs are relative to
[`https://raw.githubusercontent.com/RazrFalcon/roxmltree/3f5c78dea57662b43a2f41961397b1f5ffe27166/`](https://raw.githubusercontent.com/RazrFalcon/roxmltree/3f5c78dea57662b43a2f41961397b1f5ffe27166/).

| Upstream path | Local path | SHA-256 |
| --- | --- | --- |
| `src/lib.rs` | `mod.rs` | `ef6299f199093167f7479e6203821a3aa71ad3da60712b74e1c1debeaeb131e7` |
| `src/parse.rs` | `parse.rs` | `0f9b0c4fddec2d72347747d947ac25d641fdbfb8ce5e048cd69a67116eee3568` |
| `src/tokenizer.rs` | `tokenizer.rs` | `5ebac91a6ae3e3b37eb7abc31b218cf089202901512bb3c62700f56cd2fda785` |
| `src/tokenizer_tests.rs` | `tokenizer_tests.rs` | `db2910eb80cd7e48e35d1d6e29af8f7d3ec2240bf40025544e20d832adbd6e0a` |
| `tests/api.rs` | `api_tests.rs` | `e3c610b9c7e867dc048bc6090def994a5a90947f7f5d61c78b1c26c1f3374c39` |
| `LICENSE-MIT` | `LICENSE-MIT` | `fffe218c1e349e1d482a05677f2ce850502f88298174d77f6ac7c04821db7899` |
| `LICENSE-APACHE` | `LICENSE-APACHE` | `a60eea817514531668d7e00765731449fe14d059d3249e0bc93b36de45f759f2` |

## Local adaptations

- `lib.rs` becomes `mod.rs`. Crate-relative imports become module-relative
  imports, including the existing `alloc` namespace. The module retains the
  upstream unsafe-code prohibition and documentation lints.
- Five borrowed error-match arms omit their redundant `ref` binding modifiers
  for Rust 2024 compatibility. Match ergonomics retain the same borrowed field
  types, formatting, and behavior as upstream's Rust 2021 source.
- Upstream default `std` and `positions` behavior is always enabled, matching the
  renderer's former dependency. Their positive conditional attributes and
  disabled branches are removed. The module-level `no_std` declaration and
  unnecessary `extern crate std` declaration are removed. No renderer feature is
  added. `memchr 2` keeps the original searches and standard-library/SIMD support.
- Upstream API and tokenizer tests become private child modules. Their imports
  become module-relative, position tests always run, and the tokenizer's test
  macro stays local instead of being exported from the renderer.
- `tests.rs` compares ordinary namespace, attribute, text/CDATA/entity,
  source-position, error, DTD-default, and resource-limit behavior with the
  published parser. The renderer's admitted deep-Class subprocess regression
  separately exercises the production caller on its existing 2 MiB thread.
- The parent module declaration skips rustfmt for upstream files and permits
  unused upstream API items with a local `dead_code` reason. The complete upstream
  API is retained to keep source comparison and retirement straightforward;
  only the stylistic `collapsible_if` and `filter_next` Clippy rules are also
  locally permitted to retain upstream spelling. Production correctness lints
  remain enabled. Local `tests.rs` is formatted directly with rustfmt.

## Licenses

roxmltree is licensed under `MIT OR Apache-2.0`; both original license files are
retained here. `LICENSE-MIT` contains Copyright (c) 2018 Yevhenii Reizner.

The original module header identifies the tree structure as derived from
ego-tree under ISC. That header is retained. `LICENSE-ISC` is copied from the
[official ego-tree v0.6.3 license](https://raw.githubusercontent.com/rust-scraper/ego-tree/v0.6.3/LICENSE)
(Copyright © 2016, June McEnroe), with SHA-256
`c6a4ae2a0565d17ac7fccae808275596fc5959ffdf507107d001bc4669189e45`.
It supplements the upstream ancestry notice; it is not represented as text
extracted from roxmltree's module header.

The directory lives under `src/`, outside the renderer's excluded `tests/**`
directory, so its implementation, licenses, and provenance ship with the crate.
Project third-party license metadata also records the copied/modified component.

## Retirement

Replace this private import with a published roxmltree release that includes the
official iterative-content fix once its API, positions, normalization, error,
deep-label, and package tests pass. Move private callers together, retain the
regressions against the released parser, and remove the copied-source legal
entries with this directory. Do not switch to an unfixed registry fallback or
maintain an independent XML parser in this module.
