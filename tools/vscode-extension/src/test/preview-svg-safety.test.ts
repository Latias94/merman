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

  it("rejects non-SVG renderer output", () => {
    assert.throws(() => assertSafePreviewSvg("<html></html>"), /non-SVG/);
  });

  it("rejects active embedded SVG content", () => {
    assert.throws(() => assertSafePreviewSvg("<svg><script>alert(1)</script></svg>"), /active/);
    assert.throws(() => assertSafePreviewSvg("<svg><foreignObject></foreignObject></svg>"), /active/);
  });

  it("rejects event handlers and unsafe URL attributes", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><text onclick="alert(1)">x</text></svg>'), /event/);
    assert.throws(() => assertSafePreviewSvg('<svg><a href="javascript:alert(1)">x</a></svg>'), /unsafe URL/);
    assert.throws(() => assertSafePreviewSvg('<svg><image href="data:text/html,hello"/></svg>'), /unsafe URL/);
  });

  it("rejects external resource references", () => {
    assert.throws(() => assertSafePreviewSvg('<svg><image href="https://example.com/a.png"/></svg>'), /external/);
    assert.throws(() => assertSafePreviewSvg('<svg><use href="//example.com/sprite.svg#x"/></svg>'), /external/);
  });
});
