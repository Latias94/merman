import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as path from "node:path";
import { describe, it } from "node:test";

import { renderPreviewHtml } from "../preview-html.js";

describe("preview html", () => {
  it("uses local scripts with a nonce instead of command URIs or inline handlers", () => {
    const html = renderPreviewHtml({
      resources: previewResources(),
    });

    assert.match(html, /Content-Security-Policy/);
    assert.match(html, /script-src 'nonce-[A-Za-z0-9]+'/);
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

  it("keeps source editor focus when opening or revealing the preview", () => {
    const source = fs.readFileSync(path.join(process.cwd(), "src", "preview.ts"), "utf8");

    assert.match(
      source,
      /createWebviewPanel\(\s*"mermanPreview",\s*PREVIEW_TITLE,\s*\{[\s\S]*?preserveFocus:\s*true/,
    );
    assert.match(source, /this\.panel\.reveal\(this\.panel\.viewColumn,\s*true\)/);
    assert.match(
      source,
      /openResource[\s\S]*?showTextDocument\(document,\s*\{[\s\S]*?preserveFocus:\s*true/,
    );
    assert.doesNotMatch(source, /panel\.reveal\(vscode\.ViewColumn\.Beside,\s*false\)/);
  });

  it("retargets empty previews and guards lock before a source exists", () => {
    const source = fs.readFileSync(path.join(process.cwd(), "src", "preview.ts"), "utf8");

    assert.match(
      source,
      /const shouldRetargetSource = !this\.panel \|\| !this\.session\.isLocked \|\| !this\.session\.snapshot/,
    );
    assert.match(source, /rememberResource\(resource,\s*\{\s*preferOnce:\s*true\s*\}\)/);
    assert.match(source, /if \(locked && !this\.session\.snapshot\)/);
  });

  it("ships message-driven viewport media with persisted pan, vector zoom, and auto-fit", () => {
    const script = fs.readFileSync(path.join(process.cwd(), "media", "preview.js"), "utf8");
    const styles = fs.readFileSync(path.join(process.cwd(), "media", "preview.css"), "utf8");

    assert.match(script, /vscode\.getState/);
    assert.match(script, /vscode\.setState/);
    assert.match(script, /sourceIdentityKey/);
    assert.match(script, /key\.documentUri, key\.sourceId, key\.sourceHash/);
    assert.match(script, /state\.sourceIdentityKey !== nextSourceIdentityKey/);
    assert.match(script, /post\("ready"/);
    assert.match(script, /window\.addEventListener\("message"/);
    assert.match(script, /case "renderStarted"/);
    assert.match(script, /case "renderSucceeded"/);
    assert.match(script, /case "renderFailed"/);
    assert.match(script, /case "diagnosticsUpdated"/);
    assert.match(script, /replacePreviewContent\(message\.content, message\.snapshot\)/);
    assert.match(script, /case "renderFailed":[\s\S]*isDifferentSourceLocation\(message\.snapshot\)/);
    assert.match(script, /addEventListener\("wheel"/);
    assert.match(script, /setPointerCapture/);
    assert.match(script, /ResizeObserver/);
    assert.match(script, /fitToView/);
    assert.match(script, /setZoom\(state\.zoom \* factor/);
    assert.match(script, /--preview-zoom/);
    assert.match(script, /applyVectorZoom/);
    assert.match(script, /post\("setDiagramTheme"/);
    assert.match(script, /post\("setDisplayMode"/);
    assert.match(script, /post\("setLocked"/);
    assert.match(script, /post\("exportRendered"/);
    assert.match(script, /document\.addEventListener\("pointermove"/);
    assert.doesNotMatch(script, /dataset\.action\) {\n\s+case "theme":/);
    assert.match(styles, /touch-action:\s*none/);
    assert.match(styles, /cursor:\s*grab/);
    assert.match(styles, /\.stage/);
    assert.match(styles, /align-items:\s*center/);
    assert.match(styles, /\[data-preview-output-controls\]\[hidden\]/);
    assert.doesNotMatch(styles, /scale\(var\(--preview-zoom/);
    assert.match(styles, /\.canvas svg \{[^}]*pointer-events:\s*none/s);
  });
});

function previewResources() {
  return {
    cspSource: "vscode-resource:",
    stylesUri: "vscode-resource://preview.css",
    scriptUri: "vscode-resource://preview.js",
  };
}
