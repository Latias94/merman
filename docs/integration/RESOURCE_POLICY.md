# Resource policy for host integrations

This guide describes the current source. The input-policy inheritance described below and
recalibrated defaults are newer than published `0.8.0-alpha.7`; use matching release documentation.

Merman is headless: the host owns scheduling, UI state, cancellation, and memory capacity.
Choose resource policy for the job, not from whether the executable is native or WebAssembly.
A native editor may render untrusted text, and a WASM component may generate offline documents.

## Choose a finite starting policy

| Rust policy constructor | Typical use | Host responsibility |
| --- | --- | --- |
| `interactive()` | Editor preview, general library use | Cancel obsolete work; set a useful deadline; bound concurrent jobs |
| `trusted_native()` | Offline documents, local export, rustdoc, CLI | Accept its larger input/output envelope deliberately; bound batch concurrency |
| `constrained()` | Embedded or shared services with deliberately restricted diagrams | Define the accepted subset; tune individual limits to the deployment |
| `unbounded_for_trusted_input()` | Explicitly trusted exceptional input or controlled diagnostics | Provide isolation/capacity and accept the removal of configurable ceilings |

The default library policy is `interactive`; Merman CLI and rustdoc select `trusted_native`.
Interactive is intended to admit ordinary diagrams from the supported corpus. Constrained
intentionally accepts a smaller subset and is not a drop-in promise to render the same documents.
Profiles also set source, model, nesting, SVG, and other limits: changing profile changes more
than layout work. Use `RenderResourcePolicy::apply_override` to change only a relevant limit.
The current interactive layout allowance is 14,100,000 work units, trusted-native is 15,000,000,
and constrained remains 125,000. Published alpha.7 predates the interactive recalibration.
See the [resource catalog](../bindings/OPTIONS_JSON.md#resource-options) for available dimensions.
In Rust, inspect each `ResourceLimitId` with `RenderResourcePolicy::value()` for its effective limit.

`trusted_native` is an explicit policy name, not automatic platform detection or a claim that a
file is safe. Deterministic runtime adapters and resource profiles are separate choices.
Configurable unbounded limits also do not remove hard implementation capabilities or cancellation.

## Configure one graphical operation

For a normal source-to-SVG call, configure the SVG environment once:

```rust
use merman::svg::RenderResourcePolicy;
use merman::{
    OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgEnvironment, SvgRequest,
};

fn render_document(source: &str) -> Result<RenderOutput, RenderError> {
    let resources = RenderResourcePolicy::trusted_native();
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic().with_resource_policy(resources),
        ..SvgRequest::default()
    };
    Renderer::new().render(RenderRequest::svg(
        source,
        OperationControl::new(),
        request,
    ))
}
```

With no explicit input override, the graphical target's environment supplies input admission as
well as layout/SVG policy. The same inheritance applies to graphical layout/plan requests and
the SVG request embedded in PNG, JPEG, or PDF requests. Binary export has additional target limits.

Input-policy precedence is:

1. `RenderRequest::with_resource_policy` for this operation.
2. `Renderer::with_resource_policy`, when the caller explicitly set it.
3. The graphical target's `SvgEnvironment` policy.
4. The default input policy for targets without an SVG environment.

An explicit request/renderer **input** override does not replace target-local layout or output
limits. This lets a host impose strict source admission while allowing sufficient layout work.
For the two-stage `prepare_semantic` API, choose the input policy on the renderer **before**
preparing the artifact; a later output target cannot retroactively change that admission.

Parser-only calls do not perform SVG layout. Semantic input limits still matter. ASCII requests
have their own `AsciiResourcePolicy`, grid/label limits, and output contract; an SVG work ceiling
is not an ASCII budget. See the [host examples](../../crates/merman/examples/README.md).

## Keep three different controls separate

| Control | What it bounds | What it does not promise |
| --- | --- | --- |
| Layout work limit | Deterministic algorithm work and preflight complexity | Milliseconds, RSS bytes, or a universal node count |
| `OperationControl` deadline/cancellation | Cooperative execution lifetime at checkpoints | Forced interruption of an opaque synchronous callback |
| Host concurrency / memory admission | Simultaneous jobs and retained output in that host | A per-diagram complexity policy or process RSS cap |

A deadline cannot replace a work ceiling, and a work ceiling cannot express UI responsiveness.
An iteration allowance is not a memory measurement. A host may still reserve conservatively
against derived layout structures admitted by the policy; removing that reserve requires evidence
for a replacement, including allocations before a rejected operation stops.
Input/model shape, simultaneous layout state, raster dimensions, export buffers, and retained
results have different memory costs. Keep scheduling in the host; see the
[CLI policy](../security/CLI_RESOURCE_POLICY.md) for Merman CLI's separate admission rules.

Each independent render gets a fresh `OperationControl`. Reusing one renderer shares settings,
not a cumulative layout budget. Rendering light and dark variants performs two operations.
A resource error's `actual` can be the first rejected charge or a preflight estimate; it is not
necessarily the total budget needed to finish. Do not repeatedly retry using that value.

## Editor and preview lifecycle

The native [cancellable example](../../crates/merman/examples/render_cancellable.rs) uses only
`std::thread` and the public operation API:

```sh
cargo run --locked -p merman --example render_cancellable > preview.svg
cargo run --locked -p merman --example render_cancellable -- --cancel
```

A host should keep its existing executor and apply this lifecycle:

1. Debounce edits and retain the newest source revision.
2. Keep one `OperationControl` with each queued/running job. Call `cancel()` on superseded jobs.
3. Start a relative execution deadline when the worker begins, unless queue time should count.
4. Limit worker concurrency. Cancellation does not instantly release a running worker's memory.
5. Publish only a result belonging to the current revision; a cancellation may race completion.

Dropping an async future or join handle does not stop synchronous Merman code already executing
on a background thread. This also applies to native editor integrations such as Zed. Use the
retained control handle to request cancellation and wait for a checkpoint. For web previews,
a host Worker keeps synchronous work off the UI thread; worker lifetime is a host decision.

Preserve `RenderError` categories in the UI. A cancellation normally means discard the obsolete
result; a resource rejection should identify the exceeded limit; malformed source remains a
parse error. A preview may retain the last successful image while showing a failure state.
Do not automatically retry with an unbounded policy, silently change layout backend, or hide
resource failures as valid empty diagrams. Let users or host policy choose a larger finite limit.

## What real integrations need

These are fixed public source snapshots, mostly using alpha.6. They establish integration
patterns, **not execution verification against alpha.7 or the current source**.

| Integration | Observed behavior | Consequence for integration guidance |
| --- | --- | --- |
| [Zed][zed] | Local snapshot pins alpha.5; background synchronous SVG render, default policy, errors fall back to source | Dropping the task is insufficient for cancellation; preserve the typed error |
| [BeauTyXT][beautyxt] | Android SVG: constrained shape/output limits, 1,000,000 layout work, 1.5 s deadline | Editors can need modest memory limits and a larger work allowance |
| [mdriver][mdriver] | Terminal SVG: default policy plus 5 s deadline; ASCII separately defaults | Deadline, graphical resources, and ASCII policy stay distinct |
| [ruby-merman][ruby] | SDK exposes profiles/overrides and wires input/SVG separately | One graphical-operation policy should avoid duplicate default wiring |
| [adocers][adocers] | Static HTML/print uses library defaults; HTML may fall back to browser | Offline document hosts must deliberately select a document policy |
| [md2pdf][md2pdf] | Markdown-to-PDF uses default-resource SVG, then Typst | A CLI-only fix does not reach offline library users |
| [Andy][andy] | Alpha.5 JNI PNG renderer; JVM owns serialization and caching | Preserve host scheduling; raster memory differs from layout work |

[zed]: https://github.com/zed-industries/zed/blob/1399a804bd53e580fa3c60e5344a7c93fe15e28c/crates/markdown/src/mermaid.rs#L213
[beautyxt]: https://github.com/soupslurpr/BeauTyXT/blob/5a5110afd5af73cfd59d26e5231f65a99cdc7aec/rust/diagram-core/src/lib.rs#L39
[mdriver]: https://github.com/llimllib/mdriver/blob/d840534021202f1bb1832e6f7cc611db27ac450d/src/lib.rs#L1600
[ruby]: https://github.com/dewasm/ruby-merman/blob/12695b6073a36697274b5af8c155972a93f7b05f/wasm/src/lib.rs#L184
[adocers]: https://github.com/AlexanderThaller/adocers/blob/7383a82510a5a9696d4ffbe72129471194fc9b0d/crates/adocers-render-core/src/mermaid.rs#L226
[md2pdf]: https://github.com/Ivapo/md2pdf/blob/5738884d8fc1dc8ff54f856fafa0c60f6be611f2/core/src/diagram.rs#L74
[andy]: https://github.com/j-roskopf/Andy/blob/6b30c3ee0abc0717022b8e1c642060ab944c4ea8/native/andy-mermaid/src/lib.rs#L20

## Keep defaults usable across upgrades

When changing a default layout backend or work accounting, rerun the accepted diagram corpus
under each advertised policy and compare accepted outputs with a sufficiently large finite
budget. Include nested Class diagrams, ordinary editor previews, and document-generation cases.
Recalibrate the common-diagram work allowance from measured work plus documented headroom;
keep input/model/output limits independent. Preserve explicit host overrides and test exact
accept/reject boundaries. Update the matching release guide when the acceptance envelope changes.

The [native calibration record](../performance/native_layout_work_calibration_2026-10-01.md)
explains why source length or node count alone could not choose the Class/ELK allowance.
