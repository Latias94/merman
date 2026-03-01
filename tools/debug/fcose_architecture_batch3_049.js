// Debug helper: reproduce Cytoscape.js FCoSE positions for a single Architecture fixture.
//
// Usage:
//   NODE_PATH=repo-ref node tools/debug/fcose_architecture_batch3_049.js
//
// Notes:
// - Uses vendored upstream sources under `repo-ref/` (no npm install).
// - Seeds `Math.random()` with the same xorshift64* used by `xtask` (ADR-0055).

function makeXorShift64Star(seedStr) {
  const mask64 = (1n << 64n) - 1n;
  let state = BigInt(seedStr) & mask64;
  if (state === 0n) state = 1n;

  function nextU64() {
    let x = state;
    x ^= x >> 12n;
    x ^= (x << 25n) & mask64;
    x ^= x >> 27n;
    state = x;
    return (x * 0x2545f4914f6cdd1dn) & mask64;
  }

  function nextF64() {
    const u = nextU64() >> 11n;
    return Number(u) / 9007199254740992; // 2^53
  }

  return { nextF64 };
}

const cytoscape = require("../../repo-ref/cytoscape/package/dist/cytoscape.cjs.js");
const fcose = require("../../repo-ref/cytoscape.js-fcose/cytoscape-fcose.js");

fcose(cytoscape);

const rng = makeXorShift64Star("1");
Math.random = rng.nextF64;

function buildCy() {
  const elements = [
    // Groups
    { data: { id: "left" } },
    { data: { id: "right" } },
    // Services
    { data: { id: "l1", parent: "left" } },
    { data: { id: "l2", parent: "left" } },
    { data: { id: "r1", parent: "right" } },
    { data: { id: "r2", parent: "right" } },
    // Edges
    { data: { id: "e0", source: "l1", target: "r1" } },
    { data: { id: "e1", source: "l1", target: "r2" } },
    { data: { id: "e2", source: "l2", target: "r1" } },
    { data: { id: "e3", source: "l2", target: "r2" } },
  ];

  return cytoscape({
    headless: true,
    elements,
    style: [
      { selector: "node", style: { width: 80, height: 80 } },
      { selector: ":parent", style: { padding: 40 } },
    ],
  });
}

function runOnce(cy, label) {
  const opts = {
    name: "fcose",
    quality: "default",
    randomize: true,
    animate: false,
    fit: false,
    padding: 0,

    samplingType: true,
    sampleSize: 25,
    nodeSeparation: 75,
    piTol: 0.0000001,

    nodeRepulsion: () => 4500,
    idealEdgeLength: () => 40,
    edgeElasticity: () => 0.001,
    nestingFactor: 0.1,
    gravity: 0.25,
    gravityRange: 3.8,
    gravityCompound: 1.0,
    gravityRangeCompound: 1.5,
    initialEnergyOnIncremental: 0.3,
    numIter: 2500,

    alignmentConstraint: {
      vertical: [["l1", "l2", "r2"]],
    },
    relativePlacementConstraint: [
      { left: "l1", right: "r1", gap: 120.0 },
      { top: "l1", bottom: "r2", gap: 120.0 },
      { top: "r2", bottom: "l2", gap: 120.0 },
    ],
  };

  cy.layout(opts).run();

  const ids = ["l1", "l2", "r1", "r2", "left", "right"];
  const pos = Object.fromEntries(
    ids.map((id) => {
      const p = cy.getElementById(id).position();
      return [id, { x: p.x, y: p.y }];
    })
  );
  console.log(`[${label}]`, JSON.stringify(pos, null, 2));
}

const cy = buildCy();
runOnce(cy, "run1");
runOnce(cy, "run2");

