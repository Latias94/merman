import assert from "node:assert/strict";
import test, { mock } from "node:test";
import type { editor } from "monaco-editor";
import { bindCodeEditor } from "./code-editor-binding.ts";

test("a local edit reads once and reaches the store synchronously without an edit echo", (t) => {
  const initial = "flowchart TD";
  const monaco = fakeEditor(initial);
  const store = fakeStore(initial);
  const onLocalChange = mock.fn();
  const binding = bindCodeEditor(monaco.api, store.api, onLocalChange);
  t.after(() => binding.dispose());
  const reads = monaco.reads;
  const next = "flowchart TD\nA --> B";

  monaco.type(next);

  assert.equal(monaco.reads, reads + 1);
  assert.equal(store.value, next);
  assert.deepEqual(store.writes, [next]);
  assert.equal(onLocalChange.mock.callCount(), 1);
  assert.deepEqual(monaco.replacements, []);
  assert.deepEqual(monaco.operations, []);
});

test("external replacement and clearing preserve undo boundaries without writing back", (t) => {
  const initial = "flowchart TD\nA --> B";
  const monaco = fakeEditor(initial);
  const store = fakeStore(initial);
  const onLocalChange = mock.fn();
  const binding = bindCodeEditor(monaco.api, store.api, onLocalChange);
  t.after(() => binding.dispose());
  const next = "sequenceDiagram\nAlice->>Bob: Hello";

  store.replace(next);
  assert.equal(monaco.value, next);
  store.replace(next);
  store.replace("");

  assert.equal(monaco.value, "");
  assert.equal(store.value, "");
  assert.deepEqual(monaco.replacements, [next, ""]);
  assert.deepEqual(monaco.operations, ["edit", "undo-stop", "edit", "undo-stop"]);
  assert.deepEqual(store.writes, []);
  assert.equal(onLocalChange.mock.callCount(), 0);
});

test("mounting reconciles a store update made after the model was created", (t) => {
  const initial = "flowchart TD";
  const monaco = fakeEditor(initial);
  const store = fakeStore(initial);
  const latest = "flowchart LR\nA --> B";
  store.replace(latest);
  const onLocalChange = mock.fn();

  const binding = bindCodeEditor(monaco.api, store.api, onLocalChange);
  t.after(() => binding.dispose());

  assert.equal(monaco.value, latest);
  assert.equal(store.value, latest);
  assert.deepEqual(monaco.replacements, [latest]);
  assert.deepEqual(monaco.operations, ["edit", "undo-stop"]);
  assert.deepEqual(store.writes, []);
  assert.equal(onLocalChange.mock.callCount(), 0);
});

test("CRLF normalization ignores repeated store notifications and keeps local edits live", (t) => {
  const initial = "flowchart TD";
  const monaco = fakeEditor(initial);
  const store = fakeStore(initial);
  const onLocalChange = mock.fn();
  const binding = bindCodeEditor(monaco.api, store.api, onLocalChange);
  t.after(() => binding.dispose());
  const external = "flowchart TD\r\nA --> B\r\n";

  store.replace(external);
  store.replace(external);
  store.replace(external);

  assert.equal(monaco.value, "flowchart TD\nA --> B\n");
  assert.equal(store.value, external);
  assert.deepEqual(monaco.replacements, [external]);
  assert.deepEqual(monaco.operations, ["edit", "undo-stop"]);
  assert.deepEqual(store.writes, []);
  assert.equal(onLocalChange.mock.callCount(), 0);

  const reads = monaco.reads;
  const local = "flowchart TD\nA --> B\nB --> C\n";
  monaco.type(local);

  assert.equal(monaco.value, local);
  assert.equal(monaco.reads, reads + 1);
  assert.equal(store.value, local);
  assert.deepEqual(store.writes, [local]);
  assert.equal(onLocalChange.mock.callCount(), 1);
  assert.deepEqual(monaco.replacements, [external]);
  assert.deepEqual(monaco.operations, ["edit", "undo-stop"]);
});

test("raw EOL changes matching the model do not block later external updates", (t) => {
  const lf = "flowchart TD\nA --> B\n";
  const crlf = "flowchart TD\r\nA --> B\r\n";
  const latest = "flowchart LR\r\nB --> C\r\n";
  const monaco = fakeEditor(lf);
  const store = fakeStore(lf);
  const onLocalChange = mock.fn();
  const binding = bindCodeEditor(monaco.api, store.api, onLocalChange);
  t.after(() => binding.dispose());

  store.replace(crlf);
  store.replace(lf);
  store.replace(lf);

  assert.equal(monaco.value, lf);
  assert.equal(store.value, lf);
  assert.deepEqual(monaco.replacements, [crlf]);
  assert.deepEqual(monaco.operations, ["edit", "undo-stop"]);

  store.replace(latest);
  store.replace(latest);

  assert.equal(monaco.value, "flowchart LR\nB --> C\n");
  assert.equal(store.value, latest);
  assert.deepEqual(monaco.replacements, [crlf, latest]);
  assert.deepEqual(monaco.operations, ["edit", "undo-stop", "edit", "undo-stop"]);
  assert.deepEqual(store.writes, []);
  assert.equal(onLocalChange.mock.callCount(), 0);
});

test("store updates reentered during replacement converge on the latest document", (t) => {
  const initial = "flowchart TD";
  const first = "flowchart TD\nA --> B";
  const superseded = "flowchart TD\nA --> C";
  const latest = "flowchart TD\nA --> D";
  const store = fakeStore(initial);
  const monaco = fakeEditor(initial, (text) => {
    if (text === first) {
      store.replace(superseded);
      store.replace(latest);
    }
  });
  const onLocalChange = mock.fn();
  const binding = bindCodeEditor(monaco.api, store.api, onLocalChange);
  t.after(() => binding.dispose());

  store.replace(first);

  assert.equal(monaco.value, latest);
  assert.equal(store.value, latest);
  assert.deepEqual(monaco.replacements, [first, latest]);
  assert.deepEqual(monaco.operations, ["edit", "undo-stop", "edit", "undo-stop"]);
  assert.deepEqual(store.writes, []);
  assert.equal(onLocalChange.mock.callCount(), 0);
});

test("dispose is idempotent and disconnects both directions", (t) => {
  const initial = "flowchart TD";
  const monaco = fakeEditor(initial);
  const store = fakeStore(initial);
  const onLocalChange = mock.fn();
  const binding = bindCodeEditor(monaco.api, store.api, onLocalChange);
  t.after(() => binding.dispose());
  assert.equal(monaco.listenerCount, 1);
  assert.equal(store.listenerCount, 1);

  binding.dispose();
  binding.dispose();

  assert.equal(monaco.disposals, 1);
  assert.equal(store.unsubscriptions, 1);
  assert.equal(monaco.listenerCount, 0);
  assert.equal(store.listenerCount, 0);
  const reads = monaco.reads;
  const external = "sequenceDiagram";
  store.replace(external);
  assert.equal(monaco.value, initial);
  monaco.type("flowchart LR");

  assert.equal(store.value, external);
  assert.equal(monaco.reads, reads);
  assert.deepEqual(store.writes, []);
  assert.deepEqual(monaco.replacements, []);
  assert.equal(onLocalChange.mock.callCount(), 0);
});

function fakeEditor(initial: string, onReplace?: (text: string) => void) {
  let value = initial;
  let reads = 0;
  let disposals = 0;
  const listeners = new Set<() => void>();
  const replacements: string[] = [];
  const operations: string[] = [];
  const model = {
    isDisposed: () => false,
    getFullModelRange() {
      const lines = value.split("\n");
      return {
        startLineNumber: 1, startColumn: 1,
        endLineNumber: lines.length, endColumn: lines.at(-1)!.length + 1,
      };
    },
  };
  const change = (text: string) => {
    value = text.replace(/\r\n?/g, "\n");
    for (const listener of [...listeners]) listener();
  };
  const api: Parameters<typeof bindCodeEditor>[0] = {
    getModel: () => model as editor.ITextModel,
    getValue() { reads += 1; return value; },
    onDidChangeModelContent(listener) {
      const notify = () => listener({} as editor.IModelContentChangedEvent);
      listeners.add(notify);
      return { dispose() { disposals += 1; listeners.delete(notify); } };
    },
    executeEdits(_source, edits) {
      // Bound synchronous replay so a regression fails instead of hanging Node.
      assert(replacements.length < 10, "document replacements must settle synchronously");
      assert.equal(edits.length, 1);
      const [{ range, text }] = edits;
      assert.deepEqual(range, model.getFullModelRange());
      assert(typeof text === "string");
      replacements.push(text);
      operations.push("edit");
      change(text);
      onReplace?.(text);
      return true;
    },
    pushUndoStop() { operations.push("undo-stop"); return true; },
  };
  return {
    api, replacements, operations, type: change,
    get value() { return value; },
    get reads() { return reads; },
    get disposals() { return disposals; },
    get listenerCount() { return listeners.size; },
  };
}

function fakeStore(initial: string) {
  let code = initial;
  let unsubscriptions = 0;
  const listeners = new Set<() => void>();
  const writes: string[] = [];
  const replace = (next: string) => {
    code = next;
    for (const listener of [...listeners]) listener();
  };
  const api: Parameters<typeof bindCodeEditor>[1] = {
    getState: () => ({
      code,
      setCode(next) { writes.push(next); replace(next); },
    }),
    subscribe(listener) {
      listeners.add(listener);
      return () => { unsubscriptions += 1; listeners.delete(listener); };
    },
  };
  return {
    api, replace, writes,
    get value() { return code; },
    get unsubscriptions() { return unsubscriptions; },
    get listenerCount() { return listeners.size; },
  };
}
