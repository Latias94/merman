# DOMPurify security verification - 2026-10-24

## Scope

Select DOMPurify **3.4.17** instead of 3.4.16 for the shared Playground, reference CLI,
and generated Rust defaults. The separately authorized browser-tooling repair selects Puppeteer
**25.16.0**, Puppeteer Core **25.16.0**, and `@puppeteer/browsers` **3.2.6**. Mermaid **12.2.0**,
parser **2.0.1**, CLI **12.0.0**, ZenUML, and KaTeX remain unchanged.
This is CI security repair, not a Mermaid baseline migration or a performance experiment.

The official GitHub advisories identify 3.4.17 as the first patched version:

- [GHSA-cg2g-xh64-v4q7](https://github.com/advisories/GHSA-cg2g-xh64-v4q7): critical,
  split template expressions in attached shadow roots.
- [GHSA-mrv2-3pqr-j5jr](https://github.com/advisories/GHSA-mrv2-3pqr-j5jr): high,
  detached subtree event handlers during in-place hooks.
- [GHSA-c6gr-qvv4-5v3f](https://github.com/advisories/GHSA-c6gr-qvv4-5v3f): low,
  SMIL `attributeName=href` in XHTML parsing mode.

## Identity and supply chain

- Node 24.21.0; npm 12.0.2; lifecycle scripts disabled.
- Official release tag and npm `gitHead`: `a77a3e78b59f72fa4d1c83f22e121ad9cc7d3575`.
- Registry integrity:
  `sha512-B/pSlYVHQ58kqISD/yDjuJ1yfzI34m1gF4lAod2bwSRhiN4K5Cl3H8Af7hqB2PmTL911tdEudnnXn5FnhRoz0w==`.
- Both installed workspaces have DOMPurify tree SHA-256
  `2a67a4d0cb2f51dbde8e63138391edd707e0d4bef608b1ad19c509d162e75bb6`.
- The source checkout is clean at the selected commit; the preserved Apache-2.0 license is unchanged.

The exact official command exited 0:

```text
npm audit signatures --json --include-attestations --registry=https://registry.npmjs.org/
```

Raw stdout SHA-256: `7d0db0fecb6f398c348d680f5690523176add0d5e1a85120c742039b9f3dc58d`.
The official non-JSON summary reports **two verified registry signatures**. JSON `verified: []`
is the attestation list, not a report of missing registry signatures. No provenance attestations
were returned or claimed. The initial direct attempt timed out on the CDN; its failed output is
retained separately from the successful official-command run through the configured proxy.

The receipt binds trusted base `83e3ea74bf4da30089a8012ec2d3418d40638e52`:

- Previous identity: `20521e9ec0fa21d661a6e1f2740eb72663a72df7a0fc10aaee4c8e27a41c3389`.
- Current identity: `886f6d2bb684331b93b11498933c2615f92e363f61ebd7d7ed5d20ed5db615f8`.
- Exactly 24 changed identity fields: `declaredRange`, `integrity`, `source.commit`,
  `source.reference`, `tarballUrl`, and `version` for the sanitizer and each of the three browser
  tools. The receipt's previous identity remains the trusted base, not the incomplete DOMPurify-only
  candidate.
- All three Puppeteer release tags and npm `gitHead` resolve to
  `a207faf527ce4305b33a6c0f9706b090c21c115e`.
- The scriptless isolated browser-tooling candidate also includes selected DOMPurify and Zod.
  The same official npm command exits 0 with no invalid or missing signatures. Its JSON lists ten
  verified provenance entries, including all three changed Puppeteer packages and Zod; this does
  not assert that DOMPurify has provenance. The receipt binds this combined run's stdout SHA-256:
  `4bd1e63bbbe3a679567b7be4ce6debb172c000409b0f60482396403d2beb1674`.
- Puppeteer's installation hook remains disabled. After inspecting the published browser CLI,
  the explicit `browsers install chrome-headless-shell` command acquires its pinned browser;
  no package lifecycle script executes.

## Executed behavior evidence

- Reference-runtime builder tests: **8/8 passed**.
- Selected-browser parser-oracle tests: **9/9 passed**, including consecutive conflicting diagrams,
  real family DB observations, runtime-digest rejection, and the Windows CLI path smoke.
- Unmodified upstream-generation strict browser probe: Mermaid 12.2.0 called the exposed selected
  DOMPurify 3.4.17 instance **7 times**; the malicious HTML-label event attribute was removed.
- Pinned upstream QUnit browser suite: **1345/1345 assertions passed** on each of the same selected
  reference-runtime instance, published UMD build, and published minified build. All three runs
  reported zero failed assertions and zero uncaught page errors. The selected-instance run also
  asserted object identity with `mermanReferenceRuntime.sanitizer`.
- The browser suite and strict probe are replayed with selected Puppeteer 25.16.0 and its pinned
  HeadlessChrome 156.0.8078.47 on Windows, retaining the results described above.
  QUnit 3.0.0 and jQuery 3.7.1 match the pinned upstream lock integrities; their isolated scriptless
  installation passed official npm signature verification. The suite, fixtures, setup, and entry
  scripts run directly from the pinned source; no substitute assertion engine is used.
- Owned runtime SHA-256:
  `29d255aa0a6d1d69e6d7c56a67b64c6c89318e2acf079587ec85743c70b44a2e`.
- `gen-mermaid-reference` and `gen-dompurify-defaults` executed through the repository generator.
  The Rust default sets are byte-identical except for the generated source-version comment.
  Rebuilt `xtask.exe verify-dompurify-defaults` passed.
- `xtask.exe verify-generated` passed all 14 generated-artifact checks without running Cargo.
- Rust sanitizer and pinned-baseline selection: **72/72 tests passed**, rerun after the main thread
  removed its rejected Flowchart size trial. Command:
  `cargo nextest run --locked -p merman-core --all-features --lib -E 'test(sanitize) or test(pinned_baseline)' --no-fail-fast -j 2`.
- `npm ls --all` passed for both owned workspaces. Playground lint passed after the in-range Zod
  update. Playground production audit and the test workspace tooling audit also passed.

Temporary raw evidence and the browser orchestration script are retained under
`target/mermaid-admission/dompurify-3.4.17-ci/` and
`target/mermaid-admission/toolchain-security-2026-10-24/`; they are not standing reference inputs.

## Baseline and owner gate status

Full primary-family SVG regeneration uses the rebuilt repository generator. The initial invocation
is:

```text
target/debug/xtask.exe gen-upstream-svgs --diagram all --jobs 2
```

This is a real render, not a provenance-only relabel. The complete primary corpus finished on
2026-10-25 local time: **37/37 families, 3,737/3,737 SVGs byte-identical**, no missing or added SVGs,
and no unexpected manifest differences. The changed manifest fields are only the reviewed workspace
digests, browser version/revision, Puppeteer version, sanitizer version/package digest, and owned
runtime digest. The snapshot comparison is separate evidence from the browser-suite results.

The earlier DOMPurify-only generation and its waiting monitor were explicitly cancelled after the
browser-tooling upgrade was authorized. Their partial outputs and logs remain historical evidence,
not completed admission. `cancelled-for-toolchain-upgrade.json` records the owned process trees.
The new complete run uses the rebuilt generator and selected Puppeteer 25.16.0; the original
before-generation snapshot is retained for actual byte and DOM delta review.

After four families and 1,262 byte-identical SVGs completed, that two-worker invocation was
explicitly cancelled for a family-scoped resume; the incomplete next family is not retained.
The remaining families resumed through the same
`gen-upstream-svgs --diagram <family> --jobs 4` owner command. The host had 32 logical processors
and 21.5 GiB free memory; Cargo remains serial. A family is retained only when its complete
manifest binds the exact selected workspace, lock, config, browser-driver version, sanitizer,
and runtime artifact. `resume-plan.json` and `resumed-generation.result.json` record the retained
and newly rendered families. No incomplete family or staging SVG is treated as promoted output.

After generation and complete snapshot review, the following materialized, base-bound verification
passed, followed by `verify-generated`:

```text
target/debug/xtask.exe verify-mermaid-reference --materialized --base 83e3ea74bf4da30089a8012ec2d3418d40638e52
```

The generation results and final baseline review are retained under
`target/mermaid-admission/toolchain-security-2026-10-24/`. The earlier standing-contract run
correctly rejected the then-stale baseline workspace hashes; no check or manifest was weakened
to bypass that guard. The final reference-contract replay passes **25/25 tests**, including
standing verification without candidate artifacts. The full Linux structure/parity/parity-root owner remains a CI gate;
Windows browser and byte-equality evidence do not replace it.

## Additional audit finding

Live tooling audits also discovered [GHSA-mrhx-c7xc-m3w5](https://github.com/advisories/GHSA-mrhx-c7xc-m3w5),
a high-severity Zod constrained-collection resource issue fixed in 4.6.6. This separately authorized
tooling repair pins Playground Zod to **4.6.6**, within both `eslint-plugin-react-hooks` and
`zod-validation-error`'s `^3.25.0 || ^4.0.0` ranges. Playground's high-level audit then reports
**zero vulnerabilities**. Zod is a development dependency, not a change to the selected reference
identity or production legal closure.
The npm license report nevertheless embeds the complete lockfile digest, so it was regenerated
and checked after this development-only lock update. Rust license checks pass all 13 reports;
release legal synchronization and its check pass all 424 projections, and the third-party license
contract passes.

The old reference CLI chain's `chromium-bidi@17.0.2` requires Zod `^3.24.1`, so overriding it with
Zod 4 would be outside the parent's range. Instead, Mermaid CLI 12.0.0's published Puppeteer
peer range `^25.0.0` admits Puppeteer 25.16.0. Its `chromium-bidi@157.0.8090-0` requires Zod
`^4.6.5`, admitting the selected exact override **4.6.6**. `npm ls` confirms a single consistent
browser-driver closure. The CLI tooling audit now reports **zero high or critical findings**;
the two low dependency-path findings remain attributable to CLI's KaTeX 0.16.47. The existing
production-low and tooling-high audit policies are not weakened. KaTeX is not forced outside
its parent's range, and `npm audit fix --force` is not used.
These are two dependency-path findings for one advisory, not two distinct vulnerabilities.
Mermaid's owned reference runtime and its Playground dependency resolve the separate KaTeX 0.18.10
copy; Playground's direct KaTeX override remains 0.19.0. The old CLI
tooling dependency is still vulnerable when used directly; development-only classification is
not a claim that arbitrary untrusted tooling input is safe.

## CI repair and generated documentation

CI run `40089189461` is bound to `4244b8f56e2bcec3d5736434b7ecd2bcc17b4b65`.
Its Linux workspace nextest stage passed **13,358 tests**, with nine explicit skips. The later
`rustdoc check` stage failed because its committed capability receipt retained the pre-refactor
descriptor digest. The default CLI's existing `rustdoc build` followed by `rustdoc check` repairs
that receipt; generated Markdown and SVG bytes remain unchanged. This is a scoped local repair,
not a claim that the entire old or future CI run passes.

The same CI run found three Undici advisories in independent Tree-sitter development tooling:
[EventSource memory growth](https://github.com/advisories/GHSA-4qv8-2p9j-2685),
[SRI algorithm parsing](https://github.com/advisories/GHSA-f72v-6w45-q472), and
[content-encoding error handling](https://github.com/advisories/GHSA-xjj6-f33q-929c).
The focused lock update selects **8.12.3**, inside node-gyp 13.1.0's `^8.4.1` range. Only that
package's version, archive URL, and integrity change. Both CI audit policies then report zero
vulnerabilities. Existing-workspace regressions pass 11 generation-script tests, 259 corpus cases,
one Node binding test, and four WASM tests; because that workspace contained stale installed native
tooling, these are not clean-lock native-build evidence. A separate scriptless clean installation
confirms Undici 8.12.3, node-gyp 13.1.0, and web-tree-sitter 0.27.1, then passes the node-gyp CLI
smoke and all four WASM tests. The grammar, generated parser, native binding, WASM, and selected
Tree-sitter baseline do not change.

VS Code's separate packaging dependency still blocks its tooling-high audit on
[GHSA-xcx6-jx8p-x3h5](https://github.com/advisories/GHSA-xcx6-jx8p-x3h5). The patched MSAL Node
release is **7.1.0**, outside `@azure/identity` 4.13.3's `^6.0.0` requirement. Checked stable
4.14.0 and beta 4.15.0-beta.1 retain that requirement; VSCE stable 4.0.0 and next 4.0.1-5 do not
admit a repaired Identity chain. A targeted compatible update and an unforced audit-fix dry-run
make no change. No out-of-range override, advisory suppression, or audit-policy change is applied.
Its production audit remains clean; typecheck and all 255 extension tests pass. This is one
unresolved advisory represented by two high dependency-path nodes, not a second selected-reference
sanitizer finding. Raw metadata, audits, commands, and test evidence are retained under
`target/mermaid-admission/npm-other-security-2026-10-24/`.

Release legal projections and Cargo/security/size work are owned by the main thread. Focused
commits and normal PR-branch pushes are authorized; PR merge, publication, release, and long-term
refactor-goal resumption are not part of this patch.
The independent unresolved WASM budget result is recorded in
[the main-thread size verification](../performance/semantic_wasm_ci_2026-10-24.md); this security
patch does not change budgets or make the whole PR green.
