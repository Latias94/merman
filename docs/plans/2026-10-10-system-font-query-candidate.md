# On-demand system font query candidate

Date: 2026-10-10. Status: opt-in experimental implementation, not production
admission. Existing CLI/library/FFI default feature recipes stay unchanged.
Source baseline: dirty `feat/cli-native-font-context` at
`2928265122d5a3d35e239e3ddbce885c7e342c59` plus the current native-font work.

## Question

Can a mature discovery crate reduce the full catalogue cost without replacing
Merman's shared measurement, shaping, wrapping or painting infrastructure?

The [source audit](../research/2026-10-10-mature-project-font-discovery.md)
identified Fontique 0.12.0 as a candidate before its optional experimental
integration was approved; it remains outside default and published recipes. Its
Windows family query maps candidate files internally before Merman obtains
their paths. macOS initialization also uses scanning. A bounded read performed
after those calls cannot retroactively bound their work.

## Isolation and scope

Use a standalone Cargo workspace below
`target/bench/experiments/system-font-query-2026-10-10/fontique-bridge/`.
Gate its Fontique dependency behind `experimental-system-font-query`.
After the closed bridge passed, the maintainer approved separating trusted CLI
platform-discovery policy from bounded library acquisition. The shared export
backend, facade and CLI now expose that same opt-in feature for end-to-end
validation; it is not in default or published artifact recipes. The dependency
is optional and the existing `NativeFontOptions::load` semantics stay intact.

The first bridge resolves explicitly requested font families and styles to
paths, then passes only those paths to `NativeFontOptions`. Build one shared
`NativeFontContext` for the existing public `Renderer` and native exporters.
This exercises text in complete diagrams, not a title-only measurement hook.
Treat a TTC path as a file containing multiple faces; do not claim path loading
admits only the candidate's collection index.

Use the existing Flowchart, ER and Sequence inputs from
`docs/assets/native-font-measurement/`. Resolve Fira Code and a default-stack
control. Render SVG/PNG/JPEG/PDF through existing public APIs with the same
font configuration. Compare against an explicit-path control using the same
selected bytes. Record output hashes, selected faces, resident bytes, context
identity and failures. A different discovery/fallback policy is not automatically
a same-output optimization.

The first bridge is a closed selected-family experiment. It does not implement
general mixed-script system fallback or the eventual lazy resolver, and must
not be described as the finished default-CLI replacement. It does not pre-scan
MMD labels or introduce diagram-specific text handling.

## Evidence and admission

### Closed bridge checkpoint

The isolated release probe passed 48 calls: two font controls, three diagrams,
four formats and query/explicit controls. All 24 pairs produced identical output
hashes and context identities. Two delayed repeat PDF calls matched their
original outputs. A missing-family query failed without publishing output.
Raw receipts are retained below the ignored bridge directory; this is local
correctness evidence, not a cross-platform timing or fallback admission.

### Shared lazy candidate

Add `NativeFontOptions::load_platform(control)` behind the experimental feature,
without adding a public options field. It delegates to closed loading when
system access is not authorized. Existing `load(control)` remains the bounded
path. Experimental CLI builds use platform loading only when their runtime
policy already authorizes system fonts. Explicit-only requests do not enumerate
system fonts.

The common usvg resolver resolves real family/style requests on demand and
appends binary-only faces. Candidate glyph coverage, selected byte limits,
snapshot reuse and fatal propagation remain Merman-owned. Shared font discovery
metadata can outlive operations; actual dynamic snapshots reset with
`for_operation`. Context identity is policy plus initial admitted bytes, not a
cross-process digest of all installed or subsequently selected platform fonts.

Register the exact executable, dependency lock, commands, fixture hashes and
raw results under the ignored experiment directory. Initially use diagnostic
fresh-process observations only; set no production speedup threshold and make
no timing admission claim. Font lookup timings are not complete CLI timings.
For a later production candidate, use the performance runbook's equivalent
public-operation A/A and balanced A/B confirmation, including cold discovery.

Before production integration, require all of the following:

1. A supported boundary for resource-controlled candidate metadata acquisition,
   or an explicitly reviewed new resource policy. No hidden weakening of
   limits, cancellation or fatal-error propagation.
2. Shared selected byte snapshots for measurement and painting, including
   fallback, repeated requests and operation-local lifetime.
3. Explicit files/directories remain closed unless system access is authorized.
4. Native script/locale candidates undergo actual glyph coverage validation;
   missing families, styles, collections, variable fonts and mixed scripts have
   controls. No guessed host-specific CJK/emoji family names.
5. Real Windows, Linux and macOS evidence. Windows timings do not establish
   macOS initialization cost or portable system-font identity.
6. Only after the above, register published feature/capability/dependency/legal
   projections and choose CLI runtime defaults. Library/FFI defaults stay
   host-independent unless separately specified.

## Fearless refactor sequence

1. Stabilize the acquisition boundary: make `FontContextInner` own one source
   registry for explicit and platform-selected files, so path canonicalization,
   byte snapshots, face limits and identity cannot diverge between loaders.
2. Keep the two discovery policies behind one private provider enum. The bounded
   directory scanner and Fontique adapter return the same candidate descriptor;
   neither provider owns byte admission, measurement or painting. A trait is not
   needed for this closed pair of implementations.
3. Keep one resolver implementation. It receives CSS family/style requests,
   asks the provider for candidates, admits one verified binary snapshot, and
   installs the same face in the measurement and usvg databases. Remove the
   duplicated legacy/system resolver branches after parity tests pass.
4. Make fallback candidate selection a separate policy object. It must validate
   the actual character, record the selected face and preserve fatal errors;
   script/locale candidates are hints, not coverage proof.
5. Define context identity as policy plus admitted bytes. Do not use a complete
   installed-font catalogue hash as a cache key in platform mode. Repeated
   operations may share immutable discovery metadata but must receive fresh
   operation snapshots and control state.
6. Promote the feature only after CI proves Windows, Linux and macOS family
   matching, mixed-script fallback, explicit closed sets, output formats and
   resource/cancellation contracts. Then update generated capability and legal
   projections in one tooling run.

## CI acceptance lanes

Add a manual or label-gated matrix lane, not default timing on every pull
request. Each OS installs a deterministic fixture font (Cousine already lives
in the repository) and separately exercises a host-family smoke using the
platform's known default sans/monospace names. The fixture lane must assert
identical measurement/painting selected face and output structure; the host
lane records family availability and timing without requiring identical names or
pixels across OSes. Run SVG plus PNG/JPEG/PDF where the profile enables them.

The lane should run the experimental feature with `--locked`, the portable
closed-font tests on all hosts, and ignored host-sensitive tests only on the
matching OS. Publish raw receipts as workflow artifacts. Do not use a Linux
result to infer Windows/macOS startup cost, and do not make host-font timing a
release threshold until a balanced cross-platform corpus exists.

## Decision rule

Distinguish trusted local CLI policy from constrained service/FFI policy.
Fontique's internal mappings are not evidence of an unsafe crate, nor do they
imply every mapped byte immediately becomes resident memory. They mean that
Merman cannot advertise unchanged strict acquisition guarantees. A future CLI
policy may explicitly authorize platform-managed discovery while retaining
bounded selected snapshots; such a decision requires documented guarantees and
review, not a silent weakening of existing limits. Do not reject that product
option solely because native OS calls are not preemptively cancellable.

If the public crate cannot preserve the acquisition boundary, retain the
isolated experiment and document rejection as a production adapter. Investigate
a narrow upstream candidate-descriptor/controlled-loader interface before
maintaining a fork or platform font engine. Do not add persistent caching to
mask an unresolved safety or ownership problem.

## Shared resolver checkpoint (2026-10-11)

The export context now owns one private discovery policy, one source registry and
one usvg resolver. Both bounded directory discovery and the Fontique adapter
supply candidates to the same admission path. The registry checks total explicit
plus delayed files, resident bytes, unique admitted faces and cmap budgets across
all measurement and painting databases in an operation. Metadata discovery still
has independent limits; platform-managed metadata remains outside Merman's
bounded acquisition guarantee.

Initial bytes and their path aliases are immutable and shared. Dynamic snapshots,
face admissions and aliases reset with `for_operation`. Already admitted bytes
remain usable after their original file disappears; an unadmitted changed or
missing bounded source fails closed. Candidate acceptance verifies the actual
binary face and fallback uses usvg glyph/style checks against binary-only data.

The three-host CI matrix now runs export tests with the experimental feature and
PNG/JPEG/PDF enabled, every ignored host contract compiled for its OS, and CLI
contracts in independent steps. This includes real generic-family measurement /
painting agreement, stable residency, operation failure isolation and cancellation
at actual export boundaries. Multi-command native/bounded font steps use an
explicit fail-fast shell so Windows cannot mask an earlier native-command failure.

The Windows results are local evidence. Linux/macOS results and equivalent public
operation timing receipts remain admission gates; configuring those jobs does not
establish their success. CLI default policy must be decided after that evidence is
available, and library / FFI defaults remain host-independent.

Local validation at this checkpoint passed 111 export tests with the experimental
provider and PNG/JPEG/PDF enabled, all 11 Windows host contracts, 8 experimental
CLI contracts and 14 facade context contracts. The bounded-provider library lane
passed 26 tests. The feature-matrix verifier completed its 42 host build cases,
and `cargo fmt --all --check`, the changed-file whitespace check and export
`cargo clippy --all-targets` with the experimental / PNG / JPEG / PDF feature
combination and `-D warnings` passed. These
are current-worktree observations, not results from a published commit or from
Linux/macOS CI.
