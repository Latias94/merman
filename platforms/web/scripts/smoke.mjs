import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.join(packageRoot, "..", "..");

const api = await import(pathToFileURL(path.join(packageRoot, "dist", "index.js")).href);
const exportedWasmModule = await import("@mermanjs/web/pkg/merman_wasm.js");

assert.equal(typeof exportedWasmModule.default, "function");
if (typeof import.meta.resolve === "function") {
  assert.match(
    import.meta.resolve("@mermanjs/web/pkg/merman_wasm_bg.wasm"),
    /merman_wasm_bg\.wasm$/
  );
}

await api.initMerman({
  wasm: {
    module_or_path: await readFile(
      path.join(packageRoot, "pkg", "merman_wasm_bg.wasm")
    ),
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
assert.equal(Number.isInteger(api.abiVersion()), true);
assert.match(api.packageVersion(), /^\d+\.\d+\.\d+/);
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

const capabilities = api.bindingCapabilities();
assert.equal(typeof capabilities.render, "boolean");
assert.equal(typeof capabilities.ascii, "boolean");
assert.equal(typeof capabilities.core_full, "boolean");
assert.equal(typeof capabilities.core_host, "boolean");
assert.equal(typeof capabilities.ratex_math, "boolean");

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

if (capabilities.render) {
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

  assert.equal(typeof api.parseObject(source, deterministicTime), "object");
  assert.equal(typeof api.layoutObject(source, options), "object");

  const valid = api.validate(source, deterministicTime);
  assert.equal(valid.valid, true);
  assert.equal(api.isBindingStatusCodeName(valid.code_name), true);

  const invalid = api.validate("not a diagram", deterministicTime);
  assert.equal(invalid.valid, false);
  assert.equal(api.isBindingStatusCodeName(invalid.code_name), true);
} else {
  const unsupported = api.validate(source, deterministicTime);
  assert.equal(unsupported.valid, false);
  assert.equal(unsupported.code_name, "MERMAN_UNSUPPORTED_FORMAT");
}

if (capabilities.ascii) {
  const ascii = api.renderAscii(source, deterministicTime);
  assert.match(ascii, /Hello/);
  assert.match(ascii, /World/);
} else {
  assert.deepEqual(api.asciiSupportedDiagrams(), []);
}

assert.match(api.encodeOptions(options), /deterministic/);
assert.throws(() => api.renderSvgElement(source), /requires a browser DOM/);

assert.deepEqual(api.supportedThemes(), [...api.SUPPORTED_THEMES]);
if (capabilities.render) {
  assert.deepEqual(api.supportedHostThemePresets(), [
    ...api.SUPPORTED_HOST_THEME_PRESETS,
  ]);
} else {
  assert.deepEqual(api.supportedHostThemePresets(), []);
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

const asciiDiagrams = api.asciiSupportedDiagrams();
for (const diagram of asciiDiagrams) {
  assert.equal(api.isDiagramType(diagram), true);
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
    `diagrams=${api.supportedDiagrams().length}`,
    `render=${capabilities.render}`,
    `ascii=${capabilities.ascii}`,
    `core_full=${capabilities.core_full}`,
    `ratex_math=${capabilities.ratex_math}`,
  ].join(" ")
);
