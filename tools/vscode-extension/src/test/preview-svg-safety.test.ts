import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { assertSafePreviewSvg } from "../preview-svg-safety.js";

describe("preview SVG safety", () => {
  it("accepts local inert SVG output", () => {
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        '<svg viewBox="0 0 100 50"><defs><marker id="arrow"></marker></defs><a href="#node"><text>ok</text></a></svg>',
      ),
    );
  });

  it("accepts inert Mermaid HTML labels inside foreignObject", () => {
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        '<svg viewBox="0 0 100 50"><foreignObject width="10" height="24" overflow="visible"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="nodeLabel"><p>A</p></span></div></foreignObject></svg>',
      ),
    );
  });

  it("accepts local fragment and raster data URL references", () => {
    assert.doesNotThrow(() =>
      assertSafePreviewSvg(
        '<svg><defs><linearGradient id="fill"></linearGradient></defs><rect fill="url(#fill)"/><a href="#node">x</a><image href="data:image/png;base64,iVBORw0KGgo="/></svg>',
      ),
    );
  });

  it("rejects non-SVG renderer output", () => {
    assert.throws(() => assertSafePreviewSvg("<html></html>"), /non-SVG/);
  });

  it("rejects active embedded SVG content", () => {
    assert.throws(() => assertSafePreviewSvg("<svg><script>alert(1)</script></svg>"), /active/);
    assert.throws(() => assertSafePreviewSvg("<svg><iframe></iframe></svg>"), /active/);
  });

  it("rejects event handlers and unsafe URL attributes", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><text onclick="alert(1)">x</text></svg>'), /event/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><foreignObject><div onclick="alert(1)">x</div></foreignObject></svg>'),
      /event/,
    );
    assert.throws(() => assertSafePreviewSvg('<svg><a href="javascript:alert(1)">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="java&#115;cript:alert(1)">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a xlink:href="JavaScript:alert(1)">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><image href="data:text/html,hello"/></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><image href="file:///etc/passwd"/></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="command:workbench.action.openSettings">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="vscode://file/path">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="foo:bar">x</a></svg>'), /unsafe URL/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><image href="data:image/svg+xml,%3Csvg%20onload%3Dalert(1)%3E"/></svg>'),
      /unsafe URL/,
    );
  });

  it("rejects external resource references", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><image href="https://example.com/a.png"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><use href="//example.com/sprite.svg#x"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><image href="images/a.png"/></svg>'), /external/);
  });

  it("rejects unsafe CSS references", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(javascript:alert(1))">x</text></svg>'), /CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(jav\\61script:alert(1))">x</text></svg>'), /CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(file:///tmp/a.svg)">x</text></svg>'), /CSS URL/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><text style="fill:url(data:image/svg+xml,%3Csvg%3E)">x</text></svg>'),
      /CSS URL/,
    );
    assert.throws(() => assertSafePreviewSvg('<svg><style>@import "https://example.com/a.css";</style></svg>'), /CSS resource/);
    assert.throws(() => assertSafePreviewSvg('<svg><style>text { fill: url(//example.com/a.svg#x); }</style></svg>'), /CSS resource/);
  });

  it("rejects CSS resource keywords hidden behind CSS escapes", () => {
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>@im\\70ort "https://example.com/a.css";</style></svg>'),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>text { fill: u\\72l(//example.com/a.svg#x); }</style></svg>'),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>text { fill: u\\000072 l(javascript:alert(1)); }</style></svg>'),
      /CSS URL/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>@im/*hidden*/port "https://example.com/a.css";</style></svg>'),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>text { fill: u/*hidden*/rl(//example.com/a.svg#x); }</style></svg>'),
      /CSS resource/,
    );
  });
});
