import * as vscode from "vscode";

import {
  EMPTY_PREVIEW_LOCK_WARNING,
  PreviewInstance,
} from "./preview-instance.js";
import {
  isActiveEditorSelectionChange,
  isTrackedPreviewDiagnosticsChange,
  isTrackedPreviewDocumentChange,
} from "./preview-manager-routing.js";
import type { PreviewUpdateReason } from "./preview-policy.js";
import type { MermaidSourceCommandArgument } from "./source-actions.js";

const PREVIEW_COMMAND = "merman.openPreview";
const TOGGLE_PREVIEW_LOCK_COMMAND = "merman.togglePreviewLock";
const REFRESH_PREVIEW_COMMAND = "merman.refreshPreview";
const SHOW_PREVIEW_SOURCE_COMMAND = "merman.showPreviewSource";

export function registerPreview(context: vscode.ExtensionContext): void {
  const manager = new MermanPreviewManager(context);
  context.subscriptions.push(manager);
}

class MermanPreviewManager implements vscode.Disposable {
  private readonly outputChannel: vscode.LogOutputChannel;
  private readonly disposables: vscode.Disposable[] = [];
  private readonly instances = new Set<PreviewInstance>();
  private activeInstance: PreviewInstance | undefined;
  private followInstance: PreviewInstance | undefined;

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
      vscode.commands.registerCommand(REFRESH_PREVIEW_COMMAND, () => {
        this.refreshAll();
      }),
    );
    this.disposables.push(
      vscode.commands.registerCommand(SHOW_PREVIEW_SOURCE_COMMAND, async () => {
        await this.showSource();
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
        for (const instance of this.instances) {
          instance.trackDocumentChange(event);
          const trackedEditor = instance.resolvePreviewEditor();
          if (!isTrackedPreviewDocumentChange(trackedEditor, event.document) && !instance.tracksDocument(event.document.uri)) {
            continue;
          }
          instance.invalidateRenderedOutput("document-change");
          instance.scheduleRefresh("document-change");
        }
      }),
    );
    this.disposables.push(
      vscode.languages.onDidChangeDiagnostics((event) => {
        for (const instance of this.instances) {
          const trackedEditor = instance.resolvePreviewEditor();
          const trackedUri = trackedEditor?.document.uri;
          if (!isTrackedPreviewDiagnosticsChange(trackedUri, event.uris) && !event.uris.some((uri) => instance.tracksDocument(uri))) {
            continue;
          }
          instance.scheduleRefresh("diagnostics");
        }
      }),
    );
  }

  dispose(): void {
    const instances = Array.from(this.instances);
    for (const instance of instances) {
      instance.dispose();
    }
    this.instances.clear();
    this.activeInstance = undefined;
    this.followInstance = undefined;
    for (const disposable of this.disposables) {
      disposable.dispose();
    }
  }

  private async open(target?: MermaidSourceCommandArgument): Promise<void> {
    const instance = this.ensureFollowInstance();
    await instance.open(target);
    this.followInstance = instance.isLocked ? undefined : instance;
    this.closeRedundantFollowInstances(instance);
  }

  private toggleLock(): void {
    const instance = this.commandTargetInstance();
    if (!instance) {
      void vscode.window.showWarningMessage(EMPTY_PREVIEW_LOCK_WARNING);
      return;
    }
    const changed = instance.setLocked(!instance.isLocked, true);
    if (!changed) {
      return;
    }
  }

  private refreshAll(): void {
    for (const instance of this.instances) {
      instance.forceRefresh();
    }
  }

  private async showSource(): Promise<void> {
    await this.commandTargetInstance()?.showSource();
  }

  private ensureFollowInstance(): PreviewInstance {
    if (this.activeInstance && !this.activeInstance.isLocked) {
      this.followInstance = this.activeInstance;
      return this.activeInstance;
    }
    if (this.followInstance && !this.followInstance.isLocked) {
      return this.followInstance;
    }
    const instance = this.createInstance();
    this.followInstance = instance;
    return instance;
  }

  private handleLockStateChanged(instance: PreviewInstance): void {
    if (instance.isLocked) {
      if (this.followInstance === instance) {
        this.followInstance = undefined;
      }
      return;
    }
    this.followInstance = instance;
    this.closeRedundantFollowInstances(instance);
  }

  private handleInstanceDisposed(instance: PreviewInstance): void {
    this.instances.delete(instance);
    if (this.activeInstance === instance) {
      this.activeInstance = undefined;
    }
    if (this.followInstance === instance) {
      this.followInstance = undefined;
    }
  }

  private createInstance(): PreviewInstance {
    const instance = new PreviewInstance(
      this.context,
      this.outputChannel,
      (disposed) => this.handleInstanceDisposed(disposed),
      (changed, active) => this.handleInstanceActiveState(changed, active),
      (changed) => this.handleLockStateChanged(changed),
    );
    this.instances.add(instance);
    return instance;
  }

  private handleInstanceActiveState(instance: PreviewInstance, active: boolean): void {
    if (active) {
      this.activeInstance = instance;
      return;
    }
    if (this.activeInstance === instance) {
      this.activeInstance = undefined;
    }
  }

  private commandTargetInstance(): PreviewInstance | undefined {
    return this.activeInstance ?? this.followInstance ?? singleInstance(this.instances);
  }

  private closeRedundantFollowInstances(keep: PreviewInstance): void {
    for (const instance of Array.from(this.instances)) {
      if (instance !== keep && !instance.isLocked) {
        instance.dispose();
      }
    }
  }

  private scheduleRefresh(reason: PreviewUpdateReason): void {
    this.followInstance?.scheduleRefresh(reason);
  }
}

function singleInstance(instances: ReadonlySet<PreviewInstance>): PreviewInstance | undefined {
  if (instances.size !== 1) {
    return undefined;
  }
  return instances.values().next().value;
}
