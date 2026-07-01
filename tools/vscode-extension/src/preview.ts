import * as vscode from "vscode";

import { PreviewInstance } from "./preview-instance.js";
import {
  isActiveEditorSelectionChange,
  isTrackedPreviewDiagnosticsChange,
  isTrackedPreviewDocumentChange,
} from "./preview-manager-routing.js";
import type { PreviewUpdateReason } from "./preview-policy.js";
import type { MermaidSourceCommandArgument } from "./source-actions.js";

const PREVIEW_COMMAND = "merman.openPreview";
const TOGGLE_PREVIEW_LOCK_COMMAND = "merman.togglePreviewLock";
const EMPTY_PREVIEW_LOCK_WARNING = "Open a Mermaid preview before locking it to a source.";

export function registerPreview(context: vscode.ExtensionContext): void {
  const manager = new MermanPreviewManager(context);
  context.subscriptions.push(manager);
}

class MermanPreviewManager implements vscode.Disposable {
  private readonly outputChannel: vscode.LogOutputChannel;
  private readonly disposables: vscode.Disposable[] = [];
  private currentInstance: PreviewInstance | undefined;

  constructor(private readonly context: vscode.ExtensionContext) {
    this.outputChannel = vscode.window.createOutputChannel("Merman Preview", { log: true });
    this.disposables.push(this.outputChannel);
    this.disposables.push(
      vscode.commands.registerCommand(
        PREVIEW_COMMAND,
        async (target?: MermaidSourceCommandArgument) => {
          await this.open(target);
        },
      ),
    );
    this.disposables.push(
      vscode.commands.registerCommand(TOGGLE_PREVIEW_LOCK_COMMAND, () => {
        this.toggleLock();
      }),
    );
    this.disposables.push(
      vscode.window.onDidChangeActiveTextEditor(() => {
        this.scheduleRefresh("active-editor");
      }),
    );
    this.disposables.push(
      vscode.window.onDidChangeTextEditorSelection((event) => {
        if (isActiveEditorSelectionChange(event.textEditor, vscode.window.activeTextEditor)) {
          this.scheduleRefresh("selection");
        }
      }),
    );
    this.disposables.push(
      vscode.workspace.onDidChangeTextDocument((event) => {
        const instance = this.currentInstance;
        if (!instance) {
          return;
        }
        const trackedEditor = instance.resolvePreviewEditor();
        if (isTrackedPreviewDocumentChange(trackedEditor, event.document)) {
          instance.scheduleRefresh("document-change");
        }
      }),
    );
    this.disposables.push(
      vscode.languages.onDidChangeDiagnostics((event) => {
        const instance = this.currentInstance;
        if (!instance) {
          return;
        }
        const trackedEditor = instance.resolvePreviewEditor();
        const trackedUri = trackedEditor?.document.uri;
        if (isTrackedPreviewDiagnosticsChange(trackedUri, event.uris)) {
          instance.scheduleRefresh("diagnostics");
        }
      }),
    );
  }

  dispose(): void {
    this.currentInstance?.dispose();
    this.currentInstance = undefined;
    for (const disposable of this.disposables) {
      disposable.dispose();
    }
  }

  private async open(target?: MermaidSourceCommandArgument): Promise<void> {
    await this.ensureInstance().open(target);
  }

  private toggleLock(): void {
    const instance = this.currentInstance;
    if (!instance) {
      void vscode.window.showWarningMessage(EMPTY_PREVIEW_LOCK_WARNING);
      return;
    }
    instance.setLocked(!instance.isLocked, true);
  }

  private ensureInstance(): PreviewInstance {
    if (!this.currentInstance) {
      this.currentInstance = new PreviewInstance(
        this.context,
        this.outputChannel,
        (instance) => this.handleInstanceDisposed(instance),
      );
    }
    return this.currentInstance;
  }

  private handleInstanceDisposed(instance: PreviewInstance): void {
    if (this.currentInstance === instance) {
      this.currentInstance = undefined;
    }
  }

  private scheduleRefresh(reason: PreviewUpdateReason): void {
    this.currentInstance?.scheduleRefresh(reason);
  }
}
