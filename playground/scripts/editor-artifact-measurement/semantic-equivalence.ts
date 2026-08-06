import languageWorkerUrl from "../../src/editor/merman-language.worker.ts?worker&url";
import {
  EditorWorkerProtocolError,
  startMermanLanguageWorkerClient,
  type MermanLanguageWorkerClient,
} from "../../src/editor/worker-client.ts";
import type { EditorWorkerQuery } from "../../src/editor/protocol.ts";
import { SEMANTIC_TOKEN_DESCRIPTOR_DIGEST } from "../../../platforms/web/src/generated/token-descriptor.ts";
import {
  EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
  EDITOR_ARTIFACT_FAMILY_COUNT,
  EDITOR_ARTIFACT_QUERY_KINDS,
  canonicalStringify,
  compareCanonicalStrings,
} from "./equivalence-shared.mjs";

type EditorWorkerQueryKind = EditorWorkerQuery["kind"];

const QUERY_KINDS =
  EDITOR_ARTIFACT_QUERY_KINDS satisfies readonly EditorWorkerQueryKind[];
type MissingQueryKind = Exclude<
  EditorWorkerQueryKind,
  (typeof QUERY_KINDS)[number]
>;
const QUERY_KINDS_ARE_EXHAUSTIVE: [MissingQueryKind] extends [never]
  ? true
  : never = true;

interface EquivalenceQueryDigest {
  readonly kind: EditorWorkerQueryKind;
  readonly outcome: "error" | "result";
  readonly sha256: string;
}

interface EquivalenceFamilyResult {
  readonly diagramType: string;
  readonly baselineId: string;
  readonly fixture: string;
  readonly sourceSha256: string;
  readonly queries: readonly EquivalenceQueryDigest[];
}

interface EquivalenceMatrixBody {
  readonly schemaVersion: number;
  readonly familyCount: number;
  readonly queryCount: number;
  readonly cellCount: number;
  readonly queryKinds: readonly string[];
  readonly families: readonly EquivalenceFamilyResult[];
}

interface EquivalenceMatrix extends EquivalenceMatrixBody {
  readonly aggregateSha256: string;
}

type EquivalencePageState =
  | { readonly status: "ready" | "running" }
  | { readonly status: "complete"; readonly matrix: EquivalenceMatrix }
  | {
      readonly status: "error";
      readonly error: {
        readonly message: string;
        readonly stack: string | null;
      };
    };

declare global {
  interface Window {
    __mermanEditorArtifactEquivalenceV1: EquivalencePageState;
    __runMermanEditorArtifactEquivalenceV1(
      baselines: readonly EquivalenceBaseline[],
    ): void;
  }
}

interface EquivalenceBaseline {
  readonly id: string;
  readonly family: string;
  readonly fixture: string;
  readonly source: string;
  readonly sourceSha256: string;
  readonly detectionValidity: string;
  readonly syntaxId: string;
  readonly effectiveLayoutId: string;
  readonly semanticTokensSha256: string;
}

window.__mermanEditorArtifactEquivalenceV1 = { status: "ready" };
window.__runMermanEditorArtifactEquivalenceV1 = (baselines) => {
  if (window.__mermanEditorArtifactEquivalenceV1.status !== "ready") {
    throw new Error("The editor equivalence probe may run only once.");
  }
  window.__mermanEditorArtifactEquivalenceV1 = { status: "running" };
  void measureEquivalence(baselines).then(
    (matrix) => {
      window.__mermanEditorArtifactEquivalenceV1 = {
        status: "complete",
        matrix,
      };
    },
    (error: unknown) => {
      const failure = error instanceof Error ? error : new Error(String(error));
      window.__mermanEditorArtifactEquivalenceV1 = {
        status: "error",
        error: { message: failure.message, stack: failure.stack ?? null },
      };
    },
  );
};

async function measureEquivalence(
  input: readonly EquivalenceBaseline[],
): Promise<EquivalenceMatrix> {
  if (!QUERY_KINDS_ARE_EXHAUSTIVE) {
    throw new Error("Editor equivalence query kinds are incomplete.");
  }
  const baselines = [...input].sort((left, right) =>
    compareCanonicalStrings(left.family, right.family),
  );
  const families = new Set(baselines.map((example) => example.family));
  if (
    baselines.length !== EDITOR_ARTIFACT_FAMILY_COUNT ||
    families.size !== EDITOR_ARTIFACT_FAMILY_COUNT
  ) {
    throw new Error(
      `Expected exactly one generated baseline for each of ${EDITOR_ARTIFACT_FAMILY_COUNT} families; found ${baselines.length} baselines across ${families.size} families.`,
    );
  }

  const worker = new Worker(languageWorkerUrl, {
    name: "merman-editor-artifact-equivalence",
    type: "module",
  });
  const startup = startMermanLanguageWorkerClient(
    worker,
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
    30_000,
  );
  const uri = "file:///merman/editor-artifact-equivalence.mmd";
  const matrixFamilies: EquivalenceFamilyResult[] = [];
  try {
    await startup.ready;
    for (const [index, example] of baselines.entries()) {
      const version = index + 1;
      const document = { source: example.source, uri, version };
      const identity = { uri, version };
      if (index === 0) await startup.client.openDocument(document);
      else await startup.client.changeDocument(document);

      const sourceSha256 = await sha256Text(example.source);
      if (sourceSha256 !== example.sourceSha256) {
        throw new Error(`${example.family} baseline source digest is stale.`);
      }
      const probePosition = positionForSource(example.source);
      const completionPosition = {
        line: 0,
        character: example.source.split(/\r?\n/u, 1)[0]?.length ?? 0,
      };
      let semanticPosition = probePosition;
      let renameName = "merman_equivalence";
      const queryDigests: EquivalenceQueryDigest[] = [];
      for (const kind of QUERY_KINDS) {
        const query = queryForKind(
          kind,
          semanticPosition,
          completionPosition,
          renameName,
        );
        const outcome = await queryDocument(startup.client, identity, query);
        if (kind === "diagramDetection") {
          assertDetectedFamily(expectQueryResult(outcome, kind), example);
        }
        if (kind === "documentSymbols" && outcome.outcome === "result") {
          semanticPosition = symbolPosition(outcome.value) ?? semanticPosition;
        }
        if (kind === "prepareRename" && outcome.outcome === "result") {
          renameName = renameNameForPreparation(outcome.value) ?? renameName;
        }
        const resultSha256 = await sha256Text(
          canonicalStringify(outcome.value),
        );
        if (
          kind === "semanticTokens" &&
          resultSha256 !== example.semanticTokensSha256
        ) {
          throw new Error(
            `${example.family} semantic tokens differ from generated baseline evidence.`,
          );
        }
        queryDigests.push({
          kind,
          outcome: outcome.outcome,
          sha256: resultSha256,
        });
      }
      matrixFamilies.push({
        diagramType: example.family,
        baselineId: example.id,
        fixture: example.fixture,
        sourceSha256,
        queries: queryDigests,
      });
    }
  } finally {
    startup.client.dispose();
  }

  const body: EquivalenceMatrixBody = {
    schemaVersion: EDITOR_ARTIFACT_EQUIVALENCE_SCHEMA_VERSION,
    familyCount: matrixFamilies.length,
    queryCount: QUERY_KINDS.length,
    cellCount: matrixFamilies.length * QUERY_KINDS.length,
    queryKinds: [...QUERY_KINDS],
    families: matrixFamilies,
  };
  return {
    ...body,
    aggregateSha256: await sha256Text(canonicalStringify(body)),
  };
}

function queryForKind(
  kind: EditorWorkerQueryKind,
  position: { readonly line: number; readonly character: number },
  completionPosition: { readonly line: number; readonly character: number },
  renameName: string,
): EditorWorkerQuery {
  switch (kind) {
    case "diagnostics":
    case "diagramDetection":
    case "codeActions":
    case "documentSymbols":
    case "semanticTokens":
      return { kind };
    case "completions":
      return { kind, position: completionPosition };
    case "hover":
    case "definition":
    case "prepareRename":
      return { kind, position };
    case "references":
      return { kind, position, includeDeclaration: true };
    case "rename":
      return { kind, position, newName: renameName };
    default:
      return assertNever(kind);
  }
}

function assertNever(value: never): never {
  throw new Error(`Unsupported editor equivalence query ${String(value)}.`);
}

async function queryDocument(
  client: MermanLanguageWorkerClient,
  document: { readonly uri: string; readonly version: number },
  query: EditorWorkerQuery,
): Promise<
  | { readonly outcome: "result"; readonly value: unknown }
  | { readonly outcome: "error"; readonly value: unknown }
> {
  try {
    return { outcome: "result", value: await client.query(document, query) };
  } catch (error) {
    if (
      error instanceof EditorWorkerProtocolError &&
      (error.code === "OPERATION_REJECTED" || error.code === "QUERY_FAILED")
    ) {
      return {
        outcome: "error",
        value: {
          code: error.code,
          detail: error.detail,
          message: error.message,
          nativeCode: error.nativeCode,
        },
      };
    }
    throw error;
  }
}

function expectQueryResult(
  outcome:
    | { readonly outcome: "result"; readonly value: unknown }
    | { readonly outcome: "error"; readonly value: unknown },
  kind: string,
): unknown {
  if (outcome.outcome === "error") {
    throw new Error(`${kind} returned a request-local error.`);
  }
  return outcome.value;
}

function symbolPosition(value: unknown): {
  readonly line: number;
  readonly character: number;
} | null {
  if (!Array.isArray(value) || value.length === 0) return null;
  const position = value[0]?.selectionRange?.start;
  return isPosition(position) ? position : null;
}

function renameNameForPreparation(value: unknown): string | null {
  if (!value || typeof value !== "object") return null;
  const placeholder = (value as { placeholder?: unknown }).placeholder;
  return typeof placeholder === "string" && placeholder.length > 0
    ? placeholder
    : null;
}

function isPosition(
  value: unknown,
): value is { readonly line: number; readonly character: number } {
  return (
    !!value &&
    typeof value === "object" &&
    Number.isSafeInteger((value as { line?: unknown }).line) &&
    Number.isSafeInteger((value as { character?: unknown }).character)
  );
}

function positionForSource(source: string): {
  readonly line: number;
  readonly character: number;
} {
  const lines = source.split(/\r?\n/u);
  for (let line = 1; line < lines.length; line += 1) {
    const match = /[A-Za-z_][A-Za-z0-9_.-]*/u.exec(lines[line] ?? "");
    if (match?.index !== undefined) return { line, character: match.index };
  }
  return { line: 0, character: 0 };
}

function assertDetectedFamily(
  result: unknown,
  expected: EquivalenceBaseline,
): void {
  if (
    !result ||
    typeof result !== "object" ||
    (result as { status?: unknown }).status !== "available" ||
    (result as { validity?: unknown }).validity !==
      expected.detectionValidity ||
    (result as { diagramType?: unknown }).diagramType !== expected.family ||
    (result as { syntaxId?: unknown }).syntaxId !== expected.syntaxId ||
    (result as { effectiveLayoutId?: unknown }).effectiveLayoutId !==
      expected.effectiveLayoutId
  ) {
    throw new Error(
      `${expected.family} baseline returned inconsistent detection.`,
    );
  }
}

async function sha256Text(value: string): Promise<string> {
  const bytes = new TextEncoder().encode(value);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join(
    "",
  );
}
