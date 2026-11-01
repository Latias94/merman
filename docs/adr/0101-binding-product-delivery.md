# ADR 0101: Ecosystem-Native Binding Products

- Accepted for the main development line on 2026-11-01.
- Extends the Apple/C/WASI product pilot in ADR-0100.
- Amends the prebuilt host selections in ADR-0097, preserving its source-feature policy.

## Decision

Ordinary prebuilt rendering products include mathematical SVG and both existing layout
backends. Select `math` explicitly in Android, Python, Flutter and Web full/render recipes,
and in both Node transport recipes. Do not change Rust source defaults or convenience
aggregates. Dedicated browser analysis, editor and ASCII products retain their task scope;
Typst keeps its independently qualified plugin product.

Published Web 0.9.2 full/render already includes math. Restoring this expectation on main
is a capability decision, not a claim of equal-capability size reduction. Measure final
same-source products and document the resulting Web budgets. Other hosts need actual final
package consumer evidence before their expanded product can be published.

Use each ecosystem's package manager: SwiftPM, Maven repositories, PyPI wheels, pub.dev
native assets, and npm platform packages. Preserve their transports and public package
identities. Native C/WASI keep their existing full recommendation and reduced recipes.

## Delivery ownership

The artifact descriptors and existing Node recipe owner select compiled capabilities.
Generated contracts and loaded catalogs describe the actual host interface. Package metadata
must not infer support from the workspace version or a Cargo feature in a different product.

Keep matching generated wrappers and binaries in one versioned delivery. A thin SwiftPM
delivery tree and staged Dart target manifest reference frozen native bytes by URL and
digest. Native producer source and the later wrapper/delivery commit are distinct identities;
packaging must not require a self-referential source-commit/checksum cycle.

Android preserves the full Maven publication, including dependency and variant metadata.
Python retains UniFFI/ctypes and platform wheels; architecture and libc coverage are separate
from a Python binding migration. Flutter uses its existing build hook to select native bytes
for the requested target, with explicit local/offline access. Node keeps exact platform
optional dependencies and a separately selected Node-WASM product.

## Consequences

Additional encoders, native font services, system discovery and ambient runtime adapters
remain separate host capabilities. Formula availability does not override constrained
resource policy. Preserve notices for the selected math backend, font payload and ELK.

Installed final-package consumers prove the supported OS, CPU, libc and host-runtime range.
Source builds, candidate artifacts, public publication and registry installation are distinct
states. This decision does not authorize a registry publication or modify v0.9.x artifacts.

See the [implementation plan](../plans/2026-11-01-1248-refactor-binding-delivery-plan.md)
and [binding delivery study](../research/2026-11-01-binding-delivery.md).
