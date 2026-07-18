import assert from "node:assert/strict";
import test from "node:test";

import { createBrowserTextMeasurer } from "../dist/index.js";

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
]);

class FakeMeasureElement {
  constructor(tagName, canvasContext = null) {
    this.tagName = tagName;
    this.canvasContext = canvasContext;
  }

  style = {};
  attributes = new Map();
  children = [];
  _textContent = "";

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

  inheritedAttribute(name) {
    return this.attributes.get(name) ?? this.parentElement?.inheritedAttribute?.(name);
  }

  appendChild(child) {
    child.parentElement = this;
    this.children.push(child);
    return child;
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
  }

  getContext(kind) {
    assert.equal(this.tagName, "canvas");
    assert.equal(kind, "2d");
    return this.canvasContext;
  }

  fontSize() {
    return parseFloat(this.style.fontSize) || parseFloat(this.parentElement?.style?.fontSize) || 16;
  }

  getBBox() {
    const fontSize = this.fontSize();
    const childLines = this.children
      .filter((child) => child.tagName === "tspan")
      .map((child) => child.textContent || "");
    const lines = childLines.length > 0 ? childLines : [this.textContent || ""];
    const width = Math.max(...lines.map((line) => line.length * fontSize * 0.6), 0);
    const middleBaseline = this.inheritedAttribute("dominant-baseline") === "middle";
    return {
      x: this.inheritedAttribute("text-anchor") === "middle" ? -width / 2 : 0,
      y: -fontSize + (middleBaseline ? fontSize * 0.25 : 0),
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
    const naturalWidth = (this.textContent || "").length * fontSize * 0.5;
    const fixedWidth =
      typeof this.style.width === "string" && this.style.width.endsWith("px")
        ? parseFloat(this.style.width)
        : null;
    const width = fixedWidth !== null && Number.isFinite(fixedWidth) ? fixedWidth : naturalWidth;
    const lineCount =
      fixedWidth !== null && fixedWidth > 0 ? Math.max(1, Math.ceil(naturalWidth / fixedWidth)) : 1;
    return { width, height: lineHeight * lineCount };
  }
}

test("browser text measurer routes exact operations to their DOM primitives", () => {
  const originalDocument = globalThis.document;
  let canvasCreates = 0;
  const canvasCalls = [];
  const canvasContext = {
    font: "",
    measureText(text) {
      canvasCalls.push({ font: this.font, text });
      return { width: text.length * 7.25 };
    },
  };
  const body = {
    tagName: "body",
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
      assert.ok(tagName === "div" || tagName === "canvas");
      if (tagName === "canvas") {
        canvasCreates += 1;
      }
      return new FakeMeasureElement(tagName, tagName === "canvas" ? canvasContext : null);
    },
    createElementNS(namespace, tagName) {
      assert.equal(namespace, "http://www.w3.org/2000/svg");
      return new FakeMeasureElement(tagName);
    },
  };

  try {
    const measure = createBrowserTextMeasurer();

    const computed = measure(request("Computed", null, "computed-length", "svg-like"));
    assert.equal(computed.kind, "length");
    assert.equal(computed.length, "Computed".length * 16 * 0.4);

    const raw = measure(request("Raw", null, "raw-bbox-width", "svg-like"));
    assert.equal(raw.kind, "length");
    assert.equal(raw.length, "Raw".length * 16 * 0.6);

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

    const svgWrapped = measure(request("one two three four five", 60, "wrapped", "svg-like"));
    assert.equal(svgWrapped.kind, "metrics");
    assert.ok(svgWrapped.line_count > 1);

    const mermaidDimensions = measure({
      ...request("Body attached", null, "mermaid-calculate-text-dimensions", "svg-like"),
      font_family: "Configured Serif;",
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
    const mermaidSvg = body.appended.at(-1);
    assert.equal(mermaidSvg.tagName, "svg");
    assert.equal(mermaidSvg.removed, true);
    assert.equal(body.children.length, 2);
    const mermaidText = mermaidSvg.children[0];
    assert.equal(mermaidText.tagName, "text");
    assert.equal(mermaidText.style.fontFamily, "Configured Serif;");
    assert.equal(mermaidText.style.fontSize, "18px");
    assert.equal(mermaidText.style.fontWeight, "700");
    assert.equal(mermaidText.style.fontStyle, undefined);
    assert.equal(mermaidText.style.textAnchor, "start");
    assert.equal(mermaidText.children[0].tagName, "tspan");
    assert.equal(mermaidText.children[0].attributes.get("x"), "0");
    assert.equal(mermaidText.children[0].textContent, "Body attached");

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

    assert.equal(OPERATION_CONTRACTS.size, 19);
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
  } finally {
    if (originalDocument === undefined) {
      delete globalThis.document;
    } else {
      globalThis.document = originalDocument;
    }
  }
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
