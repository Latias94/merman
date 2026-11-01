import * as api from "__MERMAN_PACKAGE__";
import wasmAssetUrl from "__MERMAN_PACKAGE__/wasm?url";

export function require(condition, message) {
  if (!condition) throw new Error(message);
}

export async function exercise(realm) {
  require(realm === "main"
    ? typeof window !== "undefined" && typeof document !== "undefined"
    : typeof WorkerGlobalScope === "function" && self instanceof WorkerGlobalScope,
  `Expected the actual browser ${realm} realm`);
  const wasm = new URL(wasmAssetUrl, self.location.href);
  wasm.searchParams.set("consumer", realm);
  const initStarted = performance.now();
  const module = await api.initMerman({ wasm: wasm.href });
  const initMs = performance.now() - initStarted;
  const exports = await module.default();
  require(exports.memory instanceof WebAssembly.Memory, "Initialized module has no linear memory");
  const initialMemoryBytes = exports.memory.buffer.byteLength;
  const catalog = api.runtimeCatalog();
  const capabilities = catalog.capabilities.capability_ids;
  const operations = [];
  const output = {};
  const id = __MERMAN_PACKAGE_ID__;
  const source = "flowchart TD\nA[Start] --> B[Done]";
  const operationsStarted = performance.now();
  if (id === "full" || id === "render") {
    for (const capability of ["svg", "math", "layout-cytoscape", "layout-elk"]) {
      require(capabilities.includes(capability), `Missing ${capability} capability`);
    }
    const options = {
      site_config: { htmlLabels: true, layout: "elk" },
      environment: { text_measurement: "deterministic" },
    };
    const plan = api.svgPlanJson(source, options);
    require(plan.ready && plan.required_capability_ids.includes("layout-elk"),
      "Explicit ELK plan is not executable");
    const renderStarted = performance.now();
    output.mathSvg = api.renderSvg('flowchart TD\nA["$$x^2$$"] --> B[Done]', options);
    output.firstMathRenderMs = performance.now() - renderStarted;
    output.elkSvg = api.renderSvg(source, options);
    operations.push("svg-plan-json:elk", "svg:math", "svg:elk");
  } else if (id === "analysis") {
    require(capabilities.includes("analysis") && !capabilities.includes("svg"),
      "Expected the independent analysis package");
    const analysis = api.analyzeDocument(
      "# Installed consumer\n\n```mermaid\nflowchart TD\nA-->\n```\n", "file:///consumer.md",
    );
    require(!analysis.valid && analysis.source.kind === "markdown"
      && analysis.diagnostics.some(diagnostic => diagnostic.span?.line === 5),
    "Analysis lost the invalid diagram or host-document diagnostic coordinates");
    output.diagnosticCount = analysis.diagnostics.length;
    operations.push("document-analysis-json");
  } else if (id === "editor") {
    require(capabilities.includes("editor") && !capabilities.includes("svg"),
      "Expected the independent editor package");
    const session = api.createEditorSession(source, 1, "file:///consumer.mmd");
    try {
      require(session.searchDocumentSymbols("A").some(symbol => symbol.name === "A"),
        "Editor session did not expose the source symbol");
      session.update("flowchart TD\nA-->B\nB-->C", 2);
      require(session.version === 2
        && session.searchDocumentSymbols("C").some(symbol => symbol.name === "C"),
      "Editor session did not apply the document update");
      output.editorVersion = session.version;
      operations.push("createEditorSession", "searchDocumentSymbols", "update");
    } finally {
      session.dispose();
    }
  } else if (id === "ascii") {
    require(capabilities.includes("ascii") && !capabilities.includes("svg")
      && !capabilities.includes("analysis"), "Expected the independent ASCII package");
    const ascii = api.renderAscii(source);
    require(ascii.includes("Start") && ascii.includes("Done") && ascii.includes("\n"),
      "ASCII output lost its nodes or diagram structure");
    output.asciiLength = ascii.length;
    operations.push("ascii");
  } else {
    throw new Error(`Unknown consumer package: ${id}`);
  }
  await api.initMerman();
  require(api.isMermanInitialized(), "Repeated initialization lost the installed runtime");
  return {
    realm, version: catalog.package_version, capabilities, operations,
    observations: {
      initMs, operationsMs: performance.now() - operationsStarted,
      initialMemoryBytes, finalMemoryBytes: exports.memory.buffer.byteLength,
    },
    wasmUrl: wasm.href, defaultWasmUrl: api.MERMAN_WASM_URL, ...output,
  };
}
