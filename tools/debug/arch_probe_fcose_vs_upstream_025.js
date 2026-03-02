// Browser (Puppeteer) probe: reproduce Cytoscape.js FCoSE positions for an Architecture fixture
// and compare against the upstream baselined SVG service transforms.
//
// Usage:
//   node tools/debug/arch_probe_fcose_vs_upstream_025.js
//
// Notes:
// - Runs in Chromium so Cytoscape can measure node dimensions consistently.
// - Seeds Math.random with the same xorshift64* seeding used by `xtask` upstream baselines (ADR-0055).
// - Uses Mermaid only for parsing (ArchitectureDB), then runs Cytoscape+FCoSE independently.

const fs = require("fs");
const path = require("path");
const { createRequire } = require("module");

const workspaceRoot = path.resolve(__dirname, "..", "..");
const toolsRoot = path.join(workspaceRoot, "tools", "mermaid-cli");
const requireFromTools = createRequire(path.join(toolsRoot, "package.json"));

const puppeteer = requireFromTools("puppeteer");

function scriptPath(relFromWorkspaceRoot) {
  return path.join(workspaceRoot, relFromWorkspaceRoot);
}

function xorshiftSeedScript(seedStr) {
  return `
(() => {
  const mask64 = (1n << 64n) - 1n;
  let state = (BigInt(${JSON.stringify(seedStr)}) & mask64);
  if (state === 0n) state = 1n;
  const capture = { enabled: false, log: [] };
  globalThis.__randCapture = capture;
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
  Math.random = () => {
    const v = nextF64();
    if (capture.enabled) capture.log.push(v);
    return v;
  };

  if (globalThis.crypto && typeof globalThis.crypto.getRandomValues === 'function') {
    const orig = globalThis.crypto.getRandomValues.bind(globalThis.crypto);
    globalThis.crypto.getRandomValues = (arr) => {
      if (!arr || typeof arr.length !== 'number') return orig(arr);
      try {
        const bytes = new Uint8Array(arr.buffer, arr.byteOffset || 0, arr.byteLength || 0);
        for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(nextF64() * 256);
        return arr;
      } catch (e) {
        return orig(arr);
      }
    };
  }
})();
`;
}

function parseUpstreamServiceTransforms(svgText) {
  const re = /<g id="service-([^"]+)"[^>]*?transform="translate\(([^,]+),([^)]+)\)"/g;
  const out = new Map();
  let m;
  while ((m = re.exec(svgText))) {
    const id = m[1];
    const x = Number(m[2]);
    const y = Number(m[3]);
    if (Number.isFinite(x) && Number.isFinite(y)) out.set(id, { x, y });
  }
  return out;
}

function scoreMapping(up, local) {
  let sumAbs = 0;
  let maxAbs = 0;
  for (const [id, u] of up.entries()) {
    const l = local.get(id);
    if (!l) continue;
    const dx = l.x - u.x;
    const dy = l.y - u.y;
    const s = Math.max(Math.abs(dx), Math.abs(dy));
    sumAbs += s;
    maxAbs = Math.max(maxAbs, s);
  }
  return { sumAbs, maxAbs };
}

async function main() {
  const fixtureStem = "stress_architecture_many_small_groups_025";
  const fixtureMmd = path.join(workspaceRoot, "fixtures", "architecture", `${fixtureStem}.mmd`);
  const upstreamSvg = path.join(
    workspaceRoot,
    "fixtures",
    "upstream-svgs",
    "architecture",
    `${fixtureStem}.svg`
  );

  const code = fs.readFileSync(fixtureMmd, "utf8");
  const upstreamSvgText = fs.readFileSync(upstreamSvg, "utf8");
  const upstreamTx = parseUpstreamServiceTransforms(upstreamSvgText);

  const mermaidIifePath = path.join(toolsRoot, "node_modules", "mermaid", "dist", "mermaid.js");

  // Match Mermaid dependency versions used for baselines:
  const cytoscapeUmd = path.join(toolsRoot, "node_modules", "cytoscape", "dist", "cytoscape.min.js");
  const fcoseUmd = path.join(toolsRoot, "node_modules", "cytoscape-fcose", "cytoscape-fcose.js");
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

  const browser = await puppeteer.launch({
    headless: "shell",
    args: ["--no-sandbox", "--disable-setuid-sandbox", "--allow-file-access-from-files"],
  });
  const page = await browser.newPage();
  await page.setViewport({ width: 1200, height: 800, deviceScaleFactor: 1 });
  await page.evaluateOnNewDocument(xorshiftSeedScript("1"));

  // Use a clean page to avoid any preloaded bundles affecting UMD global detection or prototypes.
  await page.goto("about:blank");
  // Load scripts in dependency order (UMD globals).
  await page.addScriptTag({ path: mermaidIifePath });
  await page.addScriptTag({ path: cytoscapeUmd });
  await page.addScriptTag({ path: layoutBaseUmd });
  await page.addScriptTag({ path: coseBaseUmd });
  await page.addScriptTag({ path: fcoseUmd });

  const probe = await page.evaluate(async (code) => {
    const mermaid = globalThis.mermaid;
    if (!mermaid) throw new Error("missing global mermaid");

    const cytoscape = globalThis.cytoscape;
    const cytoscapeFcose = globalThis.cytoscapeFcose;
    if (!cytoscape) throw new Error("missing global cytoscape");
    if (!cytoscapeFcose) throw new Error("missing global cytoscapeFcose");
    cytoscape.use(cytoscapeFcose);

    // Optional introspection: capture the first SVD input matrix used by spectral layout (PHI).
    // This helps verify transformed-graph construction without modifying node_modules.
    const spectralSvdInputs = [];
    const svdHost =
      globalThis.coseBase?.layoutBase?.SVD ??
      globalThis.layoutBase?.SVD ??
      globalThis.SVD ??
      null;
    const origSvd = svdHost?.svd?.bind(svdHost);
    if (origSvd) {
      svdHost.svd = (m) => {
        try {
          if (
            spectralSvdInputs.length === 0 &&
            Array.isArray(m) &&
            m.length > 0 &&
            Array.isArray(m[0]) &&
            m.length === m[0].length &&
            m.length <= 25
          ) {
            // Best-effort deep clone.
            spectralSvdInputs.push(JSON.parse(JSON.stringify(m)));
          }
        } catch {}
        return origSvd(m);
      };
    }

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
              // ArchitectureDirectionName[dir] = newId
              // ArchitectureDirectionName[getOppositeArchitectureDirection(dir)] = id
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

    function buildCy() {
      const renderEl = document.createElement("div");
      renderEl.className = "cy-probe";
      renderEl.style = "width: 800px; height: 600px; display: none;";
      document.body.appendChild(renderEl);

      const cy = cytoscape({
        container: renderEl,
        style: [
        {
          selector: "edge",
          style: {
            "curve-style": "straight",
            label: "data(label)",
            "source-endpoint": "data(sourceEndpoint)",
            "target-endpoint": "data(targetEndpoint)",
          },
        },
        {
          selector: "edge.segments",
          style: {
            "curve-style": "segments",
            "segment-weights": "0",
            "segment-distances": [0.5],
            "edge-distances": "endpoints",
            "source-endpoint": "data(sourceEndpoint)",
            "target-endpoint": "data(targetEndpoint)",
          },
        },
        { selector: "node", style: { "compound-sizing-wrt-labels": "include" } },
        {
          selector: "node[label]",
          style: { "text-valign": "bottom", "text-halign": "center", "font-size": `${db.getConfigField("fontSize")}px` },
        },
        { selector: ".node-service", style: { label: "data(label)", width: "data(width)", height: "data(height)" } },
        { selector: ".node-junction", style: { width: "data(width)", height: "data(height)" } },
        { selector: ".node-group", style: { padding: `${db.getConfigField("padding")}px` } },
      ],
        layout: { name: "grid", boundingBox: { x1: 0, x2: 100, y1: 0, y2: 100 } },
      });

      addGroups(cy, groups);
      addServices(cy, services);
      addJunctions(cy, junctions);
      addEdges(cy, edges);

      return cy;
    }

    function addGroups(cy, groups) {
      for (const group of groups) {
        cy.add({
          group: "nodes",
          data: { type: "group", id: group.id, icon: group.icon, label: group.title, parent: group.in },
          classes: "node-group",
        });
      }
    }
    function addServices(cy, services) {
      for (const service of services) {
        cy.add({
          group: "nodes",
          data: {
            type: "service",
            id: service.id,
            icon: service.icon,
            label: service.title,
            parent: service.in,
            width: db.getConfigField("iconSize"),
            height: db.getConfigField("iconSize"),
          },
          classes: "node-service",
        });
      }
    }
    function addJunctions(cy, junctions) {
      for (const junction of junctions) {
        cy.add({
          group: "nodes",
          data: {
            type: "junction",
            id: junction.id,
            parent: junction.in,
            width: db.getConfigField("iconSize"),
            height: db.getConfigField("iconSize"),
          },
          classes: "node-junction",
        });
      }
    }
    function addEdges(cy, edges) {
      for (const parsedEdge of edges) {
        const { lhsId, rhsId, lhsInto, lhsGroup, rhsInto, lhsDir, rhsDir, rhsGroup, title } = parsedEdge;
        const edgeType = isArchitectureDirectionXY(parsedEdge.lhsDir, parsedEdge.rhsDir) ? "segments" : "straight";
        const edge = {
          id: `${lhsId}-${rhsId}`,
          label: title,
          source: lhsId,
          sourceDir: lhsDir,
          sourceArrow: lhsInto,
          sourceGroup: lhsGroup,
          sourceEndpoint:
            lhsDir === "L" ? "0 50%" : lhsDir === "R" ? "100% 50%" : lhsDir === "T" ? "50% 0" : "50% 100%",
          target: rhsId,
          targetDir: rhsDir,
          targetArrow: rhsInto,
          targetGroup: rhsGroup,
          targetEndpoint:
            rhsDir === "L" ? "0 50%" : rhsDir === "R" ? "100% 50%" : rhsDir === "T" ? "50% 0" : "50% 100%",
        };
        cy.add({ group: "edges", data: edge, classes: edgeType });
      }
    }

    const iconSize = db.getConfigField("iconSize");
    function layoutOpts(step) {
      return {
        name: "fcose",
        quality: "proof",
        randomize: true,
        styleEnabled: false,
        animate: false,
        nodeDimensionsIncludeLabels: false,
        step,
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

    async function runDraftSpectralOnly() {
      const cy = buildCy();
      const bb0 = cy.elements().boundingBox();
      const preBboxCenter = { x: bb0.x1 + bb0.w / 2, y: bb0.y1 + bb0.h / 2 };
      if (globalThis.__randCapture) {
        globalThis.__randCapture.log = [];
        globalThis.__randCapture.enabled = true;
      }
      const layout = cy.layout({
        name: "fcose",
        quality: "draft",
        randomize: true,
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
      });
      const done = new Promise((resolve) => layout.one("layoutstop", () => resolve()));
      layout.run();
      await done;
      if (globalThis.__randCapture) {
        globalThis.__randCapture.enabled = false;
      }
      const randLog = globalThis.__randCapture ? globalThis.__randCapture.log.slice(0, 40) : null;
      const out = {};
      for (const svc of services) {
        const n = cy.getElementById(svc.id);
        const p = n.position();
        out[svc.id] = { x: p.x, y: p.y };
      }

      // Recompute spectral coords using the same captured RNG stream and transformed-graph rules
      // from `spectral.js`, to introspect the effective adjacency/PHI used by the layout.
      let debug = null;
      try {
        const Matrix =
          globalThis.coseBase?.layoutBase?.Matrix ??
          globalThis.layoutBase?.Matrix ??
          null;
        const SVD =
          globalThis.coseBase?.layoutBase?.SVD ??
          globalThis.layoutBase?.SVD ??
          null;
        if (Matrix && SVD && Array.isArray(randLog) && randLog.length >= 13) {
          const nodes = cy.nodes();
          const parentNodes = cy.nodes(":parent");

          // Build nodeIndexes (childless nodes only).
          const nodeIndexes = new Map();
          let idx = 0;
          for (let i = 0; i < nodes.length; i++) {
            const n = nodes[i];
            if (!n.isParent()) nodeIndexes.set(n.id(), idx++);
          }

          // Representative childless node per compound (parentChildMap).
          const parentChildMap = new Map();
          parentNodes.forEach((ele) => {
            let children = ele.children();
            while (children.nodes(":childless").length === 0) {
              children = children.nodes()[0].children();
            }
            let best = 0;
            let min = children.nodes(":childless")[0].connectedEdges().length;
            children.nodes(":childless").forEach((c, i) => {
              const d = c.connectedEdges().length;
              if (d < min) {
                min = d;
                best = i;
              }
            });
            parentChildMap.set(ele.id(), children.nodes(":childless")[best].id());
          });

          const nodeSize = nodeIndexes.size;
          const allNodesNeighborhood = Array.from({ length: nodeSize }, () => []);

          nodes.forEach((ele) => {
            const eleIndex = ele.isParent()
              ? nodeIndexes.get(parentChildMap.get(ele.id()))
              : nodeIndexes.get(ele.id());
            ele.neighborhood()
              .nodes()
              .forEach((n2) => {
                if (cy.elements().intersection(ele.edgesWith(n2)).length > 0) {
                  if (n2.isParent()) allNodesNeighborhood[eleIndex].push(parentChildMap.get(n2.id()));
                  else allNodesNeighborhood[eleIndex].push(n2.id());
                }
              });
          });

          const adj = Array.from({ length: nodeSize }, () => []);
          for (const [id, i] of nodeIndexes.entries()) {
            for (const nid of allNodesNeighborhood[i]) {
              const j = nodeIndexes.get(nid);
              if (typeof j === "number") adj[i].push(j);
            }
          }

          const nodeSeparation = 75;
          const sampleSize = Math.min(nodeSize, 25);
          const C = Array.from({ length: nodeSize }, () => Array(sampleSize).fill(0));
          const samples = Array(sampleSize).fill(0);
          const INF = 100000000;
          const minDist = Array(nodeSize).fill(INF);

          let r = 0;
          let sample = Math.floor(randLog[r++] * nodeSize);
          for (let col = 0; col < sampleSize; col++) {
            samples[col] = sample;

            // BFS distances
            const dist = Array(nodeSize).fill(INF);
            const q = [sample];
            dist[sample] = 0;
            for (let qi = 0; qi < q.length; qi++) {
              const cur = q[qi];
              for (const nxt of adj[cur]) {
                if (dist[nxt] === INF) {
                  dist[nxt] = dist[cur] + 1;
                  q.push(nxt);
                }
              }
              C[cur][col] = dist[cur] * nodeSeparation;
            }
            for (let i = 0; i < nodeSize; i++) {
              C[i][col] = C[i][col] * C[i][col];
              if (C[i][col] < minDist[i]) minDist[i] = C[i][col];
            }

            // Pick next sample (greedy).
            let max = 0;
            let maxInd = 1;
            for (let i = 0; i < nodeSize; i++) {
              if (minDist[i] > max) {
                max = minDist[i];
                maxInd = i;
              }
            }
            sample = maxInd;
          }

          // PHI
          const PHI = Array.from({ length: sampleSize }, () => Array(sampleSize).fill(0));
          for (let i = 0; i < sampleSize; i++) {
            for (let j = 0; j < sampleSize; j++) {
              PHI[i][j] = C[samples[j]][i];
            }
          }

          const svd = SVD.svd(PHI);
          const q0 = svd.S[0];
          const max_s = q0 * q0 * q0;
          const Sig = Array.from({ length: sampleSize }, () => Array(sampleSize).fill(0));
          for (let i = 0; i < sampleSize; i++) {
            Sig[i][i] = svd.S[i] / (svd.S[i] * svd.S[i] + max_s / (svd.S[i] * svd.S[i]));
          }
          const INV = Matrix.multMat(Matrix.multMat(svd.V, Sig), Matrix.transpose(svd.U));

          // Initial guesses for eigenvectors (interleaved, as in `spectral.js`).
          const Y1 = [];
          const Y2 = [];
          for (let i = 0; i < nodeSize; i++) {
            Y1[i] = randLog[r++];
            Y2[i] = randLog[r++];
          }
          let y1 = Matrix.normalize(Y1);
          let y2 = Matrix.normalize(Y2);
          const piTol = 0.0000001;
          const small = 0.000000001;

          function power1(Y) {
            let current = small;
            let previous = small;
            while (true) {
              const V = Y.slice();
              Y = Matrix.multGamma(Matrix.multL(Matrix.multGamma(V), C, INV));
              const theta = Matrix.dotProduct(V, Y);
              Y = Matrix.normalize(Y);
              current = Matrix.dotProduct(V, Y);
              const temp = Math.abs(current / previous);
              if (temp <= 1 + piTol && temp >= 1) return { V: Y.slice(), theta };
              previous = current;
            }
          }

          function power2(Y2, V1) {
            let current = small;
            let previous = small;
            while (true) {
              let V2 = Y2.slice();
              V2 = Matrix.minusOp(V2, Matrix.multCons(V1, Matrix.dotProduct(V1, V2)));
              Y2 = Matrix.multGamma(Matrix.multL(Matrix.multGamma(V2), C, INV));
              const theta = Matrix.dotProduct(V2, Y2);
              Y2 = Matrix.normalize(Y2);
              current = Matrix.dotProduct(V2, Y2);
              const temp = Math.abs(current / previous);
              if (temp <= 1 + piTol && temp >= 1) return { V: Y2.slice(), theta };
              previous = current;
            }
          }

          const r1 = power1(y1);
          const V1 = r1.V;
          const r2 = power2(y2, V1);

          const xCoords = Matrix.multCons(V1, Math.sqrt(Math.abs(r1.theta)));
          const yCoords = Matrix.multCons(r2.V, Math.sqrt(Math.abs(r2.theta)));

          const idOrder = Array.from(nodeIndexes.keys());
          const recomputed = {};
          for (const id of idOrder) {
            const i = nodeIndexes.get(id);
            recomputed[id] = { x: xCoords[i], y: yCoords[i] };
          }

          debug = { idOrder, adj, samples, PHI, Y1, Y2, recomputed };
        }
      } catch {}
      return {
        pos: out,
        randLog,
        debug,
        preBboxCenter,
      };
    }

    async function runEnforcedStep(step) {
      const cy = buildCy();
      const layout = cy.layout(layoutOpts(step));
      const done = new Promise((resolve) => layout.one("layoutstop", () => resolve()));
      layout.run();
      await done;

      const out = {};
      for (const svc of services) {
        const n = cy.getElementById(svc.id);
        const p = n.position();
        out[svc.id] = { x: p.x, y: p.y };
      }
      return out;
    }

    async function runConstraintHandlingFromSpectral(startPos, randLog) {
      // Apply the draft spectral positions as the starting point for constraint handling.
      const cy = buildCy();
      cy.startBatch();
      for (const svc of services) {
        const p = startPos?.[svc.id];
        if (p) cy.getElementById(svc.id).position(p);
      }
      cy.endBatch();

      // `step=transformed`: apply the Procrustes/relative-only transform (no enforcement, no CoSE).
      const layoutT = cy.layout(layoutOpts("transformed"));
      const doneT = new Promise((resolve) => layoutT.one("layoutstop", () => resolve()));
      layoutT.run();
      await doneT;
      const transformed = {};
      for (const svc of services) {
        const p = cy.getElementById(svc.id).position();
        transformed[svc.id] = { x: p.x, y: p.y };
      }

      // `step=enforced`: enforce constraints in position space (no transform, no CoSE).
      const layoutE = cy.layout(layoutOpts("enforced"));
      const doneE = new Promise((resolve) => layoutE.one("layoutstop", () => resolve()));
      layoutE.run();
      await doneE;
      const enforced = {};
      for (const svc of services) {
        const p = cy.getElementById(svc.id).position();
        enforced[svc.id] = { x: p.x, y: p.y };
      }

      return { draft: startPos, transformed, enforced, randLog };
    }

    async function runFinal() {
      const cy = buildCy();
      const iter1Runs = [];
      let iterHookInstalled = false;
      let iterTickCalls = 0;
      let iterRunLayoutCalls = 0;
      let iterRunSpringEmbedderCalls = 0;
      let iterCalcSpringCalls = 0;
      let iterCalcRepulsionCalls = 0;
      let iterMoveNodesCalls = 0;
      let iterLastTotalIterations = null;
      const iterHookDebug = {
        hasGlobalLayoutBase: false,
        hasCoseBaseLayoutBase: false,
        sameLayoutBaseObject: null,
        hasLayoutProtoRunLayout: false,
        hasCoSELayoutProtoTick: false,
      };
      let finalLayoutOptionsSnapshot = null;
      let sanityTickDelta = null;
      try {
        const CoSELayout = globalThis.coseBase?.CoSELayout ?? null;
        const Layout = globalThis.layoutBase?.Layout ?? globalThis.coseBase?.layoutBase?.Layout ?? null;
        const layoutProto = Layout?.prototype ?? null;

        iterHookDebug.hasGlobalLayoutBase = !!globalThis.layoutBase;
        iterHookDebug.hasCoseBaseLayoutBase = !!globalThis.coseBase?.layoutBase;
        if (globalThis.layoutBase && globalThis.coseBase?.layoutBase) {
          iterHookDebug.sameLayoutBaseObject = globalThis.layoutBase === globalThis.coseBase.layoutBase;
        }
        iterHookDebug.hasLayoutProtoRunLayout = !!layoutProto?.runLayout;
        iterHookDebug.hasCoSELayoutProtoTick = !!CoSELayout?.prototype?.tick;

        // IMPORTANT: In some dependency load orders, patching `coseBase.CoSELayout.prototype`
        // does not affect the CoSELayout instance actually used by cytoscape-fcose (e.g. when
        // multiple copies of layout-base/cose-base are present). `Layout.prototype.runLayout`
        // is part of layout-base and is reliably invoked by cytoscape-fcose's `coseLayout(...)`
        // path (`coseLayout.runLayout()`), so we hook there and then patch the *instance* `tick`.
        const restoreFns = [];

        // Always patch `CoSELayout.prototype.tick` as a baseline hook (this is the most direct
        // observer for per-iteration data).
        if (CoSELayout?.prototype?.tick && CoSELayout?.prototype?.getPositionsData) {
          const origTick = CoSELayout.prototype.tick;
          CoSELayout.prototype.tick = function () {
            iterTickCalls++;
            const res = origTick.call(this);
            iterLastTotalIterations = this.totalIterations;
            if (this.totalIterations === 1 && iter1Runs.length < 2) {
              iter1Runs.push({
                coolingFactor: this.coolingFactor,
                totalDisplacement: this.totalDisplacement,
                positions: this.getPositionsData(),
              });
            }
            return res;
          };
          restoreFns.push(() => {
            try {
              CoSELayout.prototype.tick = origTick;
            } catch {}
          });
        }

        if (CoSELayout?.prototype?.runSpringEmbedder && typeof CoSELayout.prototype.runSpringEmbedder === "function") {
          const origRunSpringEmbedder = CoSELayout.prototype.runSpringEmbedder;
          CoSELayout.prototype.runSpringEmbedder = function () {
            iterRunSpringEmbedderCalls++;
            return origRunSpringEmbedder.call(this);
          };
          restoreFns.push(() => {
            try {
              CoSELayout.prototype.runSpringEmbedder = origRunSpringEmbedder;
            } catch {}
          });
        }

        if (layoutProto?.runLayout && typeof layoutProto.runLayout === "function") {
          const origRunLayout = layoutProto.runLayout;
          layoutProto.runLayout = function () {
            iterRunLayoutCalls++;

            try {
              // Patch the instance methods (not prototypes) so we always hit the actual object
              // used during this run.
              if (!this.__mermanProbeTickHooked && typeof this.tick === "function") {
                const origTick = this.tick;
                this.tick = function () {
                  iterTickCalls++;
                  const res = origTick.call(this);
                  iterLastTotalIterations = this.totalIterations;
                  if (this.totalIterations === 1 && iter1Runs.length < 2 && typeof this.getPositionsData === "function") {
                    iter1Runs.push({
                      coolingFactor: this.coolingFactor,
                      totalDisplacement: this.totalDisplacement,
                      positions: this.getPositionsData(),
                    });
                  }
                  return res;
                };
                this.__mermanProbeTickHooked = true;
                restoreFns.push(() => {
                  try {
                    this.tick = origTick;
                    delete this.__mermanProbeTickHooked;
                  } catch {}
                });
              }

              if (!this.__mermanProbeSpringHooked && typeof this.runSpringEmbedder === "function") {
                const origRunSpringEmbedder = this.runSpringEmbedder;
                this.runSpringEmbedder = function () {
                  iterRunSpringEmbedderCalls++;
                  return origRunSpringEmbedder.call(this);
                };
                this.__mermanProbeSpringHooked = true;
                restoreFns.push(() => {
                  try {
                    this.runSpringEmbedder = origRunSpringEmbedder;
                    delete this.__mermanProbeSpringHooked;
                  } catch {}
                });
              }
            } catch {}

            return origRunLayout.call(this);
          };
          restoreFns.push(() => {
            try {
              layoutProto.runLayout = origRunLayout;
            } catch {}
          });
          iterHookInstalled = true;
        } else if (restoreFns.length > 0) {
          iterHookInstalled = true;
        }

        // Store on core for retrieval in the `finally` block.
        cy.scratch("_iterHookRestore", () => {
          for (let i = restoreFns.length - 1; i >= 0; i--) {
            try {
              restoreFns[i]();
            } catch {}
          }
        });
      } catch {}
      const layout = cy.layout(layoutOpts("all"));
      try {
        // Snapshot the effective options visible to the cytoscape-fcose Layout instance.
        const o = layout && (layout.options ?? layout._private?.options ?? null);
        if (o) {
          finalLayoutOptionsSnapshot = {
            name: o.name,
            quality: o.quality,
            step: o.step,
            animate: o.animate,
            randomize: o.randomize,
            nodeDimensionsIncludeLabels: o.nodeDimensionsIncludeLabels,
            packComponents: o.packComponents,
            tile: o.tile,
            fit: o.fit,
            padding: o.padding,
          };
        }
      } catch {}

      // Sanity: verify the hook can observe at least one CoSE tick in this page context.
      // If this stays 0 while the layout completes and positions change, we are likely looking
      // at a different CoSELayout implementation than the globals we patched.
      try {
        const before = iterTickCalls;
        const el = document.createElement("div");
        el.style = "width: 300px; height: 200px; position: absolute; left: -10000px; top: -10000px;";
        document.body.appendChild(el);
        const cy2 = cytoscape({
          container: el,
          elements: [
            { group: "nodes", data: { id: "sa", width: 80, height: 80 }, classes: "node-service" },
            { group: "nodes", data: { id: "sb", width: 80, height: 80 }, classes: "node-service" },
            { group: "edges", data: { id: "s-sa-sb", source: "sa", target: "sb" } },
          ],
          style: [{ selector: "node", style: { width: "data(width)", height: "data(height)" } }],
        });
        const l2 = cy2.layout({ name: "fcose", quality: "proof", randomize: true, animate: false, nodeDimensionsIncludeLabels: false, step: "all" });
        const done2 = new Promise((resolve) => l2.one("layoutstop", () => resolve()));
        l2.run();
        await done2;
        cy2.destroy();
        el.remove();
        sanityTickDelta = iterTickCalls - before;
      } catch {}

      const run1Pos = {};
      await new Promise((resolve) => {
        layout.one("layoutstop", () => {
          for (const svc of services) {
            const n = cy.getElementById(svc.id);
            const p = n.position();
            run1Pos[svc.id] = { x: p.x, y: p.y };
          }
          cy.startBatch();
          for (const edge of Object.values(cy.edges())) {
            if (edge.data?.()) {
              const { x: sX, y: sY } = edge.source().position();
              const { x: tX, y: tY } = edge.target().position();
              if (sX !== tX && sY !== tY) {
                const sEP = edge.sourceEndpoint();
                const tEP = edge.targetEndpoint();
                const sourceDir = edge.data("sourceDir");
                const [pointX, pointY] = isArchitectureDirectionY(sourceDir) ? [sEP.x, tEP.y] : [tEP.x, sEP.y];
                let W, D;
                D =
                  (pointY - sY + ((sX - pointX) * (sY - tY)) / (sX - tX)) /
                  Math.sqrt(1 + Math.pow((sY - tY) / (sX - tX), 2));
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

          layout.one("layoutstop", () => resolve());
          layout.run();
        });
        layout.run();
      });

      const nodeOut = {};
      for (const svc of services) {
        const n = cy.getElementById(svc.id);
        const p = n.position();
        const bb = n.boundingBox();
        const dim = n.layoutDimensions({ nodeDimensionsIncludeLabels: false });
        const dimWithLabels = n.layoutDimensions({ nodeDimensionsIncludeLabels: true });
        nodeOut[svc.id] = {
          pos: { x: p.x, y: p.y },
          bb: { x1: bb.x1, y1: bb.y1, w: bb.w, h: bb.h },
          dim: { w: dim.w, h: dim.h },
          dimWithLabels: { w: dimWithLabels.w, h: dimWithLabels.h },
          outer: { w: n.outerWidth(), h: n.outerHeight() },
        };
      }
      try {
        const restore = cy.scratch("_iterHookRestore");
        if (typeof restore === "function") restore();
      } catch {}
      return {
        run1Pos,
        run2: nodeOut,
        iter1Runs,
        iterHookInstalled,
        iterHookDebug,
        finalLayoutOptionsSnapshot,
        sanityTickDelta,
        iterTickCalls,
        iterRunLayoutCalls,
        iterRunSpringEmbedderCalls,
        iterCalcSpringCalls,
        iterCalcRepulsionCalls,
        iterMoveNodesCalls,
        iterLastTotalIterations,
      };
    }

    const draft = await runDraftSpectralOnly();
    return {
      iconSize,
      padding: db.getConfigField("padding"),
      fontSize: db.getConfigField("fontSize"),
      constraints: { alignmentConstraint, relativePlacementConstraint },
      spectralPhi: spectralSvdInputs[0] ?? null,
      draft,
      enforced: await runEnforcedStep("enforced"),
      transformed: await runEnforcedStep("transformed"),
      fromSpectral: await runConstraintHandlingFromSpectral(
        draft.debug?.recomputed ?? draft.pos,
        draft.randLog
      ),
      final: await runFinal(),
    };
  }, code);

  function fmt(n) {
    return Number.isFinite(n) ? n.toFixed(6) : String(n);
  }

  const iconSize = probe.iconSize;
  const half = iconSize / 2;

  const candidates = [
    {
      name: "pos",
      map: (n) => ({ x: n.pos.x, y: n.pos.y }),
    },
    {
      name: "pos-half",
      map: (n) => ({ x: n.pos.x - half, y: n.pos.y - half }),
    },
    {
      name: "pos-half-1",
      map: (n) => ({ x: n.pos.x - half - 1, y: n.pos.y - half - 1 }),
    },
    {
      name: "bb.x1y1",
      map: (n) => ({ x: n.bb.x1, y: n.bb.y1 }),
    },
    {
      name: "bb.x1y1+1",
      map: (n) => ({ x: n.bb.x1 + 1, y: n.bb.y1 + 1 }),
    },
  ];

  const probeNodes = new Map(Object.entries(probe.final.run2));
  const mappedScores = [];
  for (const c of candidates) {
    const mapped = new Map();
    for (const [id, n] of probeNodes.entries()) mapped.set(id, c.map(n));
    const score = scoreMapping(upstreamTx, mapped);
    mappedScores.push({ name: c.name, ...score });
  }
  mappedScores.sort((a, b) => a.sumAbs - b.sumAbs);

  console.log(`# ${fixtureStem}`);
  console.log(`iconSize=${probe.iconSize} padding=${probe.padding} fontSize=${probe.fontSize}`);
  console.log(`constraints: ${JSON.stringify(probe.constraints)}`);
  console.log("");
  console.log("Best mapping candidates (lower is better):");
  for (const s of mappedScores) {
    console.log(`- ${s.name}: sumAbs=${fmt(s.sumAbs)} maxAbs=${fmt(s.maxAbs)}`);
  }
  console.log("");
  console.log("Per-node deltas (using best mapping):");

  const best = candidates.find((c) => c.name === mappedScores[0].name);
  const mapped = new Map();
  for (const [id, n] of probeNodes.entries()) mapped.set(id, best.map(n));
  for (const id of Array.from(upstreamTx.keys()).sort()) {
    const u = upstreamTx.get(id);
    const n = probe.final.run2[id];
    const m = mapped.get(id);
    if (!u || !n || !m) continue;
    const dx = m.x - u.x;
    const dy = m.y - u.y;
    console.log(
      `${id}: upstream=(${fmt(u.x)},${fmt(u.y)}) mapped=(${fmt(m.x)},${fmt(m.y)}) d=(${fmt(dx)},${fmt(dy)}) pos=(${fmt(
        n.pos.x
      )},${fmt(n.pos.y)}) dim=(${fmt(n.dim.w)},${fmt(n.dim.h)}) dim+labels=(${fmt(
        n.dimWithLabels.w
      )},${fmt(n.dimWithLabels.h)}) outer=(${fmt(n.outer.w)},${fmt(n.outer.h)}) bb=(${fmt(
        n.bb.x1
      )},${fmt(n.bb.y1)},${fmt(n.bb.w)},${fmt(n.bb.h)})`
    );
  }

  console.log("");
  console.log("Constraint-only checkpoints (fcose `step`):");
  console.log(
    JSON.stringify(
      {
        draft: probe.draft,
        transformed: probe.transformed,
        enforced: probe.enforced,
        fromSpectral: probe.fromSpectral,
        finalRun1Pos: probe.final.run1Pos,
        finalIter1Runs: probe.final.iter1Runs,
        finalIterHookInstalled: probe.final.iterHookInstalled,
        finalIterHookDebug: probe.final.iterHookDebug,
        finalLayoutOptionsSnapshot: probe.final.finalLayoutOptionsSnapshot,
        sanityTickDelta: probe.final.sanityTickDelta,
        finalIterTickCalls: probe.final.iterTickCalls,
        finalIterRunLayoutCalls: probe.final.iterRunLayoutCalls,
        finalIterRunSpringEmbedderCalls: probe.final.iterRunSpringEmbedderCalls,
        finalIterCalcSpringCalls: probe.final.iterCalcSpringCalls,
        finalIterCalcRepulsionCalls: probe.final.iterCalcRepulsionCalls,
        finalIterMoveNodesCalls: probe.final.iterMoveNodesCalls,
        finalIterLastTotalIterations: probe.final.iterLastTotalIterations,
      },
      null,
      2
    )
  );

  console.log("");
  console.log("Captured spectral PHI (first SVD input):");
  console.log(JSON.stringify(probe.spectralPhi, null, 2));

  await browser.close();
}

main().catch((e) => {
  console.error(e && e.stack ? e.stack : String(e));
  process.exit(1);
});
