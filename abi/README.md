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
explicit native-code-to-binding-output mapping, function-table slots, record field order and field
ownership, opaque scalar domains, caller-memory preconditions, append points, and each format's
`requires_uri` contract. Capability semantic IDs remain owned by
`capabilities/feature-surface-v1.json`; generation validates every non-null native capability
reference against that authority.

Regenerate the native C header and Rust ABI definitions with:

```sh
cargo run -p xtask -- gen-native-abi
```

`cargo run -p xtask -- verify-native-abi` and `verify-generated` reject descriptor or projection
drift. The generated minimum-prefix layout digest is ABI 3's frozen compatibility key. The
published six-slot layout digest remains frozen separately. The full descriptor digest identifies
complete producer provenance, while the capability catalog digest identifies the loaded artifact.
Hosts validate generated record sizes and field offsets through their surface-owned compile-run
tests. The ABI table's `runtime_catalog` result is the flat bindings-core catalog. Native hosts
must validate its capability, output, operation, system-adapter, text-measurement, and resource
relations before accepting the compiled contract; this avoids maintaining a second
language-specific list of capability IDs.

ABI 3 also has two readable semantic projections:

- `merman-v3-published-six.semantic.json` is the immutable projection for the public six-slot
  surface anchored to baseline commit `5117c0ae12da2c0346b47061642286174cea3f5f`. It also freezes
  the reviewed ABI 3 semantic hardening introduced while establishing this verifier; the separate
  six-slot header fixture remains byte-identical to the baseline. Generation never rewrites the
  projection.
- `merman-v3-current-full.semantic.json` is the reviewed current-full ABI 3 projection. It includes
  resolved operation semantics, call signatures, record field ownership, opaque token domains,
  caller-memory rules, and ownership/lifecycle rules.

The verifier compares stable keys and complete frozen entries before checking generated-file
freshness. Existing entries cannot be deleted, reordered, or changed. Status codes, error kinds,
callbacks, opaque scalars, and existing record fields are closed within ABI 3. Operations may be
appended only after the declared operation append point; new records and function slots may be
appended only at their descriptor-owned append points.

For a reviewed valid append, run `gen-native-abi`. Generation first verifies the old current-full
snapshot against its separately compiled digest and rejects mutation of every existing entry; it
then writes the candidate current-full projection. `verify-native-abi` intentionally remains red
and reports the candidate digest until a maintainer reviews the projection diff and explicitly
updates the frozen digest constant. Rewriting either JSON snapshot alone can therefore never
authorize a semantic change.

Native result schema `1` owns the closed failure kinds `generic`, `unknown-operation`,
`missing-capability`, `reentrant-call`, and `busy`. Every failure JSON includes a nullable
`capability_id`; it is non-null only for `missing-capability` and names the capability directly
from the descriptor vocabulary.

Engine and result-allocation tokens retain their published `uint64_t` representation but use
disjoint low-bit domains over sign-bit-preserving monotonic counters. They prevent accidental
cross-kind use and are not authorization boundaries. Result-buffer ownership is identified only
by a live result-domain `allocation_token`. Moving a complete result transfers ownership when the
source is cleared; `result_free` never trusts nested pointers or the result record address.
