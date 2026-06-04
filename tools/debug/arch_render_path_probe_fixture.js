// Browser (Puppeteer) probe: instrument the actual Mermaid Architecture render path.
//
// Usage:
//   node tools/debug/arch_render_path_probe_fixture.js stress_architecture_junction_fork_join_026
//
// Notes:
// - Unlike arch_fcose_browser_probe_fixture_025.js, this runs mermaid.render(...).
// - It patches the installed Mermaid 11.15 IIFE in memory, so the captured Cytoscape state comes
//   from the same bundled Architecture renderer path used by the upstream SVG baseline wrapper.
// - This is diagnostic-only and does not modify node_modules on disk.

const fs = require("fs");
const path = require("path");
const url = require("url");
const { createRequire } = require("module");

const workspaceRoot = path.resolve(__dirname, "..", "..");
const toolsRoot = path.join(workspaceRoot, "tools", "mermaid-cli");
const requireFromTools = createRequire(path.join(toolsRoot, "package.json"));

const puppeteer = requireFromTools("puppeteer");

function parseInitDirective(code) {
  const match = code.match(/^\s*%%\{\s*init\s*:\s*(.*?)\s*\}\s*%%/m);
  if (!match) return {};
  return JSON.parse(match[1]);
}

function deterministicPagePreludeScript(seedStr) {
  return `
(() => {
  const mask64 = (1n << 64n) - 1n;
  let state = (BigInt(${JSON.stringify(seedStr)}) & mask64);
  if (state === 0n) state = 1n;
  function nextU64() {
    let x = state;
    x ^= (x >> 12n);
    x ^= (x << 25n) & mask64;
    x ^= (x >> 27n);
    state = x;
    return (x * 0x2545F4914F6CDD1Dn) & mask64;
  }
  function nextF64() {
    const u = nextU64() >> 11n;
    return Number(u) / 9007199254740992;
  }
  Math.random = nextF64;

  if (globalThis.crypto && typeof globalThis.crypto.getRandomValues === 'function') {
    const orig = globalThis.crypto.getRandomValues.bind(globalThis.crypto);
    globalThis.crypto.getRandomValues = (arr) => {
      if (!arr || typeof arr.length !== 'number') {
        return orig(arr);
      }
      try {
        const bytes = new Uint8Array(arr.buffer, arr.byteOffset || 0, arr.byteLength || 0);
        for (let i = 0; i < bytes.length; i++) {
          bytes[i] = Math.floor(nextF64() * 256);
        }
        return arr;
      } catch (e) {
        return orig(arr);
      }
    };
  }
})();
`;
}

function probeInstallScript() {
  return `
(() => {
  const probe = {
    kind: "architecture-render-path",
    stages: [],
    errors: [],
  };
  globalThis.__mermanArchRenderPathProbe = probe;

  function cloneMetricObject(value) {
    if (value == null) return value;
    try {
      return JSON.parse(JSON.stringify(value));
    } catch (e) {
      return { error: String(e && e.message ? e.message : e) };
    }
  }

  function safeRect(rect) {
    if (!rect) return null;
    return {
      x1: Number(rect.x1),
      y1: Number(rect.y1),
      x2: Number(rect.x2),
      y2: Number(rect.y2),
      w: Number(rect.w),
      h: Number(rect.h),
    };
  }

  function dumpElements(cy) {
    const nodes = [];
    for (const n of cy.nodes().toArray()) {
      let childrenBoundingBoxIncludeLabels = null;
      let childrenBoundingBoxBodyOnly = null;
      if (n.isParent && n.isParent()) {
        try {
          childrenBoundingBoxIncludeLabels = safeRect(
            n.children().boundingBox({
              includeLabels: true,
              includeOverlays: false,
              useCache: false,
            })
          );
          childrenBoundingBoxBodyOnly = safeRect(
            n.children().boundingBox({
              includeLabels: false,
              includeOverlays: false,
              useCache: false,
            })
          );
        } catch (e) {
          childrenBoundingBoxIncludeLabels = { error: String(e && e.message ? e.message : e) };
          childrenBoundingBoxBodyOnly = { error: String(e && e.message ? e.message : e) };
        }
      }

      const p = n.position();
      const scratch = n._private?.rscratch ?? {};
      const style = n._private?.rstyle ?? {};
      nodes.push({
        id: n.id(),
        data: n.data ? cloneMetricObject(n.data()) : null,
        classes: n.classes ? n.classes() : undefined,
        pos: { x: p.x, y: p.y },
        bb: safeRect(n.boundingBox()),
        bodyBounds: cloneMetricObject(n._private?.bodyBounds ?? {}),
        labelBounds: cloneMetricObject(n._private?.labelBounds ?? {}),
        metrics: {
          width: n.width ? n.width() : undefined,
          height: n.height ? n.height() : undefined,
          outerWidth: n.outerWidth ? n.outerWidth() : undefined,
          outerHeight: n.outerHeight ? n.outerHeight() : undefined,
          padding: n.padding ? n.padding() : undefined,
          autoWidth: n._private?.autoWidth,
          autoHeight: n._private?.autoHeight,
          autoPadding: n._private?.autoPadding,
          labelX: scratch.labelX ?? style.labelX,
          labelY: scratch.labelY ?? style.labelY,
          labelWidth: scratch.labelWidth ?? style.labelWidth,
          labelHeight: scratch.labelHeight ?? style.labelHeight,
          labelLineHeight: scratch.labelLineHeight,
        },
        childrenBoundingBoxIncludeLabels,
        childrenBoundingBoxBodyOnly,
      });
    }

    const edges = [];
    for (const e of cy.edges().toArray()) {
      edges.push({
        id: e.id(),
        data: e.data ? cloneMetricObject(e.data()) : null,
        classes: e.classes ? e.classes() : undefined,
        bb: safeRect(e.boundingBox()),
        sourceEndpoint: e.sourceEndpoint ? e.sourceEndpoint() : null,
        targetEndpoint: e.targetEndpoint ? e.targetEndpoint() : null,
        style: {
          curveStyle: e.style ? e.style("curve-style") : undefined,
          segmentWeights: e.style ? e.style("segment-weights") : undefined,
          segmentDistances: e.style ? e.style("segment-distances") : undefined,
          edgeDistances: e.style ? e.style("edge-distances") : undefined,
        },
      });
    }

    return {
      graphBoundingBox: safeRect(cy.elements().boundingBox()),
      nodes,
      edges,
    };
  }

  globalThis.__mermanArchRenderPathProbeDump = (tag, cy, extra) => {
    try {
      probe.stages.push({
        tag,
        time: performance.now(),
        extra: extra ? cloneMetricObject(extra) : null,
        elements: dumpElements(cy),
      });
    } catch (e) {
      probe.errors.push({
        tag,
        error: String(e && e.message ? e.message : e),
      });
    }
  };
})();
`;
}

function replaceOnce(source, marker, replacement, label) {
  if (!source.includes(marker)) {
    throw new Error(`unable to instrument Mermaid bundle: missing ${label}`);
  }
  return source.replace(marker, replacement);
}

function instrumentMermaidBundle(source) {
  let out = source;

  out = replaceOnce(
    out,
    `      const layout7 = cy.layout({\n        name: "fcose",`,
    `      try {
        globalThis.__mermanArchRenderPathProbeDump && globalThis.__mermanArchRenderPathProbeDump("layout-before-run1", cy, {
          config: {
            iconSize,
            sameGroupIdealLength,
            crossGroupIdealLength,
            sameGroupElasticity,
            randomize: db10.getConfigField("randomize"),
            nodeSeparation: db10.getConfigField("nodeSeparation"),
            numIter: db10.getConfigField("numIter"),
            padding: db10.getConfigField("padding"),
            fontSize: db10.getConfigField("fontSize"),
          },
          constraints: { alignmentConstraint, relativePlacementConstraint },
        });
      } catch (e) {}
      const layout7 = cy.layout({\n        name: "fcose",`,
    "Architecture layout creation"
  );

  out = replaceOnce(
    out,
    `      layout7.one("layoutstop", () => {\n        function getSegmentWeights(source, target, pointX, pointY) {`,
    `      layout7.one("layoutstop", () => {
        try {
          globalThis.__mermanArchRenderPathProbeDump && globalThis.__mermanArchRenderPathProbeDump("layoutstop-run1-before-segments", cy, null);
        } catch (e) {}
        function getSegmentWeights(source, target, pointX, pointY) {`,
    "Architecture first layoutstop"
  );

  out = replaceOnce(
    out,
    `        cy.endBatch();\n        layout7.run();`,
    `        cy.endBatch();
        try {
          globalThis.__mermanArchRenderPathProbeDump && globalThis.__mermanArchRenderPathProbeDump("layoutstop-run1-after-segments-before-run2", cy, null);
        } catch (e) {}
        layout7.run();`,
    "Architecture segment adjustment"
  );

  out = replaceOnce(
    out,
    `      cy.ready((e3) => {\n        log.info("Ready", e3);\n        resolve2(cy);\n      });`,
    `      cy.ready((e3) => {
        try {
          globalThis.__mermanArchRenderPathProbeDump && globalThis.__mermanArchRenderPathProbeDump("cy-ready-before-resolve", cy, null);
        } catch (e) {}
        log.info("Ready", e3);
        resolve2(cy);
      });`,
    "Architecture cy.ready resolve"
  );

  out = replaceOnce(
    out,
    `        const cy = await layoutArchitecture(services, junctions, groups, edges3, db10, ds);\n        await drawEdges(edgesElem, cy, db10, id35);`,
    `        const cy = await layoutArchitecture(services, junctions, groups, edges3, db10, ds);
        try {
          globalThis.__mermanArchRenderPathProbeDump && globalThis.__mermanArchRenderPathProbeDump("draw-after-layout-before-svg-emission", cy, null);
        } catch (e) {}
        await drawEdges(edgesElem, cy, db10, id35);`,
    "Architecture draw after layout"
  );

  out = replaceOnce(
    out,
    `        positionNodes(db10, cy);\n        setupGraphViewbox(void 0, svg2, db10.getConfigField("padding"), db10.getConfigField("useMaxWidth"));`,
    `        positionNodes(db10, cy);
        try {
          globalThis.__mermanArchRenderPathProbeDump && globalThis.__mermanArchRenderPathProbeDump("draw-after-position-nodes-before-viewbox", cy, null);
        } catch (e) {}
        setupGraphViewbox(void 0, svg2, db10.getConfigField("padding"), db10.getConfigField("useMaxWidth"));`,
    "Architecture draw after positionNodes"
  );

  return out;
}

function parseSvgFacts(svgText, stem) {
  if (typeof svgText !== "string") return null;
  const viewBox = svgText.match(/\bviewBox="([^"]+)"/)?.[1] ?? null;
  const maxWidth = svgText.match(/max-width:\s*([0-9.]+)px/)?.[1] ?? null;
  const groups = {};
  const groupRe = new RegExp(
    `<rect\\b(?=[^>]*\\bid="${stem}-group-([^"]+)")[^>]*\\bx="([^"]+)"[^>]*\\by="([^"]+)"[^>]*\\bwidth="([^"]+)"[^>]*\\bheight="([^"]+)"`,
    "g"
  );
  let groupMatch;
  while ((groupMatch = groupRe.exec(svgText))) {
    groups[groupMatch[1]] = {
      x: Number(groupMatch[2]),
      y: Number(groupMatch[3]),
      w: Number(groupMatch[4]),
      h: Number(groupMatch[5]),
    };
  }

  const services = {};
  const serviceRe = new RegExp(
    `<g\\b(?=[^>]*\\bid="${stem}-service-([^"]+)")[^>]*\\btransform="translate\\(([^,]+),([^\\)]+)\\)"`,
    "g"
  );
  let serviceMatch;
  while ((serviceMatch = serviceRe.exec(svgText))) {
    services[serviceMatch[1]] = {
      x: Number(serviceMatch[2]),
      y: Number(serviceMatch[3]),
    };
  }

  return {
    viewBox,
    maxWidth: maxWidth == null ? null : Number(maxWidth),
    groups,
    services,
  };
}

function readPackageVersion(packagePath) {
  return JSON.parse(fs.readFileSync(packagePath, "utf8")).version;
}

async function main() {
  const fixtureStem = process.argv[2] || "stress_architecture_junction_fork_join_026";
  const fixtureMmd = path.join(workspaceRoot, "fixtures", "architecture", `${fixtureStem}.mmd`);
  const upstreamSvg = path.join(
    workspaceRoot,
    "fixtures",
    "upstream-svgs",
    "architecture",
    `${fixtureStem}.svg`
  );
  const code = fs.readFileSync(fixtureMmd, "utf8");
  const initConfig = parseInitDirective(code);

  const mermaidHtmlPath = path.join(
    toolsRoot,
    "node_modules",
    "@mermaid-js",
    "mermaid-cli",
    "dist",
    "index.html"
  );
  const mermaidIifePath = path.join(toolsRoot, "node_modules", "mermaid", "dist", "mermaid.js");
  const mermaidSource = fs.readFileSync(mermaidIifePath, "utf8");
  const instrumentedMermaidSource = instrumentMermaidBundle(mermaidSource);

  const browser = await puppeteer.launch({
    headless: "shell",
    timeout: 120000,
    args: ["--no-sandbox", "--disable-setuid-sandbox", "--allow-file-access-from-files"],
  });
  const page = await browser.newPage();
  await page.setViewport({ width: 800, height: 600, deviceScaleFactor: 1 });
  await page.evaluateOnNewDocument(deterministicPagePreludeScript("1"));
  await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
  await page.addScriptTag({ content: probeInstallScript() });
  await page.addScriptTag({ content: instrumentedMermaidSource });

  const result = await page.evaluate(async ({ code, initConfig, fixtureStem }) => {
    const mermaid = globalThis.mermaid;
    if (!mermaid) throw new Error("global mermaid instance not found");

    if (document.fonts && typeof document.fonts[Symbol.iterator] === "function") {
      await Promise.all(Array.from(document.fonts, (font) => font.load()));
    }

    mermaid.initialize({ startOnLoad: false, ...initConfig });
    const container = document.getElementById("container") || document.body;
    container.innerHTML = "";
    container.style.width = "800px";

    const rendered = await mermaid.render(fixtureStem, code, container);
    let svgText =
      typeof rendered === "string"
        ? rendered
        : Array.isArray(rendered)
          ? rendered[0]
          : rendered && rendered.svg;
    if (typeof svgText !== "string") {
      const domSvg = container.querySelector && container.querySelector("svg");
      svgText = domSvg && typeof domSvg.outerHTML === "string" ? domSvg.outerHTML : "";
    }

    container.innerHTML = svgText;
    const svgEl = container.getElementsByTagName?.("svg")?.[0];
    const serialized = svgEl ? new XMLSerializer().serializeToString(svgEl) : svgText;

    await new Promise((resolve) => setTimeout(resolve, 250));

    return {
      svg: serialized,
      probe: globalThis.__mermanArchRenderPathProbe,
    };
  }, { code, initConfig, fixtureStem });

  const storedSvgText = fs.existsSync(upstreamSvg) ? fs.readFileSync(upstreamSvg, "utf8") : "";
  const fcoseRoot = path.join(toolsRoot, "node_modules", "cytoscape-fcose");

  console.log(
    JSON.stringify(
      {
        fixture: fixtureStem,
        versions: {
          mermaid: readPackageVersion(path.join(toolsRoot, "node_modules", "mermaid", "package.json")),
          cytoscape: readPackageVersion(path.join(toolsRoot, "node_modules", "cytoscape", "package.json")),
          cytoscapeFcose: readPackageVersion(path.join(fcoseRoot, "package.json")),
          coseBase: readPackageVersion(path.join(fcoseRoot, "node_modules", "cose-base", "package.json")),
          layoutBase: readPackageVersion(path.join(fcoseRoot, "node_modules", "layout-base", "package.json")),
        },
        renderedFacts: parseSvgFacts(result.svg, fixtureStem),
        storedFacts: parseSvgFacts(storedSvgText, fixtureStem),
        probe: result.probe,
      },
      null,
      2
    )
  );

  await browser.close();
}

main().catch((e) => {
  console.error(e && e.stack ? e.stack : String(e));
  process.exit(1);
});
