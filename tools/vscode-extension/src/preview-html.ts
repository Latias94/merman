export type {
  PreviewDiagramTheme,
  PreviewDiagnosticTarget,
  PreviewDiagnostics,
} from "./preview-model.js";

const PREVIEW_TITLE = "Merman Preview";

export interface PreviewHtmlResources {
  cspSource: string;
  stylesUri: string;
  scriptUri: string;
}

export interface RenderPreviewHtmlRequest {
  resources: PreviewHtmlResources;
}

export function renderPreviewHtml(request: RenderPreviewHtmlRequest): string {
  const nonce = createNonce();

  return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta
      http-equiv="Content-Security-Policy"
      content="default-src 'none'; script-src 'nonce-${nonce}'; style-src ${request.resources.cspSource} 'unsafe-inline'; img-src ${request.resources.cspSource} data:;"
    />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>${PREVIEW_TITLE}</title>
    <link rel="stylesheet" href="${escapeHtml(request.resources.stylesUri)}" />
  </head>
  <body>
    <main class="frame" data-theme="light" data-background="paper" data-display-mode="svg">
      <section class="diagnostics" data-preview-diagnostics hidden></section>
      <section class="viewport" aria-label="Mermaid preview canvas">
        <div class="preview-sourcebar" data-preview-sourcebar hidden>
          <select data-action="source" data-preview-source-list title="Preview source"></select>
        </div>
        ${renderToolbar()}
        <div class="stage"><div class="canvas" data-preview-canvas></div></div>
        <section class="preview-status" data-preview-status hidden></section>
        <section class="empty" data-preview-empty>
          <h2>No preview</h2>
          <p>Focus a .mmd, .mermaid, or Markdown document with a Mermaid fence, then run Merman: Open Preview.</p>
        </section>
      </section>
    </main>
    <script nonce="${nonce}" src="${escapeHtml(request.resources.scriptUri)}"></script>
  </body>
</html>`;
}

function renderToolbar(): string {
  return `<nav class="toolbar" aria-label="Preview controls">
    <span class="toolbar-group" data-preview-zoom-controls>
      <button type="button" data-action="zoom-out" title="Zoom out">-</button>
      <span class="zoom-readout" data-zoom-value>100%</span>
      <button type="button" data-action="zoom-in" title="Zoom in">+</button>
      <button type="button" data-action="fit" title="Fit to view">Fit</button>
      <button type="button" data-action="reset" title="Reset to actual size">1:1</button>
    </span>
    <span class="toolbar-group" data-preview-output-controls>
      <button type="button" data-action="copy-svg" title="Copy rendered SVG">Copy SVG</button>
      <button type="button" data-action="export-svg" title="Export SVG">Export SVG</button>
      <button type="button" data-action="export-png" title="Export PNG">Export PNG</button>
    </span>
    <span class="toolbar-group">
      <button type="button" data-action="lock" data-preview-lock title="Open a Mermaid preview before locking it to a source" disabled>Follow</button>
    </span>
    <details class="preview-menu" data-preview-menu>
      <summary title="Preview settings">...</summary>
      <label>
        <span>Mode</span>
        <select data-action="display-mode" title="Preview display mode">
          <option value="svg">SVG</option>
          <option value="ascii">ASCII</option>
          <option value="unicode">Unicode</option>
        </select>
      </label>
      <label>
        <span>Theme</span>
        <select data-action="diagram-theme" title="Mermaid theme">
          <option value="source">Source</option>
          <option value="default">Default</option>
          <option value="dark">Dark</option>
          <option value="forest">Forest</option>
          <option value="neutral">Neutral</option>
          <option value="base">Base</option>
        </select>
      </label>
      <label>
        <span>Background</span>
        <select data-action="background" title="Preview background">
          <option value="paper">Paper</option>
          <option value="transparent">Transparent</option>
          <option value="dark">Dark</option>
        </select>
      </label>
    </details>
  </nav>`;
}

function createNonce(): string {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  let out = "";
  for (let index = 0; index < 32; index += 1) {
    out += alphabet[Math.floor(Math.random() * alphabet.length)] ?? "A";
  }
  return out;
}

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}
