// Browser (Puppeteer) probe: dump Cytoscape FCoSE (cose-base) stages for an Architecture fixture.
//
// Usage:
//   node tools/debug/arch_fcose_browser_probe_fixture_025.js > /tmp/fcose-stages-025.json
//
// Notes:
// - Runs inside Chromium via Puppeteer so Cytoscape can measure node dimensions consistently.
// - Seeds RNG using the same xorshift64* as `xtask` upstream baselines (ADR-0055).
// - Uses Mermaid only to parse the fixture into ArchitectureDB model objects.

const fs = require("fs");
const path = require("path");
const url = require("url");
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
  Math.random = nextF64;
})();
`;
}

async function main() {
  const fixtureStem = process.argv[2] || "stress_architecture_many_small_groups_025";
  const fixtureMmd = path.join(workspaceRoot, "fixtures", "architecture", `${fixtureStem}.mmd`);
  const code = fs.readFileSync(fixtureMmd, "utf8");

  const mermaidHtmlPath = path.join(
    toolsRoot,
    "node_modules",
    "@mermaid-js",
    "mermaid-cli",
    "dist",
    "index.html"
  );
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

  await page.goto(url.pathToFileURL(mermaidHtmlPath).href);
  await page.addScriptTag({ path: mermaidIifePath });
  await page.addScriptTag({ path: cytoscapeUmd });
  await page.addScriptTag({ path: layoutBaseUmd });
  await page.addScriptTag({ path: coseBaseUmd });

  // Install probes before loading cytoscape-fcose so it captures patched prototypes.
  await page.addScriptTag({
    content: `
(() => {
  globalThis.__archFcoseStages = [];
  globalThis.__archFcoseStages.push({
    tag: 'probe-installed',
    hasCoseBase: !!globalThis.coseBase,
    hasCoSELayout: !!(globalThis.coseBase && globalThis.coseBase.CoSELayout),
  });
  function dumpLayoutNodes(tag, layout) {
    try {
      const leaf = layout.getAllNodes().filter((n) => n.getChild() == null);
      return {
        tag,
        leaf: leaf.map((n) => ({
          id: n.id,
          center: [n.getCenterX(), n.getCenterY()],
          leftTop: [n.getLeft(), n.getTop()],
          size: [n.getWidth(), n.getHeight()],
          owner: n.getOwner && n.getOwner() && n.getOwner().getParent ? (n.getOwner().getParent() ? n.getOwner().getParent().id : 'root') : 'unknown',
        })),
      };
    } catch (e) {
      return { tag, error: String(e && e.message ? e.message : e) };
    }
  }

  const coseBase = globalThis.coseBase;
  if (!coseBase || !coseBase.CoSELayout) return;
  const CoSELayoutOrig = coseBase.CoSELayout;

  // Wrap constructor to verify cytoscape-fcose actually instantiates this class.
  function CoSELayoutWrapped() {
    globalThis.__archFcoseStages.push({ tag: 'CoSELayout.new' });
    // eslint-disable-next-line prefer-rest-params
    return new CoSELayoutOrig(...arguments);
  }
  CoSELayoutWrapped.prototype = CoSELayoutOrig.prototype;
  coseBase.CoSELayout = CoSELayoutWrapped;
  globalThis.__archFcoseStages.push({ tag: 'CoSELayout.wrapped' });
  const CoSELayout = coseBase.CoSELayout;

  const origClassic = CoSELayout.prototype.classicLayout;
  CoSELayout.prototype.classicLayout = function () {
    globalThis.__archFcoseStages.push(dumpLayoutNodes('classicLayout.start', this));
    const ret = origClassic.apply(this, arguments);
    globalThis.__archFcoseStages.push(dumpLayoutNodes('classicLayout.end', this));
    return ret;
  };

  const origInit = CoSELayout.prototype.initConstraintVariables;
  CoSELayout.prototype.initConstraintVariables = function () {
    globalThis.__archFcoseStages.push(dumpLayoutNodes('initConstraintVariables', this));
    return origInit.apply(this, arguments);
  };

  const origTick = CoSELayout.prototype.tick;
  CoSELayout.prototype.tick = function () {
    const ended = origTick.apply(this, arguments);
    if (this.totalIterations === 1) {
      globalThis.__archFcoseStages.push(dumpLayoutNodes('iter1', this));
      globalThis.__archFcoseStages.push({
        tag: 'iter1.meta',
        totalIterations: this.totalIterations,
        coolingFactor: this.coolingFactor,
        maxNodeDisplacement: this.maxNodeDisplacement,
        totalDisplacement: this.totalDisplacement,
      });
    }
    if (ended) {
      globalThis.__archFcoseStages.push({
        tag: 'end.meta',
        totalIterations: this.totalIterations,
        coolingFactor: this.coolingFactor,
        totalDisplacement: this.totalDisplacement,
      });
    }
    return ended;
  };
})();
`,
  });

  await page.addScriptTag({ path: fcoseUmd });

  const out = await page.evaluate(async (code) => {
    const mermaid = globalThis.mermaid;
    const cytoscape = globalThis.cytoscape;
    const cytoscapeFcose = globalThis.cytoscapeFcose;
    const coseBase = globalThis.coseBase;
    if (!mermaid) throw new Error("missing global mermaid");
    if (!cytoscape) throw new Error("missing global cytoscape");
    if (!coseBase) throw new Error("missing global coseBase");
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

    function isX(dir) {
      return dir === "L" || dir === "R";
    }
    function isY(dir) {
      return dir === "T" || dir === "B";
    }
    function isXY(a, b) {
      return isX(a) !== isX(b);
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
              if (alignment === alignmentDir || aGroupId === "default" || bGroupId === "default") {
                prev[dir] ??= [];
                prev[dir] = [...prev[dir], ...aNodeIds, ...bNodeIds];
              } else {
                prev[`${dir}-${cnt++}`] = aNodeIds;
                prev[`${dir}-${cnt++}`] = bNodeIds;
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
          horiz: Object.values(flattenAlignments(horizontalAlignments, "horizontal")).filter((arr) => arr.length > 1),
          vert: Object.values(flattenAlignments(verticalAlignments, "vertical")).filter((arr) => arr.length > 1),
        };
      });

      return alignments.reduce(
        (prev, { horiz, vert }) => {
          prev.horizontal.push(...horiz);
          prev.vertical.push(...vert);
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

    // Stages are captured by the injected prototype probes.

    const renderEl = document.createElement("div");
    renderEl.id = "cy-probe";
    renderEl.style = "width: 800px; height: 600px; display: none;";
    document.body.appendChild(renderEl);

    const cy = cytoscape({
      container: renderEl,
      style: [
        { selector: "edge", style: { "curve-style": "straight", label: "data(label)", "source-endpoint": "data(sourceEndpoint)", "target-endpoint": "data(targetEndpoint)" } },
        { selector: "edge.segments", style: { "curve-style": "segments", "segment-weights": "0", "segment-distances": [0.5], "edge-distances": "endpoints", "source-endpoint": "data(sourceEndpoint)", "target-endpoint": "data(targetEndpoint)" } },
        { selector: "node", style: { "compound-sizing-wrt-labels": "include" } },
        { selector: "node[label]", style: { "text-valign": "bottom", "text-halign": "center", "font-size": `${db.getConfigField("fontSize")}px` } },
        { selector: ".node-service", style: { label: "data(label)", width: "data(width)", height: "data(height)" } },
        { selector: ".node-junction", style: { width: "data(width)", height: "data(height)" } },
        { selector: ".node-group", style: { padding: `${db.getConfigField("padding")}px` } },
      ],
    });

    for (const group of groups) {
      cy.add({ group: "nodes", data: { type: "group", id: group.id, icon: group.icon, label: group.title, parent: group.in }, classes: "node-group" });
    }
    for (const service of services) {
      cy.add({
        group: "nodes",
        data: { type: "service", id: service.id, icon: service.icon, label: service.title, parent: service.in, width: db.getConfigField("iconSize"), height: db.getConfigField("iconSize") },
        classes: "node-service",
      });
    }
    for (const junction of junctions) {
      cy.add({
        group: "nodes",
        data: { type: "junction", id: junction.id, parent: junction.in, width: db.getConfigField("iconSize"), height: db.getConfigField("iconSize") },
        classes: "node-junction",
      });
    }
    for (const parsedEdge of edges) {
      const { lhsId, rhsId, lhsInto, lhsGroup, rhsInto, lhsDir, rhsDir, rhsGroup, title } = parsedEdge;
      const edgeType = isXY(lhsDir, rhsDir) ? "segments" : "straight";
      const edge = {
        id: `${lhsId}-${rhsId}`,
        label: title,
        source: lhsId,
        sourceDir: lhsDir,
        sourceArrow: lhsInto,
        sourceGroup: lhsGroup,
        sourceEndpoint: lhsDir === "L" ? "0 50%" : lhsDir === "R" ? "100% 50%" : lhsDir === "T" ? "50% 0" : "50% 100%",
        target: rhsId,
        targetDir: rhsDir,
        targetArrow: rhsInto,
        targetGroup: rhsGroup,
        targetEndpoint: rhsDir === "L" ? "0 50%" : rhsDir === "R" ? "100% 50%" : rhsDir === "T" ? "50% 0" : "50% 100%",
      };
      cy.add({ group: "edges", data: edge, classes: edgeType });
    }

    const iconSize = db.getConfigField("iconSize");
    const layout = cy.layout({
      name: "fcose",
      quality: "proof",
      styleEnabled: false,
      animate: false,
      nodeDimensionsIncludeLabels: false,
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
    });

    const layoutElesInfo = (() => {
      try {
        const eles = layout && layout.options && layout.options.eles ? layout.options.eles : null;
        if (!eles) return null;
        return {
          nodes: eles.nodes().length,
          edges: eles.edges().length,
          bb: eles.boundingBox(),
        };
      } catch (e) {
        return { error: String(e && e.message ? e.message : e) };
      }
    })();

    function dumpPreLayoutElements() {
      const nodes = [];
      for (const n of cy.nodes().toArray()) {
        const id = n.id();
        const p = n.position();
        const bb = n.boundingBox();
        nodes.push({
          id,
          pos: { x: p.x, y: p.y },
          bb,
          classes: n.classes ? n.classes() : undefined,
          data: n.data ? n.data() : undefined,
        });
      }
      const edgesOut = [];
      for (const e of cy.edges().toArray()) {
        const id = e.id();
        const bb = e.boundingBox();
        const sEp = e.sourceEndpoint ? e.sourceEndpoint() : null;
        const tEp = e.targetEndpoint ? e.targetEndpoint() : null;
        edgesOut.push({
          id,
          bb,
          classes: e.classes ? e.classes() : undefined,
          data: e.data ? e.data() : undefined,
          sourceEndpoint: sEp,
          targetEndpoint: tEp,
          style: {
            curveStyle: e.style ? e.style("curve-style") : undefined,
            segmentWeights: e.style ? e.style("segment-weights") : undefined,
            segmentDistances: e.style ? e.style("segment-distances") : undefined,
            edgeDistances: e.style ? e.style("edge-distances") : undefined,
          },
        });
      }
      return { nodes, edges: edgesOut };
    }

    const bbBeforeRun1 = cy.elements().boundingBox();
    const preLayout = dumpPreLayoutElements();

    const final = await new Promise((resolve) => {
      layout.one("layoutstop", () => {
        // Match architectureRenderer: adjust segment weights for XY edges, then re-run layout.
        const bbBeforeRun2 = cy.elements().boundingBox();
        globalThis.__archFcoseStages.push({ tag: "bbBeforeRun2", bb: bbBeforeRun2 });
        cy.startBatch();
        for (const edge of Object.values(cy.edges())) {
          if (edge.data?.()) {
            const { x: sX, y: sY } = edge.source().position();
            const { x: tX, y: tY } = edge.target().position();
            if (sX !== tX && sY !== tY) {
              const sEP = edge.sourceEndpoint();
              const tEP = edge.targetEndpoint();
              const sourceDir = edge.data("sourceDir");
              const [pointX, pointY] = isY(sourceDir) ? [sEP.x, tEP.y] : [tEP.x, sEP.y];
              let W, D;
              D = (pointY - sY + ((sX - pointX) * (sY - tY)) / (sX - tX)) / Math.sqrt(1 + Math.pow((sY - tY) / (sX - tX), 2));
              W = Math.sqrt(Math.pow(pointY - sY, 2) + Math.pow(pointX - sX, 2) - Math.pow(D, 2));
              const distAB = Math.sqrt(Math.pow(tX - sX, 2) + Math.pow(tY - sY, 2));
              W = W / distAB;
              let delta1 = (tX - sX) * (pointY - sY) - (tY - sY) * (pointX - sX);
              delta1 = delta1 >= 0 ? 1 : -1;
              let delta2 = (tX - sX) * (pointX - sX) + (tY - sY) * (pointY - sY);
              delta2 = delta2 >= 0 ? 1 : -1;
              D = Math.abs(D) * delta1;
              W = W * delta2;
              edge.style("segment-distances", D);
              edge.style("segment-weights", W);
            }
          }
        }
        cy.endBatch();
        const bbAfterSegments = cy.elements().boundingBox();
        globalThis.__archFcoseStages.push({ tag: "bbAfterSegments", bb: bbAfterSegments });

        layout.one("layoutstop", () => {
          const nodeOut = {};
          for (const svc of services) {
            const n = cy.getElementById(svc.id);
            const p = n.position();
            nodeOut[svc.id] = { x: p.x, y: p.y };
          }
          resolve(nodeOut);
        });
        layout.run();
      });
      layout.run();
    });

    return {
      constraints: { alignmentConstraint, relativePlacementConstraint },
      stages: globalThis.__archFcoseStages || [],
      final,
      bbBeforeRun1,
      preLayout,
      layoutElesInfo,
    };
  }, code);

  console.log(JSON.stringify(out, null, 2));
  await browser.close();
}

main().catch((e) => {
  console.error(e && e.stack ? e.stack : String(e));
  process.exit(1);
});
