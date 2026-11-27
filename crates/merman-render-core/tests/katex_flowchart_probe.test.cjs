const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

test('math probes use Mermaid dependencies and preserve standalone hosts', (t) => {
  const workspace = fs.mkdtempSync(path.join(os.tmpdir(), 'merman-katex-resolution-'));
  t.after(() => {
    assert.equal(path.dirname(workspace), path.resolve(os.tmpdir()));
    assert.match(path.basename(workspace), /^merman-katex-resolution-/);
    fs.rmSync(workspace, { recursive: true, force: true });
  });
  function put(relative, source) {
    const file = path.join(workspace, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, source);
  }
  function render() {
    return JSON.parse(execFileSync(process.execPath, [
      path.join(__dirname, '../assets/katex_flowchart_probe.cjs'), 'render',
    ], {
      cwd: workspace,
      input: JSON.stringify({ text: '$$x$$', config: {} }),
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'pipe'],
    })).html;
  }
  put('package.json', '{}');
  put('node_modules/katex/package.json', '{"main":"index.cjs"}');
  put('node_modules/katex/index.cjs', 'exports.renderToString = () => "host-math";');
  assert.match(render(), /host-math/);

  put('node_modules/mermaid/package.json', '{"main":"dist/mermaid.core.mjs"}');
  put('node_modules/mermaid/dist/mermaid.core.mjs', 'export default {};');
  assert.match(render(), /host-math/);

  put('node_modules/mermaid/node_modules/katex/package.json', '{"main":"index.cjs"}');
  put('node_modules/mermaid/node_modules/katex/index.cjs', 'exports.renderToString = () => "mermaid-math";');
  assert.match(render(), /mermaid-math/);
  assert.doesNotMatch(render(), /host-math/);

  put('node_modules/mermaid/package.json', '{"main":"missing-entry.mjs"}');
  assert.throws(render, /missing-entry\.mjs/);
});
