import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const { assertSafeSvgForDom } = await import(
  pathToFileURL(path.join(packageRoot, "dist", "svg-safety.js")).href
);

assert.doesNotThrow(() =>
  assertSafeSvgForDom(
    '<svg><defs><linearGradient id="fill"></linearGradient><filter id="shadow"></filter></defs><rect fill="url(#fill)" filter="url(#shadow)"/></svg>',
  ),
);

assert.throws(
  () => assertSafeSvgForDom('<svg><image href="https://example.com/a.png"/></svg>'),
  /external/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><rect fill="url(https://example.com/fill.svg#x)"/></svg>'),
  /external/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><text style="fill:url(javascript:alert(1))">x</text></svg>'),
  /CSS URL/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><foreignObject><div onclick="alert(1)">x</div></foreignObject></svg>'),
  /event/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><foreignObject><img srcset="https://example.com/a.png 1x"/></foreignObject></svg>'),
  /srcset/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><a href="#node" ping="https://example.com/ping">x</a></svg>'),
  /external/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg xml:base="https://example.com/sprite.svg"><use href="#icon"/></svg>'),
  /base/,
);
assert.throws(
  () =>
    assertSafeSvgForDom(
      '<svg><foreignObject><div style="background-image:image-set(&quot;https://example.com/a.png&quot; 1x)">x</div></foreignObject></svg>',
    ),
  /CSS resource/,
);
assert.throws(
  () => assertSafeSvgForDom('<svg><animate attributeName="href" to="https://example.com/x"/></svg>'),
  /active/,
);

console.log("@mermanjs/web DOM safety smoke passed");
