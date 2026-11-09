import type { editor, IDisposable } from "monaco-editor";

interface CodeState {
  readonly code: string;
  setCode(code: string): void;
}

interface CodeStore {
  getState(): CodeState;
  subscribe(listener: () => void): () => void;
}

type CodeEditor = Pick<editor.IStandaloneCodeEditor,
  "executeEdits" | "getModel" | "getValue" | "onDidChangeModelContent" | "pushUndoStop"
>;

/** Keep local edits in Monaco; only external store changes replace the document. */
export function bindCodeEditor(
  instance: CodeEditor,
  store: CodeStore,
  onLocalChange: () => void,
): IDisposable {
  const model = instance.getModel();
  if (!model) throw new Error("The code editor requires a document model.");
  let value = instance.getValue();
  let storeValue = value;
  let replacing = false;
  let disposed = false;

  const content = instance.onDidChangeModelContent(() => {
    if (disposed) return;
    value = instance.getValue();
    if (replacing) return;
    storeValue = value;
    onLocalChange();
    store.getState().setCode(value);
  });
  const synchronize = () => {
    if (disposed || replacing || model.isDisposed()) return;
    replacing = true;
    try {
      let next = store.getState().code;
      while (storeValue !== next) {
        storeValue = next;
        if (value !== next) {
          instance.executeEdits("", [{
            range: model.getFullModelRange(), text: next, forceMoveMarkers: true,
          }]);
          instance.pushUndoStop();
        }
        next = store.getState().code;
        if (disposed || model.isDisposed()) break;
      }
    } finally {
      replacing = false;
    }
  };
  const unsubscribe = store.subscribe(synchronize);
  synchronize();

  return {
    dispose() {
      if (disposed) return;
      disposed = true;
      unsubscribe();
      content.dispose();
    },
  };
}
