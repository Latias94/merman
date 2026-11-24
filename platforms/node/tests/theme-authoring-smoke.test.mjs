import assert from "node:assert/strict";
import test from "node:test";
import { assertMathSvg } from "../scripts/theme-authoring-smoke.mjs";

test("math smoke requires outlined formula geometry inside its own SVG viewport", () => {
  const formula = '<svg xmlns="http://www.w3.org/2000/svg"><foreignObject>'
    + '<svg xmlns="http://www.w3.org/2000/svg" width="1em" height="1em" viewBox="0 0 1 1">'
    + '<path d="M0 0L1 1"/></svg></foreignObject></svg>';
  assert.equal(assertMathSvg(formula), 1);
  for (const invalid of [
    '<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0L1 1"/><text>$$x^2$$</text></svg>',
    formula.replace('<path d="M0 0L1 1"/>', '<text>x^2</text>'),
    formula.replace('viewBox="0 0 1 1"', 'viewBox="0 0 0 1"'),
    formula.replace('d="M0 0L1 1"', 'd=""'),
    formula.replace('</foreignObject>', () => '<span>$$x^2$$</span></foreignObject>'),
  ]) {
    assert.throws(() => assertMathSvg(invalid));
  }
});
