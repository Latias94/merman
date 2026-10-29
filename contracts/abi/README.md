# ABI Descriptors

This directory owns native ABI and text-measurement descriptions consumed by generators
and SDK packaging.
Transport versions are independent of each other and of the Merman workspace version.

| Descriptor | Authority | Direct consumers |
| --- | --- | --- |
| [Native ABI 3](merman-v3.json) | C statuses and operation codes, function slots, record layouts, discovery prefix, caller-memory and ownership rules. | [The native ABI generator](../../crates/xtask/src/cmd/native_abi.rs) emits Rust definitions, the C header, and Dart operation values. |
| [Text measurement v1](text-measurement-v1.json) | Callback operation codes, external names, result kinds, and platform vocabularies, independent of the enclosing transport's record layout. | [The text-measurement generator](../../crates/xtask/src/cmd/text_measurement/abi.rs) emits Rust, C, Kotlin, Dart, TypeScript, and Python projections. |

Regenerate the native ABI or text-measurement projections after editing their descriptor:

```sh
cargo run -p xtask -- gen-native-abi
cargo run -p xtask -- verify-native-abi
cargo run -p xtask -- gen-text-measurement-protocol
cargo run -p xtask -- verify-text-measurement-protocol
```

`verify-generated` includes both projection checks.
[SDK assembly](../../scripts/sdk_archive.py) also packages the native ABI and text-measurement
descriptors with the matching native headers.

WASI ABI 1 is documented by the [WASI crate guide](../../crates/merman-wasi/README.md),
packaged as `docs/WASI_ABI.md` with the actual module and hash-bound runtime catalog.
Its wire layout, envelopes and quotas are implemented by the crate and exercised by
[runtime tests](../../crates/merman-wasi/src/tests.rs) and
[real-host qualification](../../platforms/wasi/README.md). The redundant hand-maintained
WASI JSON mirror has been removed; this changes archive contents only, with no wire,
ABI version or runtime behavior change.

The [C protocol guide](../../docs/bindings/FFI_PROTOCOL.md) owns host discovery, memory, token,
callback, and error guidance. The [host measurement guide](../../docs/bindings/HOST_TEXT_MEASUREMENT.md)
owns display-stack integration; the [WASI crate guide](../../crates/merman-wasi/README.md) owns WASI
host usage. Keep those explanations with their transport owners rather than duplicating them here.
Capability semantic IDs and artifact recipes remain owned by [the capability descriptors](../../capabilities/README.md).
