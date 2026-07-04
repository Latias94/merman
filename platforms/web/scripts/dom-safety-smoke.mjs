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

console.log("@mermanjs/web DOM safety smoke passed");
