import type {
  EditorCodeAction,
  EditorCompletionList,
  EditorDiagnosticsResult,
  EditorDocumentSymbol,
  EditorHover,
  EditorLocation,
  EditorPosition,
  EditorPrepareRename,
  EditorSemanticToken,
  EditorSemanticTokenLegend,
  EditorWorkspaceEdit,
} from "@mermanjs/web";

export const EDITOR_WORKER_PROTOCOL = 1 as const;
export const EDITOR_SCHEMA_VERSION = 1 as const;
export const MERMAN_NATIVE_ABI_VERSION = 2 as const;

export interface EditorDocumentSnapshot {
  readonly uri: string;
  readonly version: number;
  readonly source: string;
}

export type EditorWorkerQuery =
  | { readonly kind: "diagnostics" }
  | { readonly kind: "codeActions" }
  | {
      readonly kind: "completions";
      readonly position: EditorPosition;
    }
  | {
      readonly kind: "hover";
      readonly position: EditorPosition;
    }
  | { readonly kind: "documentSymbols" }
  | {
      readonly kind: "definition";
      readonly position: EditorPosition;
    }
  | {
      readonly kind: "references";
      readonly position: EditorPosition;
      readonly includeDeclaration: boolean;
    }
  | {
      readonly kind: "prepareRename";
      readonly position: EditorPosition;
    }
  | {
      readonly kind: "rename";
      readonly position: EditorPosition;
      readonly newName: string;
    }
  | { readonly kind: "semanticTokens" };

export interface EditorWorkerQueryResults {
  diagnostics: EditorDiagnosticsResult;
  codeActions: EditorCodeAction[];
  completions: EditorCompletionList;
  hover: EditorHover | null;
  documentSymbols: EditorDocumentSymbol[];
  definition: EditorLocation | null;
  references: EditorLocation[];
  prepareRename: EditorPrepareRename | null;
  rename: EditorWorkspaceEdit | null;
  semanticTokens: EditorSemanticToken[];
}

export type EditorWorkerQueryResult<Query extends EditorWorkerQuery> =
  EditorWorkerQueryResults[Query["kind"]];

interface EditorWorkerRequestBase {
  readonly protocol: typeof EDITOR_WORKER_PROTOCOL;
  readonly requestId: number;
}

export type EditorWorkerRequest =
  | (EditorWorkerRequestBase & { readonly type: "initialize" })
  | (EditorWorkerRequestBase & {
      readonly type: "didOpen";
      readonly document: EditorDocumentSnapshot;
    })
  | (EditorWorkerRequestBase & {
      readonly type: "didChange";
      readonly document: EditorDocumentSnapshot;
    })
  | (EditorWorkerRequestBase & {
      readonly type: "query";
      readonly uri: string;
      readonly version: number;
      readonly query: EditorWorkerQuery;
    })
  | (EditorWorkerRequestBase & { readonly type: "cancel" })
  | {
      readonly protocol: typeof EDITOR_WORKER_PROTOCOL;
      readonly type: "dispose";
    };

interface EditorWorkerResponseBase {
  readonly protocol: typeof EDITOR_WORKER_PROTOCOL;
  readonly requestId: number;
}

export type EditorWorkerResponse =
  | (EditorWorkerResponseBase & {
      readonly type: "ready";
      readonly nativeAbi: typeof MERMAN_NATIVE_ABI_VERSION;
      readonly editorSchema: typeof EDITOR_SCHEMA_VERSION;
      readonly legend: EditorSemanticTokenLegend;
    })
  | (EditorWorkerResponseBase & {
      readonly type: "result";
      readonly result: unknown;
    })
  | (EditorWorkerResponseBase & {
      readonly type: "error";
      readonly code:
        | "CANCELED"
        | "INITIALIZATION_FAILED"
        | "INVALID_STATE"
        | "PROTOCOL_MISMATCH"
        | "QUERY_FAILED"
        | "STALE_DOCUMENT";
      readonly message: string;
    });
