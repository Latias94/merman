# Machine descriptors

`text-measurement-v1.json` owns the host text-measurement protocol only: its operation codes,
external names, result shapes, and platform enum projections. It is intentionally independent of
the native ABI version so a native transport redesign cannot silently change callback semantics.

Regenerate every committed text-measurement projection after changing the descriptor:

```sh
cargo run -p xtask -- gen-text-measurement-protocol
```

`cargo run -p xtask -- verify-text-measurement-protocol` and the umbrella `verify-generated`
command reject descriptor errors and committed projection drift.

Native ABI record layouts, function-table entries, and ownership rules belong in
`merman-v3.json`; they are not duplicated in the text-measurement protocol descriptor.

`merman-v3.json` is the native ABI 3 authority. It owns native numeric status/output codes, the
explicit native-code-to-binding-output mapping, function-table slots, record field order,
ownership rules, and each format's `requires_uri` contract. Capability semantic IDs remain owned by
`capabilities/feature-surface-v1.json`; generation validates every non-null native capability
reference against that authority.

Regenerate the native C header, Rust ABI definitions, and bindings-core operation projection with:

```sh
cargo run -p xtask -- gen-native-abi
```

`cargo run -p xtask -- verify-native-abi` and `verify-generated` reject descriptor or projection
drift. The generated ABI layout descriptor digest identifies the declared wire contract; hosts
validate generated record sizes and field offsets through their surface-owned compile-run tests.
The ABI table's `runtime_catalog` result is the flat bindings-core catalog. Native hosts must
validate its capability, output, operation, system-adapter, text-measurement, and resource
relations before accepting the compiled contract; this avoids maintaining a second
language-specific list of capability IDs.

Native result schema `1` owns the closed failure kinds `generic`, `unknown-operation`,
`missing-capability`, and `reentrant-call`. Every failure JSON includes a nullable `capability_id`;
it is non-null only for `missing-capability` and names the capability directly from the descriptor
vocabulary.
Result-buffer ownership is registered against the exact `MermanNativeResult` address written by
Merman; copying or moving a live result does not transfer ownership to the new record address.
