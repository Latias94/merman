# Workflow Recommendation Framework

Use this as a report skeleton, not a catalog of current products. Fill every selection from the target revision's release facts: `capabilities/feature-surface-v1.json`, `capabilities/artifact-profiles-v1.json`, `platforms/web/web-surface-descriptor.json`, `docs/release/SURFACES.json`, package manifests, and the relevant runtime or admission evidence.

| User need | Selected surface | Exact target selection | Evidence | User-visible trade-off | Migration action |
| --- | --- | --- | --- | --- | --- |
| Static site or documentation rendering | `<crate, package, or CLI>` | `<features or artifact profile>` | `<descriptor, manifest, and artifact evidence>` | `<capabilities, size, and runtime limits>` | `<only if the compared range changed it>` |
| Dynamic browser rendering | `<browser package or declared absence>` | `<package identity and realm model>` | `<Web descriptor and package contract>` | `<worker/realm, missing capabilities, and bundle cost>` | `<old import or package migration>` |
| Editor or LSP integration | `<library, server, or browser package>` | `<direct features, binary profile, or package identity>` | `<manifest and protocol/runtime evidence>` | `<host lifecycle and intentionally omitted outputs>` | `<configuration or protocol migration>` |
| Lint or CI validation | `<library or CLI>` | `<direct features or artifact profile>` | `<manifest and executable evidence>` | `<rendering and host capabilities intentionally omitted>` | `<feature or invocation migration>` |
| Markdown rendering | `<crate, package, or CLI>` | `<features or artifact profile>` | `<manifest and conversion fixture>` | `<output formats, batching cost, and document limits>` | `<pipeline or option migration>` |
| Terminal or ASCII output | `<crate, package, or CLI>` | `<features or package identity>` | `<capability catalog and output fixture>` | `<supported families and fidelity grade>` | `<feature, command, or API migration>` |
| Node or SSR | `<admitted product or declared absence>` | `<target-specific package or fallback workflow>` | `<admission report and target evidence>` | `<unsupported targets or unproven behavior>` | `<only after admission>` |
| Typst | `<package and published mapping>` | `<artifact profile and package version mapping>` | `<package manifest and package smoke>` | `<independent versioning and constrained transport>` | `<version or import migration>` |

## Selection Principles

- Compare the same user-visible capability contract before reporting a size, dependency, or performance change; a product gaining capabilities is not a like-for-like regression.
- Distinguish Cargo features from exact artifact profiles, package identities, and runtime capability catalogs. A feature name alone does not prove shipped contents or omissions.
- Treat release descriptors as declared state and use release-status probes or registry evidence before calling a version published.
- Keep private candidates and artifact-only channels out of present-tense product recommendations until their admission and target evidence are complete.
- Use concrete migration replacements only when the compared range changed them, and derive those replacements from the target revision's user guide and migration evidence.
- Add native SDK or language-binding rows when the compared range changes those surfaces; do not force unaffected products into every report.
