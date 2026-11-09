# Complete SVG binding budget, 2026-11-01

The main binding-delivery change explicitly includes math in `web-full` and `web-render`.
This restores the complete SVG product expectation of published Web 0.9.2. It is a capability
change relative to the intervening math-free main recipes, not an equal-capability size saving.
Analysis, editor and ASCII retain their separate task scopes.

Measurements use the final assembled npm packages from the implementation based on main
`939ccec9f8d1391ae672a5397d5b57d10a1307b8`, Rust 1.95.0, wasm-pack 0.15.0 and
wasm-tools 1.253.0 on macOS ARM64. The existing package producer selects the descriptor-owned
`wasm-size` Cargo profile. The size owner strips final WASM with `wasm-tools strip --all`
and applies its existing gzip and Brotli settings; no alternate Cargo-only artifact is measured.

## Final artifact sizes

| Artifact | Raw bytes | Stripped bytes | Gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: | ---: |
| `web-analysis` | 4,058,595 | 4,058,330 | 1,550,929 | 1,180,298 |
| `web-ascii` | 5,750,323 | 5,750,058 | 2,120,693 | 1,600,529 |
| `web-editor` | 4,217,499 | 4,217,234 | 1,613,238 | 1,225,700 |
| `web-full` | 17,200,560 | 17,200,295 | 6,359,194 | 4,678,479 |
| `web-render` | 15,173,229 | 15,172,964 | 5,663,850 | 4,172,049 |

The full/render raw WASM SHA-256 values are respectively
`7482040107513ab84e26d57a97daea01c1af2e9d76e01ed8bceb46eb9e139c1b` and
`0cda808d218ebee0e60bc497ec5edcf22c541a871242598963be11fdd858955c`.

Only the full/render ceilings in `docs/release/WASM_SIZE_BUDGETS.json` change. Each replacement
uses its measured value plus 3%, rounded upward to 1,000 bytes, following the existing policy.
The other three Web budgets and the Typst budget remain unchanged. Render saves approximately
11.8% stripped and 10.8% Brotli against full on this same build; it remains the dedicated
complete SVG product rather than a new basic-SVG SKU. These values do not isolate the cost of
math from every change since the historical budget baseline.

## Installed browser observations

The installed-consumer owner packs and installs one task package at a time outside the checkout,
builds a Vite production application under a non-root base, and runs Chromium 156.0.8078.4.
Each package executes in an actual browser main thread and a module Worker. Full/render exercise
ELK planning, ordinary SVG and formulas with positive nested viewports and nonempty vector glyph
paths. The remaining packages execute their own analysis, editing and ASCII operations.

All five packages passed CSP checks and loaded exactly one explicitly selected WASM URL in each
realm, without an extra default fetch. Repeated public initialization retained the same runtime.

| Artifact | Main initialization, ms | Worker initialization, ms | Initial linear memory, bytes | Linear memory after operations, bytes |
| --- | ---: | ---: | ---: | ---: |
| `web-full` | 33.2 | 21.1 | 4,390,912 | 6,946,816 |
| `web-render` | 27.9 | 16.8 | 4,325,376 | 6,684,672 |
| `web-analysis` | 8.4 | 7.3 | 2,228,224 | 2,555,904 |
| `web-editor` | 8.6 | 6.5 | 2,228,224 | 2,555,904 |
| `web-ascii` | 10.4 | 8.5 | 2,228,224 | 2,555,904 |

Initialization timing starts after the JavaScript imports and includes the local fetch and WASM
initialization. Linear memory is the actual exported `WebAssembly.Memory.buffer.byteLength`,
not process RSS, JavaScript heap usage or a peak-memory estimate. Both realms reached the same
linear-memory sizes in this run. First math renders took 61.2/58.7 ms on the main thread and
4.1/3.4 ms in the later full/render Workers. The Workers may benefit from browser compilation
caches. These are single-run observations on a shared development machine with other work in
progress; they establish neither network download latency nor a performance regression verdict.

## Reproduce

```text
node platforms/web/scripts/build-wasm.mjs --all-packages
npm --prefix platforms/web run build:ts
npm --prefix platforms/web run build:packages
npm --prefix platforms/web run smoke
cargo run --locked -p xtask -- wasm-size-matrix --surface web \
  --web-package-root platforms/web/packages \
  --budget-file docs/release/WASM_SIZE_BUDGETS.json
npm ci --prefix playground
npm ci --prefix playground/tests
npm run install:browsers --prefix playground/tests
node platforms/web/scripts/installed-browser-consumer.mjs
```

The consumer prints tarball digests, browser requests, operations, timing and linear-memory
observations. Pages and immutable release preflight own the same installed-consumer command.
New published versions still require their normal package-group and registry reconciliation.

## Render Brotli reassessment, 2026-11-06

The theme-correctness follow-up in [PR 278](https://github.com/Latias94/merman/pull/278)
uses the same Web artifact recipes, Cargo profile, dependency lockfile and compression settings.
Only `web-render.max_brotli_bytes` is reassessed; the other 23 Web and Typst ceilings remain
unchanged.

The Linux final-package measurement is from [Pages run 42136213837](https://github.com/Latias94/merman/actions/runs/42136213837/job/127632281976)
at merge source `d6afd25d9d0ff1afaf45c8f7e784240b74110f23`. Its adjacent-main control is
[Pages run 42020850645](https://github.com/Latias94/merman/actions/runs/42020850645/job/127233620511)
at `a4498a22e7e4112402014dcb3e36213ddf1ed6b2`. Both use Rust 1.95.0, wasm-pack 0.15.0
and wasm-tools 1.261.0. These are Linux CI measurements, separate from the November 1 macOS
measurements above.

| `web-render` metric | Adjacent main | After PR 278 | Previous ceiling | Accepted ceiling |
| --- | ---: | ---: | ---: | ---: |
| Raw bytes | 15,541,160 | 15,600,460 | 15,629,000 | 15,629,000 |
| Stripped bytes | 15,540,925 | 15,600,225 | 15,629,000 | 15,629,000 |
| Gzip bytes | 5,806,856 | 5,824,925 | 5,834,000 | 5,834,000 |
| Brotli bytes | 4,287,641 | 4,301,347 | 4,298,000 | 4,431,000 |

The downloaded `pages-web-package-1` archive (artifact ID `13465344293`) matches its GitHub
SHA-256 digest. All five final npm WASM files match the lengths and SHA-256 values in their
package provenance; the render WASM SHA-256 is
`3e24b39c17fa0b252ebc08b7760aeb4bb4599c25ebee4c1fb4b491f1e7963ec7`.

The Brotli increase across the merge is 13,706 bytes (0.32%); adjacent main already has only
10,359 bytes of allowance left. The 3,347-byte excess is therefore not the implementation's
total size cost. The source repairs Flowchart source-style ownership and font measurement,
State XHTML labels and style resolution, and visible theme-paint evidence across several
families. `web-full`, which includes the same renderer, also grows by 59,272 raw bytes and
13,052 Brotli bytes; `web-render` grows by 59,300 raw bytes. The lockfile, artifact descriptor,
Web producer and size owner do not change across the merge. This is consistent with added
reachable renderer behavior, without attributing exact machine-code bytes to individual fixes.

The replacement applies the existing rule to this one insufficient metric:

```text
((4301347 * 103 + 99999) // 100000) * 1000 = 4431000
```

It leaves 129,653 bytes (3.014%, including rounding) above the measured artifact. No additional
platform margin, cross-metric offset, capability removal or automatic future rebaseline is
introduced. All other 19 Web measurements fit their existing ceilings. Both runs pass Web
package smoke; the follow-up run also passes installed Chromium/Worker consumers and the
Playground Chromium, Firefox and WebKit owner tests.

The existing native size owner on macOS ARM64, with wasm-tools 1.253.0, remeasures those same
downloaded packages and passes all 20 Web limits without rebuilding WASM. Nineteen metrics
match Linux exactly; render Brotli is 4,301,346 bytes, one byte below the Linux result. The new
ceiling uses the Linux measurement, without adding a margin for that local difference. This
decision accepts artifact size only, without making latency or memory claims. The historical
failed run remains failed; the updated branch still requires its own CI result.

## Mermaid parity reassessment, 2026-11-08

[PR 287](https://github.com/Latias94/merman/pull/287) integrates the accumulated Mermaid 12.3.0
parser, model, editor, layout and SVG corrections with main's prepared-label typography.
The selected reference, supported capabilities and Web product boundaries are unchanged.
This decision reassesses only the five insufficient Web metrics below; the other 15 Web
ceilings and all four Typst ceilings remain unchanged.

### Frozen production artifacts

The measurements come from the existing Linux Web package producer and size owner, using
Rust 1.99.1, Node 26.13.0, wasm-pack 0.15.1 and wasm-tools 1.263.0. Both runs select the
descriptor-owned `wasm-size` profile and measure stripped final assembled npm WASM with
the existing gzip and Brotli settings. The current main control is separate from the older
November 1 and November 6 measurements above.

| Role | Source and run | Package-group artifact |
| --- | --- | --- |
| Current main | `ec6f180312cc0efc079434dcf1a633afbc3a0b5b`; [run 42480344060, size job 128737075388](https://github.com/Latias94/merman/actions/runs/42480344060/job/128737075388), attempt 1 | `pages-web-package-1`, ID `13617219109`, 36,986,084 archive bytes |
| Integrated PR | Head `9dd7a3a686bd52070a5f0ddddc7a4a8b0fe5d492`; checkout `87f2d365a9cefca27252bdd4248dd0065a548ff4`; [run 42484922245, size job 128747130588](https://github.com/Latias94/merman/actions/runs/42484922245/job/128747130588), attempt 1 | `pages-web-package-1`, ID `13619780533`, 37,231,419 archive bytes |

Both downloaded archives match their GitHub SHA-256 digests:

- Main: `750fbac2b9fac15ee7e244e68b271789e5b3e83c39c1059c8e002d76ee98211a`.
- Integrated PR: `ddb1904b9bcd4d1c103896edc7d77622f9b9bcb2506b2d1ce2e0b0b82c7aa6db`.

All five packages in each archive match every `artifact_files` length and SHA-256 entry
in schema-2 provenance: 66 analysis, 66 ASCII, 68 editor, 82 full and 76 render files.
The integrated package input manifests also match the committed source inputs and their
complete source/input digests through the maintained input verifier. Six untouched Windows
checkout files have CRLF bytes; their corresponding Git blobs match the Linux manifest
exactly, so the checkout is left untouched. The Linux producer passes its current-input,
package-closure, package-smoke and DOM-safety checks.

| Final WASM | Main SHA-256 | Integrated PR SHA-256 |
| --- | --- | --- |
| `web-full` | `95411a59f4bb49fe40ef19f2499ae7b88709bf6b9407d3938b9155c8fb1c0bd4` | `076194591efd6dcdca7c8a0d468a7063cd4a3ee2b1a744834d1d03f8d9b9f6a2` |
| `web-render` | `154df084400776005498073369a505e4b7623d64a2f6539b8af4775ef6d5f0aa` | `ba77227a9eabc410b88d4b6078133f71faae8e656bb729fa91f7acce30ae7c9d` |

The integrated full source/input digests are respectively
`05422f89cf35e23a290b8b500ac1955360b66b03d1af308781fed39c21901971` and
`c499be7a11b69ec466abd2937b21d6b60e716825246abbc3bf69f5350a7804a5`.
The render source/input digests are
`5463573b152bd5d390e0dea9be64c538a43603279811bc4e535d55b0d9a393ec` and
`7c5328f4b283d4005675cc62551f0e011e2151603b94d85318327121970c136e`.

### Measurements and attribution

All values are bytes. The native size owner on Windows, with the same wasm-tools 1.263.0,
remeasures the downloaded main packages without rebuilding WASM; all 20 metrics match Linux
exactly. The integrated values below are the Linux size owner's measurements of its own
source-bound package upload.

| Artifact / source | Raw | Stripped | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| `web-analysis` / main | 4,038,888 | 4,038,611 | 1,551,914 | 1,184,039 |
| `web-analysis` / integrated | 4,062,577 | 4,062,300 | 1,563,196 | 1,191,567 |
| `web-ascii` / main | 5,696,880 | 5,696,603 | 2,113,817 | 1,595,634 |
| `web-ascii` / integrated | 5,724,917 | 5,724,640 | 2,124,717 | 1,605,730 |
| `web-editor` / main | 4,193,804 | 4,193,527 | 1,613,579 | 1,227,345 |
| `web-editor` / integrated | 4,217,515 | 4,217,238 | 1,623,753 | 1,236,601 |
| `web-full` / main | 17,612,443 | 17,612,166 | 6,535,429 | 4,813,698 |
| `web-full` / integrated | 17,712,625 | 17,712,348 | 6,577,331 | 4,841,367 |
| `web-render` / main | 15,596,883 | 15,596,606 | 5,840,299 | 4,313,881 |
| `web-render` / integrated | 15,693,337 | 15,693,060 | 5,879,144 | 4,338,821 |

Current main already exceeds the render gzip ceiling by 6,299 bytes. The integrated PR adds
96,454 raw/stripped bytes (0.62%) and 38,845 gzip bytes (0.67%) to render. Full adds 100,182
raw bytes, 41,902 gzip bytes and 27,669 Brotli bytes. The amount above a previous ceiling
therefore differs from the implementation's total size cost.

The source adds reachable semantics across several owners: ER Swimlane graph construction
and painting, Gantt exclusion/calendar handling, shared XHTML terminal-font measurement,
Sequence prepared drawing geometry, and parser/model corrections. The main integration
consolidates the terminal-font measurement API and removes the branch's duplicate height
path. Similar full/render growth is consistent with shared renderer/model changes; the
smaller analysis/editor growth is consistent with shared parser changes. These observations
do not assign exact machine-code bytes to individual fixes.

`Cargo.lock`, root Cargo/toolchain configuration, capability descriptors, Web producer and
package sources, Pages workflow, optimizer arguments and size-owner implementation are
unchanged against this main control. Both artifacts lose exactly 277 bytes when stripped;
custom-section removal cannot recover the raw or compressed excess. No capability removal,
alternate artifact, compression change or speculative renderer deletion is used to fit the
old limits.

### Narrow acceptance

| Metric | Integrated bytes | Previous ceiling | Accepted ceiling |
| --- | ---: | ---: | ---: |
| `web-full.max_gzip_bytes` | 6,577,331 | 6,550,000 | 6,775,000 |
| `web-full.max_brotli_bytes` | 4,841,367 | 4,819,000 | 4,987,000 |
| `web-render.max_raw_bytes` | 15,693,337 | 15,629,000 | 16,165,000 |
| `web-render.max_stripped_bytes` | 15,693,060 | 15,629,000 | 16,164,000 |
| `web-render.max_gzip_bytes` | 5,879,144 | 5,834,000 | 6,056,000 |

Each replacement applies the unchanged integer rule independently:

```text
((measured_bytes * 103 + 99999) // 100000) * 1000
```

All other 15 Web measurements fit their existing ceilings, including full raw with only
4,375 bytes left and editor Brotli with 5,399 bytes left. Those ceilings are retained.
The four Typst limits, runtime capabilities, artifact recipes and functional gates also
remain unchanged. This explicitly accepts the five measured size costs; it introduces no
automatic future rebaseline, cross-metric offset or additional platform margin.

The integration passes 277 shared-text/ER unit tests, 531 affected render integration tests,
the complete layout snapshot corpus, all-target/all-feature Clippy, the defaults-off renderer
check and the full SVG corpus in all three DOM modes. The budget decision makes no latency,
memory, pixel-identity or complete-platform-qualification claim. Historical failed runs
remain failed; the budget follow-up requires its own CI result.

## 2026-11-09 font-correctness follow-up

The current main and PR #289 both exceed only the `web-full` raw and stripped ceilings.
The size owner measures the final assembled npm package with the unchanged `wasm-size`
profile, Rust 1.99.1, wasm-pack 0.15.1, wasm-tools 1.263.0, default features disabled,
and compression applied to stripped bytes. This decision uses the frozen PR head below;
subsequent renderer changes must pass the same owner against their own produced packages.

| Source | Exact revision | Linux size owner |
| --- | --- | --- |
| Current main | `b5e2e6c576ab32d7b7d65e482d6d4eed9e029e33` | [Run 42542343658, job 128923346670](https://github.com/Latias94/merman/actions/runs/42542343658/job/128923346670) |
| PR #289 | `bf6d9aa31a250f14a4e37954285968905c3d3bda` | [Run 42548444488, job 128941943854](https://github.com/Latias94/merman/actions/runs/42548444488/job/128941943854) |

The PR checkout `65155ba1489965849b9194e844c871b4a2aad5f5` has the same Git tree
as its head. The source-bound package archives are `pages-web-package-1`, IDs
`13642009035` for main and `13644966941` for the PR. The CI download step verifies
their respective SHA-256 digests:

- Main: `85fb28c5d1b27800cb1e1219a439c08a2d3a57dbe146bd40010a311c45747d93`.
- PR: `3a616c2312060e28e7cfd0fa3ea41ce8297ffc774e1fc17bee4b79207be56d23`.

The following values come from the completed size-job logs. A local archive download
timed out; no independent local package remeasurement or raw-WASM digest is claimed.

| Artifact / source | Raw | Stripped | Gzip | Brotli |
| --- | ---: | ---: | ---: | ---: |
| `web-full` / main | 17,727,462 | 17,727,185 | 6,584,904 | 4,842,828 |
| `web-full` / PR | 17,738,083 | 17,737,806 | 6,592,375 | 4,850,628 |
| `web-render` / main | 15,708,448 | 15,708,171 | 5,885,467 | 4,338,997 |
| `web-render` / PR | 15,719,074 | 15,718,797 | 5,892,367 | 4,350,739 |

Main already exceeds full raw and stripped by 10,462 and 10,185 bytes. The PR adds
10,621 bytes to both measures, approximately 0.060%; its gzip and Brotli increases
are 7,471 and 7,800 bytes. Analysis, ASCII and editor have identical measurements
on both sources, and the other 18 Web measurements fit their ceilings. Both full
artifacts lose only 277 bytes when stripped, so custom-section removal cannot recover
the excess. The November 8 full raw baseline had only 4,375 bytes of headroom.

The fixed source delta adds prepared C4 wrapping and resource checks, final-font geometry
for Ishikawa, Pie and Tree View, and visible-label accounting for Radar and Quadrant.
It also removes the duplicate Journey break scanner and Pie size parser. Similar
full/render growth is consistent with shared renderer changes; it does not assign
machine-code bytes to individual fixes. Cargo dependencies, capability recipes, Web
producer, optimizer, size owner and compression settings are unchanged. No measured
redundancy justifies deleting supported behavior to recover this amount.

| Metric | Frozen PR bytes | Previous ceiling | Accepted ceiling |
| --- | ---: | ---: | ---: |
| `web-full.max_raw_bytes` | 17,738,083 | 17,717,000 | 18,271,000 |
| `web-full.max_stripped_bytes` | 17,737,806 | 17,717,000 | 18,270,000 |

Each replacement uses the existing measured-plus-3% integer rule above. The other
22 ceilings, functional gates, artifact recipes and compression settings remain
unchanged. This is a bounded acceptance of observed artifact growth, not a claim about
latency, memory or equal-capability savings. The historical failed runs remain failed;
the integrated follow-up still needs its own package-size and functional CI results.
