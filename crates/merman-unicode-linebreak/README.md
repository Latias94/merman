# Merman Unicode Line Breaking

A support fork of [`unicode-linebreak` 0.1.5](https://github.com/axelf4/unicode-linebreak),
implementing the default Unicode 15.0 line breaking algorithm under Apache-2.0.

The generated `src/tables.rs`, shared break classes in `src/shared.rs`, and upstream
license are copied without modification. `UPSTREAM_README.md` preserves the original
package documentation.

`UPSTREAM.json` records the exact `v0.1.5` revision
`829adeed3c07ed160c1c721b61806eb902a4c1f0`, the crates.io package archive SHA-256, and
the original published source-file hashes. The package archive hash is
`3b09c83c3c29d37506a3e260c08c03743a6bb66a9cd432c6934ab501a190571f`.
The generated tables are taken from that published archive; they are not stored in
the upstream Git tree.

The only algorithm-adapter change is exposing `LineBreaks<I>` and `linebreaks_iter`
for arbitrary character-index iterators. The original `linebreaks(&str)` API delegates
to the same state machine and remains clonable. `LineBreaks::get_mut` lets callers
inspect an input iterator's cancellation state after advancing it.

An equivalent `Option::is_some_and` expression replaces the upstream `map_or`
expression to satisfy the workspace's pinned Clippy checks.

Merman checks cancellation while supplying characters, so a single iterator advance
cannot scan an unbounded segment before reaching a checkpoint. Interrupted input
produces the ordinary synthetic end-of-text opportunity; callers must inspect their
input's error state and discard that opportunity when cancellation occurs.

The implementation remains `no_std` and has no third-party dependencies. Future
Unicode updates should replace the upstream generated data and algorithm together;
do not independently tailor or reconstruct its state tables.

## License

This fork remains Apache-2.0. The repository's offline third-party component
contract records the copied and modified source. Release legal projections provide
the upstream notice and canonical license under this crate's `THIRD_PARTY_NOTICES.md`
and `THIRD_PARTY_LICENSES/unicode-linebreak/LICENSE`.
