import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

import { Graph } from 'dagre-d3-es/src/graphlib/index.js';
import { layout as dagreLayout } from 'dagre-d3-es/src/dagre/index.js';

function usage() {
  console.error('usage: node tools/dagre-harness/run.mjs --in <input.json> --out <output.json>');
}

function parseArgs(argv) {
  const args = { in: null, out: null };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--in') {
      i++;
      args.in = argv[i] ?? null;
      continue;
    }
    if (a === '--out') {
      i++;
      args.out = argv[i] ?? null;
      continue;
    }
    if (a === '--help' || a === '-h') {
      return { help: true, ...args };
    }
    return { error: `unknown arg: ${a}`, ...args };
  }
  return args;
}

function requirePath(p, what) {
  if (!p || typeof p !== 'string' || p.trim() === '') {
    throw new Error(`missing ${what}`);
  }
  return p;
}

function normalizeRankDir(v) {
  if (v == null) return v;
  if (typeof v !== 'string') return v;
  const t = v.trim();
  if (t === '') return v;
  // Mermaid passes 'TB'/'LR' etc. Dagre-d3-es internally lowercases in some places, but
  // keep the Mermaid-style uppercase to match its own `makeSpaceForEdgeLabels` checks.
  return t.toUpperCase();
}

function findCommonEdges(g, id1, id2) {
  const edges = g.edges();
  const edges1 = edges.filter((e) => e.v === id1 || e.w === id1);
  const edges2 = edges.filter((e) => e.v === id2 || e.w === id2);
  const edges1Prim = edges1.map((e) => ({ v: e.v === id1 ? id2 : e.v, w: e.w === id1 ? id1 : e.w }));
  const edges2Prim = edges2.map((e) => ({ v: e.v, w: e.w }));
  return edges1Prim.filter((eIn1) => edges2Prim.some((e) => eIn1.v === e.v && eIn1.w === e.w));
}

// Ported (minimal) from Mermaid's `findNonClusterChild`:
// repo-ref/mermaid/packages/mermaid/src/rendering-util/layout-algorithms/dagre/mermaid-graphlib.js
function findNonClusterChild(id, g, clusterId) {
  const children = g.children(id) ?? [];
  if (children.length < 1) return id;

  let reserve = null;
  for (const child of children) {
    const leafId = findNonClusterChild(child, g, clusterId);
    if (!leafId) continue;

    const commonEdges = findCommonEdges(g, clusterId, leafId);
    if (commonEdges.length > 0) {
      reserve = leafId;
    } else {
      return leafId;
    }
  }

  return reserve ?? id;
}

function normalizeClusterEdgeEndpoints(g) {
  // Dagre's layout pipeline assumes edges never touch compound nodes (nodes with children).
  // Mermaid enforces this via `adjustClustersAndEdges`. Here we apply a minimal, deterministic
  // normalization so the harness can run and match Mermaid's expectations.
  const nodes = g.nodes();
  const clusters = new Set(nodes.filter((id) => (g.children(id) ?? []).length > 0));
  if (clusters.size === 0) return;

  const anchorByCluster = new Map();
  for (const clusterId of clusters) {
    anchorByCluster.set(clusterId, findNonClusterChild(clusterId, g, clusterId));
  }

  const edges = g.edges();
  for (const e of edges) {
    const v2 = clusters.has(e.v) ? anchorByCluster.get(e.v) : e.v;
    const w2 = clusters.has(e.w) ? anchorByCluster.get(e.w) : e.w;
    if (v2 === e.v && w2 === e.w) continue;

    const label = g.edge(e);
    g.removeEdge(e);
    g.setEdge(v2, w2, label, e.name);
  }
}

function buildGraph(input) {
  const options = input.options ?? { directed: true, multigraph: true, compound: true };
  const g = new Graph(options);
  // Mermaid's dagre integration relies on node/edge labels being mutable objects.
  // Graphlib's default label is `undefined`, which breaks dagre's rank assignment.
  g.setDefaultNodeLabel(() => ({}));
  g.setDefaultEdgeLabel(() => ({}));

  const graphLabel = { ...(input.graph ?? {}) };
  if (Object.prototype.hasOwnProperty.call(graphLabel, 'rankdir')) {
    graphLabel.rankdir = normalizeRankDir(graphLabel.rankdir);
  }
  g.setGraph(graphLabel);

  const nodes = Array.isArray(input.nodes) ? input.nodes : [];
  // Pass 1: create all nodes with labels first (avoid implicit undefined labels).
  for (const n of nodes) {
    const id = n?.id;
    if (typeof id !== 'string' || id.length === 0) continue;
    const label = { ...(n.label ?? {}) };
    g.setNode(id, label);
  }
  // Pass 2: set parents after all nodes exist.
  for (const n of nodes) {
    const id = n?.id;
    if (typeof id !== 'string' || id.length === 0) continue;
    if (typeof n.parent === 'string' && n.parent.length > 0) {
      g.setParent(id, n.parent);
    }
  }

  const edges = Array.isArray(input.edges) ? input.edges : [];
  for (const e of edges) {
    const v = e?.v;
    const w = e?.w;
    if (typeof v !== 'string' || typeof w !== 'string') continue;
    const name = typeof e.name === 'string' ? e.name : undefined;
    const label = { ...(e.label ?? {}) };
    // Graphlib signature: setEdge(v, w, value, name)
    g.setEdge(v, w, label, name);
  }

  return g;
}

function snapshotGraph(g) {
  const graph = g.graph();
  const nodes = g.nodes().map((id) => {
    const n = g.node(id) ?? {};
    return { id, label: n, parent: g.parent(id) ?? null };
  });
  const edges = g.edges().map((e) => {
    const lbl = g.edge(e) ?? {};
    return {
      v: e.v,
      w: e.w,
      name: e.name ?? null,
      label: lbl,
    };
  });
  return { graph, nodes, edges };
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    usage();
    process.exit(0);
  }
  if (args.error) {
    usage();
    throw new Error(args.error);
  }

  const inPath = requirePath(args.in, '--in');
  const outPath = requirePath(args.out, '--out');

  const inputRaw = fs.readFileSync(inPath, 'utf8');
  const input = JSON.parse(inputRaw);

  const g = buildGraph(input);
  normalizeClusterEdgeEndpoints(g);
  dagreLayout(g);

  const out = snapshotGraph(g);
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, JSON.stringify(out, null, 2) + '\n', 'utf8');
}

main().catch((err) => {
  console.error(String(err?.stack ?? err));
  process.exit(1);
});
