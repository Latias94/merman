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
    fcoseStages: [],
    fcoseShuffles: [],
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

  function safeLayoutRect(node) {
    if (!node) return null;
    try {
      return {
        x1: Number(node.getLeft()),
        y1: Number(node.getTop()),
        x2: Number(node.getLeft() + node.getWidth()),
        y2: Number(node.getTop() + node.getHeight()),
        w: Number(node.getWidth()),
        h: Number(node.getHeight()),
      };
    } catch (e) {
      return { error: String(e && e.message ? e.message : e) };
    }
  }

  function safeNumber(value) {
    return Number.isFinite(Number(value)) ? Number(value) : null;
  }

  function dumpMapEntries(map, valueMapper) {
    if (!map || typeof map.entries !== "function") {
      return null;
    }
    const out = [];
    for (const [key, value] of map.entries()) {
      out.push({
        key: String(key),
        value: valueMapper ? valueMapper(value) : value,
      });
    }
    return out;
  }

  function dumpConstraintList(value) {
    if (!Array.isArray(value)) {
      return value;
    }
    return value.map((constraint) => {
      if (!constraint || typeof constraint !== "object") {
        return constraint;
      }
      return {
        left: constraint.left == null ? null : String(constraint.left),
        right: constraint.right == null ? null : String(constraint.right),
        top: constraint.top == null ? null : String(constraint.top),
        bottom: constraint.bottom == null ? null : String(constraint.bottom),
        gap: safeNumber(constraint.gap),
      };
    });
  }

  function dumpRelativeState(layout) {
    if (!layout) {
      return null;
    }
    return {
      nodesInRelativeHorizontal: Array.isArray(layout.nodesInRelativeHorizontal)
        ? layout.nodesInRelativeHorizontal.map((v) => String(v))
        : null,
      nodesInRelativeVertical: Array.isArray(layout.nodesInRelativeVertical)
        ? layout.nodesInRelativeVertical.map((v) => String(v))
        : null,
      nodeToTempPositionMapHorizontal: dumpMapEntries(layout.nodeToTempPositionMapHorizontal, safeNumber),
      nodeToTempPositionMapVertical: dumpMapEntries(layout.nodeToTempPositionMapVertical, safeNumber),
      nodeToRelativeConstraintMapHorizontal: dumpMapEntries(
        layout.nodeToRelativeConstraintMapHorizontal,
        dumpConstraintList
      ),
      nodeToRelativeConstraintMapVertical: dumpMapEntries(
        layout.nodeToRelativeConstraintMapVertical,
        dumpConstraintList
      ),
      dummyToNodeForVerticalAlignment: dumpMapEntries(layout.dummyToNodeForVerticalAlignment, (value) =>
        Array.isArray(value) ? value.map((v) => String(v)) : value
      ),
      dummyToNodeForHorizontalAlignment: dumpMapEntries(layout.dummyToNodeForHorizontalAlignment, (value) =>
        Array.isArray(value) ? value.map((v) => String(v)) : value
      ),
      fixedNodesOnHorizontal:
        layout.fixedNodesOnHorizontal && typeof layout.fixedNodesOnHorizontal.values === "function"
          ? Array.from(layout.fixedNodesOnHorizontal.values()).map((v) => String(v))
          : null,
      fixedNodesOnVertical:
        layout.fixedNodesOnVertical && typeof layout.fixedNodesOnVertical.values === "function"
          ? Array.from(layout.fixedNodesOnVertical.values()).map((v) => String(v))
          : null,
    };
  }

  function dumpLayout(layout) {
    if (!layout || typeof layout.getAllNodes !== "function") {
      return null;
    }

    const nodes = layout.getAllNodes().map((node) => {
      let parent = "root";
      try {
        const ownerParent = node.getOwner?.()?.getParent?.();
        if (ownerParent && ownerParent.id != null) {
          parent = ownerParent.id;
        }
      } catch (e) {
        parent = "<error>";
      }

      let childCount = 0;
      try {
        childCount = node.getChild?.()?.getNodes?.()?.length ?? 0;
      } catch (e) {
        childCount = 0;
      }

      return {
        id: node.id,
        parent,
        childCount,
        rect: safeLayoutRect(node),
        center: {
          x: safeNumber(node.getCenterX?.()),
          y: safeNumber(node.getCenterY?.()),
        },
        forces: {
          displacementX: safeNumber(node.displacementX),
          displacementY: safeNumber(node.displacementY),
          springForceX: safeNumber(node.springForceX),
          springForceY: safeNumber(node.springForceY),
          repulsionForceX: safeNumber(node.repulsionForceX),
          repulsionForceY: safeNumber(node.repulsionForceY),
          gravitationForceX: safeNumber(node.gravitationForceX),
          gravitationForceY: safeNumber(node.gravitationForceY),
        },
        metrics: {
          nodeRepulsion: safeNumber(node.nodeRepulsion),
          noOfChildren: safeNumber(node.noOfChildren),
          labelWidth: safeNumber(node.labelWidth),
          labelHeight: safeNumber(node.labelHeight),
          paddingLeft: safeNumber(node.paddingLeft),
          paddingRight: safeNumber(node.paddingRight),
          paddingTop: safeNumber(node.paddingTop),
          paddingBottom: safeNumber(node.paddingBottom),
          fixedNodeWeight: safeNumber(node.fixedNodeWeight),
        },
      };
    });

    let minX = Number.POSITIVE_INFINITY;
    let minY = Number.POSITIVE_INFINITY;
    let maxX = Number.NEGATIVE_INFINITY;
    let maxY = Number.NEGATIVE_INFINITY;
    for (const node of nodes) {
      const rect = node.rect;
      if (!rect || rect.error) continue;
      minX = Math.min(minX, rect.x1);
      minY = Math.min(minY, rect.y1);
      maxX = Math.max(maxX, rect.x2);
      maxY = Math.max(maxY, rect.y2);
    }

    return {
      totalIterations: safeNumber(layout.totalIterations),
      coolingFactor: safeNumber(layout.coolingFactor),
      totalDisplacement: safeNumber(layout.totalDisplacement),
      maxIterations: safeNumber(layout.maxIterations),
      constraints: layout.constraints ? Object.keys(layout.constraints).sort() : [],
      relative: dumpRelativeState(layout),
      bbox:
        Number.isFinite(minX) && Number.isFinite(minY) && Number.isFinite(maxX) && Number.isFinite(maxY)
          ? { x1: minX, y1: minY, x2: maxX, y2: maxY, w: maxX - minX, h: maxY - minY }
          : null,
      nodes,
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

  globalThis.__mermanArchRenderPathProbeRegisterFcoseRun = (options, extra) => {
    const runIndex = probe.fcoseStages.filter((stage) => stage && stage.tag === "coseLayout.start").length;
    probe.fcoseStages.push({
      tag: "coseLayout.start",
      runIndex,
      time: performance.now(),
      extra: extra ? cloneMetricObject(extra) : null,
      options: {
        quality: options && options.quality,
        randomize: options && options.randomize,
        nodeSeparation: options && options.nodeSeparation,
        numIter: options && options.numIter,
        animate: options && options.animate,
        nodeDimensionsIncludeLabels: options && options.nodeDimensionsIncludeLabels,
        step: options && options.step,
        hasAlignmentConstraint: !!(options && options.alignmentConstraint),
        hasRelativePlacementConstraint: !!(options && options.relativePlacementConstraint),
        hasFixedNodeConstraint: !!(options && options.fixedNodeConstraint),
      },
    });
    return runIndex;
  };

  globalThis.__mermanArchRenderPathProbeDumpFcoseLayout = (tag, layout, extra) => {
    try {
      probe.fcoseStages.push({
        tag,
        runIndex: layout && layout.__mermanArchFcoseRunIndex,
        time: performance.now(),
        extra: extra ? cloneMetricObject(extra) : null,
        layout: dumpLayout(layout),
      });
    } catch (e) {
      probe.errors.push({
        tag: "fcose:" + tag,
        error: String(e && e.message ? e.message : e),
      });
    }
  };

  globalThis.__mermanArchRenderPathProbeRecordFcoseShuffle = (layout, axis, i, j, randomValue, before, after) => {
    try {
      probe.fcoseShuffles.push({
        runIndex: layout && layout.__mermanArchFcoseRunIndex,
        totalIterations: safeNumber(layout && layout.totalIterations),
        axis,
        i: safeNumber(i),
        j: safeNumber(j),
        randomValue: safeNumber(randomValue),
        before: Array.isArray(before) ? before.map((v) => String(v)) : null,
        after: Array.isArray(after) ? after.map((v) => String(v)) : null,
      });
    } catch (e) {
      probe.errors.push({
        tag: "fcose:shuffle",
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
    `                  CoSELayout.prototype.classicLayout = function() {\n                    this.nodesWithGravity = this.calculateNodesToApplyGravitationTo();`,
    `                  CoSELayout.prototype.classicLayout = function() {
                    try {
                      globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout("classicLayout.start", this, null);
                    } catch (e) {}
                    this.nodesWithGravity = this.calculateNodesToApplyGravitationTo();`,
    "bundled FCoSE CoSELayout classicLayout start"
  );

  out = replaceOnce(
    out,
    `                    if (CoSEConstants.APPLY_LAYOUT) {\n                      this.runSpringEmbedder();\n                    }\n                    return true;`,
    `                    if (CoSEConstants.APPLY_LAYOUT) {
                      this.runSpringEmbedder();
                    }
                    try {
                      globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout("classicLayout.end", this, null);
                    } catch (e) {}
                    return true;`,
    "bundled FCoSE CoSELayout classicLayout end"
  );

  out = replaceOnce(
    out,
    `                  CoSELayout.prototype.tick = function() {\n                    this.totalIterations++;`,
    `                  CoSELayout.prototype.tick = function() {
                    this.totalIterations++;
                    if ([1, 2, 10, 11, 12, 20, 21, 30, 31, 50, 51, 75, 90, 91, 99, 100, 200].includes(this.totalIterations)) {
                      try {
                        const __tag = this.totalIterations === 1 ? "tick-1.start" : "tick-" + this.totalIterations + ".start";
                        globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout(__tag, this, null);
                      } catch (e) {}
                    }`,
    "bundled FCoSE CoSELayout tick start"
  );

  out = replaceOnce(
    out,
    `                    this.calcGravitationalForces();\n                    this.moveNodes();\n                    this.animate();`,
    `                    this.calcGravitationalForces();
                    this.moveNodes();
                    if ([1, 2, 10, 11, 12, 20, 21, 30, 31, 50, 51, 75, 90, 91, 99, 100, 200].includes(this.totalIterations)) {
                      try {
                        const __tag = this.totalIterations === 1 ? "tick-1.after-move" : "tick-" + this.totalIterations + ".after-move";
                        globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout(__tag, this, null);
                      } catch (e) {}
                    }
                    this.animate();`,
    "bundled FCoSE CoSELayout tick after move"
  );

  out = replaceOnce(
    out,
    `                  CoSELayout.prototype.initConstraintVariables = function() {\n                    var self2 = this;`,
    `                  CoSELayout.prototype.initConstraintVariables = function() {
                    try {
                      globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout("initConstraintVariables.start", this, null);
                    } catch (e) {}
                    var self2 = this;`,
    "bundled FCoSE CoSELayout initConstraintVariables start"
  );

  out = replaceOnce(
    out,
    `                        this.shuffle = function(array4) {\n                          var j3, x5, i3;\n                          for (i3 = array4.length - 1; i3 >= 2 * array4.length / 3; i3--) {\n                            j3 = Math.floor(Math.random() * (i3 + 1));\n                            x5 = array4[i3];\n                            array4[i3] = array4[j3];\n                            array4[j3] = x5;\n                          }\n                          return array4;\n                        };`,
    `                        this.shuffle = function(array4) {
                          var j3, x5, i3;
                          for (i3 = array4.length - 1; i3 >= 2 * array4.length / 3; i3--) {
                            var __axis = array4 === this.nodesInRelativeHorizontal ? "horizontal" : array4 === this.nodesInRelativeVertical ? "vertical" : "unknown";
                            var __before = array4.slice();
                            var __random = Math.random();
                            j3 = Math.floor(__random * (i3 + 1));
                            x5 = array4[i3];
                            array4[i3] = array4[j3];
                            array4[j3] = x5;
                            try {
                              globalThis.__mermanArchRenderPathProbeRecordFcoseShuffle && globalThis.__mermanArchRenderPathProbeRecordFcoseShuffle(this, __axis, i3, j3, __random, __before, array4.slice());
                            } catch (e) {}
                          }
                          return array4;
                        };`,
    "bundled FCoSE CoSELayout constraint shuffle"
  );

  out = replaceOnce(
    out,
    `                  CoSELayout.prototype.updateDisplacements = function() {\n                    var self2 = this;`,
    `                  CoSELayout.prototype.updateDisplacements = function() {
                    if ([1, 2, 10, 11, 12, 20, 21, 30, 31, 50, 51, 75, 90, 91, 99, 100, 200].includes(this.totalIterations)) {
                      try {
                        const __tag = this.totalIterations === 1 ? "updateDisplacements.start" : "updateDisplacements.iter-" + this.totalIterations + ".start";
                        globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout(__tag, this, null);
                      } catch (e) {}
                    }
                    var self2 = this;`,
    "bundled FCoSE CoSELayout updateDisplacements start"
  );

  out = replaceOnce(
    out,
    `                    if (Object.keys(this.constraints).length > 0) {\n                      this.updateDisplacements();\n                    }\n                    for (var i2 = 0; i2 < lNodes.length; i2++) {`,
    `                    if (Object.keys(this.constraints).length > 0) {
                      this.updateDisplacements();
                    }
                    if ([1, 2, 10, 11, 12, 20, 21, 30, 31, 50, 51, 75, 90, 91, 99, 100, 200].includes(this.totalIterations)) {
                      try {
                        const __tag = this.totalIterations === 1 ? "tick-1.after-displacements" : "tick-" + this.totalIterations + ".after-displacements";
                        globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout(__tag, this, null);
                      } catch (e) {}
                    }
                    for (var i2 = 0; i2 < lNodes.length; i2++) {`,
    "bundled FCoSE CoSELayout moveNodes after-displacements"
  );

  out = replaceOnce(
    out,
    `                    var coseLayout3 = new CoSELayout();\n                    var gm = coseLayout3.newGraphManager();`,
    `                    var __mermanArchFcoseRunIndex = -1;
                    try {
                      __mermanArchFcoseRunIndex = globalThis.__mermanArchRenderPathProbeRegisterFcoseRun ? globalThis.__mermanArchRenderPathProbeRegisterFcoseRun(options2, {
                        nodes: nodes5.length,
                        edges: edges3.length,
                        hasConstraints: !!(options2.fixedNodeConstraint || options2.alignmentConstraint || options2.relativePlacementConstraint)
                      }) : -1;
                      options2.__mermanArchFcoseRunIndex = __mermanArchFcoseRunIndex;
                    } catch (e) {}
                    var coseLayout3 = new CoSELayout();
                    try { coseLayout3.__mermanArchFcoseRunIndex = __mermanArchFcoseRunIndex; } catch (e) {}
                    var gm = coseLayout3.newGraphManager();`,
    "bundled FCoSE coseLayout run registration"
  );

  out = replaceOnce(
    out,
    `                    processChildrenList(gm.addRoot(), aux.getTopMostNodes(nodes5), coseLayout3, options2);\n                    processEdges(coseLayout3, gm, edges3);`,
    `                    processChildrenList(gm.addRoot(), aux.getTopMostNodes(nodes5), coseLayout3, options2);
                    try {
                      globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout("coseLayout.after-process-children", coseLayout3, null);
                    } catch (e) {}
                    processEdges(coseLayout3, gm, edges3);`,
    "bundled FCoSE coseLayout after process children"
  );

  out = replaceOnce(
    out,
    `                    processConstraints(coseLayout3, options2);\n                    coseLayout3.runLayout();`,
    `                    processConstraints(coseLayout3, options2);
                    try {
                      globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout("coseLayout.after-process-edges-constraints", coseLayout3, null);
                    } catch (e) {}
                    coseLayout3.runLayout();`,
    "bundled FCoSE coseLayout after process edges and constraints"
  );

  out = replaceOnce(
    out,
    `                    coseLayout3.runLayout();\n                    return idToLNode;`,
    `                    coseLayout3.runLayout();
                    try {
                      globalThis.__mermanArchRenderPathProbeDumpFcoseLayout && globalThis.__mermanArchRenderPathProbeDumpFcoseLayout("coseLayout.after-runLayout", coseLayout3, null);
                    } catch (e) {}
                    return idToLNode;`,
    "bundled FCoSE coseLayout after runLayout"
  );

  out = replaceOnce(
    out,
    `                        var _diffOnX = originalCenter.x - (maxXCoord + minXCoord) / 2;\n                        var _diffOnY = originalCenter.y - (maxYCoord + minYCoord) / 2;\n                        Object.keys(componentResult).forEach(function(item) {`,
    `                        var _diffOnX = originalCenter.x - (maxXCoord + minXCoord) / 2;
                        var _diffOnY = originalCenter.y - (maxYCoord + minYCoord) / 2;
                        try {
                          globalThis.__mermanArchRenderPathProbe && globalThis.__mermanArchRenderPathProbe.fcoseStages.push({
                            tag: "relocateComponent.before-shift",
                            runIndex: options2 && options2.__mermanArchFcoseRunIndex,
                            time: performance.now(),
                            originalCenter: { x: originalCenter.x, y: originalCenter.y },
                            rectBbox: { x1: minXCoord, y1: minYCoord, x2: maxXCoord, y2: maxYCoord, w: maxXCoord - minXCoord, h: maxYCoord - minYCoord },
                            rectCenter: { x: (maxXCoord + minXCoord) / 2, y: (maxYCoord + minYCoord) / 2 },
                            delta: { x: _diffOnX, y: _diffOnY }
                          });
                        } catch (e) {}
                        Object.keys(componentResult).forEach(function(item) {`,
    "bundled FCoSE relocateComponent"
  );

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
