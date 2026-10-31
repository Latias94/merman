import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { test } from "node:test";
import { runInNewContext } from "node:vm";

const require = createRequire(import.meta.url);
const clientRoot = path.dirname(require.resolve("vscode-languageclient"));
const vscode = {
  workspace: { onDidCreateFiles: () => ({ dispose() {} }) },
  Uri: {
    parse(value) {
      const url = new URL(value);
      return {
        path: url.pathname,
        with: ({ path: pathname }) => ({ fsPath: decodeURIComponent(pathname) }),
      };
    },
  },
};

// Run the installed client's actual consumers; only the editor host is substituted.
function loadClientModule(relative) {
  const filename = path.join(clientRoot, relative);
  const moduleRequire = createRequire(filename);
  const exports = {};
  runInNewContext(readFileSync(filename, "utf8"), {
    exports,
    require: (name) => name === "vscode" ? vscode : moduleRequire(name),
  }, { filename });
  return exports;
}

const { matchGlobPattern } = loadClientModule("utils/globPattern.js");
const { DidCreateFilesFeature } = loadClientModule("fileOperations.js");

test("the installed language client matches document globs with the patched matcher", () => {
  const pattern = "**/*.{mmd,mermaid}";
  assert.equal(matchGlobPattern(pattern, { fsPath: "/workspace/docs/diagram.mmd" }), true);
  assert.equal(matchGlobPattern(pattern, { fsPath: "/workspace/docs/diagram.mermaid" }), true);
  assert.equal(matchGlobPattern(pattern, { fsPath: "/workspace/docs/readme.txt" }), false);
  assert.equal(matchGlobPattern("", { fsPath: "" }), false);
  assert.equal(matchGlobPattern("#comment", { fsPath: "#comment" }), false);
});

test("the installed language client retains relative glob bases", () => {
  const pattern = { baseUri: "file:///workspace/docs", pattern: "*.mmd" };
  assert.equal(matchGlobPattern(pattern, { fsPath: "/workspace/docs/diagram.mmd" }), true);
  assert.equal(matchGlobPattern(pattern, { fsPath: "/workspace/diagram.mmd" }), false);
  assert.equal(matchGlobPattern({ baseUri: "invalid", pattern: "*.mmd" }, {
    fsPath: "/workspace/docs/diagram.mmd",
  }), false);
});

function fileOperationFeature() {
  const notifications = [];
  const feature = new DidCreateFilesFeature({
    middleware: {},
    code2ProtocolConverter: { asDidCreateFilesParams: (event) => event },
    sendNotification: async (_type, params) => { notifications.push(params); },
  });
  return { feature, notifications };
}

test("the installed language client registers and applies file operation filters", async () => {
  const { feature, notifications } = fileOperationFeature();
  feature.register({
    id: "merman",
    registerOptions: { filters: [{
      scheme: "file",
      pattern: { glob: "**/*.{mmd,mermaid}", options: { ignoreCase: true } },
    }] },
  });
  try {
    const files = [
      { scheme: "file", fsPath: "/workspace/diagram.MMD" },
      { scheme: "file", fsPath: "/workspace/.hidden.mermaid" },
      { scheme: "file", fsPath: "/workspace/readme.txt" },
      { scheme: "untitled", fsPath: "/workspace/diagram.mmd" },
    ];
    await feature.send({ files });
    assert.equal(notifications.length, 1);
    assert.deepEqual(notifications[0].files, files.slice(0, 2));
  } finally {
    feature.clear();
  }
});

test("the installed language client rejects empty and comment file operation filters", () => {
  for (const glob of ["", "#comment"]) {
    const { feature } = fileOperationFeature();
    try {
      assert.throws(() => feature.register({
        id: "invalid",
        registerOptions: { filters: [{ pattern: { glob } }] },
      }), /Invalid pattern/);
    } finally {
      feature.clear();
    }
  }
});
