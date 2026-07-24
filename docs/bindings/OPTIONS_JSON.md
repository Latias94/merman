# Binding Options JSON

Status: experimental shared binding contract.
Last updated: 2026-07-23

All public binding surfaces accept an optional `options_json` string. Passing null, `None`, `nil`,
or an empty string uses defaults. The same JSON contract is shared by the C ABI, Android JNI, Apple
Swift, Flutter/Dart FFI, and Python UniFFI package.

Reusable engines keep construction options as an immutable baseline. Each operation may supply
request options that deeply merge over that baseline: nested objects merge recursively, while
arrays and scalar leaves replace the baseline value. Request-local overrides do not mutate later
operations. Runtime policy is engine-owned, so reusable requests cannot set it; one-shot operations
may select it while constructing their temporary engine.

Unknown top-level fields are ignored. The `layout` and `environment` service objects reject unknown
fields so removed service paths cannot be silently ignored. Invalid JSON, invalid UTF-8,
unsupported enum values, or non-finite numeric values return binding errors instead of panicking.

## Full Shape

```json
{
  "version": 1,
  "runtime_policy": "deterministic",
  "fixed_today": "2026-02-15",
  "fixed_local_offset_minutes": 0,
  "host_theme": {
    "preset": "one-dark",
    "appearance": "dark",
    "font_family": "Inter, system-ui, sans-serif",
    "roles": {
      "canvas": "#0f172a",
      "surface": "#111827",
      "text": "#e5e7eb",
      "border": "#475569",
      "line": "#94a3b8",
      "success": "#34d399"
    },
    "series_palette": ["#60a5fa", "#34d399", "#f59e0b"],
    "output": {
      "pipeline": "resvg-safe",
      "root_background": "canvas",
      "css_override_policy": "strip-existing-important",
      "drop_native_duplicate_fallbacks": false
    }
  },
  "site_config": {
    "theme": "base",
    "themeVariables": {
      "mainBkg": "#111827",
      "nodeTextColor": "#f8fafc"
    },
    "themeCSS": ".node rect { stroke-width: 2px; }"
  },
  "parse": {
    "suppress_errors": false
  },
  "ascii": {
    "charset": "unicode",
    "defaultDirection": "leftRight",
    "colorMode": "plain",
    "sequenceMirrorActors": false,
    "xychartVerticalPlotHeight": 5,
    "xychartCategoryBandWidth": 3,
    "xychartHorizontalPlotWidth": 10,
    "maxGridCells": 250000,
    "relationSummaryDiagnostics": false,
    "theme": {
      "foreground": "#e5e7eb",
      "background": "#111827",
      "line": "#94a3b8",
      "accent": "#60a5fa",
      "muted": "#9ca3af",
      "surface": "#1f2937",
      "border": "#475569"
    }
  },
  "layout": {
    "container_width": 1024,
    "container_height": 768
  },
  "environment": {
    "text_measurement": "vendored",
    "math_renderer": "none"
  },
  "resources": {
    "profile": "interactive",
    "limits": {
      "max_source_bytes": 2097152,
      "max_model_items": 32000,
      "max_model_text_bytes": 2097152,
      "max_model_nesting_depth": 256,
      "max_layout_work_units": 250000,
      "max_svg_elements": 250000,
      "max_svg_bytes": 25165824
    }
  },
  "lint": {
    "profile": "recommended",
    "enable_rules": [
      "merman.authoring.flowchart.explicit_direction"
    ],
    "disable_rules": [
      "merman.authoring.config.prefer_init_directive",
      "merman.git_graph.duplicate_commit_id"
    ],
    "rule_severities": [
      {
        "rule_id": "merman.block.width_exceeds_columns",
        "severity": "hint"
      }
    ]
  },
  "svg": {
    "diagram_id": "my-diagram",
    "pipeline": "parity",
    "scoped_css": ".node rect { stroke-width: 2px; }",
    "css_override_policy": "preserve",
    "root_background_color": "#0f172a",
    "drop_native_duplicate_fallbacks": false
  }
}
```

Every field is optional.

## Top-Level Fields

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `version` | integer | `1` | Options-schema version. Omit it for schema 1 compatibility; any supplied value other than `1` is rejected at the version boundary. |
| `runtime_policy` | string | `deterministic` | `deterministic` or `native`. The native policy is an explicit opt-in and fails with a typed missing-capability error unless the artifact contains the required system clock, time-zone, and random adapters. |
| `fixed_today` | string | selected policy date | Overrides the selected policy's local "today" date in `YYYY-MM-DD` format for time-dependent diagrams such as Gantt. The deterministic policy otherwise uses `1970-01-01`; the native policy reads the system date. |
| `fixed_local_offset_minutes` | integer | selected policy time zone | Replaces the selected policy's time-zone rules with one fixed offset in minutes. The deterministic policy otherwise uses UTC; the native policy uses discovered system time-zone rules. |
| `host_theme` | object | none | Opt-in host/editor theme profile compiled into Mermaid config and SVG output settings. |
| `site_config` | object | defaults | Mermaid site configuration merged onto the pinned Mermaid defaults before diagram directives are applied. |
| `parse` | object | defaults | Parse behavior. |
| `ascii` | object | defaults | ASCII/Unicode text rendering behavior. |
| `layout` | object | defaults | Per-request layout container dimensions. |
| `environment` | object | defaults | Public selection of operation-owned text measurement and optional math rendering. |
| `resources` | object | `interactive` | Source, layout-model, label, and SVG byte/cardinality budgets. |
| `lint` | object | none | Lint rule enable/disable and severity overrides shared across analysis consumers. |
| `svg` | object | defaults | SVG postprocessing behavior. |

## Runtime Policy

Omitting `runtime_policy` always selects `deterministic`, regardless of which system adapters were
compiled into the artifact. That policy uses Unix epoch time, UTC, a fixed operation seed, and no
timing instrumentation. This makes identical options reproducible across native artifacts, WASM,
and hosts with different local settings.

Set `"runtime_policy": "native"` only when the operation should consult the host clock, complete
system time-zone rules, and random source. Compiling system adapters makes the policy available; it
does not select it. An artifact missing any required adapter rejects the native policy at engine
creation with a typed missing-capability error instead of silently falling back to deterministic
state. Native timing instrumentation remains a separate explicit capability and is not enabled by
the native policy.

Generic binding operation metadata records the selected policy as
`"runtime_policy": "deterministic"` or `"runtime_policy": "native"`. Hosts should retain that
metadata with rendered or analyzed output when reproducibility matters.

## Fixed Time Options

`fixed_today` and `fixed_local_offset_minutes` are host-level deterministic controls for diagrams
whose semantics depend on local time. Gantt uses them for date parsing, relative fallback dates,
and render-model generation. They apply to parse JSON, layout JSON, SVG rendering, validation, and
ASCII render entry points that parse Mermaid source through the shared engine. These values
override the selected runtime policy; they do not implicitly switch a deterministic engine to the
native policy.

## Lint Options

`lint` controls shared analysis rule configuration for diagnostics-first consumers. It uses stable
rule ids from `merman-analysis`.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `lint.profile` | string | `core` | Built-in rule profile: `core`, `recommended`, or `strict`. `core` is conservative and does not enable Merman authoring recommendations. |
| `lint.enable_rules` | array of strings | none | Rule ids to enable even when their profile is not active. Entries must name configurable analysis rules. |
| `lint.disable_rules` | array of strings | none | Rule ids to disable. Entries must name configurable analysis rules. Unknown or internal ids return `MERMAN_INVALID_ARGUMENT`. |
| `lint.rule_severities` | array of objects | none | Per-rule severity overrides as `{ "rule_id": "...", "severity": "error|warning|info|hint" }`. `rule_id` must name a configurable analysis rule. |

`profile`, `enable_rules`, `disable_rules`, and `rule_severities` apply to source lint rules and
semantic warnings alike. They are validated against the public analysis rule registry and can be
used by FFI, UniFFI, WASM, CLI lint, and future editor adapters. `disable_rules` has the highest
precedence. Severity overrides do not enable a rule whose profile is inactive; use
`lint.profile = "recommended"` or `enable_rules` for Merman authoring recommendations.
Bindings expose the same rule registry through their lint-rule catalog metadata surfaces; hosts
should read that catalog when building settings UI instead of duplicating rule ids, evidence
references, and origins.

Only Merman rule ids from the lint-rule catalog are accepted here. External linter ids such as
markdownlint, remark, textlint, or `mermaid-lint` rules must stay in the host tool's own
configuration. For example, `mermaid-lint` rules such as `require-direction`, `duplicate-ids`, or
`no-empty-labels` should not be passed through `lint.enable_rules`, `lint.disable_rules`, or
`lint.rule_severities`. Adapters can convert Merman diagnostics outward into an external report
format, but they should not translate external rule ids into `lint.*` options unless Merman exposes
a distinct source-backed `merman.*` rule.

`analyzeDocument(source, options, uri)` uses this same options contract. The URI determines whether
the payload source is a standalone Mermaid diagram, Markdown, or MDX document; Markdown and MDX
diagnostics, related locations, and fixes are remapped to host-document coordinates. Use
`analyze()` for a single Mermaid diagram body and `analyzeDocument()` for lint integrations that
scan files or Markdown fences.

Rule governance is intentionally conservative because Merman is not the Mermaid project:

| Origin | Meaning | Default profile |
| --- | --- | --- |
| `mermaid_syntax` | Syntax or config behavior backed by Mermaid source/docs/fixtures. | `core` |
| `mermaid_compatibility` | Compatibility warnings backed by Mermaid source/docs/fixtures. | `core` |
| `merman_authoring` | Merman recommendations and safe editor assists, not official Mermaid standards. | `recommended` |
| `merman_resource_policy` | Host/runtime budget diagnostics. | `core` |
| `merman_internal` | Contract gaps and internal safety diagnostics. | not configurable |

Current authoring rule ids are `merman.authoring.config.prefer_init_directive`,
`merman.authoring.config.prefer_frontmatter_config`, and
`merman.authoring.flowchart.explicit_direction`.

`fixed_today` must be a `YYYY-MM-DD` date. `fixed_local_offset_minutes` must be an integer offset
accepted by the fixed-offset timezone model, currently `-1439` through `1439`. Invalid values return
`MERMAN_INVALID_ARGUMENT`.

## Site Config

`site_config` accepts the same Mermaid configuration object that Rust users pass through
`HeadlessRenderer::with_site_config(...)`. It is intended for host-level Mermaid defaults such as
theme selection, `themeVariables`, and Mermaid `themeCSS`:

```json
{
  "site_config": {
    "theme": "base",
    "themeVariables": {
      "mainBkg": "#111827",
      "nodeTextColor": "#f8fafc",
      "nodeBorder": "#38bdf8"
    },
    "themeCSS": ".node rect { filter: drop-shadow(1px 1px 1px #000); }"
  }
}
```

`site_config` must be a JSON object. Non-object values return `MERMAN_INVALID_ARGUMENT`. This option
does not apply host palette replacement or product-specific CSS postprocessing; use explicit host
postprocessing for editor-specific colors.

## Host Theme Profile

`host_theme` is an opt-in semantic profile for editor and application previews. It compiles host
roles into Mermaid-compatible `themeVariables`, selected diagram config defaults, and SVG
postprocessing options. Default rendering is unchanged when `host_theme` is omitted.

```json
{
  "host_theme": {
    "preset": "one-dark",
    "appearance": "dark",
    "font_family": "Inter, system-ui, sans-serif",
    "font_size": "14px",
    "roles": {
      "canvas": "#0f172a",
      "surface": "#111827",
      "surface_alt": "#1f2937",
      "text": "#e5e7eb",
      "subtle_text": "#cbd5e1",
      "border": "#475569",
      "line": "#94a3b8",
      "note_background": "#422006",
      "note_border": "#f59e0b",
      "success": "#34d399"
    },
    "series_palette": ["#60a5fa", "#34d399", "#f59e0b"],
    "theme_variables": {
      "nodeBorder": "#38bdf8"
    },
    "output": {
      "pipeline": "resvg-safe",
      "root_background": "canvas",
      "css_override_policy": "strip-existing-important",
      "drop_native_duplicate_fallbacks": false
    }
  }
}
```

`host_theme.appearance` accepts `light` or `dark`. `host_theme.output.pipeline` accepts `parity`,
`readable`, or `resvg-safe`. `host_theme.output.root_background` accepts `none`,
`canvas`, or a single CSS declaration value. `host_theme.output.drop_native_duplicate_fallbacks`
opts into removing fallback groups whose text duplicates native `<text>` after readable or
`resvg-safe` fallback generation. It is off by default because repeated labels can be intentional in
unrelated nodes. An empty `{ "host_theme": {} }` is a no-op and does not force Mermaid `theme=base`.

`host_theme.preset` accepts `editor-light`, `editor-dark`, `one-dark`, `gruvbox-light`,
`gruvbox-dark`, `ayu-light`, or `ayu-dark`. Explicit `roles`, `series_palette`,
`theme_variables`, `site_config`, and `output` fields override the preset. Host theme presets are
separate from Mermaid core theme names returned by `supported_themes`. Binding surfaces expose the
stable preset list through `supported_host_theme_presets` / `supportedHostThemePresets`-style
metadata helpers.

Merge precedence is Mermaid defaults, then `host_theme` derived config, then explicit
`host_theme.theme_variables` / `host_theme.site_config`, then top-level `site_config`, then diagram
directives. Explicit `svg.*` options override profile output options.

## Parse Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `parse.suppress_errors` | boolean | `false` | Enables lenient parse, render, and ASCII operations when true. It is not an analysis option. |

## Analysis Consumers

Diagnostics-first analysis, validation projection, CLI linting, Markdown/MDX scanning, and future
LSP adapters use the same `options_json` envelope. Analysis consumers honor options that affect
deterministic time, Mermaid site config, and resource limits while retaining family parse failures
in the closed analysis snapshot:

- `fixed_today` and `fixed_local_offset_minutes` for time-dependent diagram semantics;
- `site_config` and diagram directives for Mermaid-compatible parse/config behavior;
- `resources.*` for source and model budgets.

`parse.suppress_errors` is deliberately excluded from analysis. It remains a top-level shared
binding option for parse, render, and ASCII operations, and is rejected inside `analysis` or
`merman` analysis wrappers.

Render-only options such as `layout.*`, `svg.*`, and host text-measurement settings should not be
required for the default analyzer. Layout-backed or render-backed diagnostics may opt into those
fields later, but they must be profile-controlled and reported through the same diagnostic payload
defined by ADR 0070.

## ASCII Options

`ascii` applies to `render_ascii` and reusable engines that call ASCII rendering. These options do
not affect SVG, parse JSON, layout JSON, or validation output.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `ascii.charset` | string | `unicode` | `unicode` or `ascii`. |
| `ascii.default_direction` / `ascii.defaultDirection` | string | `leftRight` | `leftRight`/`left_right` or `topDown`/`top_down` for families that need a default terminal direction. |
| `ascii.color_mode` / `ascii.colorMode` | string | `plain` | `plain`, `truecolor`, or `html`. |
| `ascii.theme` | object | none | Terminal color palette with required `foreground` and `background` plus optional `line`, `accent`, `muted`, `surface`, and `border`. |
| `ascii.sequence_mirror_actors` / `ascii.sequenceMirrorActors` | boolean | `false` | Renders mirrored bottom participant boxes for sequence diagrams. |
| `ascii.xychart_vertical_plot_height` / `ascii.xychartVerticalPlotHeight` | positive integer | `5` | Compact vertical XYChart plot height. |
| `ascii.xychart_category_band_width` / `ascii.xychartCategoryBandWidth` | positive integer | `3` | Compact vertical XYChart category width. |
| `ascii.xychart_horizontal_plot_width` / `ascii.xychartHorizontalPlotWidth` | positive integer | `10` | Compact horizontal XYChart value axis width. |
| `ascii.max_grid_cells` / `ascii.maxGridCells` | positive integer | `250000` | Maximum terminal grid cells for graph-like ASCII layouts before fallback or error behavior. |
| `ascii.relation_summary_diagnostics` / `ascii.relationSummaryDiagnostics` | boolean | `false` | When true, Class/ER `relations:` fallback summaries include a `reason:` row such as `grid_budget actual=12 limit=1`, `crossing`, `route_collision`, or `overlay_collision`. |

`relationSummaryDiagnostics` is intentionally opt-in. Default text output stays stable and omits
internal fallback reasons; hosts can enable the field for support logs, diagnostics panels, or tests
that need to classify why a dense Class/ER relation layout used a summary.

## Layout Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `layout.container_width` | positive finite number | renderer default | Available width of the host layout container in CSS pixels. |
| `layout.container_height` | positive finite number | renderer default | Available height of the host layout container in CSS pixels. |

Container dimensions describe the element that owns diagram layout, not the browser page viewport
or the final SVG viewBox. The removed `layout.viewport_width` and `layout.viewport_height` names
are rejected; update requests rather than relying on an alias.

## Render Environment Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `environment.text_measurement` | string | `vendored` | `vendored`, `parity`, or `deterministic`. |
| `environment.math_renderer` | string | `none` | `none` or `ratex`. `ratex` requires the `math` feature. |

This is a breaking schema change: `layout.text_measurer` and `layout.math_renderer` are rejected.
Move them to `environment.text_measurement` and `environment.math_renderer`, respectively.

Root Viewport is an operation-owned rendering protocol, not a binding option. Diagram families
provide source-backed content bounds and family-specific root semantics; the shared protocol
normalizes dimensions, computes sizing and `viewBox`, emits root attributes and accessibility
chrome, and finalizes deferred roots. The public `environment` object configures only text
measurement and optional math rendering. Container dimensions belong under `layout.*`; SVG identity
and host-owned output postprocessing belong under `svg.*`. Unknown `environment` fields are rejected.

## Resource Options

`resources` controls render-wide deterministic budgets. These limits are separate from Cargo
features and from raster/PDF/image budgets. Cargo features decide which capabilities are compiled;
the resource profile bounds work inside an available semantic/SVG capability. PNG/JPG pixmap,
vector-PDF filter, embedded-image, and aggregate encoding budgets remain independent.

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `resources.profile` | string | `interactive` | `interactive`, `constrained`, `trusted-native`, or `unbounded-for-trusted-input`. |
| `resources.limits.max_source_bytes` | positive integer | profile value | Source bytes checked before parse/render work. |
| `resources.limits.max_model_items` | positive integer | profile value | Aggregate semantic entities and relationships across every diagram family. |
| `resources.limits.max_model_text_bytes` | positive integer | profile value | Aggregate UTF-8 text retained by the typed semantic model. |
| `resources.limits.max_model_nesting_depth` | positive integer | profile value | Maximum semantic nesting depth before layout. |
| `resources.limits.max_layout_work_units` | positive integer | profile value | Deterministic family-accounted derived geometry and layout candidate work. |
| `resources.limits.max_svg_bytes` | positive integer | profile value | SVG bytes checked after emission and after postprocessing. |
| `resources.limits.max_svg_elements` | positive integer | profile value | SVG element cardinality checked before recursive postprocessing. |

The seven public limits are intentionally family-neutral. Each family performs source-backed,
deterministic accounting for its own nodes, relationships, nesting, synthesized geometry, and
candidate scans, then charges those values to the shared model and layout budgets. Hosts therefore
choose a workload profile instead of maintaining diagram-specific threshold tables.

`interactive` is the default for binding surfaces. `constrained` is tighter and is enforced by
the Typst plugin for every call; caller-provided `resources` values are replaced at that transport
boundary. `trusted-native` is intended for CLI or
controlled batch rendering. `unbounded-for-trusted-input` is an explicit opt-out for trusted inputs,
not a browser or server default.

The profile is a resource-policy choice, not a promise of isolation. `interactive` assumes a
cooperative author and is suitable for an editor or local preview; it does not stop a hostile or
multi-tenant caller from consuming CPU after admission. A public service should select
`constrained` and additionally enforce a wall-clock timeout, memory limit, concurrency quota, and
preemption or process isolation at the host boundary. `trusted-native` is for a controlled CLI or
batch job. `unbounded-for-trusted-input` must only be used inside an outer trusted sandbox.

### Profile Decision Table

| Workload | Profile | Required host controls | Output guidance |
| --- | --- | --- | --- |
| Browser editor/Playground preview | `interactive` | Abort stale requests; cap concurrent renders | `parity` SVG for browser display |
| Public upload or multi-tenant API | `constrained` | Timeout, memory, concurrency, and preemption/isolation | `resvg-safe` before raster/PDF conversion |
| Local `merman-cli` export | `trusted-native` | Process-level cancellation for batch automation | Choose `parity` or `resvg-safe` per consumer |
| Typst package transport | `constrained` | Typst host remains responsible for process limits | Package-owned SVG contract |
| Large, fully trusted offline export | `unbounded-for-trusted-input` | Outer process/container isolation and explicit output quotas | Caller owns final output limits |

These defaults are engineering admission baselines, not latency or memory SLOs. Hosts should
measure their own diagrams and set explicit overrides only after observing peak memory and timeout
behavior. The SVG backend also enforces an internal tree-depth capability; it is not a public
override because increasing it would not make the backend stack-safe.

### Calibration Evidence

Resource-policy changes require a reproducible stress run and a record of the observed boundary:

```sh
cargo bench -p merman --bench flowchart_stress
cargo bench -p merman --bench mindmap_layout_stress
cargo nextest run -p merman-render -p merman-bindings-core
```

For each changed budget, record the fixture/source hash, profile, explicit overrides, host target,
peak RSS (or WASM linear memory), timeout, successful output size, and the first rejected cardinality.
Do not infer a safe limit from a single warm render: compare cold parse, layout, SVG postprocess, and
failure paths separately. The benchmark methodology documents the phase boundaries and evidence
format used by the Playground and comparison tools.

Limit ids are closed under resource-contract schema `1`: an unknown id, zero value, or removed flat
or family-specific field is rejected.
The runtime contract publishes every accepted id, its phase, whether it is overridable, and the
exact value or `null` for every profile. This avoids copying profile values into host libraries.

## Runtime Contract Discovery

Query the loaded artifact rather than inferring capabilities or resource values from a package
name. Runtime-contract schema `1` includes the transport API version, package and payload schema
versions, compiled capability and output IDs, complete language-catalog facts, plus the resource
descriptor for every compiled resource-aware operation. Render, analysis, and ASCII artifacts
expose only the limit ids their operations can enforce; an artifact with none of those operations
returns `resources: null`.

| Surface | API |
| --- | --- |
| C | `MermanNativeApi.runtime_catalog()` after `merman_get_native_api()` |
| Android/Kotlin | `MermanEngine.runtimeCatalogJson()` |
| Apple/Swift | `MermanEngine().runtimeCatalogJson()` |
| Flutter/Dart | `Merman.open().runtimeCatalog` |
| UniFFI/Python | `MermanEngine.runtime_catalog_json()` / `merman.get_runtime_catalog(engine)` |
| Web/TypeScript | `runtimeCatalog()` |

The runtime-contract schema is independent of native ABI `3`, UniFFI binding API `3`, and payload
schema numbers. Reject a contract schema newer than the host understands before interpreting its
nested fields. The atomic runtime catalog carries the vocabulary beside the contract; validate it
before trusting the contract's capability IDs, so its descriptor-owned direct implications do not
need to be copied into each host package.

## SVG Options

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `svg.diagram_id` | string | renderer default | Overrides the root SVG diagram id. |
| `svg.pipeline` | string | `parity` | `parity`, `readable`, or `resvg-safe`. |
| `svg.scoped_css` | string | none | Host-owned CSS injected after Mermaid CSS and scoped to the root SVG id. |
| `svg.css_override_policy` | string | `preserve` | `preserve` or `strip-existing-important`. Controls whether existing Mermaid `!important` flags are stripped before host CSS is applied, and can override `host_theme.output.css_override_policy`. |
| `svg.root_background_color` | string | none | Host-owned root `<svg>` inline `background-color` replacement. |
| `svg.drop_native_duplicate_fallbacks` | boolean | `false` | Adds generic duplicate fallback cleanup after readable or `resvg-safe` fallback generation. `resvg-safe` already removes generated fallback groups for native SVG `<switch>` text fallbacks, and this option covers additional native/fallback duplicate surfaces. |

`readable` keeps a more inspectable SVG structure. `resvg-safe` rewrites SVG output toward stricter
renderer compatibility, including structural cleanup for labels that already include native SVG
`<switch>` text fallbacks. `drop_native_duplicate_fallbacks` remains an explicit host choice for
additional native/fallback duplicate surfaces, including hosts that already request `resvg-safe`.
Its generic text matching should be treated as an opt-in postprocessing policy. HTML label fallback
text inherits Mermaid label/root fill colors when
available, so dark host profiles do not fall back to unreadable legacy text colors.

`svg.pipeline` also selects the output contract. The default `parity` value intentionally preserves
Mermaid-compatible SVG and can include `<foreignObject>` HTML labels. Hosts that need to feed SVG
bytes into strict SVG renderers, rasterizers, or PDF converters should request `resvg-safe`
explicitly instead of treating the default SVG as export-safe input.

`svg.scoped_css` is for host-owned styling, not Mermaid parity CSS. Selectors are scoped to the
root SVG id and injected after Mermaid's styles so host rules have normal cascade priority. When
`svg.pipeline` is `resvg-safe`, merman sanitizes the injected CSS after insertion to preserve the
raster-safe contract as far as the built-in sanitizer can. Hosts still own CSS trust, palette
semantics, and renderer-specific compatibility.

`svg.root_background_color` is narrower than host CSS. It rewrites the root `<svg>` inline
`background-color` value, or adds one when missing. This is useful for editor previews that need the
diagram canvas to match the host surface. The value must be a single CSS declaration value; use
`"transparent"` when the host wants no opaque root background.

## Examples

Readable SVG with a stable id:

```json
{
  "svg": {
    "diagram_id": "docs-flow",
    "pipeline": "readable"
  }
}
```

External Mermaid theme defaults for plain source:

```json
{
  "site_config": {
    "theme": "base",
    "themeVariables": {
      "mainBkg": "#111827",
      "nodeTextColor": "#f8fafc"
    }
  },
  "svg": {
    "diagram_id": "host-preview"
  }
}
```

Readable SVG with generic duplicate native/fallback labels removed:

```json
{
  "svg": {
    "pipeline": "readable",
    "drop_native_duplicate_fallbacks": true
  }
}
```

Resvg-safe SVG with host-scoped CSS:

```json
{
  "svg": {
    "pipeline": "resvg-safe",
    "diagram_id": "host-preview",
    "scoped_css": ".node rect { fill: #111827; } .merman-foreignobject-fallback-text { fill: #f8fafc; }",
    "css_override_policy": "strip-existing-important"
  }
}
```

Resvg-safe SVG with a host-owned canvas color:

```json
{
  "svg": {
    "pipeline": "resvg-safe",
    "diagram_id": "host-preview",
    "root_background_color": "#0f172a"
  }
}
```

Deterministic layout for tests:

```json
{
  "runtime_policy": "deterministic",
  "fixed_today": "2026-02-15",
  "fixed_local_offset_minutes": 0,
  "environment": {
    "text_measurement": "deterministic"
  },
  "layout": {
    "container_width": 1024,
    "container_height": 768
  }
}
```

Lenient parsing:

```json
{
  "parse": {
    "suppress_errors": true
  }
}
```

Strict resource profile override:

```json
{
  "resources": {
    "profile": "constrained",
    "limits": {
      "max_model_items": 500
    }
  }
}
```

## Error Behavior

Invalid options produce binding errors:

| Error | Native ABI 3 status |
| --- | --- |
| Invalid UTF-8 | `MERMAN_NATIVE_STATUS_UTF8_ERROR` |
| Invalid JSON | `MERMAN_NATIVE_STATUS_OPTIONS_JSON_ERROR` |
| Unsupported option value | `MERMAN_NATIVE_STATUS_INVALID_ARGUMENT` |
| Feature-gated operation disabled | `MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION` |
| Resource budget exceeded | `MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED` |

Platform wrappers surface those errors through their native exception type:

- C ABI: non-zero `MermanNativeStatus`, mirrored in `MermanNativeResult.status`, with the structured
  JSON error payload in `MermanNativeResult.metadata_or_error_json`.
- Android: `MermanException`.
- Apple: `MermanError.binding`.
- Flutter/Dart: `MermanException`.
- Python UniFFI: `MermanError.Binding`.

## Typed Wrapper Follow-On

The stable low-level contract remains JSON so the C ABI does not grow for every option. Generated
typed builders now sit above that contract and are produced from the Rust resource descriptor:

| Platform | Generated API |
| --- | --- |
| C | `MermanResourceProfile`, `MermanResourceLimitId`, `MermanResourceLimitOverride` and `merman_resource_options_json()` |
| Android/Kotlin | `MermanResourceOptionsBuilder` / `MermanResourceOptions` |
| Apple/Swift | generated `resourceOptionsJson(profile:overrides:)` |
| Flutter/Dart | `MermanResourceOptionsBuilder` / `MermanResourceOptions` |
| Python/UniFFI | `ResourceOptionsBuilder` / `ResourceOptions` |
| Web/TypeScript | closed `ResourceProfile`/`ResourceLimitId` unions and `resourceOptions()`; use `rawResourceOptionsJson()` only for an explicitly external contract |

Builders validate profile and overridable limit ids before serialization. They do not duplicate the
budget table; hosts should query the runtime contract when presenting values or settings UI.
