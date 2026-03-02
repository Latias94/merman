// Node (headless Cytoscape) probe: capture CoSELayout checkpoints for Architecture fixture 025.
//
// Usage:
//   node tools/debug/arch_fcose_node_probe_fixture_025.js > /tmp/arch025_fcose_node.json
//
// Notes:
// - Uses `tools/mermaid-cli` dependency tree to match Mermaid baseline versions.
// - IMPORTANT: `cose-base` is resolved from `cytoscape-fcose`'s nested `node_modules` so that
//   prototype hooks affect the exact implementation used by the layout.
// - This is *not* a browser run, so it won't match SVG baselines 1:1. It is meant for step-by-step
//   parity debugging against our Rust port (`manatee`).

const path = require("path");
const { createRequire } = require("module");

const workspaceRoot = path.resolve(__dirname, "..", "..");
const toolsRoot = path.join(workspaceRoot, "tools", "mermaid-cli");
const requireTools = createRequire(path.join(toolsRoot, "package.json"));
const requireFcose = createRequire(path.join(toolsRoot, "node_modules", "cytoscape-fcose", "package.json"));

function seedMathRandomXorShift64Star(seedStr) {
  const mask64 = (1n << 64n) - 1n;
  let state = (BigInt(seedStr) & mask64);
  if (state === 0n) state = 1n;

  function nextU64() {
    let x = state;
    x ^= (x >> 12n);
    x ^= (x << 25n) & mask64;
    x ^= (x >> 27n);
    state = x;
    return (x * 0x2545f4914f6cdd1dn) & mask64;
  }

  function nextF64() {
    const u = nextU64() >> 11n;
    return Number(u) / 9007199254740992;
  }

  globalThis.__randCalls = 0;
  Math.random = () => {
    globalThis.__randCalls++;
    return nextF64();
  };
}

async function main() {
  seedMathRandomXorShift64Star("1");
  // Match `manatee` spectral sampler offset: upstream Mermaid baselines can consume a small
  // amount of `Math.random()` before the spectral sampler starts (see ADR-0055 notes).
  Math.random();

  const cytoscape = requireTools("cytoscape");
  const cytoscapeFcose = requireTools("cytoscape-fcose");
  const coseBase = requireFcose("cose-base");

  cytoscape.use(cytoscapeFcose);

  const iconSize = 80;
  const padding = 40;

  const services = ["a", "b", "c", "d", "e", "f"];
  const groups = ["g1", "g2", "g3", "g4", "g5", "g6"];
  const edges = [
    ["a", "b"],
    ["b", "c"],
    ["c", "d"],
    ["d", "e"],
    ["e", "f"],
    ["f", "a"],
  ];

  const elements = [];
  for (const g of groups) elements.push({ data: { id: g } });
  for (let i = 0; i < services.length; i++) {
    elements.push({ data: { id: services[i], parent: groups[i], w: iconSize, h: iconSize } });
  }
  let eid = 0;
  for (const [s, t] of edges) elements.push({ data: { id: `e${eid++}`, source: s, target: t } });

  function createCy() {
    return cytoscape({
      headless: true,
      styleEnabled: true,
      elements,
      style: [
        { selector: "node[w][h]", style: { width: "data(w)", height: "data(h)" } },
        { selector: ":parent", style: { padding: `${padding}px` } },
      ],
    });
  }

  // Constraints extracted from Mermaid ArchitectureDB for fixture 025.
  const alignmentConstraint = {
    horizontal: [["a", "f"], ["c", "d"]],
    vertical: [["b", "c"], ["d", "e"]],
  };
  const relativePlacementConstraint = [
    { left: "a", right: "f", gap: 120 },
    { left: "e", right: "b", gap: 120 },
    { top: "b", bottom: "c", gap: 120 },
    { top: "e", bottom: "d", gap: 120 },
    { left: "d", right: "c", gap: 120 },
  ];

  const checkpoints = {
    tickCalls: 0,
    iter1: [],
    constraints: [],
    randCallsAt: [],
  };

  // Hook ConstraintHandler.handleConstraints(layout).
  const origHandle = coseBase.ConstraintHandler.handleConstraints;
  coseBase.ConstraintHandler.handleConstraints = function (layout) {
    const before = layout?.getPositionsData ? layout.getPositionsData() : null;
    const res = origHandle(layout);
    const after = layout?.getPositionsData ? layout.getPositionsData() : null;
    checkpoints.constraints.push({ before, after });
    return res;
  };

  // Hook CoSELayout.tick().
  const origTick = coseBase.CoSELayout.prototype.tick;
  coseBase.CoSELayout.prototype.tick = function () {
    checkpoints.tickCalls++;
    const res = origTick.call(this);
    if (this.totalIterations === 1 && checkpoints.iter1.length < 2) {
      checkpoints.iter1.push({ run: checkpoints.iter1.length, positions: this.getPositionsData() });
    }
    return res;
  };

  const opts = {
    name: "fcose",
    quality: "proof",
    randomize: true,
    animate: false,
    nodeDimensionsIncludeLabels: false,
    idealEdgeLength(edge) {
      const [a, b] = edge.connectedNodes();
      return a.data("parent") === b.data("parent") ? 1.5 * iconSize : 0.5 * iconSize;
    },
    edgeElasticity(edge) {
      const [a, b] = edge.connectedNodes();
      return a.data("parent") === b.data("parent") ? 0.45 : 0.001;
    },
    alignmentConstraint,
    relativePlacementConstraint,
  };

  async function runOnce() {
    const cy = createCy();
    const bb0 = cy.elements().boundingBox();
    const center0 = { x: bb0.x1 + bb0.w / 2, y: bb0.y1 + bb0.h / 2 };
    const layout = cy.layout(opts);
    await new Promise((resolve) => {
      layout.on("layoutstop", resolve);
      layout.run();
    });
    const bb1 = cy.elements().boundingBox();
    const center1 = { x: bb1.x1 + bb1.w / 2, y: bb1.y1 + bb1.h / 2 };
    const out = {};
    for (const id of services) out[id] = cy.getElementById(id).position();
    cy.destroy();
    return { positions: out, bbBefore: bb0, centerBefore: center0, bbAfter: bb1, centerAfter: center1 };
  }

  const run1 = await runOnce();
  checkpoints.randCallsAt.push({ label: "after_run1", calls: globalThis.__randCalls });
  const run2 = await runOnce();
  checkpoints.randCallsAt.push({ label: "after_run2", calls: globalThis.__randCalls });

  console.log(
    JSON.stringify(
      {
        iconSize,
        padding,
        constraints: { alignmentConstraint, relativePlacementConstraint },
        checkpoints,
        randCalls: globalThis.__randCalls,
        run1,
        run2,
      },
      null,
      2
    )
  );
}

main().catch((e) => {
  console.error(e && e.stack ? e.stack : String(e));
  process.exit(1);
});
