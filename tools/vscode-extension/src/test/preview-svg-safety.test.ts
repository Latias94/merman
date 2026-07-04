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
        '<svg><defs><linearGradient id="fill"></linearGradient><filter id="shadow"></filter><clipPath id="clip"></clipPath><mask id="mask"></mask><marker id="arrow"></marker></defs><rect fill="url(#fill)" filter="url(#shadow)" clip-path="url(#clip)" mask="url(#mask)" marker-end="url(#arrow)"/><a href="#node">x</a><image href="data:image/png;base64,iVBORw0KGgo="/></svg>',
      ),
    );
  });

  it("rejects non-SVG renderer output", () => {
    assert.throws(() => assertSafePreviewSvg("<html></html>"), /non-SVG/);
  });

  it("rejects active embedded SVG content", () => {
    assert.throws(() => assertSafePreviewSvg("<svg><script>alert(1)</script></svg>"), /active/);
    assert.throws(() => assertSafePreviewSvg("<svg><iframe></iframe></svg>"), /active/);
    assert.throws(() => assertSafePreviewSvg('<svg><animate attributeName="href" to="https://example.com/x"/></svg>'), /active/);
    assert.throws(() => assertSafePreviewSvg("<svg><set attributeName=\"fill\" to=\"url(https://example.com/x)\"/></svg>"), /active/);
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
      () => assertSafePreviewSvg('<svg><foreignObject><img srcset="https://example.com/a.png 1x"/></foreignObject></svg>'),
      /srcset/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><foreignObject><button formaction="https://example.com/post">x</button></foreignObject></svg>'),
      /external/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><a href="#node" ping="https://example.com/ping">x</a></svg>'),
      /external/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg xml:base="https://example.com/sprite.svg"><use href="#icon"/></svg>'),
      /base/,
    );
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

  it("rejects external resources in SVG URL-bearing attributes", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><rect fill="url(https://example.com/fill.svg#x)"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><rect stroke="url(file:///tmp/stroke.svg#x)"/></svg>'), /unsafe/);
    assert.throws(() => assertSafePreviewSvg('<svg><rect filter="url(data:image/svg+xml,%3Csvg%3E)"/></svg>'), /unsafe/);
    assert.throws(() => assertSafePreviewSvg('<svg><rect clip-path="url(//example.com/clip.svg#x)"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><rect mask="url(images/mask.svg#x)"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><path marker-end="url(javascript:alert(1))"/></svg>'), /unsafe/);
  });

  it("rejects unsafe CSS references", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(javascript:alert(1))">x</text></svg>'), /CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(jav\\61script:alert(1))">x</text></svg>'), /CSS URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><text style="fill:url(file:///tmp/a.svg)">x</text></svg>'), /CSS URL/);
    assert.throws(
      () => assertSafePreviewSvg('<svg><text style="fill:url(data:image/svg+xml,%3Csvg%3E)">x</text></svg>'),
      /CSS URL/,
    );
    assert.throws(
      () =>
        assertSafePreviewSvg(
          '<svg><foreignObject><div style="background-image:image-set(&quot;https://example.com/a.png&quot; 1x)">x</div></foreignObject></svg>',
        ),
      /CSS resource/,
    );
    assert.throws(
      () => assertSafePreviewSvg('<svg><style>text { background-image: -webkit-image-set("https://example.com/a.png" 1x); }</style></svg>'),
      /CSS resource/,
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
