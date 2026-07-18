# ABI descriptors

`merman-v2.json` is the single source of truth for native ABI version 2. Its text-measurement
section owns operation codes, external names, result shapes, and platform enum projections.

Regenerate every committed projection after changing the descriptor:

```sh
cargo run -p xtask -- gen-text-measurement-abi
```

`cargo run -p xtask -- verify-text-measurement-abi` and the umbrella `verify-generated` command
reject descriptor errors and committed projection drift.
