import type {
  EditorCodeAction,
  EditorCompletionList,
  DiagramDetectionFacts,
  EditorDiagnosticsResult,
  EditorDocumentSymbol,
  EditorHover,
  EditorLocation,
  EditorPosition,
  EditorPrepareRename,
  EditorSemanticTokenLegend,
  EditorWorkspaceEdit,
} from "@mermanjs/web";

export const EDITOR_WORKER_PROTOCOL = 3 as const;
export const EDITOR_SCHEMA_VERSION = 1 as const;

export type EditorWorkerErrorCode =
  | "INITIALIZATION_FAILED"
  | "INVALID_STATE"
  | "OPERATION_REJECTED"
  | "PROTOCOL_MISMATCH"
  | "QUERY_FAILED"
  | "STALE_DOCUMENT";

export interface EditorDocumentSnapshot {
  readonly uri: string;
  readonly version: number;
  readonly source: string;
}

export type EditorWorkerQuery =
  | { readonly kind: "diagnostics" }
  | { readonly kind: "diagramDetection" }
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
  diagramDetection: DiagramDetectionFacts;
  codeActions: EditorCodeAction[];
  completions: EditorCompletionList;
  hover: EditorHover | null;
  documentSymbols: EditorDocumentSymbol[];
  definition: EditorLocation | null;
  references: EditorLocation[];
  prepareRename: EditorPrepareRename | null;
  rename: EditorWorkspaceEdit | null;
  semanticTokens: Uint32Array;
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
      readonly legendDigest: string;
      readonly query: EditorWorkerQuery;
    })
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
      readonly transportApiVersion: number;
      readonly editorSchema: typeof EDITOR_SCHEMA_VERSION;
      readonly legendDigest: string;
      readonly legend: EditorSemanticTokenLegend;
    })
  | (EditorWorkerResponseBase & {
      readonly type: "result";
      readonly result: unknown;
    })
  | (EditorWorkerResponseBase & {
      readonly type: "queryResult";
      readonly uri: string;
      readonly version: number;
      readonly legendDigest: string;
      readonly result: unknown;
    })
  | (EditorWorkerResponseBase & {
      readonly type: "error";
      readonly code: EditorWorkerErrorCode;
      readonly message: string;
      readonly detail: string | null;
      readonly nativeCode: string | null;
    });
