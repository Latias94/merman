import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  isPreviewDiagramTheme,
  isPreviewFromWebviewMessage,
} from "../preview-messages.js";

describe("preview message validation", () => {
  it("accepts only known diagram themes", () => {
    assert.equal(isPreviewDiagramTheme("forest"), true);
    assert.equal(isPreviewDiagramTheme("source"), true);
    assert.equal(isPreviewDiagramTheme("solarized"), false);
    assert.equal(isPreviewDiagramTheme(undefined), false);
  });

  it("accepts valid webview command payloads", () => {
    assert.equal(isPreviewFromWebviewMessage({ type: "ready" }), true);
    assert.equal(isPreviewFromWebviewMessage({ type: "copySvg", svg: "<svg></svg>" }), true);
    assert.equal(isPreviewFromWebviewMessage({ type: "exportRendered", format: "svg" }), true);
    assert.equal(isPreviewFromWebviewMessage({ type: "exportRendered", format: "png" }), true);
    assert.equal(isPreviewFromWebviewMessage({ type: "selectSource", sourceId: "fence-2" }), true);
    assert.equal(isPreviewFromWebviewMessage({ type: "setDiagramTheme", theme: "dark" }), true);
    assert.equal(isPreviewFromWebviewMessage({ type: "setDisplayMode", mode: "ascii" }), true);
    assert.equal(isPreviewFromWebviewMessage({ type: "setDisplayMode", mode: "unicode" }), true);
    assert.equal(isPreviewFromWebviewMessage({ type: "setBackground", background: "paper" }), true);
  });

  it("rejects malformed or unknown webview command payloads", () => {
    assert.equal(isPreviewFromWebviewMessage(null), false);
    assert.equal(isPreviewFromWebviewMessage({ type: "copySvg", svg: 1 }), false);
    assert.equal(isPreviewFromWebviewMessage({ type: "exportRendered", format: "pdf" }), false);
    assert.equal(isPreviewFromWebviewMessage({ type: "selectSource" }), false);
    assert.equal(isPreviewFromWebviewMessage({ type: "setDiagramTheme", theme: "solarized" }), false);
    assert.equal(isPreviewFromWebviewMessage({ type: "setDisplayMode", mode: "png" }), false);
    assert.equal(isPreviewFromWebviewMessage({ type: "setBackground", background: "blue" }), false);
    assert.equal(isPreviewFromWebviewMessage({ type: "togglePin" }), false);
    assert.equal(isPreviewFromWebviewMessage({ type: "deleteEverything" }), false);
  });
});
