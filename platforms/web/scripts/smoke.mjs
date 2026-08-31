import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";
import { parseSmokeCli, smokeUsage } from "./smoke-cli.mjs";
import {
  allPackageRuntimeExportNames,
  allPackageValueExportNames,
  webPackages,
} from "./surface-manifest.mjs";
import { assertRuntimeOwnerEvidence } from "./wasm-build/runtime-evidence.mjs";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.join(packageRoot, "..", "..");
const args = process.argv.slice(2);
const fullPackage = webPackages.find((descriptor) => descriptor.id === "full");
if (!fullPackage) {
  throw new Error("package manifest is missing the full package");
}
const editorRuntimeExports = fullPackage.runtimeExportNames.filter((name) =>
  name.startsWith("editor")
);

const packageSmokeCases = webPackages;

if (args.length === 0) {
  await runPureDistSmoke();
  for (const descriptor of packageSmokeCases) {
    const result = spawnSync(
      process.execPath,
      [
        fileURLToPath(import.meta.url),
        "--package-id",
        descriptor.id,
      ],
      {
        cwd: packageRoot,
        stdio: "inherit",
      }
    );
    if (result.error) {
      console.error(
        `@mermanjs/web smoke failed to spawn ${descriptor.id}: ${result.error.message}`
      );
      process.exit(1);
    }
    if (result.status !== 0) {
      process.exit(result.status ?? 1);
    }
  }
  await runSameProcessPackageSmoke();
  console.log(
    `@mermanjs/web smoke matrix passed packages=${packageSmokeCases
      .map((descriptor) => descriptor.id)
      .join(",")}`
  );
  process.exit(0);
}

const {
  packageId,
} = parseCli(args);
const packageDescriptor = packageDescriptorForId(packageId);
const browserPackageRoot = path.join(packageRoot, packageDescriptor.package_dir);

const packageApi = await import(packageEntryUrl(packageDescriptor));
const textMeasurementAbi = await import(sharedDistUrl("generated/text-measurement-abi.js"));
const exportedWasmModule = await import(
  pathToFileURL(path.join(browserPackageRoot, "artifacts", "wasm", "merman_wasm.js")).href,
);
const surfaceContract = packageContract(packageDescriptor);
const api = projectInternalApiForSurface(
  await import(sharedDistUrl("index.js")),
  surfaceContract,
);
const cytoscapeLayoutDiagramTypes = new Set(["architecture", "mindmap"]);

assert.equal(typeof exportedWasmModule.default, "function");
assertSurfaceExports(packageApi, surfaceContract);
const wasmBinary = await readFile(path.join(browserPackageRoot, "artifacts", "wasm", "merman_wasm_bg.wasm"));
let customLoaderCalled = false;
const initializeBrowserPackage = () =>
  packageApi.initMerman({
    loader: async () => {
      customLoaderCalled = true;
      return exportedWasmModule;
    },
    wasm: wasmBinary,
  });
assert.throws(initializeBrowserPackage, /browser main-thread or Web Worker realm/);
assert.equal(customLoaderCalled, false, "Node must not invoke a custom browser-package loader");
await withNodeDomShim(() => {
  assert.throws(initializeBrowserPackage, /browser main-thread or Web Worker realm/);
});
assert.equal(customLoaderCalled, false, "Node DOM shims must not invoke a browser-package loader");
await assertNoDeprecatedWasmBindgenInitWarning(() =>
  api.initMerman({ loader: async () => exportedWasmModule, wasm: wasmBinary })
);

const source = "flowchart TD\nA[Hello] --> B[World]";
const deterministicTime = {
  fixed_today: "2026-06-10",
  fixed_local_offset_minutes: 0,
};
const options = {
  ...deterministicTime,
  svg: { pipeline: "readable" },
  environment: { text_measurement: "deterministic" },
};
const hostTextMeasurementOptions = {
  ...deterministicTime,
  svg: { pipeline: "readable" },
};
const presetManifest = JSON.parse(
  await readFile(path.join(browserPackageRoot, "artifacts", "provenance.json"), "utf8")
);

class FakeMeasureElement {
  constructor(tagName = "div", canvasContext = null) {
    this.tagName = tagName;
    this.canvasContext = canvasContext;
  }

  style = {};
  attributes = new Map();
  children = [];
  _textContent = "";

  get textContent() {
    if (this.children.length > 0) {
      return this.children.map((child) => child.textContent || "").join("");
    }
    return this._textContent;
  }

  set textContent(value) {
    this._textContent = value || "";
    this.children = [];
  }

  setAttribute(name, value) {
    this.attributes.set(name, String(value));
  }

  removeAttribute(name) {
    this.attributes.delete(name);
  }

  getAttributeNames() {
    return [...this.attributes.keys()];
  }

  inheritedAttribute(name) {
    return this.attributes.get(name) ?? this.parentElement?.inheritedAttribute?.(name);
  }

  appendChild(child) {
    child.parentElement = this;
    this.children.push(child);
    return child;
  }

  replaceChildren(...children) {
    this.children = [];
    for (const child of children) {
      this.appendChild(child);
    }
  }

  remove() {
    this.removed = true;
  }

  getContext(kind) {
    assert.equal(this.tagName, "canvas");
    assert.equal(kind, "2d");
    return this.canvasContext;
  }

  effectiveFontSize() {
    return (
      parseFloat(this.style.fontSize) ||
      parseFloat(this.parentElement?.style?.fontSize) ||
      16
    );
  }

  getBBox() {
    const fontSize = this.effectiveFontSize();
    const rows =
      this.children.filter((child) => child.tagName === "tspan").map((child) => child.textContent) ||
      [];
    const lines = rows.length > 0 ? rows : [this.textContent || ""];
    const width = Math.max(...lines.map((line) => line.length * fontSize * 0.6), 0);
    const height = Math.max(1, lines.length) * fontSize * 1.1;
    const anchor = this.inheritedAttribute("text-anchor");
    const middleBaseline = this.inheritedAttribute("dominant-baseline") === "middle";
    return {
      x: anchor === "middle" ? -width / 2 : 0,
      y: -fontSize + (middleBaseline ? fontSize * 0.25 : 0),
      width,
      height,
    };
  }

  getComputedTextLength() {
    return (this.textContent || "").length * this.effectiveFontSize() * 0.4;
  }

  getBoundingClientRect() {
    const fontSize = this.effectiveFontSize();
    const lineHeight = parseFloat(this.style.lineHeight) || fontSize;
    const naturalWidth = (this.textContent || "").length * fontSize * 0.5;
    const fixedWidth =
      typeof this.style.width === "string" && this.style.width.endsWith("px")
        ? parseFloat(this.style.width)
        : null;
    const width =
      fixedWidth !== null && Number.isFinite(fixedWidth)
        ? fixedWidth
        : naturalWidth;
    const lineCount =
      fixedWidth !== null && fixedWidth > 0
        ? Math.max(1, Math.ceil(naturalWidth / fixedWidth))
        : 1;
    return {
      width,
      height: lineHeight * lineCount,
    };
  }
}

assert.equal(api.isMermanInitialized(), true);
assert.ok(Number.isSafeInteger(api.transportApiVersion()));
assert.ok(api.transportApiVersion() > 0);
assert.equal(
  api.MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION,
  textMeasurementAbi.MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION
);
assert.equal(Object.isFrozen(api.UNAVAILABLE_DIAGRAM_DETECTION), true);
assert.match(api.packageVersion(), /^\d+\.\d+\.\d+/);
const runtimeCatalog = api.runtimeCatalog();
const presentationCatalog = api.presentationCatalog();
const capabilities = runtimeCatalog.capabilities;
const hasCapability = (id) => capabilities.capability_ids.includes(id);
const completeCytoscapeRenderSurface = hasCapability("layout-cytoscape");
if (hasCapability("svg")) {
  assert.equal(typeof api.renderSvgWithTextMeasurer, "function");
  assert.equal(typeof api.layoutJsonWithTextMeasurer, "function");
  assert.equal(typeof api.createBrowserTextMeasurementSession, "function");
  assert.equal(typeof api.createBrowserTextMeasurer, "undefined");
  const unavailableSession = api.createBrowserTextMeasurementSession();
  assert.equal(unavailableSession.measure({ text: "Node", font_size: 16 }), undefined);
  unavailableSession.dispose();
  unavailableSession.dispose();
  assert.equal(unavailableSession.measure({ text: "Node", font_size: 16 }), undefined);
  withFakeMeasureDom(() => {
    const measurementSession = api.createBrowserTextMeasurementSession();
    const browserMeasurer = measurementSession.measure;
    const shortLabel = browserMeasurer(textMeasureRequest("Condition?", 200));
    assert.equal(shortLabel.kind, "metrics");
    assert.ok(shortLabel.width > 0);
    assert.ok(
      shortLabel.width < 200,
      `short max-width labels should use natural width, got ${shortLabel.width}`
    );

    const longLabel = browserMeasurer(
      textMeasureRequest("Condition ".repeat(40), 200)
    );
    assert.equal(longLabel.width, 200);
    assert.ok(longLabel.line_count > 1);
    assert.ok(longLabel.raw_width === undefined);

    const computed = browserMeasurer(
      textMeasureRequest("Computed", null, "computed-length", "svg-like")
    );
    assert.equal(computed.kind, "length");
    assert.equal(computed.length, "Computed".length * 16 * 0.4);

    const rawBBox = browserMeasurer(
      textMeasureRequest("Raw", null, "raw-bbox-width", "svg-like")
    );
    assert.equal(rawBBox.kind, "length");
    assert.equal(rawBBox.length, "Raw".length * 16 * 0.6);

    const clientRect = browserMeasurer(
      textMeasureRequest("Client", null, "bounding-client-rect-width", "svg-like")
    );
    assert.equal(clientRect.kind, "length");
    assert.equal(clientRect.length, "Client".length * 16 * 0.5);

    const rawCreateTextYOffset = browserMeasurer(
      textMeasureRequest("Formatted", null, "create-text-bbox-y-offset", "svg-like")
    );
    const middleCreateTextYOffset = browserMeasurer(
      textMeasureRequest("Formatted", null, "create-text-middle-bbox-y-offset", "svg-like")
    );
    assert.equal(rawCreateTextYOffset.length, -16);
    assert.equal(middleCreateTextYOffset.length, -12);
    assert.equal(
      browserMeasurer(
        textMeasureRequest("Formatted", null, "create-text-bbox-y-offset", "svg-like")
      ).length,
      -16
    );

    const mermaidDimensions = browserMeasurer(
      textMeasureRequest(
        "Mermaid dimensions",
        null,
        "mermaid-calculate-text-dimensions",
        "svg-like"
      )
    );
    assert.equal(mermaidDimensions.kind, "metrics");
    assert.equal(mermaidDimensions.line_count, 1);
    assert.ok(mermaidDimensions.width > 0);
    assert.ok(mermaidDimensions.height > 0);

    const canvasWidth = browserMeasurer(
      textMeasureRequest("Canvas", null, "canvas-measure-text-width", "svg-like")
    );
    assert.equal(canvasWidth.kind, "length");
    assert.equal(canvasWidth.length, "Canvas".length * 16 * 0.55);

    const extents = browserMeasurer(
      textMeasureRequest("Centered", null, "bbox-x", "svg-like")
    );
    assert.equal(extents.bbox_left, extents.bbox_right);
    assert.ok(extents.bbox_left > 0);

    const wrappedWithRaw = browserMeasurer(
      textMeasureRequest(
        "Condition ".repeat(40),
        200,
        "wrapped-with-raw-width",
        "html-like"
      )
    );
    assert.equal(wrappedWithRaw.width, 200);
    assert.ok(wrappedWithRaw.raw_width > wrappedWithRaw.width);

    const svgWrapped = browserMeasurer(
      textMeasureRequest("one two three four five", 60, "wrapped", "svg-like")
    );
    assert.ok(svgWrapped.line_count > 1);
    measurementSession.dispose();
    measurementSession.dispose();
    assert.equal(
      browserMeasurer(textMeasureRequest("Disposed", null, "measure", "svg-like")),
      undefined
    );
  });
}

assert.equal(runtimeCatalog.schema_version, 1);
assert.equal(runtimeCatalog.transport_api_version, api.transportApiVersion());
assert.equal(runtimeCatalog.package_version, api.packageVersion());
assert.deepEqual(runtimeCatalog.capabilities, capabilities);
assert.equal(
  runtimeCatalog.registry.diagram_family_count,
  api.diagramFamilyCapabilities().length
);
assert.equal(
  runtimeCatalog.resources.general_binding_default_profile,
  "interactive"
);
assert.equal(runtimeCatalog.resources.cli_default_profile, "trusted-native");
assert.ok(runtimeCatalog.metadata_ids.includes("presentation-catalog"));
assert.equal(presentationCatalog.schema_version, 1);
if (hasCapability("svg")) {
  assert.deepEqual(
    presentationCatalog.theme_presets.map(({ id }) => id),
    [...api.BUNDLED_THEME_PRESETS],
  );
  const modernProfile = presentationCatalog.profiles.find(
    ({ id }) => id === "merman-modern",
  );
  assert.ok(modernProfile);
  assert.equal(modernProfile.fully_available, hasCapability("layout-elk"));
  assert.equal(
    modernProfile.aspects.find(({ id }) => id === "flowchart-elk-default")
      ?.available,
    hasCapability("layout-elk"),
  );
} else {
  assert.deepEqual(presentationCatalog.theme_presets, []);
  assert.deepEqual(presentationCatalog.profiles, []);
}
const resourceLimitIds = runtimeCatalog.resources.limits
  .map((limit) => limit.id)
  .sort();
const generatedAsciiResourceLimitIds = api.RESOURCE_LIMIT_IDS
  .filter((id) => id.startsWith("max_ascii_"));
assert.deepEqual(generatedAsciiResourceLimitIds, [
  "max_ascii_grid_cells",
  "max_ascii_layout_work_units",
  "max_ascii_document_cells",
  "max_ascii_output_bytes",
  "max_ascii_grapheme_bytes",
  "max_ascii_nesting_depth",
]);
const expectedResourceLimitIds = [
  ...(hasCapability("ascii") ? generatedAsciiResourceLimitIds : []),
  ...(hasCapability("analysis") ? ["max_document_diagrams"] : []),
  ...(hasCapability("svg") ? ["max_layout_work_units"] : []),
  "max_model_items",
  "max_model_nesting_depth",
  "max_model_text_bytes",
  "max_source_bytes",
  ...(hasCapability("svg")
    ? [
        "max_svg_bytes",
        "max_svg_elements",
        "svg_backend_tree_depth",
        "svg_backend_tree_nodes",
      ]
    : []),
].sort();
assert.deepEqual(resourceLimitIds, expectedResourceLimitIds);
assert.equal(Object.isFrozen(api.RESOURCE_PROFILES), true);
assert.equal(Object.isFrozen(api.RESOURCE_LIMIT_IDS), true);
assert.equal(Object.isFrozen(api.RESOURCE_OVERRIDE_IDS), true);
assert.equal(Object.isFrozen(api.RESOURCE_LIMIT_METADATA), true);
for (const metadata of Object.values(api.RESOURCE_LIMIT_METADATA)) {
  assert.equal(Object.isFrozen(metadata), true);
}
assert.throws(
  () => api.RESOURCE_PROFILES.push("future-profile"),
  TypeError,
);
assert.throws(
  () => api.RESOURCE_OVERRIDE_IDS.push("future-limit"),
  TypeError,
);
assert.equal(api.isKnownResourceLimitId("max_source_bytes"), true);
assert.equal(api.isKnownResourceLimitId("future-limit"), false);
assert.equal(
  api.resourceLimitMetadata("max_source_bytes")?.id,
  "max_source_bytes",
);
assert.equal(api.resourceLimitMetadata("future-limit"), undefined);
assert.throws(
  () => api.resourceOptions("future-profile"),
  /unsupported resource profile/,
);
assert.throws(
  () => api.resourceOptions(undefined, { "future-limit": 1 }),
  /resource limit is not overridable/,
);
assertRuntimeOwnerEvidence(capabilities, {
  runtime_capability_ids: presetManifest.runtime_capability_ids,
  runtime_output_ids: presetManifest.outputs,
});
assert.ok(
  capabilities.output_ids.every((outputId) =>
    capabilities.operation_ids.includes(outputId)
  )
);
assert.ok(
  capabilities.system_adapter_ids.every((adapterId) =>
    capabilities.capability_ids.includes(adapterId)
  )
);
assert.deepEqual(capabilities.system_adapter_ids, []);
if (hasCapability("svg")) {
  assert.deepEqual(capabilities.text_measurement, {
    protocol_version: textMeasurementAbi.MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION,
    provider_ids: ["deterministic", "host-callback"],
  });
} else {
  assert.equal(capabilities.text_measurement, null);
}

const familyCapabilities = api.diagramFamilyCapabilities();
assert.equal(Array.isArray(familyCapabilities), true);
assert.equal(
  familyCapabilities.some(
    (capability) =>
      capability.diagram_type === "flowchart" &&
      capability.logical_family_kind === "flowchart" &&
      capability.metadata_id === "flowchart" &&
      capability.render_model_kind === "flowchart" &&
      capability.has_detector &&
      capability.has_semantic_parser &&
      capability.has_editor_parser &&
      capability.has_combined_parser &&
      capability.has_render_parser &&
      !capability.has_header &&
      capability.config_namespace === "flowchart"
  ),
  true
);

if (hasCapability("analysis")) {
  const lintRules = api.lintRuleCatalog();
  assert.equal(Array.isArray(lintRules), true);
  const rawLintRuleCatalog = exportedWasmModule.lintRuleCatalog();
  assert.equal(rawLintRuleCatalog.version, 1);
  assert.equal(Array.isArray(rawLintRuleCatalog.rules), true);
  assert.equal(rawLintRuleCatalog.rules.length, lintRules.length);
  assert.equal(
    lintRules.some(
      (rule) =>
        rule.id === "merman.authoring.flowchart.explicit_direction" &&
        rule.default_severity === "hint" &&
        rule.origin === "merman_authoring" &&
        rule.evidence.includes("docs/adr/0072-lint-rule-governance.md") &&
        rule.configurable &&
        rule.fixable
    ),
    true
  );
  const deprecatedRule = lintRules.find(
    (rule) =>
      rule.id === "merman.compatibility.config.deprecated_flowchart_html_labels"
  );
  assert.ok(deprecatedRule);
  assert.deepEqual(deprecatedRule.tags, ["deprecated"]);
  assert.deepEqual(
    rawLintRuleCatalog.rules.find(
      (rule) =>
        rule.id === "merman.compatibility.config.deprecated_flowchart_html_labels"
    )?.tags,
    ["deprecated"]
  );
  deprecatedRule.tags.push("mutated-copy");
  assert.deepEqual(
    api
      .lintRuleCatalog()
      .find(
        (rule) =>
          rule.id === "merman.compatibility.config.deprecated_flowchart_html_labels"
      )?.tags,
    ["deprecated"]
  );

  const deprecatedAnalysis = api.analyze(
    '%%{init: { "flowchart": { "htmlLabels": false } }}%%\nflowchart TD\nA-->B\n',
    deterministicTime
  );
  assert.deepEqual(deprecatedAnalysis.diagnostics[0].tags, ["deprecated"]);

  const markdownAnalysis = api.analyzeDocument(
    "before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
    "file:///tmp/example.md",
    deterministicTime
  );
  assert.equal(markdownAnalysis.valid, false);
  assert.equal(markdownAnalysis.source.kind, "markdown");
  assert.equal(markdownAnalysis.diagnostics[0].span.line, 4);
  assert.equal(
    markdownAnalysis.diagnostics[0].related.some(
      (related) => related.message === "Mermaid fence 1"
    ),
    true
  );

  const flowchartFacts = api.analysisFacts("flowchart TD\nA-->B\n", deterministicTime);
  assert.equal(flowchartFacts.version, 2);
  assert.equal(flowchartFacts.valid, true);
  assert.equal(flowchartFacts.diagrams[0].syntax.fact_source, "parser_complete");
  assert.equal(flowchartFacts.diagrams[0].syntax.source_mapped_spans, true);
  assert.equal(flowchartFacts.diagrams[0].syntax.effective_layout, "dagre");
  assert.equal(
    flowchartFacts.diagrams[0].syntax.semantic_items.some(
      (item) => item.name === "A" && item.span.document
    ),
    true
  );

  assert.deepEqual(api.detectDiagramFacts("flowchart TD\nA-->B\n", deterministicTime), {
    status: "available",
    validity: "valid",
    diagramType: "flowchart",
    syntaxId: "flowchart-v2",
    effectiveLayoutId: "dagre",
  });
  assert.deepEqual(
    api.detectDiagramFacts("classDiagram\nclass A\n", {
      ...deterministicTime,
      site_config: { layout: "elk" },
    }),
    {
      status: "available",
      validity: "valid",
      diagramType: "class",
      syntaxId: "classDiagram",
      effectiveLayoutId: "elk",
    }
  );
  const unavailableDetection = {
    status: "unavailable",
    validity: "unknown",
    diagramType: null,
    syntaxId: null,
    effectiveLayoutId: null,
  };
  assert.deepEqual(api.detectDiagramFacts("", deterministicTime), unavailableDetection);
  assert.deepEqual(
    api.detectDiagramFacts("unknownDiagram\nA-->B\n", deterministicTime),
    unavailableDetection
  );
  assert.deepEqual(
    api.detectDiagramFacts("flowchart TD\nA[unterminated\n", deterministicTime),
    {
      status: "available",
      validity: "recoverable-invalid",
      diagramType: "flowchart",
      syntaxId: "flowchart-v2",
      effectiveLayoutId: "dagre",
    }
  );
  assert.deepEqual(api.detectDiagramFacts("flowchart-elk TD\nA-->B\n", deterministicTime), {
    status: "available",
    validity: "valid",
    diagramType: "flowchart",
    syntaxId: "flowchart-elk",
    effectiveLayoutId: "elk",
  });

  const mappedSequenceFacts = api.analysisFacts(
    [
      "---",
      "title: quoted",
      "---",
      "sequenceDiagram",
      "participant Alice",
      "Alice->>Bob: #quot;",
      "",
    ].join("\n"),
    deterministicTime
  );
  assert.equal(mappedSequenceFacts.valid, true);
  assert.equal(
    mappedSequenceFacts.diagrams[0].syntax.fact_source,
    "parser_complete"
  );
  assert.equal(mappedSequenceFacts.diagrams[0].syntax.source_mapped_spans, true);

  const markdownFacts = api.analyzeDocumentFacts(
    "before\n```mermaid\nflowchart TD\nA@{\n  shape: rou\n}\n```\nafter\n",
    "file:///tmp/example.md",
    deterministicTime
  );
  assert.equal(markdownFacts.valid, false);
  assert.equal(markdownFacts.source.kind, "markdown");
  assert.equal(markdownFacts.diagrams[0].source_id, "mermaid-fence-1");
  assert.equal(markdownFacts.diagrams[0].syntax.parser_backed, true);
  assert.equal(
    markdownFacts.diagrams[0].syntax.expected_syntax.some(
      (expected) => expected.kind === "shape" && expected.span.document
    ),
    true
  );

  const mdxAnalysis = api.analyzeDocument(
    "before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
    "file:///tmp/example.mdx?rev=1#fence",
    deterministicTime
  );
  assert.equal(mdxAnalysis.valid, false);
  assert.equal(mdxAnalysis.source.kind, "mdx");
  assert.equal(mdxAnalysis.source.language, "mdx");
  assert.equal(mdxAnalysis.source.path, "file:///tmp/example.mdx?rev=1#fence");
  assert.equal(mdxAnalysis.diagnostics[0].span.line, 4);

  const markdownFixAnalysis = api.analyzeDocument(
    '```mermaid\n%%{ initialize: {"theme":"dark"} }%%\nflowchart TD\nA-->B\n```\n',
    "file:///tmp/example.md",
    {
      ...deterministicTime,
      lint: { profile: "recommended" },
    }
  );
  const configFixDiagnostic = markdownFixAnalysis.diagnostics.find(
    (diagnostic) =>
      diagnostic.category === "config" &&
      (diagnostic.fixes ?? []).some((fix) => fix.edits.length > 0)
  );
  assert.ok(configFixDiagnostic);
  assert.equal(configFixDiagnostic.fixes[0].edits[0].span.line, 2);
} else {
  assert.equal(typeof api.analyze, "undefined");
  assert.equal(typeof api.analyzeJson, "undefined");
  assert.equal(typeof api.analysisFacts, "undefined");
  assert.equal(typeof api.detectDiagramFacts, "undefined");
  assert.equal(typeof api.analyzeDocument, "undefined");
  assert.equal(typeof api.analyzeDocumentFacts, "undefined");
  assert.equal(typeof api.validate, "undefined");
  assert.equal(typeof api.lintRuleCatalog, "undefined");
}

assertEditorLanguageSurface(hasCapability("editor"));

if (hasCapability("svg")) {
  assert.ok(capabilities.text_measurement?.provider_ids.includes("host-callback"));

  const rawGantt = `gantt
title Project Development Plan
dateFormat YYYY-MM-DD
section Design
Requirements    :a1, 2024-01-01, 7d
UI Design       :a2, after a1, 10d
section Development
Frontend Dev    :b1, after a2, 15d
Backend Dev     :b2, after a2, 15d
section Testing
Integration     :c1, after b1, 7d
User Testing    :c2, after c1, 5d`;
  assert.match(
    api.renderSvg(rawGantt, {
      svg: { pipeline: "readable" },
      environment: { text_measurement: "deterministic" },
    }),
    /<svg/
  );

  const svg = api.renderSvg(source, options);
  assert.match(svg, /<svg/);
  assert.match(svg, /Hello/);

  let measureCallCount = 0;
  const measurementPhases = new Set();
  const measurementOperations = new Set();
  const hostTextMeasurer = (request) => {
    measureCallCount += 1;
    measurementPhases.add(request.phase);
    measurementOperations.add(request.operation);
    return hostTextMeasurementResult(request);
  };
  const measuredSvg = api.renderSvgWithTextMeasurer(
    source,
    hostTextMeasurer,
    hostTextMeasurementOptions
  );
  assert.match(measuredSvg, /<svg/);
  assert.match(measuredSvg, /Hello/);
  assert.ok(
    measurementPhases.size > 0 &&
    [...measurementPhases].every((phase) =>
      ["layout", "wrap", "svg-bbox", "computed-length"].includes(phase)
    )
  );
  assert.ok(measureCallCount > 0);
  assert.ok(measurementOperations.has("wrapped"));

  let explicitHandledCallCount = 0;
  const explicitHandledSvg = api.renderSvgWithTextMeasurer(
    source,
    (request) => {
      explicitHandledCallCount += 1;
      const result = hostTextMeasurementResult(request);
      return result === undefined ? undefined : { handled: true, ...result };
    },
    hostTextMeasurementOptions
  );
  assert.equal(explicitHandledSvg, measuredSvg);
  assert.equal(explicitHandledCallCount, measureCallCount);

  if (completeCytoscapeRenderSurface) {
    const architectureOperations = new Map();
    const architectureSvg = api.renderSvgWithTextMeasurer(
      "architecture-beta\n  service api(server)[API service]\n",
      (request) => {
        architectureOperations.set(request.operation, request.phase);
        return hostTextMeasurementResult(request);
      },
      hostTextMeasurementOptions
    );
    assert.match(architectureSvg, /<svg/);
    assert.ok(
      architectureOperations.has("create-text-middle-bbox-y-offset"),
      "Architecture host measurement must transport the signed middle-baseline offset"
    );
    assert.equal(
      architectureOperations.get("create-text-middle-bbox-y-offset"),
      "svg-bbox"
    );
  }

  const measuredLayout = api.layoutJsonWithTextMeasurer(
    source,
    hostTextMeasurer,
    hostTextMeasurementOptions
  );
  assert.equal(typeof JSON.parse(measuredLayout), "object");
  const explicitHandledLayout = api.layoutJsonWithTextMeasurer(
    source,
    (request) => {
      const result = hostTextMeasurementResult(request);
      return result === undefined ? undefined : { handled: true, ...result };
    },
    hostTextMeasurementOptions
  );
  assert.equal(explicitHandledLayout, measuredLayout);

  let fallbackReferenceSvg;
  for (const fallbackResult of [
    null,
    undefined,
    { handled: false },
  ]) {
    let fallbackCallCount = 0;
    const fallbackSvg = api.renderSvgWithTextMeasurer(
      source,
      () => {
        fallbackCallCount += 1;
        return fallbackResult;
      },
      hostTextMeasurementOptions
    );
    assert.match(fallbackSvg, /<svg/);
    assert.ok(fallbackCallCount > 0);
    fallbackReferenceSvg ??= fallbackSvg;
    assert.equal(fallbackSvg, fallbackReferenceSvg);
  }

  const resultFields = [
    "handled",
    "kind",
    "width",
    "height",
    "length",
    "line_count",
    "bbox_left",
    "bbox_right",
    "raw_width",
  ];
  const unhandledAccesses = [];
  const unhandledProxy = new Proxy(
    { handled: false },
    {
      get(target, property, receiver) {
        if (typeof property === "string" && resultFields.includes(property)) {
          unhandledAccesses.push(property);
          if (property !== "handled") {
            throw new Error(`fallback result unexpectedly read ${property}`);
          }
        }
        return Reflect.get(target, property, receiver);
      },
    }
  );
  const proxyFallbackSvg = api.renderSvgWithTextMeasurer(
    source,
    () => unhandledProxy,
    hostTextMeasurementOptions
  );
  assert.equal(proxyFallbackSvg, fallbackReferenceSvg);
  assert.ok(unhandledAccesses.length > 0);
  assert.deepEqual(unhandledAccesses, Array(unhandledAccesses.length).fill("handled"));

  const handledAccesses = [];
  const proxyMeasuredSvg = api.renderSvgWithTextMeasurer(
    source,
    (request) => {
      const accesses = [];
      handledAccesses.push(accesses);
      return new Proxy(hostTextMeasurementResult(request), {
        get(target, property, receiver) {
          if (typeof property === "string" && resultFields.includes(property)) {
            accesses.push(property);
          }
          return Reflect.get(target, property, receiver);
        },
      });
    },
    hostTextMeasurementOptions
  );
  assert.equal(proxyMeasuredSvg, measuredSvg);
  assert.ok(handledAccesses.length > 0);
  for (const accesses of handledAccesses) {
    assert.deepEqual(accesses, resultFields);
  }

  for (const invalidResult of [
    { handled: "false", width: 1, height: 1 },
    { width: Number.POSITIVE_INFINITY, height: 1 },
    { width: -1, height: 1 },
    { width: 1, height: 1, line_count: 0 },
  ]) {
    let fallbackCallCount = 0;
    const fallbackSvg = api.renderSvgWithTextMeasurer(
      source,
      () => {
        fallbackCallCount += 1;
        return invalidResult;
      },
      hostTextMeasurementOptions
    );
    assert.match(fallbackSvg, /<svg/);
    assert.match(fallbackSvg, /Hello/);
    assert.ok(fallbackCallCount > 0);
  }
  let throwingCallCount = 0;
  const throwingFallbackSvg = api.renderSvgWithTextMeasurer(
    source,
    () => {
      throwingCallCount += 1;
      throw new Error("host measurer failed");
    },
    hostTextMeasurementOptions
  );
  assert.match(throwingFallbackSvg, /<svg/);
  assert.match(throwingFallbackSvg, /Hello/);
  assert.ok(throwingCallCount > 0);

  assert.equal(typeof api.parseObject(source, deterministicTime), "object");
  assert.equal(typeof api.layoutObject(source, options), "object");

  if (hasCapability("analysis")) {
    const valid = api.validate(source, deterministicTime);
    assert.equal(valid.valid, true);
    assert.equal(api.isBindingStatusCodeName(valid.code_name), true);

    const invalid = api.validate("not a diagram", deterministicTime);
    assert.equal(invalid.valid, false);
    assert.equal(api.isBindingStatusCodeName(invalid.code_name), true);
  }
} else {
  if (hasCapability("analysis")) {
    const valid = api.validate(source, deterministicTime);
    assert.equal(valid.valid, true);
    assert.equal(api.isBindingStatusCodeName(valid.code_name), true);
  }

  assert.equal(typeof api.renderSvg, "undefined");
  assert.equal(typeof api.renderSvgWithTextMeasurer, "undefined");
  assert.equal(typeof api.layoutJsonWithTextMeasurer, "undefined");
  assert.equal(typeof api.renderSvgElement, "undefined");
  assert.equal(typeof api.renderSvgToElement, "undefined");
  assert.equal(typeof api.parseJson, "undefined");
  assert.equal(typeof api.parseObject, "undefined");
  assert.equal(typeof api.layoutJson, "undefined");
  assert.equal(typeof api.layoutObject, "undefined");
  assert.equal(typeof api.createBrowserTextMeasurementSession, "undefined");
  assert.equal(typeof api.createBrowserTextMeasurer, "undefined");
  assert.equal(capabilities.text_measurement, null);
}

if (hasCapability("ascii")) {
  const ascii = api.renderAscii(source, deterministicTime);
  assert.match(ascii, /Hello/);
  assert.match(ascii, /World/);
  const wrappedAscii = api.renderAscii(
    'flowchart TD\nA["Alpha Beta Gamma Delta"]',
    {
      ...deterministicTime,
      ascii: { flowchartNodeLabelWrapWidth: 8 },
    },
  );
  assert.match(wrappedAscii, /Alpha/);
  assert.match(wrappedAscii, /Gamma/);
  assert.equal(wrappedAscii.includes("Alpha Beta Gamma Delta"), false);
} else {
  assert.equal(typeof api.renderAscii, "undefined");
  assert.equal(typeof api.asciiSupportedDiagrams, "undefined");
  assert.equal(typeof api.asciiCapabilities, "undefined");
}

assert.match(api.encodeOptions(options), /deterministic/);
if (hasCapability("svg")) {
  assert.throws(() => api.renderSvgElement(source), /requires a browser DOM/);
}

assert.deepEqual(api.supportedThemes(), [...api.SUPPORTED_THEMES]);

assert.deepEqual(api.supportedDiagrams(), [...api.SUPPORTED_DIAGRAMS]);
assert.equal(
  familyCapabilities.some((capability) => capability.diagram_type === "mindmap"),
  true
);
for (const diagram of api.supportedDiagrams()) {
  assert.equal(api.isDiagramType(diagram), true);
}

if (hasCapability("ascii")) {
  const asciiDiagrams = api.asciiSupportedDiagrams();
  assert.deepEqual(asciiDiagrams, [...api.SUPPORTED_ASCII_DIAGRAMS]);
  assert.deepEqual(api.asciiDiagrammaticDiagrams(), [
    ...api.DIAGRAMMATIC_ASCII_DIAGRAMS,
  ]);
  for (const diagram of asciiDiagrams) {
    assert.equal(api.isAsciiDiagramType(diagram), true);
  }
}

function textMeasureRequest(text, maxWidth, operation = "wrapped", wrapMode = "html-like") {
  return {
    operation,
    phase:
      operation.startsWith("wrapped")
        ? "wrap"
        : operation === "canvas-measure-text-width"
          ? "layout"
          : "svg-bbox",
    text,
    font_family: "Trebuchet MS, sans-serif",
    font_size: 16,
    font_weight: "normal",
    font_style: "normal",
    max_width: maxWidth,
    has_max_width: maxWidth !== null,
    line_height: 24,
    letter_spacing: 0,
    word_spacing: 0,
    wrap_mode: wrapMode,
    direction: "ltr",
    white_space: "break-spaces",
  };
}

function hostTextMeasurementResult(request) {
  const width = Math.max(1, request.text.length * 8);
  const height = Math.max(1, request.line_height || request.font_size);
  switch (request.operation) {
    case "measure":
    case "wrapped":
    case "mermaid-calculate-text-dimensions":
      return { kind: "metrics", width, height, line_count: 1 };
    case "computed-length":
    case "simple-bbox-width":
    case "raw-bbox-width":
    case "bounding-client-rect-width":
    case "tspan-bbox-width":
    case "wrap-probe-bbox-width":
    case "canvas-measure-text-width":
      return { kind: "length", length: width };
    case "tspan-bbox-height":
    case "simple-bbox-height":
    case "raw-bbox-height":
      return { kind: "length", length: height };
    case "create-text-bbox-y-offset":
      return { kind: "length", length: 1 };
    case "create-text-middle-bbox-y-offset":
      return { kind: "length", length: -1 };
    case "bbox-x":
    case "bbox-x-with-ascii-overhang":
    case "title-bbox-x":
      return { kind: "horizontal-extents", bbox_left: width / 2, bbox_right: width / 2 };
    case "wrapped-with-raw-width":
      return {
        kind: "wrapped-with-raw-width",
        width,
        height,
        line_count: 1,
        raw_width: width,
      };
    default:
      return undefined;
  }
}

function withFakeMeasureDom(run) {
  const originalDocument = globalThis.document;
  const canvasContext = {
    font: "",
    measureText(text) {
      return { width: text.length * 16 * 0.55 };
    },
  };
  globalThis.document = {
    body: {
      appendChild() {},
    },
    createElement(tagName) {
      assert.ok(tagName === "div" || tagName === "canvas");
      return new FakeMeasureElement(tagName, tagName === "canvas" ? canvasContext : null);
    },
    createElementNS(namespace, tagName) {
      assert.equal(namespace, "http://www.w3.org/2000/svg");
      return new FakeMeasureElement(tagName);
    },
  };

  try {
    run();
  } finally {
    if (originalDocument === undefined) {
      delete globalThis.document;
    } else {
      globalThis.document = originalDocument;
    }
  }
}

const fixtureNames = {
  architecture: "architecture_medium",
  block: "block_medium",
  c4: "c4_medium",
  class: "class_medium",
  er: "er_medium",
  flowchart: "flowchart_medium",
  gantt: "gantt_medium",
  gitgraph: "gitgraph_medium",
  info: "info_medium",
  journey: "journey_medium",
  kanban: "kanban_medium",
  mindmap: "mindmap_medium",
  packet: "packet_medium",
  pie: "pie_medium",
  quadrantchart: "quadrant_medium",
  radar: "radar_medium",
  requirement: "requirement_medium",
  sankey: "sankey_medium",
  sequence: "sequence_medium",
  state: "state_medium",
  timeline: "timeline_medium",
  treemap: "treemap_medium",
  venn: "venn_medium",
  xychart: "xychart_medium",
  zenuml: "zenuml_medium",
};

const repositoryFixturePaths = {
  cynefin: ["fixtures", "cynefin", "basic_domains_transitions.mmd"],
  eventmodeling: ["fixtures", "eventmodeling", "upstream_docs_eventmodeling_minimum.mmd"],
  ishikawa: ["fixtures", "ishikawa", "upstream_docs_ishikawa_basic.mmd"],
  railroad: ["fixtures", "railroad", "basic_ir.mmd"],
  railroadAbnf: ["fixtures", "railroadAbnf", "repetition_optional_numval.mmd"],
  railroadEbnf: ["fixtures", "railroadEbnf", "choice_optional_repetition.mmd"],
  railroadPeg: ["fixtures", "railroadPeg", "prefix_suffix_any.mmd"],
  swimlane: ["fixtures", "swimlane", "basic_flowchart_reuse.mmd"],
  treeView: ["fixtures", "treeView", "upstream_docs_treeview_basic.mmd"],
  wardley: [
    "fixtures",
    "wardley",
    "upstream_cypress_wardley_spec_1_should_render_tea_shop_001.mmd",
  ],
};

if (hasCapability("svg")) {
  for (const diagram of api.supportedDiagrams()) {
    const fixtureName = fixtureNames[diagram];
    const repositoryFixturePath = repositoryFixturePaths[diagram];
    assert.ok(fixtureName || repositoryFixturePath, `missing fixture for ${diagram}`);
    const fixturePath = fixtureName
      ? path.join(
          repoRoot,
          "crates",
          "merman",
          "benches",
          "fixtures",
          `${fixtureName}.mmd`
        )
      : path.join(repoRoot, ...repositoryFixturePath);
    const fixture = await readFile(fixturePath, "utf8");
    try {
      assert.match(api.renderSvg(fixture, deterministicTime), /<svg/);
    } catch (error) {
      assert.equal(
        completeCytoscapeRenderSurface,
        false,
        `complete render surface failed to render ${diagram}`
      );
      assert.equal(
        cytoscapeLayoutDiagramTypes.has(diagram),
        true,
        `unexpected render feature absence for ${diagram}`
      );
      assert.equal(error?.code_name, "MERMAN_UNSUPPORTED_OPERATION");
      assert.equal(error?.kind, "missing-capability");
      assert.equal(error?.capability_id, "layout-cytoscape");
      assert.match(
        error?.message ?? "",
        new RegExp(
          "compiled renderer lacks capability `layout-cytoscape` required by diagram `" +
            diagram +
            "`"
        )
      );
    }
    if (hasCapability("analysis")) {
      const detection = api.detectDiagramFacts(fixture, deterministicTime);
      assert.equal(detection.status, "available", `detection unavailable for ${diagram}`);
      assert.equal(detection.diagramType, diagram, `detection mismatch for ${diagram}`);
      assert.equal(typeof detection.syntaxId, "string");
      assert.equal(typeof detection.effectiveLayoutId, "string");
      if (diagram === "swimlane") {
        assert.equal(detection.effectiveLayoutId, "swimlane");
      }
    }
  }
}

console.log(
  [
    "@mermanjs/web smoke passed",
    `package=${packageDescriptor.name}`,
    `diagrams=${api.supportedDiagrams().length}`,
    `capabilities=${capabilities.capability_ids.join(",") || "none"}`,
    `outputs=${capabilities.output_ids.join(",") || "none"}`,
    `text_measurement=${JSON.stringify(capabilities.text_measurement)}`,
  ].join(" ")
);

function assertEditorLanguageSurface(enabled) {
  const editorSource = "flowchart TD\nA-->B\nB-->\n";
  const editorUri = "file:///tmp/example.mmd";

  if (!enabled) {
    for (const apiName of editorRuntimeExports) {
      assert.equal(typeof api[apiName], "undefined");
      assert.equal(typeof exportedWasmModule[apiName], "undefined");
    }
    assert.equal(typeof exportedWasmModule.EditorSession, "undefined");
    return;
  }

  assert.equal(typeof exportedWasmModule.EditorSession, "function");
  const editorSession = api.createEditorSession(
    editorSource,
    1,
    editorUri,
    deterministicTime
  );
  assert.equal(editorSession.version, 1);
  assert.equal(editorSession.uri, editorUri);
  assert.equal(Array.isArray(editorSession.diagnostics().diagnostics), true);
  assert.equal(typeof editorSession.searchDocumentSymbols, "function");
  assert.equal(typeof editorSession.workspaceSymbols, "undefined");
  assert.ok(
    editorSession
      .searchDocumentSymbols("A")
      .some((symbol) => symbol.name === "A"),
  );
  editorSession.update("flowchart TD\nA-->B\nB-->C\n", 2);
  assert.equal(editorSession.version, 2);
  editorSession.dispose();
  editorSession.dispose();
  assert.throws(() => editorSession.diagnostics(), /editor session is disposed/i);

  const completions = api.editorCompletions(
    "flowchart TD\nA-->B\nC-->\n",
    { line: 2, character: 4 },
    editorUri
  );
  assert.ok(completions.items.some((item) => item.label === "B"));

  const diagnostics = api.editorDiagnostics(editorSource, deterministicTime, editorUri);
  assert.equal(Array.isArray(diagnostics.diagnostics), true);
  assert.ok(
    api
      .editorSearchDocumentSymbols(editorSource, "A", editorUri, deterministicTime)
      .some((symbol) => symbol.name === "A"),
  );

  const editorLintOptions = {
    ...deterministicTime,
    lint: { profile: "recommended" },
  };
  const codeActions = api.editorCodeActions(
    "flowchart\nA-->B\n",
    editorLintOptions,
    editorUri
  );
  const directionAction = codeActions.find((action) =>
    action.title.includes("flowchart header")
  );
  assert.ok(directionAction);
  assert.equal(directionAction.edit.changes instanceof Map, false);
  assert.equal(directionAction.edit.changes[editorUri][0].newText, " TB");

  const hover = api.editorHover(
    "flowchart TD\nAlpha-->Beta\nAlpha-->Gamma\n",
    { line: 1, character: 0 },
    editorUri
  );
  assert.ok(hover);
  assert.match(JSON.stringify(hover.contents), /Alpha/);

  const definition = api.editorDefinition(
    "flowchart TD\nAlpha-->Beta\nAlpha-->Gamma\n",
    { line: 2, character: 0 },
    editorUri
  );
  assert.equal(definition.uri, editorUri);
  assert.equal(definition.range.start.line, 1);

  const references = api.editorReferences(
    "flowchart TD\nA-->B\nA-->C\n",
    { line: 1, character: 0 },
    true,
    editorUri
  );
  assert.equal(references.length, 2);

  const prepareRename = api.editorPrepareRename(
    "flowchart TD\nAlpha-->Beta\nAlpha-->Gamma\n",
    { line: 1, character: 0 },
    editorUri
  );
  assert.equal(prepareRename.placeholder, "Alpha");

  const rename = api.editorRename(
    "flowchart TD\nAlpha-->Beta\nAlpha-->Gamma\n",
    { line: 1, character: 0 },
    "Delta",
    editorUri
  );
  assert.equal(rename.changes instanceof Map, false);
  assert.ok(rename.changes[editorUri].some((edit) => edit.newText === "Delta"));

  for (const [run, messagePattern] of [
    [
      () =>
        api.editorRename(
          "flowchart TD\nAlpha-->Beta\n",
          { line: 1, character: 0 },
          "bad name",
          editorUri
        ),
      /new name/,
    ],
    [
      () =>
        api.editorRename(
          "flowchart TD\nAlpha-->Beta\n",
          { line: 0, character: 0 },
          "Delta",
          editorUri
        ),
      /no renameable symbol|outside a Mermaid fence/,
    ],
  ]) {
    let error = null;
    try {
      run();
    } catch (caught) {
      error = caught;
    }
    assert.ok(api.isBindingErrorPayload(error), "expected structured rename binding error");
    assert.equal(error.code_name, "MERMAN_INVALID_ARGUMENT");
    assert.match(error.message, messagePattern);
  }

  const resourceError = {
    version: 1,
    ok: false,
    code: 10,
    code_name: "MERMAN_RESOURCE_LIMIT_EXCEEDED",
    kind: "generic",
    capability_id: null,
    details: {
      resource: {
        cause: "arithmetic_overflow",
        limit_id: "max_layout_work_units",
        phase: "layout_model",
        actual: "18446744073709551615",
        max: 800_000,
        profile: "interactive",
      },
    },
    message: "layout work accounting overflowed",
  };
  assert.ok(
    api.isBindingErrorPayload(resourceError),
    "expected structured resource cause to satisfy the binding error contract"
  );
  assert.equal(resourceError.details.resource.cause, "arithmetic_overflow");
  assert.equal(resourceError.details.resource.actual, "18446744073709551615");
  assert.equal(resourceError.details.resource.max, 800_000);
  assert.equal(
    api.isBindingErrorPayload({
      ...resourceError,
      details: {
        resource: {
          ...resourceError.details.resource,
          cause: undefined,
        },
      },
    }),
    false,
    "resource details without a cause must not satisfy the binding error contract"
  );
  for (const actual of [
    "5",
    "09007199254740992",
    "18446744073709551616",
    "-9007199254740992",
    Number.MAX_SAFE_INTEGER + 1,
  ]) {
    assert.equal(
      api.isBindingErrorPayload({
        ...resourceError,
        details: {
          resource: {
            ...resourceError.details.resource,
            actual,
          },
        },
      }),
      false,
      `invalid resource count ${actual} must not satisfy the binding error contract`
    );
  }

  const cancellationError = {
    version: 1,
    ok: false,
    code: 12,
    code_name: "MERMAN_CANCELLED",
    kind: "generic",
    capability_id: null,
    details: {
      cancellation: {
        reason: "deadline_exceeded",
        phase: "admission",
      },
    },
    message: "operation cancelled during admission: deadline exceeded",
  };
  assert.ok(
    api.isBindingStatusCodeName("MERMAN_BUSY") &&
      api.isBindingStatusCodeName("MERMAN_CANCELLED"),
    "expected operation status names to satisfy the public binding contract"
  );
  assert.ok(
    api.isBindingErrorPayload(cancellationError),
    "expected cancellation-only details to satisfy the binding error contract"
  );

  assert.deepEqual(
    api.editorDiagramDetection(
      "flowchart TD\nA[unterminated\n",
      undefined,
      editorUri
    ),
    {
      status: "available",
      validity: "recoverable-invalid",
      diagramType: "flowchart",
      syntaxId: "flowchart-v2",
      effectiveLayoutId: "dagre",
    }
  );

  for (const apiName of [
    "editorDiagnostics",
    "editorDiagramDetection",
    "editorCodeActions",
    "editorCompletions",
    "editorHover",
    "editorDocumentSymbols",
    "editorSearchDocumentSymbols",
    "editorDefinition",
    "editorReferences",
    "editorPrepareRename",
    "editorRename",
  ]) {
    assert.equal(typeof exportedWasmModule[apiName], "function");
  }
  assert.equal(typeof exportedWasmModule.editorWorkspaceSymbols, "undefined");
}

function assertUnsupportedOperation(run) {
  let error = null;
  try {
    run();
  } catch (caught) {
    error = caught;
  }
  assert.ok(error, "expected MERMAN_UNSUPPORTED_OPERATION error");
  assert.equal(error.code_name, "MERMAN_UNSUPPORTED_OPERATION");
}

async function runSameProcessPackageSmoke() {
  const source = "flowchart TD\nA[Hello] --> B[World]";
  const options = {
    fixed_today: "2026-06-10",
    fixed_local_offset_minutes: 0,
    svg: { pipeline: "readable" },
    environment: { text_measurement: "deterministic" },
  };
  const analysisDescriptor = packageDescriptorForId("analysis");
  const fullDescriptor = packageDescriptorForId("full");
  const { bindSurfaceRuntime } = await import(sharedDistUrl("surface-runtime.js"));
  const analysisWasm = await import(
    pathToFileURL(
      path.join(packageRoot, analysisDescriptor.package_dir, "artifacts", "wasm", "merman_wasm.js")
    ).href,
  );
  const fullWasm = await import(
    pathToFileURL(
      path.join(packageRoot, fullDescriptor.package_dir, "artifacts", "wasm", "merman_wasm.js")
    ).href,
  );
  const analysisImplementation = await runtimeImplementationForSurface(
    analysisDescriptor,
  );
  const fullImplementation = await runtimeImplementationForSurface(
    fullDescriptor,
  );
  const analysis = projectInternalApiForSurface(
    bindSurfaceRuntime(async () => analysisWasm, analysisImplementation),
    packageContract(analysisDescriptor),
  );
  const full = projectInternalApiForSurface(
    bindSurfaceRuntime(async () => fullWasm, fullImplementation),
    packageContract(fullDescriptor),
  );

  await analysis.initMerman({
    wasm: await readFile(
      path.join(packageRoot, analysisDescriptor.package_dir, "artifacts", "wasm", "merman_wasm_bg.wasm")
    ),
  });
  assert.equal(
    analysis.runtimeCatalog().capabilities.capability_ids.includes("svg"),
    false
  );
  assert.deepEqual(analysis.presentationCatalog(), {
    schema_version: 1,
    theme_presets: [],
    profiles: [],
  });
  assert.equal(typeof analysis.renderSvg, "undefined");

  await full.initMerman({
    wasm: await readFile(
      path.join(packageRoot, fullDescriptor.package_dir, "artifacts", "wasm", "merman_wasm_bg.wasm")
    ),
  });
  assert.equal(
    full.runtimeCatalog().capabilities.capability_ids.includes("svg"),
    true
  );
  assert.equal(
    full.runtimeCatalog().resources.general_binding_default_profile,
    "interactive"
  );
  assert.equal(full.presentationCatalog().theme_presets.length, 7);
  assert.equal(full.presentationCatalog().profiles[0].id, "merman-modern");
  assert.match(full.renderSvg(source, options), /<svg/);
  assert.equal(
    analysis.runtimeCatalog().capabilities.capability_ids.includes("svg"),
    false
  );
  assert.equal(analysis.presentationCatalog().theme_presets.length, 0);
  assert.equal(typeof analysis.renderSvg, "undefined");
}

async function runtimeImplementationForSurface(descriptor) {
  const implementation = {};
  for (const { specifier, exportNames } of descriptor.runtimeExportModules) {
    const module = await import(
      sharedDistUrl(specifier.replace(/^\.\.\//, ""))
    );
    for (const name of exportNames) {
      implementation[name] = module[name];
    }
  }
  return implementation;
}

async function withNodeDomShim(run) {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  const previousDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
  Object.defineProperty(globalThis, "window", { configurable: true, value: {} });
  Object.defineProperty(globalThis, "document", { configurable: true, value: {} });
  try {
    return await run();
  } finally {
    restoreGlobalProperty("window", previousWindow);
    restoreGlobalProperty("document", previousDocument);
  }
}

function restoreGlobalProperty(name, descriptor) {
  if (descriptor) {
    Object.defineProperty(globalThis, name, descriptor);
  } else {
    delete globalThis[name];
  }
}

async function assertNoDeprecatedWasmBindgenInitWarning(run) {
  const warnings = [];
  const originalWarn = console.warn;
  console.warn = (...args) => warnings.push(args.map(String).join(" "));
  try {
    await run();
  } finally {
    console.warn = originalWarn;
  }
  assert.deepEqual(
    warnings.filter((warning) =>
      warning.includes("deprecated parameters for the initialization function")
    ),
    [],
    "the public wrapper must own wasm-bindgen's object-shaped initialization contract"
  );
}

async function runPureDistSmoke() {
  const catalogSpecifier = sharedDistUrl("public-catalog.js");
  const svgSafetySpecifier = sharedDistUrl("svg-safety.js");
  const textMeasurementAbiSpecifier = sharedDistUrl("generated/text-measurement-abi.js");
  const catalogFile = fileURLToPath(catalogSpecifier);
  const svgSafetyFile = fileURLToPath(svgSafetySpecifier);
  const textMeasurementAbiFile = fileURLToPath(textMeasurementAbiSpecifier);
  assert.equal(catalogFile, path.join(packageRoot, "dist", "public-catalog.js"));
  assert.equal(svgSafetyFile, path.join(packageRoot, "dist", "svg-safety.js"));
  assert.equal(
    textMeasurementAbiFile,
    path.join(packageRoot, "dist", "generated", "text-measurement-abi.js")
  );
  await assertPureDistModuleGraph([
    catalogFile,
    svgSafetyFile,
    textMeasurementAbiFile,
  ]);

  const [catalog, svgSafety, textMeasurementAbi] = await Promise.all([
    import(catalogSpecifier),
    import(svgSafetySpecifier),
    import(textMeasurementAbiSpecifier),
  ]);
  assert.equal(catalog.SUPPORTED_DIAGRAMS.length, 35);
  assert.equal(catalog.isDiagramType("swimlane"), true);
  assert.equal(catalog.normalizeThemeName("neo-dark"), "neo-dark");
  assert.equal("initMerman" in catalog, false);
  assert.equal(typeof svgSafety.assertNavigableSvgForDom, "function");
  assert.equal(typeof svgSafety.assertSelfContainedSvgForDom, "function");
  assert.equal(typeof svgSafety.prepareNavigableSvgForDomMount, "function");
  assert.equal(typeof svgSafety.prepareSelfContainedSvgForDomMount, "function");
  assert.equal(textMeasurementAbi.MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION, 1);
  assert.deepEqual(
    textMeasurementAbi.HOST_TEXT_MEASUREMENT_OPERATIONS.map(({ code }) => code),
    Array.from({ length: 19 }, (_, code) => code)
  );
  assert.deepEqual(
    textMeasurementAbi.HOST_TEXT_MEASUREMENT_RESULT_KINDS.map(({ code }) => code),
    Array.from({ length: 4 }, (_, code) => code)
  );
  assert.equal(
    new Set(
      textMeasurementAbi.HOST_TEXT_MEASUREMENT_OPERATIONS.map(({ name }) => name)
    ).size,
    19
  );
  assert.deepEqual(
    textMeasurementAbi.HOST_TEXT_MEASUREMENT_OPERATIONS
      .filter(({ acceptsSignedLength }) => acceptsSignedLength)
      .map(({ name }) => name),
    ["create-text-bbox-y-offset", "create-text-middle-bbox-y-offset"]
  );
  svgSafety.assertSelfContainedSvgForDom('<svg xmlns="http://www.w3.org/2000/svg" />');
  assert.throws(
    () =>
      svgSafety.assertSelfContainedSvgForDom(
        '<svg xmlns="http://www.w3.org/2000/svg"><script /></svg>'
      ),
    /active embedded content/
  );
}

async function assertPureDistModuleGraph(entries) {
  const distRoot = path.join(packageRoot, "dist");
  const pending = [...entries];
  const visited = new Set();
  while (pending.length > 0) {
    const file = path.resolve(pending.pop());
    if (visited.has(file)) continue;
    const relative = path.relative(distRoot, file);
    assert.ok(
      relative && !relative.startsWith("..") && !path.isAbsolute(relative),
      `pure Web subpath escaped dist: ${file}`
    );
    assert.notEqual(relative, "index.js", "pure Web subpath reached the WASM facade");
    visited.add(file);

    const source = await readFile(file, "utf8");
    for (const specifier of moduleSpecifiers(source, file)) {
      assert.ok(
        specifier.startsWith("."),
        `pure Web subpath has a non-local dependency: ${specifier}`
      );
      pending.push(path.resolve(path.dirname(file), specifier));
    }
  }
}

function moduleSpecifiers(source, file) {
  const syntax = ts.createSourceFile(
    file,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.JS
  );
  const specifiers = [];
  const visit = (node) => {
    if (
      (ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) &&
      node.moduleSpecifier &&
      ts.isStringLiteralLike(node.moduleSpecifier)
    ) {
      specifiers.push(node.moduleSpecifier.text);
    } else if (
      ts.isCallExpression(node) &&
      node.expression.kind === ts.SyntaxKind.ImportKeyword &&
      node.arguments.length === 1 &&
      ts.isStringLiteralLike(node.arguments[0])
    ) {
      specifiers.push(node.arguments[0].text);
    }
    ts.forEachChild(node, visit);
  };
  visit(syntax);
  return specifiers;
}

function packageContract(descriptor) {
  return {
    ...descriptor,
    render: descriptor.runtimeExportNames.includes("renderSvg"),
    analysis: descriptor.runtimeExportNames.includes("analyze"),
    ascii: descriptor.runtimeExportNames.includes("renderAscii"),
    editor: descriptor.runtimeExportNames.includes("editorDiagnostics"),
  };
}

function projectInternalApiForSurface(api, contract) {
  const allowed = new Set([
    ...contract.runtimeExportNames,
    ...contract.valueExportNames,
  ]);
  const surfaceOwnedExports = new Set([
    ...allPackageRuntimeExportNames,
    ...allPackageValueExportNames,
  ]);
  return new Proxy(api, {
    get(target, property, receiver) {
      if (
        typeof property === "string" &&
        surfaceOwnedExports.has(property) &&
        !allowed.has(property)
      ) {
        return undefined;
      }
      return Reflect.get(target, property, receiver);
    },
  });
}

function assertSurfaceExports(moduleApi, contract) {
  const expectedRuntimeExports = new Set(contract.runtimeExportNames);
  const expectedValueExports = new Set(contract.valueExportNames);

  assert.equal(typeof moduleApi.MERMAN_WASM_URL, "string");
  assert.equal(typeof moduleApi.loadMermanWasmModule, "function");

  for (const name of allPackageRuntimeExportNames) {
    assertExport(moduleApi, name, expectedRuntimeExports.has(name));
  }

  for (const name of allPackageValueExportNames) {
    assertExport(moduleApi, name, expectedValueExports.has(name));
  }
}

function assertExport(moduleApi, name, enabled) {
  if (enabled) {
    assert.notEqual(moduleApi[name], undefined, `${name} should be exported`);
  } else {
    assert.equal(moduleApi[name], undefined, `${name} should not be exported`);
  }
}

function packageDescriptorForId(id) {
  const descriptor = webPackages.find((candidate) => candidate.id === id);
  if (!descriptor) throw new Error(`Unknown Web package ${id}.`);
  return descriptor;
}

function packageEntryUrl(descriptor) {
  return pathToFileURL(
    path.join(packageRoot, descriptor.package_dir, "dist", "package-entries", `${descriptor.id}.js`),
  ).href;
}

function sharedDistUrl(relative) {
  return pathToFileURL(path.join(packageRoot, "dist", relative)).href;
}

function parseCli(inputArgs) {
  try {
    return parseSmokeCli(inputArgs);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    console.error(smokeUsage());
    process.exit(2);
  }
}
