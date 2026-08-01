import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { renderPreviewHtml } from "../preview-html.js";

describe("preview html", () => {
  it("uses local scripts with a nonce instead of command URIs or inline handlers", () => {
    const html = renderPreviewHtml({
      resources: previewResources(),
    });

    assert.match(html, /Content-Security-Policy/);
    assert.match(html, /script-src 'nonce-[A-Za-z0-9_-]+'/);
    assert.match(html, /style-src vscode-resource: 'unsafe-inline'/);
    assert.match(html, /src="vscode-resource:\/\/preview\.js"/);
    assert.doesNotMatch(html, /command:merman/);
    assert.doesNotMatch(html, /onclick=/);
  });

  it("renders a stable source picker placeholder", () => {
    const html = renderPreviewHtml({
      resources: previewResources(),
    });

    assert.match(html, /data-action="source"/);
    assert.match(html, /data-preview-source-list/);
    assert.doesNotMatch(html, /value="fence-2" selected/);
  });

  it("renders a stable canvas shell for message-driven updates", () => {
    const html = renderPreviewHtml({
      resources: previewResources(),
    });

    assert.match(html, /<section class="viewport"/);
    assert.match(html, /data-preview-canvas/);
    assert.match(html, /data-preview-status/);
    assert.match(html, /data-preview-empty/);
    assert.match(html, /data-action="fit"/);
    assert.match(html, /data-action="reset"/);
    assert.match(html, /data-zoom-value/);
    assert.match(html, /data-background="paper"/);
    assert.match(html, /data-preview-output-controls/);
    assert.match(html, /data-action="refresh"/);
    assert.match(html, /data-action="show-source"/);
    assert.match(html, /data-action="export-svg"/);
    assert.match(html, /data-action="export-png"/);
    assert.match(html, /data-action="lock"/);
    assert.match(html, /data-preview-lock/);
    assert.match(html, /data-preview-lock[^>]*disabled/);
    assert.match(html, /data-action="diagram-theme"/);
    assert.match(html, /value="forest"/);
    assert.doesNotMatch(html, /<svg viewBox/);
  });

  it("does not bake diagnostics into the stable shell", () => {
    const html = renderPreviewHtml({
      resources: previewResources(),
    });

    assert.match(html, /data-preview-diagnostics/);
    assert.doesNotMatch(html, /data-action="diagnostic"/);
    assert.doesNotMatch(html, /Mermaid syntax issue/);
  });

});

function previewResources() {
  return {
    cspSource: "vscode-resource:",
    stylesUri: "vscode-resource://preview.css",
    scriptUri: "vscode-resource://preview.js",
  };
}
