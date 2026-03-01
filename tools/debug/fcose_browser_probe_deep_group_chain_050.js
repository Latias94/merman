// Browser (Puppeteer) probe: dump Cytoscape FCoSE stages for Architecture deep-group-chain fixture.
//
// Usage:
//   node tools/debug/fcose_browser_probe_deep_group_chain_050.js > /tmp/fcose.json
//
// Notes:
// - Runs inside Chromium via Puppeteer so Cytoscape can measure node dimensions correctly.
// - Uses the same xorshift64* seeding as `xtask` upstream baselines (ADR-0055).

const path = require("path");
const { createRequire } = require("module");

const workspaceRoot = path.resolve(__dirname, "..", "..");
const toolsRoot = path.join(workspaceRoot, "tools", "mermaid-cli");
const requireFromTools = createRequire(path.join(toolsRoot, "package.json"));

const puppeteer = requireFromTools("puppeteer");

function scriptPath(relFromToolsRoot) {
  return path.join(workspaceRoot, relFromToolsRoot);
}

function xorshiftSeedScript(seedStr) {
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
})();
`;
}

async function main() {
  const browser = await puppeteer.launch({
    headless: "shell",
    args: ["--no-sandbox", "--disable-setuid-sandbox"],
  });
  const page = await browser.newPage();

  await page.setViewport({ width: 1200, height: 800, deviceScaleFactor: 1 });

  // Seed RNG early.
  await page.evaluateOnNewDocument(xorshiftSeedScript("1"));

  // Load a minimal page.
  await page.setContent(
    `<!doctype html>
<html>
  <head><meta charset="utf-8"></head>
  <body>
    <div id="cy" style="width: 800px; height: 600px;"></div>
  </body>
</html>`,
    { waitUntil: "domcontentloaded" }
  );

  // Load Cytoscape + deps as browser UMD bundles.
  //
  // Mermaid's dependency tree includes these; we load them explicitly to make globals available:
  // - window.cytoscape
  // - window.coseBase
  // - window.cytoscapeFcose
  await page.addScriptTag({ path: scriptPath("repo-ref/cytoscape/package/dist/cytoscape.min.js") });
  await page.addScriptTag({ path: scriptPath("repo-ref/layout-base/layout-base.js") });
  await page.addScriptTag({ path: scriptPath("repo-ref/cose-base/cose-base.js") });
  await page.addScriptTag({ path: scriptPath("repo-ref/cytoscape.js-fcose/cytoscape-fcose.js") });

  const out = await page.evaluate(async () => {
    const cytoscape = window.cytoscape;
    const coseBase = window.coseBase;
    const cytoscapeFcose = window.cytoscapeFcose;
    if (!cytoscape) throw new Error("missing window.cytoscape");
    if (!coseBase) throw new Error("missing window.coseBase");
    if (!cytoscapeFcose) throw new Error("missing window.cytoscapeFcose");

    cytoscape.use(cytoscapeFcose);

    const iconSize = 80;
    const padding = 40;

    const elements = [
      // Groups
      { data: { id: "g1", type: "group", label: "G1" }, classes: "node-group" },
      { data: { id: "g2", type: "group", label: "G2", parent: "g1" }, classes: "node-group" },
      { data: { id: "g3", type: "group", label: "G3", parent: "g2" }, classes: "node-group" },
      { data: { id: "g4", type: "group", label: "G4", parent: "g3" }, classes: "node-group" },
      // Services
      { data: { id: "a", type: "service", parent: "g4", width: iconSize, height: iconSize }, classes: "node-service" },
      { data: { id: "b", type: "service", parent: "g3", width: iconSize, height: iconSize }, classes: "node-service" },
      { data: { id: "c", type: "service", parent: "g2", width: iconSize, height: iconSize }, classes: "node-service" },
      { data: { id: "d", type: "service", parent: "g1", width: iconSize, height: iconSize }, classes: "node-service" },
      // Edges
      { data: { id: "e0", source: "a", target: "b" } },
      { data: { id: "e1", source: "b", target: "c" } },
      { data: { id: "e2", source: "c", target: "d" } },
      { data: { id: "e3", source: "d", target: "a" } },
    ];

    const cy = cytoscape({
      container: document.getElementById("cy"),
      style: [
        {
          selector: "edge",
          style: { "curve-style": "straight" },
        },
        {
          selector: ".node-service",
          style: {
            width: "data(width)",
            height: "data(height)",
          },
        },
        {
          selector: ".node-group",
          style: { padding: `${padding}px` },
        },
      ],
      elements,
      layout: { name: "grid", boundingBox: { x1: 0, y1: 0, x2: 100, y2: 100 } },
    });

    function dumpCyPositions(tag) {
      const ids = ["a", "b", "c", "d"];
      const pos = {};
      for (const id of ids) {
        const p = cy.getElementById(id).position();
        pos[id] = { x: p.x, y: p.y };
      }
      return { tag, pos };
    }

    const stages = [];

    // Patch CoSELayout to capture "spectral-start" (before ConstraintHandler) and "pre-constraints".
    const CoSELayout = coseBase.CoSELayout;
    const classicOrig = CoSELayout.prototype.classicLayout;
    const initConstraintsOrig = CoSELayout.prototype.initConstraintVariables;
    const tickOrig = CoSELayout.prototype.tick;
    const moveNodesOrig = CoSELayout.prototype.moveNodes;

    CoSELayout.prototype.classicLayout = function () {
      stages.push({
        tag: `run${window.__probeRun}.spectral`,
        nodes: this.getAllNodes()
          .filter((n) => n.getChild() == null)
          .map((n) => ({ id: n.id, x: n.getCenterX(), y: n.getCenterY() })),
      });
      return classicOrig.apply(this, arguments);
    };
    CoSELayout.prototype.initConstraintVariables = function () {
      stages.push({
        tag: `run${window.__probeRun}.pre_constraints`,
        nodes: this.getAllNodes()
          .filter((n) => n.getChild() == null)
          .map((n) => ({ id: n.id, x: n.getCenterX(), y: n.getCenterY() })),
      });
      return initConstraintsOrig.apply(this, arguments);
    };
    CoSELayout.prototype.tick = function () {
      const ended = tickOrig.apply(this, arguments);
      if (this.totalIterations === 1) {
        stages.push({
          tag: `run${window.__probeRun}.iter1`,
          nodes: this.getAllNodes()
            .filter((n) => n.getChild() == null)
            .map((n) => ({ id: n.id, x: n.getCenterX(), y: n.getCenterY() })),
          coolingFactor: this.coolingFactor,
          maxNodeDisplacement: this.maxNodeDisplacement,
          totalDisplacement: this.totalDisplacement,
        });
      }
      if (ended) {
        stages.push({
          tag: `run${window.__probeRun}.end`,
          totalIterations: this.totalIterations,
          totalDisplacement: this.totalDisplacement,
          coolingFactor: this.coolingFactor,
        });
      }
      return ended;
    };

    CoSELayout.prototype.moveNodes = function () {
      if (this.totalIterations === 1) {
        stages.push({
          tag: `run${window.__probeRun}.forces1`,
          nodes: this.getAllNodes().map((n) => ({
            id: n.id,
            owner: n.getOwner() && n.getOwner().getParent ? (n.getOwner().getParent() ? n.getOwner().getParent().id : "root") : "unknown",
            compound: n.getChild() != null,
            spring: [n.springForceX, n.springForceY],
            rep: [n.repulsionForceX, n.repulsionForceY],
            grav: [n.gravitationForceX, n.gravitationForceY],
          })),
          edges: this.getAllEdges().map((e) => ({
            id: e.id,
            source: e.getSource().id,
            target: e.getTarget().id,
            idealLength: e.idealLength,
            edgeElasticity: e.edgeElasticity,
            length: e.getLength ? e.getLength() : e.length,
            lengthX: e.lengthX,
            lengthY: e.lengthY,
          })),
        });
      }
      return moveNodesOrig.apply(this, arguments);
    };

    const opts = {
      name: "fcose",
      quality: "proof",
      styleEnabled: false,
      animate: false,
      nodeDimensionsIncludeLabels: false,
      idealEdgeLength(edge) {
        const [na, nb] = edge.connectedNodes();
        const pa = na.data("parent");
        const pb = nb.data("parent");
        return pa === pb ? 1.5 * iconSize : 0.5 * iconSize;
      },
      edgeElasticity(edge) {
        const [na, nb] = edge.connectedNodes();
        const pa = na.data("parent");
        const pb = nb.data("parent");
        return pa === pb ? 0.45 : 0.001;
      },
      alignmentConstraint: {
        horizontal: [["a", "b"], ["d", "c"]],
        vertical: [["a", "d"]],
      },
      relativePlacementConstraint: [
        { left: "b", right: "a", gap: 120.0 },
        { top: "d", bottom: "a", gap: 120.0 },
        { left: "d", right: "c", gap: 120.0 },
      ],
      fit: false,
      padding: 0,
    };

    const layout = cy.layout(opts);

    window.__probeRun = 1;
    layout.run();
    stages.push({ tag: "run1.final", ...dumpCyPositions("run1.final") });

    window.__probeRun = 2;
    layout.run();
    stages.push({ tag: "run2.final", ...dumpCyPositions("run2.final") });

    return { stages };
  });

  console.log(JSON.stringify(out, null, 2));

  await browser.close();
}

main().catch((e) => {
  console.error(e && e.stack ? e.stack : String(e));
  process.exit(1);
});
