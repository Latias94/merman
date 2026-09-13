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
uses its existing numeric return status and `native_status_name` instead. Typst additionally pins
its numeric status and independent error category with `typst_status_code` and `typst_error_kind`.
The numeric status must agree with `code_name`; the current authoring errors use the `generic`
category, not the native status name. Flutter exposes
`native_status_name` as its exception code name.

Consumers include the native C function-table test, UniFFI API and reusable-engine tests,
Typst plugin JSON entry point, installed Node package smoke, installed Python wheel smoke,
real Web/WASM package smoke, and Flutter Native Assets one-shot/reusable-engine smoke. A test implementation or configured CI step is not evidence
that every final package or host has been executed. These vectors do not qualify artifact
profiles, admission states, public catalog cells, or the C7a release candidate.

`support.json` pins complete successful `describe-theme-support-json` responses to the reviewed
support-claim revision. It covers V1/V2 rules, base typography, ordinal palettes, unsupported
routes, ASCII applicability, unqualified browser/native outputs, and unknown identifiers. Compare
all fields, including the echoed query, claim revision, and ordered reason IDs. An Unverified or
Unsupported descriptor is a successful query, not a transport error. Review semantic changes
against the renderer-owned support manifest before updating these independent expectations.
Stale installed artifacts must be rebuilt; do not drop the revision assertion to accept them.
These static upper bounds do not certify actual document admission or qualify output artifacts.

`preset-catalog.json` freezes the complete ordered preset metadata array under the default
binding compiler policy on SVG-capable artifacts and Typst's constrained policy. Shared-core, Native C ABI, and UniFFI tests,
installed Node and Python consumers, Typst plugin smoke, and real Web/WASM smoke compare every field, including availability, appearance,
maturity, empty qualification cells, export kind, license, and attribution. Do not derive this
oracle at test time from production descriptors. Non-SVG artifacts still require an empty array;
custom resource ceilings may change availability and need a separate expectation. Output IDs and
resource limits belong to each artifact profile and are outside this preset-only vector. It does
not qualify final artifacts. Typst additionally checks its own constrained resource ceiling.

`qualified-cells.json` contains nonempty qualification metadata boundary vectors shared by the
Rust wire/projection tests and Web/Node consumers. It varies the profile and admission independently:
an unknown profile paired with `portable` must remain unknown, and a known profile paired with an
unknown admission must retain that admission. Compare every field without rewriting or dropping IDs.
These are synthetic transport inputs, not issued qualification, known-profile registrations, or
production catalog entries. Preserving `portable` in the wire value does not grant usable support
for an unknown profile. These tests establish lossless consumption, not release qualification.
