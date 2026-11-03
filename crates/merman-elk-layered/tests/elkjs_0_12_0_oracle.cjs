const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');

const [packageRoot, outputPath] = process.argv.slice(2);
assert(packageRoot && outputPath, 'Usage: node elkjs_0_12_0_oracle.cjs <elkjs-package> <output.json>');
const manifest = require(path.join(path.resolve(packageRoot), 'package.json'));
assert.equal(manifest.name, 'elkjs');
assert.equal(manifest.version, '0.12.0');
const ELK = require(path.resolve(packageRoot));
const elk = new ELK();

function graph(id, algorithm, sizes, connections, options = {}) {
  return {
    id,
    layoutOptions: {
      'elk.algorithm': algorithm,
      'elk.randomSeed': '1',
      'elk.omitNodeMicroLayout': 'true',
      ...options,
    },
    children: sizes.map(([width, height], index) => ({ id: `n${index}`, width, height })),
    edges: connections.map(([source, target], index) => ({
      id: `e${index}`, sources: [`n${source}`], targets: [`n${target}`],
    })),
  };
}

const branchSizes = [[40, 20], [30, 50], [70, 30], [20, 20], [45, 25]];
const branchEdges = [[0, 1], [0, 2], [1, 3], [2, 4]];
const cases = [
  graph('box-varied', 'box', [[60, 20], [20, 40], [45, 30], [30, 10], [80, 25]], []),
  graph('box-equal', 'box', Array.from({ length: 6 }, () => [40, 20]), []),
  graph('box-asymmetric', 'box', branchSizes, [], {
    'elk.padding': '[top=3,left=7,bottom=11,right=19]',
  }),
  graph('mrtree-components', 'mrtree', branchSizes, [[0, 1], [2, 3]]),
  graph('mrtree-asymmetric', 'mrtree', branchSizes, branchEdges, {
    'elk.padding': '[top=3,left=7,bottom=11,right=19]',
  }),
  graph('mrtree-cycle', 'mrtree', branchSizes.slice(0, 3), [[0, 1], [1, 2], [2, 0]]),
];
const boxLegacySizes = [[80, 20], [20, 80], [35, 30], [110, 15]];
cases.push(graph('box-legacy-varied', 'box', [...boxLegacySizes, [12, 12]], []));
cases.push(graph('box-legacy-ties', 'box', [[40, 40], [80, 20], [20, 80], [40, 40]], []));
const priorityBox = graph('box-legacy-priority', 'box', boxLegacySizes, []);
priorityBox.children.forEach((node, index) => { node.layoutOptions = { 'elk.priority': [0, 4, -2, 4][index] }; });
cases.push(priorityBox);
const interactiveBox = graph('box-legacy-interactive', 'box', boxLegacySizes, [], { 'elk.interactive': 'true' });
interactiveBox.children.forEach((node, index) => { [node.x, node.y] = [[100, 50], [40, 10], [10, 10], [0, 50]][index]; });
cases.push(interactiveBox);
const minimumBox = {
  'elk.nodeSize.constraints': '[MINIMUM_SIZE]',
  'elk.nodeSize.minimum': '(320,280)',
  'elk.padding': '[top=48,left=24,bottom=24,right=24]',
  'elk.spacing.nodeNode': '50',
  'elk.aspectRatio': '2',
  'elk.expandNodes': 'true',
};
cases.push(graph('box-legacy-container', 'box', boxLegacySizes, [], minimumBox));
cases.push(graph('box-legacy-minimum-height', 'box', [[40, 40], [40, 20], [20, 40]], [], {
  'elk.nodeSize.constraints': '[MINIMUM_SIZE]', 'elk.nodeSize.minimum': '(200,200)',
  'elk.aspectRatio': '2', 'elk.expandNodes': 'true',
}));
cases.push(graph('box-legacy-zero', 'box', [[0, 0], [0, 20], [20, 0]], []));
cases.push(graph('box-legacy-fractional', 'box', [[15.25, 12.125], [11.75, 33.625], [30.125, 15.25]], [], {
  'elk.spacing.nodeNode': '0.123456789',
}));
const rectpackingPreset = {
  'spacing.baseValue': '15',
  'spacing.nodeNode': '15',
  'elk.rectpacking.trybox': 'true',
  'elk.expandNodes': 'true',
  'elk.rectpacking.widthApproximation.strategy': 'SCANLINE',
  'elk.rectpacking.packing.compaction.rowHeightReevaluation': 'true',
  'elk.rectpacking.packing.compaction.iterations': '10',
  'elk.rectpacking.whiteSpaceElimination.strategy': 'EQUAL_BETWEEN_STRUCTURES',
  'elk.aspectRatio': '1.6',
};
cases.push(graph('rectpacking-stacked', 'rectpacking', [[50, 140], [30, 30], [70, 35], [40, 25], [20, 80]], [], rectpackingPreset));
cases.push(graph('rectpacking-trybox', 'rectpacking', [[40, 30], [60, 40], [80, 20], [40, 30]], [], rectpackingPreset));
cases.push(graph('rectpacking-legacy-large', 'rectpacking', [[80, 160], [40, 20], [60, 30], [100, 55], [30, 70], [45, 35], [90, 20]], [], rectpackingPreset));
cases.push(graph('rectpacking-legacy-minimum', 'rectpacking', [[50, 140], [30, 30], [70, 35], [40, 25], [20, 80]], [], {
  ...rectpackingPreset, 'elk.nodeSize.constraints': '[MINIMUM_SIZE]', 'elk.nodeSize.minimum': '(500,400)',
}));
cases.push(graph('rectpacking-legacy-asymmetric', 'rectpacking', [[50, 140], [30, 30], [70, 35], [40, 25], [20, 80]], [], {
  ...rectpackingPreset, 'spacing.nodeNode': '10', 'elk.padding': '[top=33,left=10,bottom=14,right=12]',
}));
cases.push(graph('mrtree-dag', 'mrtree', branchSizes.slice(0, 4), [[0, 1], [0, 2], [1, 3], [2, 3], [0, 3]]));
cases.push(graph('mrtree-multiroot', 'mrtree', branchSizes.slice(0, 3), [[0, 2], [1, 2]]));
cases.push(graph('mrtree-combined', 'mrtree', [[40, 20], [50, 30], [60, 25], [30, 40], [45, 35]], [[0, 1], [2, 3]], {
  'elk.separateConnectedComponents': 'false',
}));
cases.push(graph('mrtree-spacing', 'mrtree', branchSizes.slice(0, 4), [[0, 1], [0, 2], [1, 3], [2, 3], [0, 3]], {
  'elk.spacing.nodeNode': '50', 'elk.padding': '[top=45,left=25,bottom=15,right=35]',
}));
cases.push(graph('mrtree-single', 'mrtree', [[0, 0]], []));
cases.push(graph('mrtree-parallel', 'mrtree', [[40, 20], [30, 50]], [[0, 1], [0, 1]]));
cases.push(graph('mrtree-cycle-right', 'mrtree', [[40, 20], [50, 30], [60, 25]], [[0, 1], [1, 2], [2, 0]], { 'elk.direction': 'RIGHT' }));
cases.push(graph('mrtree-apportion', 'mrtree', [[40, 20], [50, 30], [60, 25], [30, 40], [45, 35], [70, 30], [35, 45], [25, 25], [50, 20], [80, 30], [25, 40], [45, 20]],
  [[0, 1], [0, 2], [0, 3], [1, 4], [1, 5], [4, 6], [5, 7], [3, 8], [3, 9], [8, 10], [9, 11]]));
cases.push(graph('mrtree-shared-gap', 'mrtree', [[40, 20], [50, 30], [60, 25], [30, 40], [45, 35], [70, 30]],
  [[0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [0, 4], [0, 5], [1, 5]]));
const coffmanOptions = {
  'elk.layered.layering.strategy': 'COFFMAN_GRAHAM',
  'elk.layered.layering.coffmanGraham.layerBound': '2',
  'elk.separateConnectedComponents': 'false',
};
cases.push(graph('coffman-disconnected', 'layered', Array.from({ length: 7 }, () => [40, 20]), [[0, 1], [1, 2], [3, 4]], coffmanOptions));
cases.push(graph('coffman-transitive', 'layered', Array.from({ length: 6 }, () => [40, 20]), [[0, 1], [0, 1], [1, 2], [0, 2], [0, 3], [3, 4], [0, 4], [1, 4], [4, 5]], coffmanOptions));
cases.push(graph('coffman-width', 'layered', [10, 70, 15, 40, 20, 90, 30, 10, 25].map(height => [40, height]), [[0, 3], [0, 4], [1, 3], [1, 5], [2, 4], [2, 5], [3, 6], [4, 7], [5, 8], [0, 8]], coffmanOptions));
cases.push(graph('coffman-position', 'layered', [30, 40, 20, 25, 50, 30, 20].map(width => [width, 20]), [[0, 2], [1, 3], [2, 4], [3, 4], [4, 5]], coffmanOptions));
const loop = graph('self-loop-inline', 'layered', [[80, 40]], [[0, 0]], {
  'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
});
loop.edges[0].labels = [{ text: 'loop', width: 60, height: 18, layoutOptions: { 'elk.edgeLabels.inline': 'true' } }];
cases.push(loop);
const opposingLoop = structuredClone(loop);
opposingLoop.id = 'self-loop-opposing-inline';
opposingLoop.children[0].layoutOptions = { 'elk.portConstraints': 'FIXED_SIDE' };
opposingLoop.children[0].ports = ['EAST', 'WEST'].map((side, index) => ({
  id: `p${index}`, width: 0, height: 0, layoutOptions: { 'elk.port.side': side },
}));
opposingLoop.edges[0].sources = ['p0'];
opposingLoop.edges[0].targets = ['p1'];
cases.push(opposingLoop);
function adapterOptions(algorithm) {
  return {
    'elk.algorithm': algorithm, 'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
    'elk.direction': 'DOWN', 'spacing.baseValue': '40',
    'elk.randomSeed': '1', 'elk.layered.nodePlacement.strategy': 'BRANDES_KOEPF',
    'elk.layered.nodePlacement.bk.fixedAlignment': 'BALANCED',
    'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
    'elk.layered.cycleBreaking.strategy': 'DEPTH_FIRST',
    'elk.layered.unnecessaryBendpoints': 'true',
    ...(algorithm === 'rectpacking' ? rectpackingPreset : {}),
  };
}
function explicitContainer(algorithm, titleWidth) {
  return {
    id: 'g',
    labels: [{ text: 'Group', width: titleWidth, height: 10 }],
    layoutOptions: {
      'elk.algorithm': algorithm, 'elk.hierarchyHandling': 'SEPARATE_CHILDREN',
      'nodeSize.constraints': '[MINIMUM_SIZE, NODE_LABELS]',
      'nodeSize.minimum': `(${titleWidth + 30},40)`,
      'spacing.baseValue': '24', 'spacing.nodeNode': '50',
      'elk.layered.spacing.edgeNodeBetweenLayers': '30', 'elk.spacing.edgeEdge': '20',
      'elk.layered.cycleBreaking.strategy': 'DEPTH_FIRST',
      'elk.layered.nodePlacement.bk.fixedAlignment': 'BALANCED',
      'elk.layered.nodePlacement.networkSimplex.nodeFlexibility': 'PORT_POSITION',
      'elk.spacing.portsSurrounding': '[top=12,left=12,bottom=12,right=12]',
      'nodeLabels.placement': '[H_CENTER V_TOP, INSIDE]',
      'elk.aspectRatio': '2', 'elk.contentAlignment': 'H_CENTER V_TOP',
      'elk.expandNodes': 'true', 'elk.padding': '[top=25,left=15,bottom=15,right=15]',
      ...(algorithm === 'rectpacking' ? {
        ...rectpackingPreset,
        'nodeSize.minimum': `(${titleWidth + 30},30)`,
        'elk.padding': '[top=20,left=10,bottom=10,right=10]',
      } : {}),
    },
    children: [{ id: 'a', width: 40, height: 20 }, { id: 'b', width: 60, height: 30 }],
    edges: [{ id: 'inside', sources: ['a'], targets: ['b'], labels: [{ text: '', width: 0, height: 0, layoutOptions: { 'elk.edgeLabels.inline': 'true' } }] }],
  };
}
function ordinaryContainer(algorithm, directional) {
  const group = explicitContainer('layered', 30);
  group.layoutOptions = {
    'nodeSize.constraints': '[MINIMUM_SIZE, NODE_LABELS]', 'nodeSize.minimum': '(30,0)',
    'spacing.baseValue': '24', 'spacing.nodeNode': '50',
    'elk.layered.spacing.edgeNodeBetweenLayers': '30', 'elk.spacing.edgeEdge': '20',
    'elk.layered.cycleBreaking.strategy': 'DEPTH_FIRST',
    'elk.layered.nodePlacement.strategy': 'BRANDES_KOEPF',
    'elk.layered.nodePlacement.bk.fixedAlignment': 'BALANCED',
    'elk.layered.nodePlacement.networkSimplex.nodeFlexibility': 'PORT_POSITION',
    'elk.spacing.portsSurrounding': '[top=12,left=12,bottom=12,right=12]',
    'nodeLabels.placement': '[H_CENTER V_TOP, INSIDE]',
    'elk.padding': '[top=24,left=24,bottom=24,right=24]',
    ...(directional ? { 'elk.algorithm': algorithm, 'elk.direction': 'DOWN', 'elk.hierarchyHandling': 'SEPARATE_CHILDREN' } : {}),
  };
  return group;
}
for (const algorithm of ['layered', 'box', 'rectpacking', 'force', 'stress', 'mrtree', 'radial', 'sporeOverlap']) {
  for (const titleWidth of [30, 300]) {
    cases.push({ id: `container-${algorithm}-${titleWidth}`, layoutOptions: adapterOptions('box'), children: [explicitContainer(algorithm, titleWidth)] });
  }
  const root = graph(`adapter-root-${algorithm}`, algorithm, [[40, 20], [60, 30], [30, 50]], [[0, 1], [1, 2]]);
  root.layoutOptions = adapterOptions(algorithm);
  root.edges.forEach((edge, index) => { edge.labels = [{ text: '', width: index === 0 ? 10 : 0, height: index === 0 ? 6 : 0,
    layoutOptions: { 'elk.edgeLabels.inline': 'true' } }]; });
  cases.push(root);
  if (['box', 'rectpacking', 'mrtree'].includes(algorithm)) {
    for (const directional of [false, true]) {
      const mixed = { id: `adapter-mixed-${algorithm}-${directional}`, layoutOptions: adapterOptions(algorithm),
        children: [ordinaryContainer(algorithm, directional), { id: 'c', width: 30, height: 50 }],
        edges: [{ id: 'outside', sources: ['g'], targets: ['c'], labels: [{ text: '', width: 10, height: 6,
          layoutOptions: { 'elk.edgeLabels.inline': 'true' } }] }],
      };
      cases.push(mixed);
    }
  }
}
for (const algorithm of ['radial', 'stress']) {
  const group = explicitContainer(algorithm, 300);
  cases.push({ id: `adapter-rectpacking-lifecycle-${algorithm}`, layoutOptions: adapterOptions('rectpacking'),
    children: [group, { id: 'c', width: 400, height: 100 }],
  });
}
for (const direction of ['DOWN', 'UP', 'RIGHT', 'LEFT']) {
  cases.push(graph(`mrtree-${direction}`, 'mrtree', branchSizes, branchEdges, {
    'elk.direction': direction,
  }));
  cases.push(graph(`layered-${direction}`, 'layered', branchSizes, branchEdges, {
    'elk.direction': direction,
    'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
  }));
}
for (const algorithm of ['force', 'stress', 'radial', 'sporeOverlap', 'rectpacking']) {
  cases.push(graph(`provider-${algorithm}`, algorithm, branchSizes, branchEdges));
}
cases.push(graph('coffman-graham', 'layered', Array.from({ length: 7 }, () => [40, 20]),
  [[0, 2], [1, 2], [2, 3], [2, 4], [3, 5], [4, 5], [1, 5]], {
    'elk.layered.layering.strategy': 'COFFMAN_GRAHAM',
    'elk.layered.layering.coffmanGraham.layerBound': '2',
    'elk.separateConnectedComponents': 'false',
  }));
cases.push(graph('model-order-parallel', 'layered', [[40, 20], [40, 20], [40, 20]],
  [[0, 2], [0, 1], [0, 2]], {
    'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
    'elk.layered.crossingMinimization.strategy': 'NONE',
  }));
cases.push({
  id: 'compound',
  layoutOptions: {
    'elk.algorithm': 'layered',
    'elk.hierarchyHandling': 'INCLUDE_CHILDREN',
    'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
    'elk.randomSeed': '1',
  },
  children: [
    { id: 'group', children: [{ id: 'a', width: 40, height: 20 }, { id: 'b', width: 30, height: 50 }] },
    { id: 'c', width: 70, height: 30 },
  ],
  edges: [
    { id: 'ab', sources: ['a'], targets: ['b'] },
    { id: 'bc', sources: ['b'], targets: ['c'] },
  ],
});
cases.push({
  id: 'hierarchical-randomized-tries',
  layoutOptions: {
    'elk.algorithm': 'layered', 'elk.direction': 'DOWN',
    'elk.hierarchyHandling': 'INCLUDE_CHILDREN', 'elk.randomSeed': '1',
    'elk.layered.considerModelOrder.strategy': 'NODES_AND_EDGES',
    'elk.layered.unnecessaryBendpoints': 'true', 'spacing.baseValue': '40',
    'elk.omitNodeMicroLayout': 'true',
  },
  children: [
    ...[['bar', ['E', 'F']], ['foo', ['C', 'D']]].map(([id, children]) => ({
      id, width: 0, height: 0,
      layoutOptions: { 'elk.direction': 'DOWN', 'spacing.baseValue': '30', 'elk.portConstraints': 'FREE' },
      children: children.map(id => ({ id, width: 54, height: 68 })),
    })),
    ...['A', 'B', 'G'].map(id => ({ id, width: 54, height: 68 })),
  ],
  edges: [['A', 'B'], ['B', 'C'], ['C', 'D'], ['B', 'D'], ['D', 'E'], ['E', 'A'],
    ['E', 'F'], ['F', 'D'], ['F', 'G'], ['B', 'G'], ['G', 'D']].map(([source, target]) => ({
      id: `L_${source}_${target}_0`, sources: [source], targets: [target],
    })),
});

(async () => {
  const observedWorkerPath = path.join(path.resolve(packageRoot), 'lib/elk-worker.js');
  const workerSource = fs.readFileSync(observedWorkerPath, 'utf8');
  const Module = require('node:module');
  const observedModule = new Module(observedWorkerPath, module);
  observedModule.filename = observedWorkerPath;
  observedModule.paths = module.paths;
  const phases = [];
  observedModule._compile(`${workerSource}\n
    module.exports.phases = [];
    const capture = (stage, graph) => module.exports.phases.push({
      stage,
      graph: $getDesignation(graph),
      layers: graph.layers.array.map(layer => layer.nodes.array.map(node => ({
        id: $getDesignation_2(node),
        type: node.type_0.toString(),
        ports: node.ports.array.map(port => ({
          id: $getDesignation(port), side: port.side.toString(),
          outgoing: port.outgoingEdges.array.map(edge => $getDesignation(edge)),
        })),
      }))),
    });
    const coffman = $process_67;
    $process_67 = function(state, graph, monitor) { coffman(state, graph, monitor); capture('coffman-graham', graph); };
    const inputSort = $process_52;
    $process_52 = function(graph, monitor) {
      if ($getDesignation(graph) === 'hierarchical-randomized-tries') capture('before-sort-by-input-model', graph);
      inputSort(graph, monitor);
      capture('sort-by-input-model', graph);
    };
    const sweep = $process_76;
    $process_76 = function(state, graph, monitor) {
      sweep(state, graph, monitor);
      if ($getDesignation(graph) === 'hierarchical-randomized-tries') capture('layer-sweep', graph);
    };
    const minimize = $minimizeCrossingsWithCounter;
    $minimizeCrossingsWithCounter = function(state, data) {
      if ($getDesignation(data.lGraph) !== 'hierarchical-randomized-tries') return minimize(state, data);
      const before = data.currentNodeOrder.map(layer => layer.map(node => $getDesignation_2(node)));
      const result = minimize(state, data);
      module.exports.phases.push({ stage: 'randomized-try', graph: $getDesignation(data.lGraph),
        before, after: data.currentNodeOrder.map(layer => layer.map(node => $getDesignation_2(node))), crossings: result });
      return result;
    };
  `, observedWorkerPath);
  const observedElk = new ELK({ workerFactory: () => new observedModule.exports.Worker() });
  const observations = [];
  for (const input of cases) {
    const output = await elk.layout(structuredClone(input));
    const replay = await elk.layout(structuredClone(input));
    const geometry = value => JSON.parse(JSON.stringify(value, (key, entry) => key === '$H' ? undefined : entry));
    assert.deepEqual(geometry(output), geometry(replay), `${input.id}: reused-instance replay`);
    observedModule.exports.phases.length = 0;
    const observedOutput = await observedElk.layout(structuredClone(input));
    assert.deepEqual(geometry(output), geometry(observedOutput), `${input.id}: observer preserves public geometry`);
    phases.push({ id: input.id, phases: structuredClone(observedModule.exports.phases) });
    observations.push({ input, output });
  }
  const workerPath = path.join(path.resolve(packageRoot), 'lib/elk-worker.min.js');
  const evidence = {
    package: `${manifest.name}@${manifest.version}`,
    elkjsCommit: 'ff5771d7165445c42c408bb8a090c8035272218c',
    eclipseComparisonCommit: '323312916048544e4e594eab3303cdb0d01e2929',
    sourceMapping: 'The elkjs CI clones Eclipse ELK master; the release tag is a comparison point, not a proven unique npm build input.',
    node: process.version,
    workerSha256: crypto.createHash('sha256').update(fs.readFileSync(workerPath)).digest('hex'),
    collectedAt: new Date().toISOString(),
    observations,
    observedWorkerSha256: crypto.createHash('sha256').update(workerSource).digest('hex'),
    phases,
  };
  fs.mkdirSync(path.dirname(path.resolve(outputPath)), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(evidence, null, 2)}\n`);
  console.log(JSON.stringify({ outputPath, cases: observations.length, workerSha256: evidence.workerSha256 }));
  for (const { input, output } of observations) {
    console.log(JSON.stringify({ id: input.id, size: [output.width, output.height],
      nodes: output.children.map(({ id, x, y, width, height, children }) => [id, x, y, width, height,
        ...(children ? [children.map(({ id, x, y, width, height }) => [id, x, y, width, height])] : [])]) }));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
