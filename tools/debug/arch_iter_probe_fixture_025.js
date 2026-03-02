// Minimal Puppeteer probe: capture Cytoscape FCoSE iteration-1 checkpoints for an Architecture
// fixture, using the same Mermaid dependency versions as our upstream SVG baselines.
//
// Usage:
//   node tools/debug/arch_iter_probe_fixture_025.js > /tmp/arch_iter_025.json
//
// Notes:
// - Seeds `Math.random` via xorshift64* to match ADR-0055 upstream baseline generation.
// - Hooks `coseBase.CoSELayout.prototype.tick` to capture `iter1` `getPositionsData()` for each run.

const fs = require("fs");
const path = require("path");
const { createRequire } = require("module");

const workspaceRoot = path.resolve(__dirname, "..", "..");
const toolsRoot = path.join(workspaceRoot, "tools", "mermaid-cli");
const requireFromTools = createRequire(path.join(toolsRoot, "package.json"));

const puppeteer = requireFromTools("puppeteer");

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
  Math.random = () => nextF64();
})();
`;
}

async function main() {
  const fixtureStem = "stress_architecture_many_small_groups_025";
  const fixtureMmd = path.join(workspaceRoot, "fixtures", "architecture", `${fixtureStem}.mmd`);
  const code = fs.readFileSync(fixtureMmd, "utf8");

  const mermaidIifePath = path.join(toolsRoot, "node_modules", "mermaid", "dist", "mermaid.js");
  const cytoscapeUmd = path.join(toolsRoot, "node_modules", "cytoscape", "dist", "cytoscape.min.js");
  const layoutBaseUmd = path.join(
    toolsRoot,
    "node_modules",
    "cytoscape-fcose",
    "node_modules",
    "layout-base",
    "layout-base.js"
  );
  const coseBaseUmd = path.join(
    toolsRoot,
    "node_modules",
    "cytoscape-fcose",
    "node_modules",
    "cose-base",
    "cose-base.js"
  );
  const fcoseUmd = path.join(toolsRoot, "node_modules", "cytoscape-fcose", "cytoscape-fcose.js");

  const browser = await puppeteer.launch({
    headless: "shell",
    args: ["--no-sandbox", "--disable-setuid-sandbox", "--allow-file-access-from-files"],
  });
  const page = await browser.newPage();
  await page.setViewport({ width: 1200, height: 800, deviceScaleFactor: 1 });
  await page.evaluateOnNewDocument(xorshiftSeedScript("1"));

  await page.goto("about:blank");
  await page.addScriptTag({ path: cytoscapeUmd });
  await page.addScriptTag({ path: layoutBaseUmd });
  await page.addScriptTag({ path: coseBaseUmd });
  await page.addScriptTag({ path: fcoseUmd });
  await page.addScriptTag({ path: mermaidIifePath });

  const out = await page.evaluate(async (code) => {
    const mermaid = globalThis.mermaid;
    const cytoscape = globalThis.cytoscape;
    const cytoscapeFcose = globalThis.cytoscapeFcose;
    if (!mermaid) throw new Error("missing global mermaid");
    if (!cytoscape) throw new Error("missing global cytoscape");
    if (!cytoscapeFcose) throw new Error("missing global cytoscapeFcose");

    cytoscape.use(cytoscapeFcose);
    mermaid.initialize({ startOnLoad: false });

    const parsed = await mermaid.mermaidAPI.getDiagramFromText(code);
    const diag = parsed && (parsed.diagram ?? parsed);
    const db = diag && diag.db;
    if (!db) throw new Error("missing ArchitectureDB from getDiagramFromText");

    const services = db.getServices();
    const junctions = db.getJunctions();
    const groups = db.getGroups();
    const edges = db.getEdges();
    const ds = db.getDataStructures();

    const iconSize = db.getConfigField("iconSize");
    const padding = db.getConfigField("padding");
    const fontSize = db.getConfigField("fontSize");

    function getOppositeArchitectureDirection(dir) {
      switch (dir) {
        case "L":
          return "R";
        case "R":
          return "L";
        case "T":
          return "B";
        case "B":
          return "T";
        default:
          return dir;
      }
    }
    function isArchitectureDirectionX(dir) {
      return dir === "L" || dir === "R";
    }
    function isArchitectureDirectionY(dir) {
      return dir === "T" || dir === "B";
    }
    function isArchitectureDirectionXY(a, b) {
      return isArchitectureDirectionX(a) !== isArchitectureDirectionX(b);
    }

    function getAlignments(spatialMaps, groupAlignments) {
      const flattenAlignments = (alignmentObj, alignmentDir) => {
        return Object.entries(alignmentObj).reduce((prev, [dir, alignments]) => {
          let cnt = 0;
          const arr = Object.entries(alignments);
          if (arr.length === 1) {
            prev[dir] = arr[0][1];
            return prev;
          }
          for (let i = 0; i < arr.length - 1; i++) {
            for (let j = i + 1; j < arr.length; j++) {
              const [aGroupId, aNodeIds] = arr[i];
              const [bGroupId, bNodeIds] = arr[j];
              const alignment = groupAlignments[aGroupId]?.[bGroupId];
              if (alignment === alignmentDir) {
                prev[dir] ??= [];
                prev[dir] = [...prev[dir], ...aNodeIds, ...bNodeIds];
              } else if (aGroupId === "default" || bGroupId === "default") {
                prev[dir] ??= [];
                prev[dir] = [...prev[dir], ...aNodeIds, ...bNodeIds];
              } else {
                const keyA = `${dir}-${cnt++}`;
                prev[keyA] = aNodeIds;
                const keyB = `${dir}-${cnt++}`;
                prev[keyB] = bNodeIds;
              }
            }
          }
          return prev;
        }, {});
      };

      const alignments = spatialMaps.map((spatialMap) => {
        const horizontalAlignments = {};
        const verticalAlignments = {};
        Object.entries(spatialMap).forEach(([id, [x, y]]) => {
          const nodeGroup = db.getNode(id)?.in ?? "default";
          horizontalAlignments[y] ??= {};
          horizontalAlignments[y][nodeGroup] ??= [];
          horizontalAlignments[y][nodeGroup].push(id);
          verticalAlignments[x] ??= {};
          verticalAlignments[x][nodeGroup] ??= [];
          verticalAlignments[x][nodeGroup].push(id);
        });
        return {
          horiz: Object.values(flattenAlignments(horizontalAlignments, "horizontal")).filter(
            (arr) => arr.length > 1
          ),
          vert: Object.values(flattenAlignments(verticalAlignments, "vertical")).filter(
            (arr) => arr.length > 1
          ),
        };
      });

      return alignments.reduce(
        (prev, { horiz, vert }) => {
          if (horiz.length) prev.horizontal.push(...horiz);
          if (vert.length) prev.vertical.push(...vert);
          return prev;
        },
        { horizontal: [], vertical: [] }
      );
    }

    function getRelativeConstraints(spatialMaps) {
      const relativeConstraints = [];
      spatialMaps.map((spatialMap) => {
        const invSpatialMap = {};
        Object.entries(spatialMap).forEach(([id, [x, y]]) => {
          invSpatialMap[[x, y]] = id;
        });

        const queue = [[0, 0]];
        const visited = {};
        while (queue.length > 0) {
          const pos = queue.shift();
          if (!pos) continue;
          const id = invSpatialMap[pos];
          if (!id) continue;

          visited[id] = 1;

          const dirs = [
            ["L", [-1, 0]],
            ["R", [1, 0]],
            ["T", [0, 1]],
            ["B", [0, -1]],
          ];

          for (const [dir, [sx, sy]] of dirs) {
            const newPos = [pos[0] + sx, pos[1] + sy];
            const newId = invSpatialMap[newPos];
            if (newId && !visited[newId]) {
              queue.push(newPos);
              relativeConstraints.push({
                ...{
                  L: { left: newId, right: id },
                  R: { left: id, right: newId },
                  T: { top: newId, bottom: id },
                  B: { top: id, bottom: newId },
                }[dir],
                gap: 1.5 * db.getConfigField("iconSize"),
              });
            }
          }
        }
      });
      return relativeConstraints;
    }

    const alignmentConstraint = getAlignments(ds.spatialMaps, ds.groupAlignments);
    const relativePlacementConstraint = getRelativeConstraints(ds.spatialMaps);

    const el = document.createElement("div");
    el.style = "width: 800px; height: 600px; position: absolute; left: -10000px; top: -10000px;";
    document.body.appendChild(el);

    const cy = cytoscape({
      container: el,
      style: [
        { selector: "node", style: { "compound-sizing-wrt-labels": "include" } },
        { selector: ".node-service", style: { label: "data(label)", width: "data(width)", height: "data(height)" } },
        { selector: ".node-junction", style: { width: "data(width)", height: "data(height)" } },
        { selector: ".node-group", style: { padding: `${padding}px` } },
        { selector: "edge", style: { "curve-style": "straight" } },
        { selector: "edge.segments", style: { "curve-style": "segments", "segment-weights": "0", "segment-distances": [0.5], "edge-distances": "endpoints" } },
      ],
    });

    for (const g of groups) {
      cy.add({
        group: "nodes",
        data: { type: "group", id: g.id, label: g.title, parent: g.in },
        classes: "node-group",
      });
    }
    for (const s of services) {
      cy.add({
        group: "nodes",
        data: {
          type: "service",
          id: s.id,
          label: s.title,
          parent: s.in,
          width: iconSize,
          height: iconSize,
        },
        classes: "node-service",
      });
    }
    for (const j of junctions) {
      cy.add({
        group: "nodes",
        data: {
          type: "junction",
          id: j.id,
          parent: j.in,
          width: iconSize,
          height: iconSize,
        },
        classes: "node-junction",
      });
    }
    for (const e of edges) {
      const edgeType = isArchitectureDirectionXY(e.lhsDir, e.rhsDir) ? "segments" : "straight";
      cy.add({
        group: "edges",
        data: {
          id: `${e.lhsId}-${e.rhsId}`,
          source: e.lhsId,
          target: e.rhsId,
          sourceDir: e.lhsDir,
          targetDir: e.rhsDir,
          sourceEndpoint:
            e.lhsDir === "L" ? "0 50%" : e.lhsDir === "R" ? "100% 50%" : e.lhsDir === "T" ? "50% 0" : "50% 100%",
          targetEndpoint:
            e.rhsDir === "L" ? "0 50%" : e.rhsDir === "R" ? "100% 50%" : e.rhsDir === "T" ? "50% 0" : "50% 100%",
        },
        classes: edgeType,
      });
    }

    function layoutOpts() {
      return {
        name: "fcose",
        quality: "proof",
        randomize: true,
        styleEnabled: false,
        animate: false,
        nodeDimensionsIncludeLabels: false,
        step: "all",
        idealEdgeLength(edge) {
          const [nodeA, nodeB] = edge.connectedNodes();
          const parentA = nodeA.data("parent");
          const parentB = nodeB.data("parent");
          return parentA === parentB ? 1.5 * iconSize : 0.5 * iconSize;
        },
        edgeElasticity(edge) {
          const [nodeA, nodeB] = edge.connectedNodes();
          const parentA = nodeA.data("parent");
          const parentB = nodeB.data("parent");
          return parentA === parentB ? 0.45 : 0.001;
        },
        alignmentConstraint,
        relativePlacementConstraint,
      };
    }

    const checkpoints = {
      handleConstraints: [],
      iter1Runs: [],
      tickCalls: 0,
    };

    // Hook ConstraintHandler.handleConstraints(layout).
    try {
      const ConstraintHandler = globalThis.coseBase?.ConstraintHandler ?? null;
      if (ConstraintHandler && typeof ConstraintHandler.handleConstraints === "function") {
        const orig = ConstraintHandler.handleConstraints;
        ConstraintHandler.handleConstraints = (layout) => {
          const before = layout?.getPositionsData ? layout.getPositionsData() : null;
          const res = orig(layout);
          const after = layout?.getPositionsData ? layout.getPositionsData() : null;
          checkpoints.handleConstraints.push({ before, after });
          return res;
        };
      }
    } catch {}

    // Hook per-tick positions.
    try {
      const CoSELayout = globalThis.coseBase?.CoSELayout ?? null;
      if (CoSELayout?.prototype?.tick && CoSELayout?.prototype?.getPositionsData) {
        const origTick = CoSELayout.prototype.tick;
        CoSELayout.prototype.tick = function () {
          checkpoints.tickCalls++;
          const res = origTick.call(this);
          if (this.totalIterations === 1 && checkpoints.iter1Runs.length < 2) {
            checkpoints.iter1Runs.push(this.getPositionsData());
          }
          return res;
        };
      }
    } catch {}

    const layout = cy.layout(layoutOpts());

    const runPositions = [];
    await new Promise((resolve) => {
      layout.one("layoutstop", () => {
        const pos1 = {};
        for (const s of services) pos1[s.id] = cy.getElementById(s.id).position();
        runPositions.push(pos1);

        layout.one("layoutstop", () => {
          const pos2 = {};
          for (const s of services) pos2[s.id] = cy.getElementById(s.id).position();
          runPositions.push(pos2);
          resolve();
        });
        layout.run();
      });
      layout.run();
    });

    cy.destroy();
    el.remove();

    return {
      iconSize,
      padding,
      fontSize,
      constraints: { alignmentConstraint, relativePlacementConstraint },
      checkpoints,
      runPositions,
    };
  }, code);

  console.log(JSON.stringify(out, null, 2));

  await browser.close();
}

main().catch((e) => {
  console.error(e && e.stack ? e.stack : String(e));
  process.exit(1);
});

