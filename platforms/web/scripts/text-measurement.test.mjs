import assert from "node:assert/strict";
import test from "node:test";

import * as webApi from "../dist/index.js";

const { createBrowserTextMeasurementSession } = webApi;

const OPERATION_CONTRACTS = new Map([
  ["measure", { kind: "metrics", wrapMode: "svg-like" }],
  ["computed-length", { kind: "length", wrapMode: "svg-like" }],
  ["bbox-x", { kind: "horizontal-extents", wrapMode: "svg-like" }],
  ["bbox-x-with-ascii-overhang", { kind: "horizontal-extents", wrapMode: "svg-like" }],
  ["title-bbox-x", { kind: "horizontal-extents", wrapMode: "svg-like" }],
  ["simple-bbox-width", { kind: "length", wrapMode: "svg-like" }],
  ["raw-bbox-width", { kind: "length", wrapMode: "svg-like" }],
  ["tspan-bbox-width", { kind: "length", wrapMode: "svg-like" }],
  ["tspan-bbox-height", { kind: "length", wrapMode: "svg-like" }],
  ["wrap-probe-bbox-width", { kind: "length", wrapMode: "svg-like" }],
  ["simple-bbox-height", { kind: "length", wrapMode: "svg-like" }],
  ["wrapped", { kind: "metrics", wrapMode: "svg-like" }],
  ["wrapped-with-raw-width", { kind: "wrapped-with-raw-width", wrapMode: "html-like" }],
  ["bounding-client-rect-width", { kind: "length", wrapMode: "svg-like" }],
  ["create-text-bbox-y-offset", { kind: "length", wrapMode: "svg-like", signed: true }],
  ["mermaid-calculate-text-dimensions", { kind: "metrics", wrapMode: "svg-like" }],
  ["canvas-measure-text-width", { kind: "length", wrapMode: "svg-like" }],
  [
    "create-text-middle-bbox-y-offset",
    { kind: "length", wrapMode: "svg-like", signed: true },
  ],
  ["raw-bbox-height", { kind: "length", wrapMode: "svg-like" }],
  ["html-line-height", { kind: "length", wrapMode: "html-like" }],
  ["raw-bbox-y-offset", { kind: "length", wrapMode: "svg-like", signed: true }],
  ["raw-middle-bbox-y-offset", { kind: "length", wrapMode: "svg-like", signed: true }],
  ["raw-hanging-bbox-y-offset", { kind: "length", wrapMode: "svg-like", signed: true }],
]);

class FakeStyle {
  get cssText() {
    return Object.entries(this)
      .map(([name, value]) => `${name}: ${value}`)
      .join("; ");
  }

  set cssText(value) {
    assert.equal(value, "");
    for (const name of Object.keys(this)) {
      delete this[name];
    }
  }
}

class FakeMeasureElement {
  constructor(tagName, canvasContext = null) {
    this.tagName = tagName;
    this.ownerDocument = globalThis.document;
    this.canvasContext = canvasContext;
  }

  style = new FakeStyle();
  attributes = new Map();
  children = [];
  _textContent = "";

  get isConnected() {
    return this.parentElement?.isConnected ?? false;
  }

  get textContent() {
    return this.children.length > 0
      ? this.children.map((child) => child.textContent || "").join("")
      : this._textContent;
  }

  set textContent(value) {
    this._textContent = value || "";
    this.children = [];
  }

  setAttribute(name, value) {
    this.attributes.set(name, String(value));
  }

  removeAttribute(name) {
    this.attributes.delete(name);
  }

  getAttributeNames() {
    return [...this.attributes.keys()];
  }

  inheritedAttribute(name) {
    return this.attributes.get(name) ?? this.parentElement?.inheritedAttribute?.(name);
  }

  appendChild(child) {
    child.parentElement = this;
    this.children.push(child);
    return child;
  }

  replaceChildren(...children) {
    for (const child of this.children) {
      if (child.parentElement === this) {
        child.parentElement = undefined;
      }
    }
    this.children = [];
    for (const child of children) {
      this.appendChild(child);
    }
  }

  remove() {
    this.removed = true;
    const siblings = this.parentElement?.children;
    if (Array.isArray(siblings)) {
      const index = siblings.indexOf(this);
      if (index >= 0) {
        siblings.splice(index, 1);
      }
    }
    this.parentElement = undefined;
  }

  getContext(kind) {
    assert.equal(this.tagName, "canvas");
    assert.equal(kind, "2d");
    return this.canvasContext;
  }

  fontSize() {
    const size = parseFloat(this.style.fontSize);
    return Number.isFinite(size) ? size : this.parentElement?.fontSize?.() ?? 16;
  }

  getBBox() {
    const fontSize = this.fontSize();
    const childLines = this.children
      .filter((child) => child.tagName === "tspan")
      .map((child) => child.textContent || "");
    const lines = childLines.length > 0 ? childLines : [this.textContent || ""];
    const width = Math.max(...lines.map((line) => line.length * fontSize * 0.6), 0);
    const middleBaseline = this.inheritedAttribute("dominant-baseline") === "middle";
    const hangingBaseline = this.inheritedAttribute("dominant-baseline") === "hanging";
    return {
      x: this.inheritedAttribute("text-anchor") === "middle" ? -width / 2 : 0,
      y: -fontSize + (middleBaseline ? fontSize * 0.25 : hangingBaseline ? fontSize * 0.8 : 0),
      width,
      height: Math.max(1, lines.length) * fontSize * 1.1,
    };
  }

  getComputedTextLength() {
    return (this.textContent || "").length * this.fontSize() * 0.4;
  }

  getBoundingClientRect() {
    const fontSize = this.fontSize();
    const lineHeight = parseFloat(this.style.lineHeight) || fontSize;
    const explicitLines = [[]];
    if (this.children.length > 0) {
      for (const child of this.children) {
        if (child.tagName === "br") {
          explicitLines.push([]);
        } else {
          explicitLines.at(-1).push(child.textContent || "");
        }
      }
    } else {
      const text = this.textContent || "";
      const preservesNewlines = ["pre", "pre-wrap", "break-spaces"].includes(
        this.style.whiteSpace
      );
      const lines = preservesNewlines ? text.split(/\r?\n/) : [text.replace(/\s+/gu, " ")];
      explicitLines.splice(0, 1, ...lines.map((line) => [line]));
    }
    const naturalWidths = explicitLines.map(
      (parts) => parts.join("").length * fontSize * 0.5
    );
    const naturalWidth = Math.max(...naturalWidths, 0);
    const fixedWidth =
      typeof this.style.width === "string" && this.style.width.endsWith("px")
        ? parseFloat(this.style.width)
        : null;
    const width = fixedWidth !== null && Number.isFinite(fixedWidth) ? fixedWidth : naturalWidth;
    const lineCount = naturalWidths.reduce(
      (count, lineWidth) =>
        count +
        (fixedWidth !== null && fixedWidth > 0
          ? Math.max(1, Math.ceil(lineWidth / fixedWidth))
          : 1),
      0
    );
    return { width, height: lineHeight * lineCount };
  }
}

class FakeCanvasContext {
  _font = "10px sans-serif";
  canonicalFonts = new Map();
  rejectedFonts = new Set();
  savedFonts = [];
  calls = [];
  saveCalls = 0;
  restoreCalls = 0;

  get font() {
    return this._font;
  }

  set font(value) {
    if (value === this.throwOnFont) {
      throw new Error("synthetic font assignment failure");
    }
    if (!this.rejectedFonts.has(value)) {
      this._font = this.canonicalFonts.get(value) ?? value;
    }
  }

  save() {
    this.saveCalls += 1;
    this.savedFonts.push(this.font);
  }

  restore() {
    this.restoreCalls += 1;
    assert.ok(this.savedFonts.length > 0);
    this._font = this.savedFonts.pop();
  }

  measureText(text) {
    this.calls.push({ font: this.font, text });
    if (this.throwOnMeasure) {
      throw new Error("synthetic measureText failure");
    }
    return { width: this.width ?? text.length * 7.25 };
  }
}

test("browser text measurement session routes exact operations to their DOM primitives", () => {
  const originalDocument = globalThis.document;
  let canvasCreates = 0;
  const canvasContext = new FakeCanvasContext();
  const canvasCalls = canvasContext.calls;
  const body = {
    tagName: "body",
    isConnected: true,
    children: [],
    appended: [],
    appendChild(child) {
      child.parentElement = this;
      this.children.push(child);
      this.appended.push(child);
      return child;
    },
  };
  globalThis.document = {
    body,
    createElement(tagName) {
      assert.ok(tagName === "div" || tagName === "canvas" || tagName === "br");
      if (tagName === "canvas") {
        canvasCreates += 1;
      }
      return new FakeMeasureElement(tagName, tagName === "canvas" ? canvasContext : null);
    },
    createTextNode(text) {
      const node = new FakeMeasureElement("#text");
      node.textContent = text;
      return node;
    },
    createElementNS(namespace, tagName) {
      assert.equal(namespace, "http://www.w3.org/2000/svg");
      return new FakeMeasureElement(tagName);
    },
  };

  try {
    const session = createBrowserTextMeasurementSession();
    const measure = session.measure;
    assert.equal(body.children.length, 0);

    const computed = measure(request("Computed", null, "computed-length", "svg-like"));
    assert.equal(computed.kind, "length");
    assert.equal(computed.length, "Computed".length * 16 * 0.4);
    assert.equal(body.children.length, 3);
    assert.deepEqual(
      body.children.map((probe) => probe.attributes.get("data-merman-text-measure-probe")),
      ["html", "svg", "mermaid-dimensions"]
    );

    const raw = measure(request("Raw", null, "raw-bbox-width", "svg-like"));
    assert.equal(raw.kind, "length");
    assert.equal(raw.length, "Raw".length * 16 * 0.6);

    assert.equal(
      measure(request("Raw", null, "raw-bbox-y-offset", "svg-like")).length,
      -16
    );
    assert.equal(
      measure(request("Raw", null, "raw-hanging-bbox-y-offset", "svg-like")).length,
      -16 + 16 * 0.8
    );
    assert.equal(
      measure(request("Raw", null, "raw-middle-bbox-y-offset", "svg-like")).length,
      -12
    );
    assert.equal(
      measure(request("Raw", null, "raw-bbox-y-offset", "svg-like")).length,
      -16
    );
    const directProbe = body.children[1].children[0];
    assert.equal(directProbe.tagName, "text");
    assert.equal(directProbe.children.length, 0);
    assert.equal(directProbe.attributes.get("y"), "0");

    const clientRect = measure(
      request("Client", null, "bounding-client-rect-width", "svg-like")
    );
    assert.equal(clientRect.kind, "length");
    assert.equal(clientRect.length, "Client".length * 16 * 0.5);

    const createTextYOffset = measure(
      request("Formatted", null, "create-text-bbox-y-offset", "svg-like")
    );
    assert.equal(createTextYOffset.kind, "length");
    assert.equal(createTextYOffset.length, -16);

    const middleYOffset = measure(
      request("Formatted", null, "create-text-middle-bbox-y-offset", "svg-like")
    );
    assert.equal(middleYOffset.kind, "length");
    assert.equal(middleYOffset.length, -12);

    const probeSvg = body.children[1];
    const wrappedProbe = probeSvg.children[2];
    const formattedProbeGroup = probeSvg.children[3];
    assert.equal(wrappedProbe.tagName, "text");
    assert.equal(formattedProbeGroup.tagName, "g");
    assert.equal(formattedProbeGroup.children[0].tagName, "text");
    assert.notEqual(wrappedProbe, formattedProbeGroup.children[0]);

    const interleavedWrapped = measure(
      request("one two three four five", 60, "wrapped", "svg-like")
    );
    assert.equal(interleavedWrapped.kind, "metrics");
    assert.ok(interleavedWrapped.line_count > 1);
    assert.equal(
      measure(request("Formatted", null, "create-text-bbox-y-offset", "svg-like")).length,
      -16
    );
    assert.equal(
      measure(request("Formatted", null, "create-text-middle-bbox-y-offset", "svg-like")).length,
      -12
    );

    const extents = measure(request("Centered", null, "bbox-x", "svg-like"));
    assert.equal(extents.kind, "horizontal-extents");
    assert.equal(extents.bbox_left, extents.bbox_right);
    assert.ok(extents.bbox_left > 0);

    const wrapped = measure(
      request("Condition ".repeat(40), 200, "wrapped-with-raw-width", "html-like")
    );
    assert.equal(wrapped.kind, "wrapped-with-raw-width");
    assert.equal(wrapped.width, 200);
    assert.ok(wrapped.line_count > 1);
    assert.ok(wrapped.raw_width > wrapped.width);

    const explicitHtmlLines = measure(
      request("supervisor.sh\nPID 1", null, "wrapped", "html-like")
    );
    assert.deepEqual(explicitHtmlLines, {
      kind: "metrics",
      width: "supervisor.sh".length * 16 * 0.5,
      height: 48,
      line_count: 2,
    });
    const explicitHtmlBreak = measure(
      request("line one<br/>line two", null, "wrapped-with-raw-width", "html-like")
    );
    assert.deepEqual(explicitHtmlBreak, {
      kind: "wrapped-with-raw-width",
      width: "line two".length * 16 * 0.5,
      height: 48,
      line_count: 2,
      raw_width: "line two".length * 16 * 0.5,
    });

    const svgWrapped = measure(request("one two three four five", 60, "wrapped", "svg-like"));
    assert.equal(svgWrapped.kind, "metrics");
    assert.ok(svgWrapped.line_count > 1);

    const graphemeWrapped = measure(request("👨‍👩‍👧‍👦", 1, "wrapped", "svg-like"));
    assert.equal(graphemeWrapped.kind, "metrics");
    assert.equal(graphemeWrapped.line_count, 1);
    assert.equal(wrappedProbe.children[0].textContent, "👨‍👩‍👧‍👦");

    const mermaidDimensions = measure({
      ...request("Body attached", null, "mermaid-calculate-text-dimensions", "svg-like"),
      font_family: '"Configured Serif;", serif',
      font_size: 18,
      font_weight: "700",
      font_style: "italic",
    });
    assert.deepEqual(mermaidDimensions, {
      kind: "metrics",
      width: "Body attached".length * 18 * 0.6,
      height: 18 * 1.1,
      line_count: 1,
    });
    const mermaidSvg = body.children[2];
    assert.equal(mermaidSvg.tagName, "svg");
    assert.notEqual(mermaidSvg.removed, true);
    assert.equal(body.children.length, 3);
    const mermaidText = mermaidSvg.children[0];
    const mermaidTspan = mermaidText.children[0];
    assert.equal(mermaidText.tagName, "text");
    assert.equal(mermaidText.style.fontFamily, '"Configured Serif;", serif');
    assert.equal(mermaidText.style.fontSize, "18px");
    assert.equal(mermaidText.style.fontWeight, "700");
    assert.equal(mermaidText.style.fontStyle, undefined);
    assert.equal(mermaidText.style.textAnchor, "start");
    assert.equal(mermaidTspan.tagName, "tspan");
    assert.equal(mermaidTspan.attributes.get("x"), "0");
    assert.equal(mermaidTspan.textContent, "Body attached");

    mermaidText.setAttribute("data-stale", "text");
    mermaidText.style.staleProperty = "stale";
    mermaidTspan.setAttribute("data-stale", "tspan");
    const reusedDimensions = measure({
      ...request("Reused", null, "mermaid-calculate-text-dimensions", "svg-like"),
      font_family: "New Family",
      font_size: 20,
      font_weight: "500",
    });
    assert.deepEqual(reusedDimensions, {
      kind: "metrics",
      width: "Reused".length * 20 * 0.6,
      height: 20 * 1.1,
      line_count: 1,
    });
    assert.equal(body.children[2], mermaidSvg);
    assert.equal(mermaidSvg.children[0], mermaidText);
    assert.equal(mermaidText.children[0], mermaidTspan);
    assert.equal(mermaidText.attributes.has("data-stale"), false);
    assert.equal(mermaidTspan.attributes.has("data-stale"), false);
    assert.equal("staleProperty" in mermaidText.style, false);
    assert.equal(mermaidText.style.fontFamily, "New Family");
    assert.equal(mermaidText.style.fontSize, "20px");
    assert.equal(mermaidText.style.fontWeight, "500");
    assert.equal(mermaidTspan.textContent, "Reused");

    assert.equal(canvasCreates, 0);
    const canvasWidth = measure({
      ...request("Canvas", null, "canvas-measure-text-width", "svg-like"),
      font_family: "Avenir, sans-serif",
      font_size: 18,
      font_weight: "700",
      font_style: "italic",
    });
    assert.deepEqual(canvasWidth, {
      kind: "length",
      length: "Canvas".length * 7.25,
    });
    assert.equal(canvasCreates, 1);
    assert.deepEqual(canvasCalls, [
      {
        font: "italic 700 18px Avenir, sans-serif",
        text: "Canvas",
      },
    ]);

    assert.equal(OPERATION_CONTRACTS.size, 23);
    for (const [operation, contract] of OPERATION_CONTRACTS) {
      const result = measure(request("Contract value", 80, operation, contract.wrapMode));
      assert.ok(result, `${operation} should be handled`);
      assert.equal(result.kind, contract.kind, `${operation} result kind`);
      switch (contract.kind) {
        case "metrics":
          assert.ok(Number.isFinite(result.width), `${operation} width`);
          assert.ok(Number.isFinite(result.height), `${operation} height`);
          assert.ok(result.line_count >= 1, `${operation} line count`);
          break;
        case "length":
          assert.ok(Number.isFinite(result.length), `${operation} length`);
          if (contract.signed) {
            assert.ok(result.length < 0, `${operation} preserves a signed CSS-pixel offset`);
          } else {
            assert.ok(result.length >= 0, `${operation} returns a non-negative CSS-pixel length`);
          }
          break;
        case "horizontal-extents":
          assert.ok(result.bbox_left >= 0, `${operation} left extent`);
          assert.ok(result.bbox_right >= 0, `${operation} right extent`);
          break;
        case "wrapped-with-raw-width":
          assert.ok(Number.isFinite(result.width), `${operation} width`);
          assert.ok(Number.isFinite(result.height), `${operation} height`);
          assert.ok(result.line_count >= 1, `${operation} line count`);
          assert.ok(Number.isFinite(result.raw_width), `${operation} raw width`);
          break;
      }
    }

    assert.deepEqual(
      measure(request("", null, "mermaid-calculate-text-dimensions", "svg-like")),
      { kind: "metrics", width: 0, height: 0, line_count: 1 }
    );
    assert.deepEqual(measure(request("", null, "canvas-measure-text-width", "svg-like")), {
      kind: "length",
      length: 0,
    });
    assert.deepEqual(
      measure(request("", null, "create-text-middle-bbox-y-offset", "svg-like")),
      { kind: "length", length: 0 }
    );

    const attachedProbes = [...body.children];
    session.dispose();
    session.dispose();
    assert.equal(body.children.length, 0);
    assert.ok(attachedProbes.every((probe) => probe.removed));
    assert.equal(measure(request("Disposed", null, "measure", "svg-like")), undefined);
    assert.equal(body.children.length, 0);
    assert.equal(canvasCreates, 1);
  } finally {
    if (originalDocument === undefined) {
      delete globalThis.document;
    } else {
      globalThis.document = originalDocument;
    }
  }
});

test("canvas accepts canonicalized weights and preserves quoted family punctuation", () => {
  const context = new FakeCanvasContext();
  const cases = [
    ["bolder", 16, "sans-serif", "bold 16px sans-serif"],
    ["lighter", 16, "sans-serif", "100 16px sans-serif"],
    ["normal", 18, '"A; B, C", sans-serif', '18px "A; B, C", sans-serif'],
    ["normal", 18, '"A\\"; B", serif', '18px "A\\"; B", serif'],
    ["normal", 10, "sans-serif", "10px sans-serif"],
    ["normal", 12, "serif", "12px serif"],
  ];
  for (const [weight, size, family, canonical] of cases) {
    context.canonicalFonts.set(`normal ${weight} ${size}px ${family}`, canonical);
  }
  context.font = "italic 700 31px serif";
  const originalFont = context.font;

  withMeasurementSession(context, (session) => {
    for (const [weight, size, family, canonical] of cases) {
      assert.deepEqual(session.measure({
        ...request("Vehicle", null, "canvas-measure-text-width", "svg-like"),
        font_weight: weight,
        font_size: size,
        font_family: family,
      }), { kind: "length", length: "Vehicle".length * 7.25 });
      assert.equal(context.calls.at(-1).font, canonical);
      assert.equal(context.font, originalFont);
      assert.equal(context.savedFonts.length, 0);
    }
    assert.equal(context.saveCalls, cases.length);
    assert.equal(context.restoreCalls, cases.length);
  });
});

test("canvas rejects malformed assignment without measuring default or previous fonts", () => {
  const context = new FakeCanvasContext();
  const invalidFamilies = ["sans-serif;", "sans-serif; font-size: 1px", ""];
  for (const family of invalidFamilies) {
    context.rejectedFonts.add(`normal normal 16px ${family}`);
  }

  withMeasurementSession(context, (session) => {
    for (const family of invalidFamilies) {
      const invalid = {
        ...request("Vehicle", null, "canvas-measure-text-width", "svg-like"),
        font_family: family,
      };
      for (const previousFont of ["10px sans-serif", "italic 700 31px serif"]) {
        context.font = previousFont;
        const callsBefore = context.calls.length;
        assert.equal(session.measure(invalid), undefined);
        assert.equal(context.calls.length, callsBefore, "rejected fonts must not be measured");
        assert.equal(context.font, previousFont);
        assert.equal(context.savedFonts.length, 0);
      }
      const valid = {
        ...invalid,
        font_size: 24,
        font_family: "monospace",
      };
      assert.ok(session.measure(valid));
      assert.equal(context.calls.at(-1).font, "normal normal 24px monospace");
      const callsBefore = context.calls.length;
      assert.equal(session.measure(invalid), undefined);
      assert.equal(context.calls.length, callsBefore);
      assert.equal(context.font, "italic 700 31px serif");
    }
    assert.equal(context.saveCalls, context.restoreCalls);
  });
});

test("canvas restores saved state on invalid metrics and throwing browser primitives", () => {
  const context = new FakeCanvasContext();
  context.font = "italic 700 31px serif";
  const originalFont = context.font;
  const canvasRequest = request("Vehicle", null, "canvas-measure-text-width", "svg-like");

  withMeasurementSession(context, (session) => {
    for (const width of [NaN, Infinity, -Infinity]) {
      context.width = width;
      assert.equal(session.measure(canvasRequest), undefined);
      assert.equal(context.font, originalFont);
      assert.equal(context.savedFonts.length, 0);
    }
    delete context.width;

    context.throwOnMeasure = true;
    assert.equal(session.measure(canvasRequest), undefined);
    assert.equal(context.font, originalFont);
    assert.equal(context.savedFonts.length, 0);
    context.throwOnMeasure = false;

    context.throwOnFont = "normal normal 16px Trebuchet MS, sans-serif";
    assert.equal(session.measure(canvasRequest), undefined);
    assert.equal(context.font, originalFont);
    assert.equal(context.savedFonts.length, 0);
    delete context.throwOnFont;

    assert.deepEqual(session.measure(canvasRequest), {
      kind: "length",
      length: "Vehicle".length * 7.25,
    });
    assert.equal(context.font, originalFont);
    assert.equal(context.saveCalls, 6);
    assert.equal(context.restoreCalls, 6);
  });
});

test("DOM probes reject ignored CSSOM assignments and accept canonicalized values", () => {
  withMeasurementSession(null, (session, body) => {
    const inputs = [
      request("Typography", null, "raw-bbox-width", "svg-like"),
      request("Typography", null, "wrapped", "html-like"),
    ];
    assert.ok(session.measure(inputs[0]));
    for (const style of [body.children[0].style, body.children[1].children[0].style]) {
      for (const property of ["fontFamily", "fontWeight", "fontStyle"]) {
        let current = style[property];
        Object.defineProperty(style, property, {
          get() { return current; },
          set(value) {
            if (value !== "invalid-css") current = value.trim();
          },
        });
      }
    }
    for (const input of inputs) {
      for (const field of ["font_family", "font_weight", "font_style"]) {
        assert.ok(session.measure(input));
        assert.equal(session.measure({ ...input, [field]: "invalid-css" }), undefined);
        assert.ok(session.measure({
          ...input,
          font_family: "  serif  ",
          font_weight: " 700 ",
          font_style: " italic ",
        }));
      }
    }
    for (const style of [body.children[0].style, body.children[1].children[0].style]) {
      assert.equal(style.fontFamily, "serif");
      assert.equal(style.fontWeight, "700");
      assert.equal(style.fontStyle, "italic");
    }
  });
});

test("canvas preserves zero and subpixel sizes and rejects invalid sizes without a default", () => {
  const context = new FakeCanvasContext();
  withMeasurementSession(context, session => {
    const input = request("MMMMMMMM", null, "canvas-measure-text-width", "svg-like");
    for (const size of [0, 0.5, 1]) {
      assert.ok(session.measure({ ...input, font_size: size }));
      assert.equal(context.calls.at(-1).font, `normal normal ${size}px Trebuchet MS, sans-serif`);
    }
    const before = context.calls.length;
    for (const size of [-1, NaN, Infinity]) {
      assert.equal(session.measure({ ...input, font_size: size }), undefined);
    }
    assert.equal(context.calls.length, before, "invalid sizes must not measure a substituted font");
  });
});

test("DOM operations preserve zero and subpixel font sizes", () => {
  withMeasurementSession(null, (session, body) => {
    for (const size of [0, 0.5, 1, 20]) {
      for (const operation of ["measure", "computed-length", "raw-bbox-width", "tspan-bbox-width",
        "simple-bbox-width", "simple-bbox-height", "bounding-client-rect-width",
        "raw-bbox-height", "raw-bbox-y-offset", "raw-middle-bbox-y-offset",
        "raw-hanging-bbox-y-offset", "create-text-bbox-y-offset", "create-text-middle-bbox-y-offset"]) {
        const result = session.measure({
          ...request("MMMMMMMM", null, operation, "svg-like"), font_size: size,
        });
        assert.ok(result, `${operation}: ${size}px is supported`);
        const svg = body.children[1];
        const index = operation.startsWith("create-text") ? 3
          : ["computed-length", "tspan-bbox-width", "simple-bbox-width", "simple-bbox-height"].includes(operation) ? 1 : 0;
        const text = index === 3 ? svg.children[index].children[0] : svg.children[index];
        assert.equal(text.style.fontSize, `${size}px`, `${operation}: preserve CSS pixels`);
        if (operation === "raw-bbox-width") assert.equal(result.length, 8 * size * 0.6);
      }
    }
  });
});

test("simple and dimensions probes replace only pinned HTML breaks", () => {
  withMeasurementSession(null, (session, body) => {
    for (const [text, expected] of [
      ["A<br>B<BR />C</br>D", "A B C D"],
      ["A\nB", "A\nB"],
      ["A&lt;br&gt;B", "A&lt;br&gt;B"],
    ]) {
      for (const operation of ["simple-bbox-width", "simple-bbox-height", "mermaid-calculate-text-dimensions"]) {
        const input = request(text, null, operation, "svg-like");
        assert.deepEqual(session.measure(input), session.measure({ ...input, text: expected }));
        session.measure(input);
        const svg = body.children[operation === "mermaid-calculate-text-dimensions" ? 2 : 1];
        const probe = svg.children[operation === "mermaid-calculate-text-dimensions" ? 0 : 1];
        assert.equal(probe.children[0].textContent, expected, operation);
      }
      for (const operation of ["raw-bbox-width", "tspan-bbox-width", "wrap-probe-bbox-width"]) {
        assert.ok(session.measure(request(text, null, operation, "svg-like")));
        const index = operation === "raw-bbox-width" ? 0 : 1;
        assert.equal(body.children[1].children[index].textContent, text, `${operation}: literal probe`);
      }
    }
  });
});

test("wrapped probes preserve subpixel line height and leave zero HTML line height unhandled", () => {
  withMeasurementSession(null, (session, body) => {
    for (const operation of ["wrapped", "wrapped-with-raw-width"]) {
      for (const wrapMode of ["svg-like", "html-like"]) {
        const input = { ...request("A\nB", null, operation, wrapMode), font_size: 0.5, line_height: 0.25 };
        const result = session.measure(input);
        assert.equal(result.line_count, 2);
        if (wrapMode === "html-like") {
          assert.equal(body.children[0].style.lineHeight, "0.25px");
          assert.equal(result.height, 0.5);
          assert.equal(session.measure({ ...input, line_height: 0 }), undefined);
        } else {
          assert.equal(body.children[1].children[2].children[1].attributes.get("dy"), "0.25px");
          assert.ok(session.measure({ ...input, line_height: 0 }));
          assert.equal(body.children[1].children[2].children[1].attributes.get("dy"), "0px");
        }
        for (const lineHeight of [-1, NaN, Infinity]) {
          assert.equal(session.measure({ ...input, line_height: lineHeight }), undefined);
        }
        assert.deepEqual(session.measure(input), result, "recover after an unsupported line height");
      }
    }
  });
});

function withMeasurementSession(context, run) {
  const originalDocument = globalThis.document;
  const body = {
    isConnected: true,
    children: [],
    appendChild(child) {
      child.parentElement = this;
      this.children.push(child);
      return child;
    },
  };
  globalThis.document = {
    body,
    createElement(tagName) {
      return new FakeMeasureElement(tagName, tagName === "canvas" ? context : null);
    },
    createTextNode(text) {
      const node = new FakeMeasureElement("#text");
      node.textContent = text;
      return node;
    },
    createElementNS(namespace, tagName) {
      assert.equal(namespace, "http://www.w3.org/2000/svg");
      return new FakeMeasureElement(tagName);
    },
  };
  const session = createBrowserTextMeasurementSession();
  try {
    run(session, body);
  } finally {
    session.dispose();
    if (originalDocument === undefined) {
      delete globalThis.document;
    } else {
      globalThis.document = originalDocument;
    }
  }
}

test("browser session replaces detached probes and falls back while the body is disconnected", () => {
  withMeasurementSession(null, (session, body) => {
    const input = request("Reattached", null, "measure", "svg-like");
    const expected = session.measure(input);
    assert.ok(expected.width > 0);

    for (const index of [0, 1, 2]) {
      const retired = [...body.children];
      retired[index].remove();
      assert.deepEqual(session.measure(input), expected);
      assert.equal(body.children.length, 3);
      assert.ok(retired.every((probe) => !probe.isConnected));
      assert.ok(body.children.every((probe) => !retired.includes(probe)));
    }

    body.isConnected = false;
    assert.equal(session.measure(input), undefined);
    body.isConnected = true;
    assert.deepEqual(session.measure(input), expected);
    assert.equal(body.children.length, 3);
  });
});

test("browser text measurement session can be disposed before first use", () => {
  const originalDocument = globalThis.document;
  let elementCreates = 0;
  globalThis.document = {
    body: { appendChild() {} },
    createElement() {
      elementCreates += 1;
      return new FakeMeasureElement("div");
    },
    createElementNS() {
      elementCreates += 1;
      return new FakeMeasureElement("svg");
    },
  };

  try {
    const session = createBrowserTextMeasurementSession();
    session.dispose();
    session.dispose();
    assert.equal(session.measure(request("Disposed", null, "measure", "svg-like")), undefined);
    assert.equal(elementCreates, 0);
  } finally {
    if (originalDocument === undefined) {
      delete globalThis.document;
    } else {
      globalThis.document = originalDocument;
    }
  }
});

test("browser text measurement session rolls back partially constructed probes", () => {
  const originalDocument = globalThis.document;
  let svgCreates = 0;
  let failConstruction = true;
  const body = {
    isConnected: true,
    children: [],
    appendChild(child) {
      child.parentElement = this;
      this.children.push(child);
      return child;
    },
  };
  globalThis.document = {
    body,
    createElement(tagName) {
      return new FakeMeasureElement(tagName);
    },
    createElementNS(namespace, tagName) {
      assert.equal(namespace, "http://www.w3.org/2000/svg");
      svgCreates += 1;
      if (failConstruction && svgCreates === 2) {
        throw new Error("synthetic probe construction failure");
      }
      return new FakeMeasureElement(tagName);
    },
  };

  try {
    const session = createBrowserTextMeasurementSession();
    assert.equal(session.measure(request("First", null, "measure", "svg-like")), undefined);
    assert.equal(body.children.length, 0);

    failConstruction = false;
    const recovered = session.measure(request("Second", null, "measure", "svg-like"));
    assert.equal(recovered.kind, "metrics");
    assert.equal(body.children.length, 3);
    session.dispose();
    assert.equal(body.children.length, 0);
  } finally {
    if (originalDocument === undefined) {
      delete globalThis.document;
    } else {
      globalThis.document = originalDocument;
    }
  }
});

test("browser text measurement session falls back without a DOM", () => {
  const originalDocument = globalThis.document;
  try {
    delete globalThis.document;
    const session = createBrowserTextMeasurementSession();
    assert.equal(session.measure(request("Node", null, "measure", "svg-like")), undefined);
    session.dispose();
    assert.equal(session.measure(request("Node", null, "measure", "svg-like")), undefined);
  } finally {
    if (originalDocument !== undefined) {
      globalThis.document = originalDocument;
    }
  }
});

test("browser text measurement session preserves fallback on browser primitive failure", () => {
  const originalDocument = globalThis.document;
  const body = {
    isConnected: true,
    children: [],
    appendChild(child) {
      child.parentElement = this;
      this.children.push(child);
      return child;
    },
  };
  globalThis.document = {
    body,
    createElement(tagName) {
      return new FakeMeasureElement(tagName);
    },
    createElementNS(namespace, tagName) {
      assert.equal(namespace, "http://www.w3.org/2000/svg");
      const element = new FakeMeasureElement(tagName);
      if (tagName === "text") {
        element.getBBox = () => {
          throw new Error("synthetic getBBox failure");
        };
      }
      return element;
    },
  };

  try {
    const session = createBrowserTextMeasurementSession();
    assert.equal(session.measure(request("Node", null, "measure", "svg-like")), undefined);
    assert.equal(body.children.length, 3);
    session.dispose();
    assert.equal(body.children.length, 0);
  } finally {
    if (originalDocument === undefined) {
      delete globalThis.document;
    } else {
      globalThis.document = originalDocument;
    }
  }
});

test("the legacy browser text measurer factory is not exported", () => {
  assert.equal("createBrowserTextMeasurer" in webApi, false);
});

function request(text, maxWidth, operation, wrapMode) {
  return {
    operation,
    phase:
      operation.startsWith("wrapped")
        ? "wrap"
        : operation === "canvas-measure-text-width"
          ? "layout"
          : "svg-bbox",
    text,
    font_family: "Trebuchet MS, sans-serif",
    font_size: 16,
    font_weight: "normal",
    font_style: "normal",
    max_width: maxWidth,
    has_max_width: maxWidth !== null,
    line_height: 24,
    letter_spacing: 0,
    word_spacing: 0,
    wrap_mode: wrapMode,
    direction: "ltr",
    white_space: wrapMode === "html-like" ? "break-spaces" : "normal",
  };
}
