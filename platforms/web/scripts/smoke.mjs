import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.join(packageRoot, "..", "..");

const api = await import(pathToFileURL(path.join(packageRoot, "dist", "index.js")).href);
const exportedWasmModule = await import("@merman/web/pkg/merman_wasm.js");

assert.equal(typeof exportedWasmModule.default, "function");
if (typeof import.meta.resolve === "function") {
  assert.match(
    import.meta.resolve("@merman/web/pkg/merman_wasm_bg.wasm"),
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
const options = {
  svg: { pipeline: "readable" },
  layout: { text_measurer: "deterministic" },
};

assert.equal(api.isMermanInitialized(), true);
assert.equal(Number.isInteger(api.abiVersion()), true);
assert.match(api.packageVersion(), /^\d+\.\d+\.\d+/);

const svg = api.renderSvg(source, options);
assert.match(svg, /<svg/);
assert.match(svg, /Hello/);

const ascii = api.renderAscii(source);
assert.match(ascii, /Hello/);
assert.match(ascii, /World/);

assert.equal(typeof api.parseObject(source), "object");
assert.equal(typeof api.layoutObject(source), "object");

const valid = api.validate(source);
assert.equal(valid.valid, true);
assert.equal(api.isBindingStatusCodeName(valid.code_name), true);

const invalid = api.validate("not a diagram");
assert.equal(invalid.valid, false);
assert.equal(api.isBindingStatusCodeName(invalid.code_name), true);

assert.match(api.encodeOptions(options), /deterministic/);
assert.throws(() => api.renderSvgElement(source), /requires a browser DOM/);

assert.deepEqual(api.themes(), [...api.SUPPORTED_THEMES]);
assert.deepEqual(api.supportedDiagrams(), [...api.SUPPORTED_DIAGRAMS]);

const asciiDiagrams = api.asciiSupportedDiagrams();
for (const diagram of asciiDiagrams) {
  assert.equal(api.isDiagramType(diagram), true);
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
  xychart: "xychart_medium",
  zenuml: "zenuml_medium",
};

for (const diagram of api.supportedDiagrams()) {
  const fixtureName = fixtureNames[diagram];
  assert.ok(fixtureName, `missing fixture for ${diagram}`);
  const fixture = await readFile(
    path.join(repoRoot, "crates", "merman", "benches", "fixtures", `${fixtureName}.mmd`),
    "utf8"
  );
  assert.match(api.renderSvg(fixture), /<svg/);
}

console.log(
  `@merman/web smoke passed (${api.supportedDiagrams().length} diagram fixtures)`
);
