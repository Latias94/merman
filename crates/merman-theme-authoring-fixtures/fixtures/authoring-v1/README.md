# Theme authoring vectors

`light` and `dark` contain the shared definition and materialized-spec oracles.
`errors.json` contains exact UTF-8 input strings for `materialize-theme-json` and
transport-independent error expectations. Send `source` unchanged and use `options_json` when
present; otherwise use default authoring options. The encoded-byte-limit vector explicitly requests
the constrained profile so every transport observes the same resource rejection, including Typst.
Compare the complete optional `resource` details too; an absent expectation requires no resource
error details. This catches profile, limit, phase, and actual/max drift in addition to authoring
codes and paths.

Compare the complete `theme_authoring` object after removing each diagnostic's `message`.
Require every removed message to be a nonempty string. Messages are explanatory text, not
stable machine identifiers. Do not normalize codes, paths, severity, details, array order,
or schema versions. Each transport must also check its outer status against `code_name`. The native C ABI
uses its existing numeric return status and `native_status_name` instead.

Consumers include the native C function-table test, UniFFI API and reusable-engine tests,
Typst plugin JSON entry point, installed Node package smoke, installed Python wheel smoke,
and real Web/WASM package smoke. A test implementation or configured CI step is not evidence
that every final package or host has been executed. These vectors do not qualify artifact
profiles, admission states, public catalog cells, or the C7a release candidate.
