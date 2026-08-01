# Parser Generation

Merman checks generated LALRPOP parsers into the repository so downstream builds do not compile
or run the LALRPOP generator. The grammar sources are:

- `crates/merman-core/src/diagrams/class_grammar.lalrpop`
- `crates/merman-core/src/diagrams/er_grammar.lalrpop`
- `crates/merman-core/src/diagrams/flowchart_grammar.lalrpop`
- `crates/merman-core/src/diagrams/sequence_grammar.lalrpop`
- `crates/merman-core/src/diagrams/state_grammar.lalrpop`

Their generated Rust files live under `crates/merman-core/src/generated/lalrpop/`. Treat the
grammar and the complete generated parser set as one change. Never edit generated Rust by hand.

## Regenerate Parsers

After changing any grammar source, regenerate all five parsers from the workspace root:

```console
cargo run -p xtask -- gen-lalrpop-parsers
```

Generation is a whole-set transaction. `xtask` writes the new parsers to a temporary location and
only replaces the checked-in set after every grammar succeeds, so a generator failure does not
leave a partially refreshed tree.

Review both the grammar source and generated diff. Unexpected changes outside the grammar being
edited can indicate a generator-version or toolchain change that should be explained separately.

## Verify Freshness

Verify that checked-in output exactly matches the current grammars and pinned generator:

```console
cargo run -p xtask -- verify-lalrpop-parsers
```

`cargo run -p xtask -- verify-generated` and the strict verification workflow include the same
byte-for-byte freshness check. They validate generated output; they do not inspect documentation
wording or headings.

## Verify Behavior

Fresh output only proves that generation is reproducible. A grammar change must also exercise the
behavioral surface it affects:

1. Run the focused parser tests for the changed diagram family.
2. Run the corresponding analysis and editor tests when token spans, recovery, diagnostics,
   completion, or semantic tokens can change.
3. Run LSP tests when the new parser behavior is exposed through an editor request.
4. Run the workspace and strict gates before release.

Parser generation belongs to the maintainer toolchain in `xtask`; published `merman-core` builds
only depend on `lalrpop-util` and compile the checked-in parser modules.
