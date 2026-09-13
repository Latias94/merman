# Class Cluster scalar cutover verification

Date: 2026-09-13

Class namespace cluster rectangles now consume typed `Cluster` fill and stroke rules for the unqualified and `Default` static selectors. Solid and transparent paints are covered for classic and handDrawn layouts, including ordinary namespace groups, extracted namespace roots, and the shared ELK writer path.

The terminal receipt binds cluster IDs, independent fill/stroke winning rule indices, and final CSS. Source-owned `clusterBkg`/`clusterBorder` and base/dark derived ownership are covered, as are mixed facet and unsupported ordinal residual cases.

Inventory after this cutover: 40 live legacy routes (32 Block, 8 Class), cutover manifest v83 with 466 routes and 694 witnesses, support claim revision 85 with 14 shared fixture vectors.

Validation:

- effective migration-before red: old production returns `LegacyFamilyThemeCompatibility` for the valid namespace fixture (`/tmp/class-cluster-baseline-valid-red.log`);
- core theme ownership tests: 53/53;
- Class Cluster focused tests: 3/3;
- Class SVG test binary: 85/85 with single-process execution;
- acceptance combination: 452/454; the two ownership tests are order-sensitive under the acceptance wrapper and pass in isolation. This remains a test-isolation follow-up before contract freeze;
- `cargo fmt --all` and `git diff --check`: passed.
