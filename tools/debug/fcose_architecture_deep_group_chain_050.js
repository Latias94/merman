// Debug helper: reproduce Mermaid Architecture (Cytoscape.js FCoSE) internals for a single fixture.
//
// Usage:
//   NODE_PATH=repo-ref node tools/debug/fcose_architecture_deep_group_chain_050.js
//
// Notes:
// - Uses vendored upstream sources under `repo-ref/` (no npm install).
// - Seeds `Math.random()` with the same xorshift64* used by `xtask` (ADR-0055).
// - Logs spectral coords and ConstraintHandler pre/post positions for each run.

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

const rng = makeXorShift64Star("1");
Math.random = rng.nextF64;

const cytoscape = require("../../repo-ref/cytoscape/package/dist/cytoscape.cjs.js");

// Register FCoSE (UMD bundle) against Cytoscape and patch CoSE internals for stage dumps.
// `cytoscape-fcose` UMD requires `cose-base` by module name. Use `NODE_PATH=repo-ref` so it
// resolves to our vendored `repo-ref/cose-base/`, and patch that same singleton.
const coseBase = require("cose-base");
const CoSELayout = coseBase.CoSELayout;
const FDLayout = coseBase.layoutBase.FDLayout;

function dumpDisplacements(layout, tag) {
  const runLabel = global.__MERMAID_FCOSE_RUN_LABEL__ || "run?";
  const all = layout.getAllNodes();
  console.log(`[upstream-disp] ${runLabel} ${tag} totalIterations=${layout.totalIterations}`);
  for (const n of all) {
    console.log(
      `[upstream-disp] ${runLabel} ${tag} id=${n.id} disp=(${Number(n.displacementX).toFixed(
        6
      )},${Number(n.displacementY).toFixed(6)}) child=${n.getChild() ? 1 : 0}`
    );
  }
}

function dumpForces(layout, tag) {
  const runLabel = global.__MERMAID_FCOSE_RUN_LABEL__ || "run?";
  const all = layout.getAllNodes();
  console.log(`[upstream-force] ${runLabel} ${tag} totalIterations=${layout.totalIterations}`);
  for (const n of all) {
    console.log(
      `[upstream-force] ${runLabel} ${tag} id=${n.id} owner=${n.getOwner().getParent() ? n.getOwner().getParent().id : 'root'} child=${n.getChild() ? 1 : 0} spring=(${Number(
        n.springForceX
      ).toFixed(6)},${Number(n.springForceY).toFixed(6)}) rep=(${Number(
        n.repulsionForceX
      ).toFixed(6)},${Number(n.repulsionForceY).toFixed(6)}) grav=(${Number(
        n.gravitationForceX
      ).toFixed(6)},${Number(n.gravitationForceY).toFixed(6)})`
    );
  }
}

// Debug: log FR-grid parameters to understand headless failures (NaN/0 -> invalid array length).
const calcGridOrig = FDLayout.prototype.calcGrid;
FDLayout.prototype.calcGrid = function patchedCalcGrid(graph) {
  const runLabel = global.__MERMAID_FCOSE_RUN_LABEL__ || "run?";
  const left = graph.getLeft();
  const right = graph.getRight();
  const top = graph.getTop();
  const bottom = graph.getBottom();
  const rep = this.repulsionRange;
  const w = right - left;
  const h = bottom - top;
  const sizeX = parseInt(Math.ceil(w / rep));
  const sizeY = parseInt(Math.ceil(h / rep));
  const sizeOk =
    Number.isFinite(sizeX) &&
    Number.isFinite(sizeY) &&
    sizeX >= 0 &&
    sizeY >= 0 &&
    sizeX <= 0xffffffff &&
    sizeY <= 0xffffffff;
  if (
    !(Number.isFinite(w) && Number.isFinite(h) && Number.isFinite(rep) && rep > 0) ||
    !sizeOk
  ) {
    console.log(
      `[upstream-frgrid] ${runLabel} left=${left} right=${right} top=${top} bottom=${bottom} w=${w} h=${h} repulsionRange=${rep} sizeX=${sizeX} sizeY=${sizeY}`
    );
  }
  return calcGridOrig.call(this, graph);
};

function dumpNodes(layout, tag) {
  const runLabel = global.__MERMAID_FCOSE_RUN_LABEL__ || "run?";
  const all = layout.getAllNodes();
  const leaves = all.filter((n) => n.getChild() == null);
  const parents = all.filter((n) => n.getChild() != null);
  console.log(
    `[upstream-${tag}] ${runLabel} leaf_count=${leaves.length} parent_count=${parents.length}`
  );

  function logNode(n, kind) {
    const w = n.getWidth();
    const h = n.getHeight();
    const cx = n.getCenterX();
    const cy = n.getCenterY();
    console.log(
      `[upstream-${tag}] ${runLabel} kind=${kind} id=${n.id} center=(${cx.toFixed(
        6
      )},${cy.toFixed(6)}) size=(${w},${h})`
    );
  }

  for (const n of leaves) logNode(n, "leaf");

  for (const n of parents) {
    const w = n.getWidth();
    const h = n.getHeight();
    const cx = n.getCenterX();
    const cy = n.getCenterY();
    const bad =
      !Number.isFinite(w) ||
      !Number.isFinite(h) ||
      !Number.isFinite(cx) ||
      !Number.isFinite(cy);
    if (bad) logNode(n, "parent_bad");
  }
}

const classicOrig = CoSELayout.prototype.classicLayout;
CoSELayout.prototype.classicLayout = function patchedClassicLayout() {
  // At this point, node rects reflect spectral initialization (FCoSE randomize=true).
  dumpNodes(this, "spectral");
  return classicOrig.apply(this, arguments);
};

const initConstraintsOrig = CoSELayout.prototype.initConstraintVariables;
CoSELayout.prototype.initConstraintVariables = function patchedInitConstraintVariables() {
  // This is called immediately after `ConstraintHandler.handleConstraints(this)`.
  dumpNodes(this, "pre_constraints");
  return initConstraintsOrig.apply(this, arguments);
};

const updateDisplacementsOrig = CoSELayout.prototype.updateDisplacements;
CoSELayout.prototype.updateDisplacements = function patchedUpdateDisplacements() {
  if (this.totalIterations === 1) {
    dumpDisplacements(this, "iter1_before_constraints");
  }
  const r = updateDisplacementsOrig.apply(this, arguments);
  if (this.totalIterations === 1) {
    dumpDisplacements(this, "iter1_after_constraints");
  }
  return r;
};

const moveNodesOrig = CoSELayout.prototype.moveNodes;
CoSELayout.prototype.moveNodes = function patchedMoveNodes() {
  if (this.totalIterations === 1) {
    dumpForces(this, "iter1_before_displacement");
  }
  return moveNodesOrig.apply(this, arguments);
};

const runSpringEmbedderOrig = CoSELayout.prototype.runSpringEmbedder;
CoSELayout.prototype.runSpringEmbedder = function patchedRunSpringEmbedder() {
  const r = runSpringEmbedderOrig.apply(this, arguments);
  const runLabel = global.__MERMAID_FCOSE_RUN_LABEL__ || "run?";
  console.log(`[upstream-iter] ${runLabel} totalIterations=${this.totalIterations}`);
  return r;
};

const fcose = require("../../repo-ref/cytoscape.js-fcose/cytoscape-fcose.js");
fcose(cytoscape);

function buildCy() {
  const iconSize = 80;
  const padding = 40;

  const elements = [
    // Groups (nested g1->g2->g3->g4)
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
    headless: true,
    // Needed so `ele.css('padding')` returns a string (otherwise parseInt() => NaN and CoSE bounds break).
    styleEnabled: true,
    elements,
    style: [
      {
        selector: ".node-service, .node-junction",
        style: { width: "data(width)", height: "data(height)" },
      },
      // Cytoscape applies compound padding via the `padding` style on parent nodes.
      // Keep both selectors to avoid headless style edge cases.
      { selector: ".node-group", style: { padding: `${padding}px` } },
      { selector: ":parent", style: { padding: `${padding}px` } },
    ],
  });

  // In headless mode with `styleEnabled: false`, cytoscape-fcose relies on `node.layoutDimensions()`
  // instead of style resolution. Provide explicit dimensions for deterministic parity.
  cy.nodes().forEach((n) => {
    const w = n.data("width") || iconSize;
    const h = n.data("height") || iconSize;
    n.layoutDimensions = () => ({ w, h });
  });

  return cy;
}

function runTwice(cy) {
  const iconSize = 80;

  const opts = {
    name: "fcose",
    quality: "proof",
    styleEnabled: false,
    animate: false,
    nodeDimensionsIncludeLabels: false,

    idealEdgeLength(edge) {
      const [a, b] = edge.connectedNodes();
      const parentA = a.data("parent");
      const parentB = b.data("parent");
      return parentA === parentB ? 1.5 * iconSize : 0.5 * iconSize;
    },
    edgeElasticity(edge) {
      const [a, b] = edge.connectedNodes();
      const parentA = a.data("parent");
      const parentB = b.data("parent");
      return parentA === parentB ? 0.45 : 0.001;
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
  };

  function dumpFinal(tag) {
    const ids = ["a", "b", "c", "d"];
    console.log(`[upstream-final] ${tag}`);
    for (const id of ids) {
      const p = cy.getElementById(id).position();
      console.log(`[upstream-final] ${tag} id=${id} center=(${p.x.toFixed(6)},${p.y.toFixed(6)})`);
    }
  }

  const layout = cy.layout(opts);

  global.__MERMAID_FCOSE_RUN_LABEL__ = "run1";
  layout.run();
  dumpFinal("run1");

  global.__MERMAID_FCOSE_RUN_LABEL__ = "run2";
  layout.run();
  dumpFinal("run2");
}

const cy = buildCy();
runTwice(cy);
