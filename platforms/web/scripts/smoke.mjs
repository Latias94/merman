import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseSmokeCli, smokeUsage } from "./smoke-cli.mjs";
import {
  allSurfaceRuntimeExportNames,
  allSurfaceValueExportNames,
  surfaces,
} from "./surface-manifest.mjs";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.join(packageRoot, "..", "..");
const args = process.argv.slice(2);
const fullSurface = surfaces.find((surface) => surface.entry === "full");
if (!fullSurface) {
  throw new Error("surface manifest is missing the full entry");
}
const editorRuntimeExports = fullSurface.runtimeExportNames.filter((name) =>
  name.startsWith("editor")
);

const surfaceSmokeCases = [
  surfaceSmokeCase("default", ".", "pkg"),
  ...surfaces.map((surface) =>
    surfaceSmokeCase(surface.entry, `./${surface.entry}`, surface.pkgDirRel)
  ),
];

if (args.length === 0) {
  for (const smokeCase of surfaceSmokeCases) {
    const result = spawnSync(
      process.execPath,
      [
        fileURLToPath(import.meta.url),
        "--entry",
        smokeCase.entry,
        "--pkg-dir-rel",
        smokeCase.pkgDirRel,
        "--wasm-module-subpath",
        smokeCase.wasmModuleSubpath,
        "--wasm-binary-rel",
        smokeCase.wasmBinaryRel,
        "--manifest-rel",
        smokeCase.manifestRel,
      ],
      {
        cwd: packageRoot,
        stdio: "inherit",
      }
    );
    if (result.error) {
      console.error(
        `@mermanjs/web smoke failed to spawn ${smokeCase.name}: ${result.error.message}`
      );
      process.exit(1);
    }
    if (result.status !== 0) {
      process.exit(result.status ?? 1);
    }
  }
  await runSameProcessSurfaceSmoke();
  console.log(
    `@mermanjs/web smoke matrix passed surfaces=${surfaceSmokeCases
      .map((smokeCase) => smokeCase.name)
      .join(",")}`
  );
  process.exit(0);
}

const {
  entrySubpath,
  pkgDirRel,
  wasmModuleSubpath,
  wasmBinaryRel,
  manifestRel,
} = parseCli(args);

const api = await import(toPackageSpecifier(entrySubpath));
const exportedWasmModule = await import(toPackageSpecifier(wasmModuleSubpath));
const surfaceContract = surfaceContractForEntry(entrySubpath);

assert.equal(typeof exportedWasmModule.default, "function");
assertSurfaceExports(api, surfaceContract);
if (typeof import.meta.resolve === "function") {
  assert.match(
    import.meta.resolve(toPackageSpecifier(wasmBinaryRel)),
    /merman_wasm_bg\.wasm$/
  );
}

const wasmBinary = await readFile(path.join(packageRoot, wasmBinaryRel));
await exportedWasmModule.default({ module_or_path: wasmBinary });
await api.initMerman({
  wasm: {
    module_or_path: wasmBinary,
  },
});

const source = "flowchart TD\nA[Hello] --> B[World]";
const deterministicTime = {
  fixed_today: "2026-06-10",
  fixed_local_offset_minutes: 0,
};
const options = {
  ...deterministicTime,
  svg: { pipeline: "readable" },
  layout: { text_measurer: "deterministic" },
};
const presetManifest = JSON.parse(
  await readFile(path.join(packageRoot, manifestRel), "utf8")
);

class FakeMeasureElement {
  style = {};
  textContent = "";

  setAttribute() {}

  getBoundingClientRect() {
    const fontSize = parseFloat(this.style.fontSize) || 16;
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
assert.equal(api.abiVersion(), 2);
assert.match(api.packageVersion(), /^\d+\.\d+\.\d+/);
if (surfaceContract.render) {
  assert.equal(typeof api.renderSvgWithTextMeasurer, "function");
  assert.equal(typeof api.layoutJsonWithTextMeasurer, "function");
  assert.equal(typeof api.createBrowserTextMeasurer, "function");
  assert.equal(api.createBrowserTextMeasurer()({ text: "Node", font_size: 16 }), undefined);
  withFakeMeasureDom(() => {
    const browserMeasurer = api.createBrowserTextMeasurer();
    const shortLabel = browserMeasurer(textMeasureRequest("Condition?", 200));
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
  });
}

const capabilities = api.bindingCapabilities();
const defaultCapabilities = api.DEFAULT_BINDING_CAPABILITIES;
assert.equal(typeof capabilities.render, "boolean");
assert.equal(typeof capabilities.analysis, "boolean");
assert.equal(typeof capabilities.ascii, "boolean");
assert.equal(typeof capabilities.core_full, "boolean");
assert.equal(typeof capabilities.core_host, "boolean");
assert.equal(typeof capabilities.ratex_math, "boolean");
assert.equal(typeof capabilities.editor_language, "boolean");
assert.equal(typeof capabilities.text_measurement, "object");
assert.equal(typeof capabilities.text_measurement.vendored, "boolean");
assert.equal(typeof capabilities.text_measurement.deterministic, "boolean");
assert.equal(typeof capabilities.text_measurement.host_callback, "boolean");
assert.equal(typeof capabilities.text_measurement.font_assets, "boolean");
assert.equal(capabilities.render, surfaceContract.render);
assert.equal(capabilities.analysis, surfaceContract.analysis);
assert.equal(capabilities.ascii, surfaceContract.ascii);
assert.equal(capabilities.editor_language, surfaceContract.editor);
assert.equal(capabilities.text_measurement.host_callback, capabilities.render);
assert.equal(capabilities.editor_language, presetManifest.capabilities.editor_language);
assert.equal(capabilities.analysis, presetManifest.capabilities.analysis);
assert.equal(defaultCapabilities.render, surfaceContract.render);
assert.equal(defaultCapabilities.analysis, surfaceContract.analysis);
assert.equal(defaultCapabilities.ascii, surfaceContract.ascii);
assert.equal(defaultCapabilities.editor_language, surfaceContract.editor);
assert.equal(defaultCapabilities.text_measurement.host_callback, defaultCapabilities.render);

const registryProfile = api.selectedRegistryProfile();
assert.match(registryProfile, /^(full|tiny)$/);
assert.equal(registryProfile, capabilities.core_full ? "full" : "tiny");

const familyCapabilities = api.diagramFamilyCapabilities();
assert.equal(Array.isArray(familyCapabilities), true);
assert.equal(
  familyCapabilities.some(
    (capability) =>
      capability.diagram_type === "flowchart" &&
      capability.metadata_id === "flowchart" &&
      capability.has_semantic_parser &&
      capability.has_render_parser
  ),
  true
);

if (capabilities.analysis) {
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

  const markdownAnalysis = api.analyzeDocument(
    "before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
    deterministicTime,
    "file:///tmp/example.md"
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
  assert.equal(flowchartFacts.valid, true);
  assert.equal(flowchartFacts.diagrams[0].syntax.fact_source, "parser_complete");
  assert.equal(flowchartFacts.diagrams[0].syntax.source_mapped_spans, true);
  assert.equal(
    flowchartFacts.diagrams[0].syntax.flowchart.nodes.some((node) => node.id === "A"),
    true
  );
  assert.equal(
    flowchartFacts.diagrams[0].syntax.flowchart.edges.some(
      (edge) => edge.from === "A" && edge.to === "B"
    ),
    true
  );
  assert.equal(
    flowchartFacts.diagrams[0].syntax.semantic_items.some(
      (item) => item.name === "A" && item.span.document
    ),
    true
  );

  const degradedSequenceFacts = api.analysisFacts(
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
  assert.equal(degradedSequenceFacts.valid, true);
  assert.equal(
    degradedSequenceFacts.diagrams[0].syntax.fact_source,
    "parser_complete_degraded_spans"
  );
  assert.equal(degradedSequenceFacts.diagrams[0].syntax.source_mapped_spans, false);

  const markdownFacts = api.analyzeDocumentFacts(
    "before\n```mermaid\nflowchart TD\nA@{\n  shape: rou\n}\n```\nafter\n",
    deterministicTime,
    "file:///tmp/example.md"
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
    deterministicTime,
    "file:///tmp/example.mdx?rev=1#fence"
  );
  assert.equal(mdxAnalysis.valid, false);
  assert.equal(mdxAnalysis.source.kind, "mdx");
  assert.equal(mdxAnalysis.source.language, "mdx");
  assert.equal(mdxAnalysis.source.path, "file:///tmp/example.mdx?rev=1#fence");
  assert.equal(mdxAnalysis.diagnostics[0].span.line, 4);

  const markdownFixAnalysis = api.analyzeDocument(
    '```mermaid\n%%{ initialize: {"theme":"dark"} }%%\nflowchart TD\nA-->B\n```\n',
    {
      ...deterministicTime,
      lint: { profile: "recommended" },
    },
    "file:///tmp/example.md"
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
  assert.equal(typeof api.analyzeDocument, "undefined");
  assert.equal(typeof api.analyzeDocumentFacts, "undefined");
  assert.equal(typeof api.validate, "undefined");
  assert.equal(typeof api.lintRuleCatalog, "undefined");
}

assertEditorLanguageSurface(capabilities.editor_language);

if (capabilities.render) {
  assert.equal(capabilities.text_measurement.host_callback, true);

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
      layout: { text_measurer: "deterministic" },
    }),
    /<svg/
  );

  const svg = api.renderSvg(source, options);
  assert.match(svg, /<svg/);
  assert.match(svg, /Hello/);

  let measureCallCount = 0;
  const hostTextMeasurer = (request) => {
    measureCallCount += 1;
    return {
      width: Math.max(1, request.text.length * 8),
      height: Math.max(1, request.line_height || request.font_size),
      line_count: 1,
    };
  };
  const measuredSvg = api.renderSvgWithTextMeasurer(source, hostTextMeasurer, options);
  assert.match(measuredSvg, /<svg/);
  assert.match(measuredSvg, /Hello/);
  assert.ok(measureCallCount > 0);
  const measuredLayout = api.layoutJsonWithTextMeasurer(source, hostTextMeasurer, options);
  assert.equal(typeof JSON.parse(measuredLayout), "object");
  for (const fallbackResult of [
    undefined,
    { handled: false, width: 1, height: 1 },
  ]) {
    let fallbackCallCount = 0;
    const fallbackSvg = api.renderSvgWithTextMeasurer(
      source,
      () => {
        fallbackCallCount += 1;
        return fallbackResult;
      },
      options
    );
    assert.match(fallbackSvg, /<svg/);
    assert.ok(fallbackCallCount > 0);
  }
  for (const invalidResult of [
    { width: Number.POSITIVE_INFINITY, height: 1 },
    { width: -1, height: 1 },
    { width: 1, height: 1, line_count: 0 },
  ]) {
    let fallbackCallCount = 0;
    assert.throws(
      () =>
        api.renderSvgWithTextMeasurer(
          source,
          () => {
            fallbackCallCount += 1;
            return invalidResult;
          },
          options
        ),
      /host text measurer returned/
    );
    assert.ok(fallbackCallCount > 0);
  }
  assert.throws(
    () =>
      api.renderSvgWithTextMeasurer(
        source,
        () => {
          throw new Error("host measurer failed");
        },
        options
      ),
    /host measurer failed/
  );

  assert.equal(typeof api.parseObject(source, deterministicTime), "object");
  assert.equal(typeof api.layoutObject(source, options), "object");

  if (capabilities.analysis) {
    const valid = api.validate(source, deterministicTime);
    assert.equal(valid.valid, true);
    assert.equal(api.isBindingStatusCodeName(valid.code_name), true);

    const invalid = api.validate("not a diagram", deterministicTime);
    assert.equal(invalid.valid, false);
    assert.equal(api.isBindingStatusCodeName(invalid.code_name), true);
  }
} else {
  if (capabilities.analysis) {
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
  assert.equal(typeof api.createBrowserTextMeasurer, "undefined");
  assert.equal(capabilities.text_measurement.host_callback, false);
}

if (capabilities.ascii) {
  const ascii = api.renderAscii(source, deterministicTime);
  assert.match(ascii, /Hello/);
  assert.match(ascii, /World/);
} else {
  assert.equal(typeof api.renderAscii, "undefined");
  assert.equal(typeof api.asciiSupportedDiagrams, "undefined");
  assert.equal(typeof api.asciiCapabilities, "undefined");
}

assert.match(api.encodeOptions(options), /deterministic/);
if (capabilities.render) {
  assert.throws(() => api.renderSvgElement(source), /requires a browser DOM/);
}

assert.deepEqual(api.supportedThemes(), [...api.SUPPORTED_THEMES]);
if (capabilities.render) {
  assert.deepEqual(api.supportedHostThemePresets(), [
    ...api.SUPPORTED_HOST_THEME_PRESETS,
  ]);
} else {
  assert.equal(typeof api.supportedHostThemePresets, "undefined");
}

if (capabilities.core_full) {
  assert.deepEqual(api.supportedDiagrams(), [...api.SUPPORTED_DIAGRAMS]);
  assert.equal(
    familyCapabilities.some((capability) => capability.diagram_type === "mindmap"),
    true
  );
} else {
  for (const diagram of api.supportedDiagrams()) {
    assert.equal(api.isDiagramType(diagram), true);
  }
  assert.equal(
    familyCapabilities.some((capability) => capability.diagram_type === "mindmap"),
    false
  );
}

if (capabilities.ascii) {
  const asciiDiagrams = api.asciiSupportedDiagrams();
  for (const diagram of asciiDiagrams) {
    assert.equal(api.isAsciiDiagramType(diagram), true);
  }
}

function textMeasureRequest(text, maxWidth) {
  return {
    text,
    font_family: "Trebuchet MS, sans-serif",
    font_size: 16,
    font_weight: "normal",
    font_style: "normal",
    max_width: maxWidth,
    has_max_width: true,
    line_height: 24,
    letter_spacing: 0,
    word_spacing: 0,
    wrap_mode: "html-like",
    direction: "ltr",
    white_space: "break-spaces",
  };
}

function withFakeMeasureDom(run) {
  const originalDocument = globalThis.document;
  globalThis.document = {
    body: {
      appendChild() {},
    },
    createElement(tagName) {
      assert.equal(tagName, "div");
      return new FakeMeasureElement();
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

if (capabilities.render) {
  for (const diagram of api.supportedDiagrams()) {
    const fixtureName = fixtureNames[diagram];
    assert.ok(fixtureName, `missing fixture for ${diagram}`);
    const fixture = await readFile(
      path.join(
        repoRoot,
        "crates",
        "merman",
        "benches",
        "fixtures",
        `${fixtureName}.mmd`
      ),
      "utf8"
    );
    assert.match(api.renderSvg(fixture, deterministicTime), /<svg/);
  }
}

console.log(
  [
    "@mermanjs/web smoke passed",
    `entry=${entrySubpath}`,
    `diagrams=${api.supportedDiagrams().length}`,
    `render=${capabilities.render}`,
    `analysis=${capabilities.analysis}`,
    `ascii=${capabilities.ascii}`,
    `core_full=${capabilities.core_full}`,
    `ratex_math=${capabilities.ratex_math}`,
    `editor_language=${capabilities.editor_language}`,
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
    return;
  }

  const completions = api.editorCompletions(
    "flowchart TD\nA-->B\nC-->\n",
    { line: 2, character: 4 },
    editorUri
  );
  assert.ok(completions.items.some((item) => item.label === "B"));

  const diagnostics = api.editorDiagnostics(editorSource, deterministicTime, editorUri);
  assert.equal(Array.isArray(diagnostics.diagnostics), true);

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

  const legend = api.editorSemanticTokenLegend();
  assert.ok(legend.tokenTypes.length > 0);
  const semanticTokens = api.editorSemanticTokens(
    "flowchart TD\nAlpha-->Beta\nAlpha-->Gamma\n",
    editorUri
  );
  assert.ok(semanticTokens.length > 0);
  assert.ok(semanticTokens.every((token) => legend.tokenTypes.includes(token.tokenType)));

  for (const apiName of [
    "editorDiagnostics",
    "editorCodeActions",
    "editorCompletions",
    "editorHover",
    "editorDocumentSymbols",
    "editorWorkspaceSymbols",
    "editorDefinition",
    "editorReferences",
    "editorPrepareRename",
    "editorRename",
    "editorSemanticTokenLegend",
    "editorSemanticTokens",
  ]) {
    assert.equal(typeof exportedWasmModule[apiName], "function");
  }
}

function assertUnsupportedFormat(run) {
  let error = null;
  try {
    run();
  } catch (caught) {
    error = caught;
  }
  assert.ok(error, "expected MERMAN_UNSUPPORTED_FORMAT error");
  assert.equal(error.code_name, "MERMAN_UNSUPPORTED_FORMAT");
}

async function runSameProcessSurfaceSmoke() {
  const source = "flowchart TD\nA[Hello] --> B[World]";
  const options = {
    fixed_today: "2026-06-10",
    fixed_local_offset_minutes: 0,
    svg: { pipeline: "readable" },
    layout: { text_measurer: "deterministic" },
  };
  const core = await import(toPackageSpecifier("./core"));
  const full = await import(toPackageSpecifier("./full"));

  await core.initMerman({
    wasm: {
      module_or_path: await readFile(
        path.join(packageRoot, "pkg/core/merman_wasm_bg.wasm")
      ),
    },
  });
  assert.equal(core.bindingCapabilities().render, false);
  assert.equal(typeof core.renderSvg, "undefined");

  await full.initMerman({
    wasm: {
      module_or_path: await readFile(
        path.join(packageRoot, "pkg/full/merman_wasm_bg.wasm")
      ),
    },
  });
  assert.equal(full.bindingCapabilities().render, true);
  assert.match(full.renderSvg(source, options), /<svg/);
  assert.equal(core.bindingCapabilities().render, false);
  assert.equal(typeof core.renderSvg, "undefined");
}

function surfaceContractForEntry(subpath) {
  const trimmed = subpath.replace(/^\.\//, "").replace(/^\//, "");
  if (subpath === "." || trimmed === "index" || trimmed === "full") {
    return buildSurfaceContract(fullSurface);
  }

  const surface = surfaces.find((candidate) => candidate.entry === trimmed);
  if (!surface) {
    throw new Error(`unknown smoke entry subpath: ${subpath}`);
  }
  return buildSurfaceContract(surface);
}

function buildSurfaceContract(surface) {
  return {
    ...surface,
    render: surface.runtimeExportNames.includes("renderSvg"),
    analysis: surface.runtimeExportNames.includes("analyze"),
    ascii: surface.runtimeExportNames.includes("renderAscii"),
    editor: surface.runtimeExportNames.includes("editorDiagnostics"),
  };
}

function assertSurfaceExports(moduleApi, contract) {
  const expectedRuntimeExports = new Set(contract.runtimeExportNames);
  const expectedValueExports = new Set(contract.valueExportNames);

  for (const name of allSurfaceRuntimeExportNames) {
    assertExport(moduleApi, name, expectedRuntimeExports.has(name));
  }

  for (const name of allSurfaceValueExportNames) {
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

function toPackageSpecifier(subpath) {
  if (subpath === "." || subpath === "" || subpath === "./index") {
    return "@mermanjs/web";
  }
  if (subpath.startsWith("./")) {
    return `@mermanjs/web/${subpath.slice(2)}`;
  }
  return `@mermanjs/web/${subpath.replace(/^\//, "")}`;
}

function parseCli(inputArgs, root = packageRoot) {
  try {
    return parseSmokeCli(inputArgs, root);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    console.error(smokeUsage());
    process.exit(2);
  }
}

function surfaceSmokeCase(name, entry, pkgDirRel) {
  return {
    name,
    entry,
    pkgDirRel,
    wasmModuleSubpath: `./${pkgDirRel}/merman_wasm.js`,
    wasmBinaryRel: `${pkgDirRel}/merman_wasm_bg.wasm`,
    manifestRel: `${pkgDirRel}/merman_wasm_preset.json`,
  };
}
