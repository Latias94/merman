// Inspect the delivered SVG in Chromium, after all renderer clocks have stopped.
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const browserTools = createRequire(new URL("../mermaid-cli/package.json", import.meta.url));

// Puppeteer serializes this function into the document containing the inert SVG.
function inspectSvg(facts) {
  const root = document.querySelector("svg");
  const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
  const epsilon = 1e-5; // CSS pixels, for comparisons of browser float geometry.
  const passed = () => ({ status: "passed" });
  const failed = (reason) => ({ status: "failed", reason });
  const unavailable = (reason) => ({ status: "unavailable", reason });
  const unique = (items) => items.length === 1 ? items[0] : null;
  const drawn = (selector) => [...(root?.querySelectorAll(selector) ?? [])].filter(
    (element) => !element.closest("defs,symbol,marker,pattern,mask,clipPath"),
  );

  function visible(element) {
    if (getComputedStyle(element).visibility !== "visible") return false;
    for (let e = element; e instanceof Element; e = e.parentElement) {
      const style = getComputedStyle(e);
      if (style.display === "none" || Number(style.opacity) === 0) return false;
    }
    return true;
  }

  function hasColor(color) {
    return color !== "none" && color !== "transparent" && !/^rgba\([^)]*,\s*0(?:\.0+)?\s*\)$/.test(color);
  }

  function painted(element, property) {
    const style = getComputedStyle(element);
    return visible(element) && hasColor(style.getPropertyValue(property)) && Number(style.getPropertyValue(`${property}-opacity`)) > 0 &&
      (property !== "stroke" || parseFloat(style.strokeWidth) > 0);
  }

  const normalizedText = (text) => text.replace(/\s+/gu, " ").trim();
  function* paintedTextRuns(label) {
    const html = label.localName === "foreignObject";
    const walker = document.createTreeWalker(label, NodeFilter.SHOW_TEXT);
    for (let text = walker.nextNode(); text; text = walker.nextNode()) {
      const element = text.parentElement;
      if (element.closest("style,script,title,desc,metadata,template,defs,symbol,marker,pattern,mask,clipPath")) continue;
      if (html ? !visible(element) || !hasColor(getComputedStyle(element).color)
        : !painted(element, "fill") && !painted(element, "stroke")) continue;
      const range = document.createRange();
      range.selectNodeContents(text);
      const boxes = [...range.getClientRects()].filter((r) => r.width > 0 && r.height > 0);
      yield { text: text.data, element, boxes };
    }
  }

  function sequenceMessageCounts() {
    if (!root?.matches('[aria-roledescription="sequence"],[aria-roledescription="sequenceDiagram"]')) return null;
    let labels;
    if (root.classList.contains("pele-sequence")) {
      // Pele puts labels in a separate layer and shares the message edge's data-id.
      const edges = drawn(":scope > g > g.pele-edges > g.pele-message[data-id]");
      if (!edges.length) return null;
      const ids = new Set(edges.map((edge) => edge.getAttribute("data-id")));
      labels = drawn(":scope > g > g.pele-edge-labels > g.pele-edge-label[data-id] > text:only-of-type")
        .filter((text) => ids.has(text.parentElement.getAttribute("data-id")));
    } else if (root.classList.contains("mermaid-sequence")) {
      if (!root.querySelector(":scope > g.messages")) return null;
      labels = drawn(":scope > g.messages > g.message[data-id] > text.messageText[data-id]")
        .filter((text) => text.getAttribute("data-id") === text.parentElement.getAttribute("data-id"));
    } else {
      // Merman and mmdc emit one messageText directly under the Sequence SVG.
      labels = drawn(":scope > text.messageText");
      if (!labels.length && !root.querySelector(":scope > [data-et='message']")) return null;
    }
    const viewport = root.getBoundingClientRect();
    const counts = new Map();
    const labeledIds = new Set();
    for (const label of labels) {
      const parts = [];
      for (const { text, element, boxes } of paintedTextRuns(label)) {
        if (!boxes.some((r) => r.right > viewport.left && r.left < viewport.right &&
          r.bottom > viewport.top && r.top < viewport.bottom)) continue;
        // Layout bounds alone cannot establish paint through a clip, mask or filter.
        for (let e = element; e instanceof SVGElement; e = e.parentElement) {
          const style = getComputedStyle(e);
          if (style.clipPath !== "none" || style.maskImage !== "none" || style.filter !== "none") return null;
        }
        parts.push(text);
      }
      const text = normalizedText(parts.join(""));
      if (!text) continue;
      if (label.parentElement !== root) {
        const id = label.parentElement.getAttribute("data-id");
        if (!id || labeledIds.has(id)) return null;
        labeledIds.add(id);
      }
      counts.set(text, (counts.get(text) ?? 0) + 1);
    }
    return counts;
  }

  function outline(group) {
    if (!group) return null;
    // Reviewed Flowcharts use direct rectangle and diamond outlines. Labels have their own groups.
    const shape = unique([...group.children].filter((e) => e.matches("rect,polygon")));
    if (!shape || !visible(shape) || (!painted(shape, "fill") && !painted(shape, "stroke"))) return null;
    const matrix = shape.getScreenCTM();
    if (!matrix) return null;
    const { x, y, width: w, height: h } = shape.getBBox();
    if (!(w > 0 && h > 0)) return null;
    const vertices = shape.localName === "polygon" ? [...shape.points] : [
      new DOMPoint(x, y), new DOMPoint(x + w, y), new DOMPoint(x + w, y + h), new DOMPoint(x, y + h),
    ];
    return { shape, points: vertices.map((p) => p.matrixTransform(matrix)), bounds: shape.getBoundingClientRect() };
  }

  function nodeId(group) {
    if (group.hasAttribute("data-id")) return group.getAttribute("data-id");
    // Mermaid.js prefixes DOM node IDs with the diagram ID and appends a numeric counter.
    const prefix = `${root.id}-flowchart-`;
    return group.id.startsWith(prefix) ? /^(.*)-\d+$/.exec(group.id.slice(prefix.length))?.[1] : null;
  }

  const nodes = drawn("g.node,g.pele-node").map((group) => ({ id: nodeId(group), outline: outline(group) }));
  const getNode = (id) => unique(nodes.filter((n) => n.id === id))?.outline;
  const edges = drawn("path[data-edge='true'],g.edge,g.pele-edge");
  const clusters = drawn("g.cluster,g.pele-cluster");
  function cluster(id) {
    const group = unique(clusters.filter((g) => g.hasAttribute("data-id")
      ? g.getAttribute("data-id") === id : g.id === id || g.id === `${root.id}-${id}`));
    const box = outline(group);
    if (!box || box.shape.localName !== "rect") return null;
    const { b, c } = box.shape.getScreenCTM();
    return Math.abs(b) < epsilon && Math.abs(c) < epsilon ? box.bounds : null;
  }

  function membership(id, group) {
    const box = getNode(id)?.bounds, frame = cluster(group);
    if (!box || !frame) return unavailable("missing or ambiguous node outline or rectangular cluster border");
    if (box.left < frame.left - epsilon || box.right > frame.right + epsilon ||
        box.top < frame.top - epsilon || box.bottom > frame.bottom + epsilon) return failed("node extends outside its cluster");
    for (const sibling of Object.keys(facts.groups)) {
      if (sibling === group) continue;
      const other = cluster(sibling);
      if (!other) return unavailable(`missing or ambiguous sibling cluster ${sibling}`);
      if (Math.min(box.right, other.right) - Math.max(box.left, other.left) > epsilon &&
          Math.min(box.bottom, other.bottom) - Math.max(box.top, other.top) > epsilon) return failed(`node overlaps sibling cluster ${sibling}`);
    }
    return passed();
  }

  function labelWidth(id, expected) {
    const node = getNode(id);
    if (!node || node.shape.localName !== "rect") return unavailable("expected a rectangular node outline");
    const transform = node.shape.getScreenCTM();
    if (Math.abs(transform.b) > epsilon || Math.abs(transform.c) > epsilon) return unavailable("rotated node has no horizontal rectangle bound");
    const labels = [...node.shape.parentElement.querySelectorAll("text,foreignObject")].filter(
      (e) => !e.closest("defs,symbol,clipPath,marker") && (e.localName === "foreignObject" || !e.closest("foreignObject")),
    );
    const label = unique(labels);
    if (!label) return unavailable("expected a single SVG text or HTML label on the node");
    const html = label.localName === "foreignObject";
    if (!visible(label) || (!html && !painted(label, "fill") && !painted(label, "stroke"))) return failed("node label is unpainted");
    const text = [], boxes = [], families = new Set();
    for (const run of paintedTextRuns(label)) {
      text.push(run.text);
      if (!run.text.trim()) continue;
      boxes.push(...run.boxes);
      families.add(getComputedStyle(run.element).fontFamily);
    }
    if (normalizedText(text.join("")) !== normalizedText(expected)) return failed("expected label text is missing from its node");
    if (!boxes.length) return unavailable("browser returned no text layout bounds");
    const bounds = [["node", node.bounds]];
    if (html && getComputedStyle(label).overflowX !== "visible") {
      const ctm = label.getScreenCTM();
      if (!ctm || Math.abs(ctm.b) > epsilon || Math.abs(ctm.c) > epsilon) return unavailable("rotated foreignObject has no horizontal rectangle bound");
      bounds.push(["foreignObject", label.getBoundingClientRect()]);
    }
    const textLeft = Math.min(...boxes.map((r) => r.left)), textRight = Math.max(...boxes.map((r) => r.right));
    const clearance = Object.fromEntries(bounds.map(([owner, box]) => [owner, { left: textLeft - box.left, right: box.right - textRight }]));
    const exceeded = Object.keys(clearance).find((owner) => Math.min(clearance[owner].left, clearance[owner].right) < -epsilon);
    return { ...(exceeded ? failed(`label exceeds the horizontal ${exceeded} bounds`) : passed()),
      clearance_px: clearance, css_font_families: [...families] };
  }

  function outlineDistance(p, points) {
    return Math.min(...points.map((a, i) => {
      const b = points[(i + 1) % points.length], dx = b.x - a.x, dy = b.y - a.y;
      const length2 = dx * dx + dy * dy;
      const t = length2 ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / length2)) : 0;
      return Math.hypot(p.x - a.x - t * dx, p.y - a.y - t * dy);
    }));
  }

  // A deliberately small marker vocabulary. Curves and the route itself use browser geometry.
  function triangle(shape) {
    if (shape?.localName === "polygon") return shape.points.length === 3 ? [...shape.points] : null;
    if (shape?.localName !== "path") return null;
    const n = "([+-]?(?:\\d+(?:\\.\\d*)?|\\.\\d+)(?:[eE][+-]?\\d+)?)";
    const pair = `${n}(?:\\s*,\\s*|\\s+)${n}`;
    const match = new RegExp(`^\\s*M\\s*${pair}\\s*L\\s*${pair}\\s*L\\s*${pair}\\s*[zZ]\\s*$`).exec(shape.getAttribute("d"));
    if (!match) return null;
    return [1, 3, 5].map((i) => new DOMPoint(Number(match[i]), Number(match[i + 1])));
  }

  function arrowTriangle(shape) {
    const vertices = triangle(shape);
    if (!vertices) return null;
    // The reference renderers round inline vertices to hundredths. Each side length
    // can move by sqrt(2) hundredths. Require one identifiable isosceles apex.
    const rounding = 2 * Math.SQRT2 * 0.01;
    const tips = vertices.map((tip, i) => [tip, vertices[(i + 1) % 3], vertices[(i + 2) % 3]])
      .filter(([tip, a, b]) => Math.abs(dist(tip, a) - dist(tip, b)) <= rounding);
    return unique(tips);
  }

  function arrow(fact) {
    const edge = unique(edges.filter((e) => e.getAttribute("data-id") === fact.edge));
    if (!edge || !getNode(fact.from) || !getNode(fact.to) || nodes.some((n) => !n.outline || !n.id)) {
      return unavailable("edge ID or node outline is missing, ambiguous or unsupported");
    }
    const path = edge.localName === "path" ? edge : unique([...edge.children].filter((e) => e.matches("path:not(.marker,.pele-marker)")));
    if (!path) return unavailable("could not identify one routed path");
    const length = path.getTotalLength(), matrix = path.getScreenCTM();
    if (!(length > 0) || !matrix || !painted(path, "stroke")) return failed("route has no visible stroke");
    const point = (at) => path.getPointAtLength(at).matrixTransform(matrix);
    const start = point(0), end = point(length);
    const markers = edge.localName === "path" ? [] : [...edge.querySelectorAll(":scope > .marker,:scope > .pele-marker")];
    const css = getComputedStyle(path);
    const ends = [css.markerStart, css.markerEnd];
    const bindings = ends.filter((url) => url !== "none");
    if (!markers.length && !bindings.length) return failed("arrowhead is absent");
    if (markers.length + bindings.length !== 1) return unavailable("expected a single arrowhead");
    let atStart, span, heading, targetPoint;
    if (bindings.length) {
      atStart = ends[0] !== "none";
      targetPoint = atStart ? start : end;
      const ref = /^url\(["']?#([^"')]+)["']?\)$/.exec(bindings[0]);
      const marker = ref && document.getElementById(ref[1]);
      if (!ref) return unavailable("unsupported marker reference");
      if (marker?.localName !== "marker") return failed("referenced arrowhead is absent");
      const shape = unique([...marker.children]);
      const points = arrowTriangle(shape);
      const orient = marker.getAttribute("orient");
      if (!points || !["auto", "auto-start-reverse"].includes(orient) || shape.hasAttribute("transform")) {
        return unavailable("unsupported SVG marker geometry or orientation");
      }
      if (!painted(shape, "fill")) return failed("arrowhead is unpainted");
      // Both point marker forms have a vertical base. A circle, cross or rotated triangle cannot qualify.
      const [tip, a, b] = points;
      if (a.x !== b.x || Math.abs(a.y - b.y) < epsilon) return unavailable("unsupported point marker");
      heading = (tip.x - a.x) * (atStart && orient === "auto" ? -1 : 1);
      const unit = marker.getAttribute("markerUnits") === "userSpaceOnUse" ? 1 : parseFloat(css.strokeWidth);
      if (!(marker.markerWidth.baseVal.value > 0 && marker.markerHeight.baseVal.value > 0)) return failed("arrowhead has an empty viewport");
      span = Math.hypot(marker.markerWidth.baseVal.value, marker.markerHeight.baseVal.value) * unit *
        Math.max(Math.hypot(matrix.a, matrix.b), Math.hypot(matrix.c, matrix.d));
      const bbox = shape.getBBox(), refX = marker.refX.baseVal.value, refY = marker.refY.baseVal.value;
      if (refX < bbox.x - epsilon || refX > bbox.x + bbox.width + epsilon || refY < bbox.y - epsilon ||
          refY > bbox.y + bbox.height + epsilon) return unavailable("marker reference is outside its triangle bounds");
    } else {
      const shape = markers[0], points = arrowTriangle(shape), ctm = shape.getScreenCTM();
      if (!points || !ctm) return unavailable("unsupported inline arrowhead geometry");
      if (!painted(shape, "fill")) return failed("arrowhead is unpainted");
      const [tip, a, b] = points.map((p) => p.matrixTransform(ctm));
      targetPoint = tip;
      const base = new DOMPoint((a.x + b.x) / 2, (a.y + b.y) / 2);
      const d0 = dist(base, start), d1 = dist(base, end);
      if (Math.abs(d0 - d1) <= epsilon) return unavailable("arrowhead location is ambiguous");
      atStart = d0 < d1;
      span = Math.max(dist(a, b), dist(tip, a), dist(tip, b));
      if (span <= epsilon || Math.min(d0, d1) > span) return failed("arrowhead is detached from its route");
      const step = Math.min(1, length / 2);
      const inner = point(atStart ? step : length - step), endpoint = atStart ? start : end;
      const forward = { x: endpoint.x - inner.x, y: endpoint.y - inner.y };
      const dx = tip.x - base.x, dy = tip.y - base.y;
      heading = dx * forward.x + dy * forward.y - Math.abs(dx * forward.y - dy * forward.x);
      const area = (tip.x - a.x) * (b.y - a.y) - (tip.y - a.y) * (b.x - a.x);
      if (Math.abs(area) <= epsilon) return failed("arrowhead has zero area");
    }
    if (!(heading > 0 && span > 0)) return failed("arrowhead does not point out of its attached route end");
    for (const [p, id] of [[atStart ? end : start, fact.from], [targetPoint, fact.to]]) {
      const distances = nodes.map((n) => ({ id: n.id, d: outlineDistance(p, n.outline.points) })).sort((a, b) => a.d - b.d);
      if (distances.length > 1 && distances[1].d - distances[0].d <= epsilon) return unavailable("route endpoint is ambiguous between nodes");
      if (distances[0].id !== id) return failed(`route endpoint is nearest ${distances[0].id}, expected ${id}`);
      if (distances[0].d > span + epsilon) return { ...failed(`route endpoint is detached from ${id}`),
        distance_px: distances[0].d, arrowhead_span_px: span };
    }
    return passed();
  }

  const results = (facts.arrows ?? []).map((f) => ({ kind: "arrow", ...f, ...arrow(f) }));
  for (const [group, members] of Object.entries(facts.groups ?? {})) {
    for (const node of members) results.push({ kind: "membership", group, node, ...membership(node, group) });
  }
  for (const [node, label] of Object.entries(facts.label_widths ?? {})) {
    results.push({ kind: "label-width", node, label, ...labelWidth(node, label) });
  }
  if (facts.sequence_messages) {
    const counts = sequenceMessageCounts();
    for (const [label, expected] of Object.entries(facts.sequence_messages)) {
      const fact = { kind: "sequence-message-count", label, expected };
      if (!counts) {
        results.push({ ...fact, ...unavailable("could not establish Sequence message identity or paint") });
        continue;
      }
      const observed = counts.get(normalizedText(label)) ?? 0;
      results.push({ ...fact, observed, ...(observed === expected ? passed() : failed("painted message count differs")) });
    }
  }
  return results;
}

export async function inspectDrawings(jobs, chrome) {
  const { default: puppeteer } = await import(pathToFileURL(browserTools.resolve("puppeteer")).href);
  const browser = await puppeteer.launch({ headless: true, executablePath: chrome });
  try {
    const page = await browser.newPage();
    await page.setOfflineMode(true);
    const viewport = { width: 1280, height: 800, deviceScaleFactor: 1 };
    await page.setViewport(viewport);
    await page.setContent(`<iframe sandbox="allow-same-origin" width="1200" height="700" srcdoc="<!doctype html><meta charset='utf-8'><meta http-equiv='Content-Security-Policy' content=&quot;default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:&quot;><body></body>"></iframe>`);
    const frame = await (await page.$("iframe")).contentFrame();
    const results = [];
    for (const { svg, facts } of jobs) {
      await frame.evaluate(async (source) => {
        const doc = new DOMParser().parseFromString(source, "image/svg+xml");
        if (doc.documentElement.localName !== "svg" || doc.documentElement.namespaceURI !== "http://www.w3.org/2000/svg") {
          document.body.replaceChildren();
          return;
        }
        document.body.replaceChildren(document.importNode(doc.documentElement, true));
        document.body.getBoundingClientRect();
        await document.fonts.ready;
      }, svg);
      results.push(await frame.evaluate(inspectSvg, facts));
    }
    return { browser: await browser.version(), observer_sha256: createHash("sha256").update(await readFile(new URL(import.meta.url))).digest("hex"),
      environment: { platform: process.platform, arch: process.arch, viewport, locale: await page.evaluate(() => navigator.language) }, results };
  } finally {
    await browser.close();
  }
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  const { values } = parseArgs({ options: { chrome: { type: "string" } } });
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  const { jobs } = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  console.log(JSON.stringify(await inspectDrawings(jobs, values.chrome)));
}
