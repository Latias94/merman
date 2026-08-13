import { expect, test } from "@playwright/test";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  SEMANTIC_TOKEN_DESCRIPTOR,
  SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  SEMANTIC_TOKEN_MODIFIER_LSP_NAMES,
  SEMANTIC_TOKEN_RECORD_WIDTH,
  SEMANTIC_TOKEN_TYPE_LSP_NAMES,
  SEMANTIC_TOKEN_VALID_MODIFIER_MASK,
} from "../../platforms/web/src/generated/token-descriptor";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  MERMAN_WEB_TRANSPORT_API_VERSION,
} from "../src/editor/protocol";
import {
  monitorBrowserErrors,
  openPlayground,
  replaceEditorSource,
} from "./helpers/playground";

interface TokenEquivalenceCase {
  readonly id: string;
  readonly family: string;
  readonly source: string;
  readonly detection_validity: string;
  readonly syntax_id: string;
  readonly effective_layout_id: string;
  readonly packed_words: number[];
  readonly packed_sha256: string;
}

interface TokenEquivalenceEvidence {
  readonly schema_version: number;
  readonly descriptor_digest: string;
  readonly packed_encoding: string;
  readonly words_per_token: number;
  readonly family_cases: TokenEquivalenceCase[];
  readonly recovery_cases: TokenEquivalenceCase[];
}

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);
const tokenEquivalenceEvidence = JSON.parse(
  readFileSync(
    path.join(repositoryRoot, "contracts/editor-language/token-equivalence-v1.json"),
    "utf8",
  ),
) as TokenEquivalenceEvidence;
const familySemanticFixtures = tokenEquivalenceEvidence.family_cases;
const editorCompletionTriggerCharacters = [
  " ",
  "\n",
  "-",
  ">",
  "%",
  "[",
  "(",
  "{",
  "/",
  "\\",
  "@",
  ":",
] as const;

test("Monaco and the Rust editor session start only local production workers", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  const requests: string[] = [];
  const workers: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  page.on("worker", (worker) => workers.push(worker.url()));

  await openPlayground(page);
  await expect(
    page.getByRole("textbox", { name: "Mermaid source" }),
  ).toBeVisible();
  await expect
    .poll(() => workers.some((url) => /merman-language\.worker/i.test(url)))
    .toBe(true);
  await expect
    .poll(() =>
      requests.some((url) => /merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/.test(url)),
    )
    .toBe(true);
  expect(workers.some((url) => /json\.worker/i.test(url))).toBe(false);

  await replaceEditorSource(page, "flowchart TD\n  A -->");
  await expect
    .poll(() => page.locator(".monaco-editor .squiggly-error").count())
    .toBeGreaterThan(0);

  await page.getByRole("tab", { name: "Config", exact: true }).click();
  await expect
    .poll(() => workers.some((url) => /json\.worker/i.test(url)))
    .toBe(true);

  const pageOrigin = new URL(page.url()).origin;
  const external = requests.filter((url) => {
    const parsed = new URL(url);
    return (
      (parsed.protocol === "http:" || parsed.protocol === "https:") &&
      parsed.origin !== pageOrigin
    );
  });
  expect(external).toEqual([]);
  expect(requests.some((url) => /cdn\.jsdelivr\.net/i.test(url))).toBe(false);
  for (const workerUrl of workers) {
    expect(new URL(workerUrl).origin).toBe(pageOrigin);
  }
  errors.assertNone();
});

test("the generated editor worker returns identity-bound packed tokens for all 35 families", async ({
  page,
}) => {
  test.setTimeout(120_000);
  let languageWorkerUrl = "";
  page.on("worker", (worker) => {
    if (/merman-language\.worker/i.test(worker.url())) {
      languageWorkerUrl = worker.url();
    }
  });

  await openPlayground(page);
  await expect.poll(() => languageWorkerUrl.length > 0).toBe(true);
  expect(tokenEquivalenceEvidence.schema_version).toBe(
    SEMANTIC_TOKEN_DESCRIPTOR.schemaVersion,
  );
  expect(tokenEquivalenceEvidence.descriptor_digest).toBe(
    SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
  );
  expect(tokenEquivalenceEvidence.packed_encoding).toBe(
    SEMANTIC_TOKEN_DESCRIPTOR.packed.encoding,
  );
  expect(tokenEquivalenceEvidence.words_per_token).toBe(
    SEMANTIC_TOKEN_RECORD_WIDTH,
  );
  expect(familySemanticFixtures).toHaveLength(35);

  const result = await page.evaluate(
    async ({
      digest,
      editorSchema,
      fixtures,
      modifierMask,
      protocol,
      recordWidth,
      tokenEquivalenceEvidence,
      tokenTypeNames,
      workerUrl,
    }) => {
      interface ResponseMessage {
        readonly protocol: number;
        readonly requestId: number;
        readonly type: string;
        readonly code?: string;
        readonly message?: string;
        readonly transportApiVersion?: number;
        readonly editorSchema?: number;
        readonly completionTriggerCharacters?: string[];
        readonly legendDigest?: string;
        readonly legend?: {
          readonly tokenTypes: string[];
          readonly tokenModifiers: string[];
        };
        readonly uri?: string;
        readonly version?: number;
        readonly result?: unknown;
      }

      const worker = new Worker(workerUrl, {
        name: "merman-editor-family-matrix",
        type: "module",
      });
      let requestId = 0;
      const send = (message: Record<string, unknown>) =>
        new Promise<ResponseMessage>((resolve, reject) => {
          const timeout = window.setTimeout(
            () =>
              reject(
                new Error(
                  `Editor worker timed out for ${String(message.type)}.`,
                ),
              ),
            10_000,
          );
          worker.addEventListener(
            "message",
            (event: MessageEvent<ResponseMessage>) => {
              window.clearTimeout(timeout);
              if (event.data.type === "error") {
                reject(
                  new Error(
                    `${event.data.code ?? "QUERY_FAILED"}: ${event.data.message ?? "unknown error"}`,
                  ),
                );
                return;
              }
              resolve(event.data);
            },
            { once: true },
          );
          worker.postMessage(message);
        });
      const request = (type: string, fields: Record<string, unknown> = {}) => {
        requestId += 1;
        return send({
          protocol,
          requestId,
          type,
          ...fields,
        });
      };
      const uri = "file:///merman/family-matrix.mmd";
      const ready = await request("initialize");
      if (
        ready.type !== "ready" ||
        ready.transportApiVersion !== MERMAN_WEB_TRANSPORT_API_VERSION ||
        ready.editorSchema !== editorSchema ||
        ready.legendDigest !== digest
      ) {
        throw new Error(
          "Editor worker returned the wrong generated language identity.",
        );
      }
      const query = async (
        version: number,
        kind: string,
        fields: Record<string, unknown> = {},
      ) => {
        const response = await request("query", {
          uri,
          version,
          legendDigest: digest,
          query: { kind, ...fields },
        });
        if (
          response.type !== "queryResult" ||
          response.uri !== uri ||
          response.version !== version ||
          response.legendDigest !== digest
        ) {
          throw new Error(`${kind} returned an obsolete snapshot identity.`);
        }
        return response.result;
      };

      const packedDigest = async (words: Uint32Array) => {
        const bytes = new TextEncoder().encode(
          JSON.stringify(Array.from(words)),
        );
        const digestBytes = new Uint8Array(
          await crypto.subtle.digest("SHA-256", bytes),
        );
        return `sha256:${Array.from(digestBytes, (byte) =>
          byte.toString(16).padStart(2, "0"),
        ).join("")}`;
      };
      const summaries: Array<{
        family: string;
        tokenWords: number;
        packedSha256: string;
      }> = [];
      let emptyDiagnostic:
        | {
            code: unknown;
            codeName: unknown;
          }
        | undefined;
      let version = 1;
      try {
        for (const [index, fixture] of fixtures.entries()) {
          const document = { uri, version, source: fixture.source };
          const synchronized = await request(
            index === 0 ? "didOpen" : "didChange",
            {
              document,
            },
          );
          if (synchronized.type !== "result") {
            throw new Error(
              `${fixture.family} document synchronization failed.`,
            );
          }

          const detection = (await query(version, "diagramDetection")) as {
            status?: unknown;
            validity?: unknown;
            diagramType?: unknown;
            syntaxId?: unknown;
            effectiveLayoutId?: unknown;
          };
          if (
            detection?.status !== "available" ||
            detection.validity !== fixture.detection_validity ||
            detection.diagramType !== fixture.family ||
            detection.syntaxId !== fixture.syntax_id ||
            detection.effectiveLayoutId !== fixture.effective_layout_id
          ) {
            throw new Error(
              `${fixture.family} returned inconsistent diagram detection.`,
            );
          }
          await query(version, "diagnostics");
          await query(version, "codeActions");
          await query(version, "completions", {
            position: {
              line: 0,
              character: fixture.source.split("\n", 1)[0]?.length ?? 0,
            },
          });
          const symbols = (await query(version, "documentSymbols")) as Array<{
            selectionRange?: { start?: { line?: number; character?: number } };
          }>;
          const position = symbols[0]?.selectionRange?.start ?? {
            line: 0,
            character: 0,
          };
          await query(version, "hover", { position });
          await query(version, "definition", { position });
          await query(version, "references", {
            position,
            includeDeclaration: true,
          });
          const prepare = (await query(version, "prepareRename", {
            position,
          })) as {
            placeholder?: unknown;
          } | null;
          if (typeof prepare?.placeholder === "string") {
            await query(version, "rename", {
              position,
              newName: prepare.placeholder,
            });
          }

          const tokens = await query(version, "semanticTokens");
          if (!(tokens instanceof Uint32Array) || tokens.length === 0) {
            throw new Error(
              `${fixture.family} returned no packed semantic tokens.`,
            );
          }
          if (tokens.length % recordWidth !== 0) {
            throw new Error(
              `${fixture.family} returned a partial token record.`,
            );
          }
          for (let offset = 0; offset < tokens.length; offset += recordWidth) {
            if (
              tokens[offset + 2] === 0 ||
              tokens[offset + 3] >= tokenTypeNames.length ||
              (tokens[offset + 4] & ~modifierMask) >>> 0 !== 0
            ) {
              throw new Error(
                `${fixture.family} returned an invalid packed token record.`,
              );
            }
          }
          const actualPacked = Array.from(tokens);
          if (
            actualPacked.length !== fixture.packed_words.length ||
            actualPacked.some(
              (word, wordIndex) => word !== fixture.packed_words[wordIndex],
            )
          ) {
            throw new Error(
              `${fixture.family} changed the planner-packed token sequence.`,
            );
          }
          const actualDigest = await packedDigest(tokens);
          if (actualDigest !== fixture.packed_sha256) {
            throw new Error(
              `${fixture.family} changed the planner-packed token digest.`,
            );
          }
          summaries.push({
            family: fixture.family,
            tokenWords: tokens.length,
            packedSha256: actualDigest,
          });
          version += 1;
        }

        const recovery = tokenEquivalenceEvidence.recovery_cases[0];
        if (!recovery) {
          throw new Error("Generated recovery evidence is missing.");
        }
        const recoverySource = recovery.source;
        await request("didChange", {
          document: { uri, version, source: recoverySource },
        });
        const recoveryDetection = (await query(
          version,
          "diagramDetection",
        )) as {
          status?: unknown;
          validity?: unknown;
          diagramType?: unknown;
          syntaxId?: unknown;
          effectiveLayoutId?: unknown;
        };
        const recoveryTokens = await query(version, "semanticTokens");
        if (
          recoveryDetection.status !== "available" ||
          recoveryDetection.validity !== recovery.detection_validity ||
          recoveryDetection.diagramType !== recovery.family ||
          recoveryDetection.syntaxId !== recovery.syntax_id ||
          recoveryDetection.effectiveLayoutId !== recovery.effective_layout_id ||
          !(recoveryTokens instanceof Uint32Array) ||
          Array.from(recoveryTokens).some(
            (word, wordIndex) => word !== recovery.packed_words[wordIndex],
          ) ||
          recoveryTokens.length !== recovery.packed_words.length ||
          (await packedDigest(recoveryTokens)) !== recovery.packed_sha256
        ) {
          throw new Error(
            "Recovered flowchart facts did not survive the packed transport.",
          );
        }

        const emptyVersion = version + 1;
        await request("didChange", {
          document: { uri, version: emptyVersion, source: "" },
        });
        const emptyDiagnostics = (await query(
          emptyVersion,
          "diagnostics",
        )) as {
          version?: unknown;
          valid?: unknown;
          diagnostics?: unknown;
        };
        const noDiagramDiagnostic = Array.isArray(emptyDiagnostics.diagnostics)
          ? emptyDiagnostics.diagnostics.find(
              (diagnostic) =>
                typeof diagnostic === "object" &&
                diagnostic !== null &&
                (diagnostic as { code?: unknown }).code ===
                  "merman.parse.no_diagram" &&
                (
                  diagnostic as {
                    data?: { codeName?: unknown };
                  }
                ).data?.codeName === "MERMAN_NO_DIAGRAM",
            )
          : undefined;
        if (
          emptyDiagnostics.version !== editorSchema ||
          emptyDiagnostics.valid !== false ||
          noDiagramDiagnostic === undefined
        ) {
          throw new Error(
            `Empty editor source did not retain the no-diagram diagnostic: ${JSON.stringify(emptyDiagnostics)}.`,
          );
        }
        emptyDiagnostic = {
          code: (noDiagramDiagnostic as { code?: unknown }).code,
          codeName: (
            noDiagramDiagnostic as {
              data?: { codeName?: unknown };
            }
          ).data?.codeName,
        };
      } finally {
        worker.postMessage({
          protocol,
          type: "dispose",
        });
        worker.terminate();
      }

      return {
        completionTriggerCharacters: ready.completionTriggerCharacters,
        legend: ready.legend,
        summaries,
        emptyDiagnostic,
      };
    },
    {
      digest: SEMANTIC_TOKEN_DESCRIPTOR_DIGEST,
      editorSchema: EDITOR_SCHEMA_VERSION,
      fixtures: familySemanticFixtures,
      modifierMask: SEMANTIC_TOKEN_VALID_MODIFIER_MASK,
      protocol: EDITOR_WORKER_PROTOCOL,
      recordWidth: SEMANTIC_TOKEN_RECORD_WIDTH,
      tokenTypeNames: [...SEMANTIC_TOKEN_TYPE_LSP_NAMES],
      tokenEquivalenceEvidence,
      workerUrl: languageWorkerUrl,
    },
  );

  expect(result.completionTriggerCharacters).toEqual(
    editorCompletionTriggerCharacters,
  );
  expect(result.legend).toEqual({
    tokenTypes: [...SEMANTIC_TOKEN_TYPE_LSP_NAMES],
    tokenModifiers: [...SEMANTIC_TOKEN_MODIFIER_LSP_NAMES],
  });
  expect(result.summaries).toEqual(
    tokenEquivalenceEvidence.family_cases.map((tokenCase) => ({
      family: tokenCase.family,
      tokenWords: tokenCase.packed_words.length,
      packedSha256: tokenCase.packed_sha256,
    })),
  );
  expect(result.emptyDiagnostic).toEqual({
    code: "merman.parse.no_diagram",
    codeName: "MERMAN_NO_DIAGRAM",
  });
});

test("language worker failure keeps the Monaco model editable and Retry reconnects it", async ({
  page,
}) => {
  let rejectLanguageWorker = true;
  let resumeLanguageWorker: (() => void) | undefined;
  await page.route(
    /merman-language\.worker[^/]*\.js(?:\?|$)/i,
    async (route) => {
      if (rejectLanguageWorker) {
        await route.abort("failed");
        return;
      }
      await new Promise<void>((resolve) => {
        resumeLanguageWorker = resolve;
      });
      await route.continue();
    },
  );

  await openPlayground(page);
  const editor = page.locator(".monaco-editor").first();
  await expect(editor).toBeVisible();
  await editor.evaluate((element) => {
    element.setAttribute("data-retry-model-owner", "original");
  });

  const retry = page.getByRole("button", { name: "Retry", exact: true });
  await expect(retry).toBeVisible();
  await replaceEditorSource(page, "flowchart TD\n  Alpha -->");
  await expect(
    editor.locator(".view-line").filter({ hasText: "Alpha" }),
  ).toBeVisible();

  rejectLanguageWorker = false;
  await retry.click();
  await expect(
    page.getByText("Language tools reconnecting", { exact: true }),
  ).toBeVisible();
  await expect.poll(() => Boolean(resumeLanguageWorker)).toBe(true);
  resumeLanguageWorker?.();
  await expect(retry).toBeHidden();
  await expect(editor).toHaveAttribute("data-retry-model-owner", "original");
  await expect(
    editor.locator(".view-line").filter({ hasText: "Alpha" }),
  ).toBeVisible();
  await expect
    .poll(() => page.locator(".monaco-editor .squiggly-error").count())
    .toBeGreaterThan(0);
});

test("an idle language worker failure exposes Retry without another request", async ({
  page,
}) => {
  await page.addInitScript(() => {
    const NativeWorker = window.Worker;
    class ObservableWorker extends NativeWorker {
      constructor(
        scriptURL: string | URL,
        options?: { readonly name?: string },
      ) {
        super(scriptURL, options);
        if (options?.name === "merman-editor-language") {
          Object.defineProperty(window, "__mermanLanguageWorker", {
            configurable: true,
            value: this,
          });
        }
      }
    }
    Object.defineProperty(window, "Worker", {
      configurable: true,
      value: ObservableWorker,
      writable: true,
    });
  });

  await openPlayground(page);
  await expect(
    page.getByText("Language tools initializing", { exact: true }),
  ).toBeHidden();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          "__mermanLanguageWorker" in window &&
          Boolean(
            (window as Window & { __mermanLanguageWorker?: Worker })
              .__mermanLanguageWorker,
          ),
      ),
    )
    .toBe(true);

  await page.evaluate(() => {
    const worker = (window as Window & { __mermanLanguageWorker?: Worker })
      .__mermanLanguageWorker;
    if (!worker) throw new Error("The Merman language worker was not captured.");
    worker.dispatchEvent(
      new ErrorEvent("error", { message: "injected idle failure" }),
    );
  });

  await expect(
    page.getByRole("button", { name: "Retry", exact: true }),
  ).toBeVisible();
  await expect(page.getByText(/injected idle failure/i)).toBeVisible();
});

test("an invalid F2 rename is request-local and diagnostics continue", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  await openPlayground(page);
  await replaceEditorSource(page, "flowchart TD\n  Alpha --> Beta");

  const editorInput = page.getByRole("textbox", { name: "Mermaid source" });
  await editorInput.focus();
  await editorInput.press("ArrowLeft");
  await page.keyboard.press("F2");
  const renameInput = page.locator(".monaco-editor .rename-input");
  await expect(renameInput).toBeVisible();
  await renameInput.fill("bad name");
  await renameInput.press("Enter");

  await expect(
    page
      .getByRole("alert")
      .filter({ hasText: /new name.*MERMAN_INVALID_ARGUMENT/i }),
  ).toBeVisible();
  await expect(
    page.getByText(/Language tools unavailable/i),
  ).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Retry", exact: true })).toBeHidden();

  await replaceEditorSource(page, "flowchart TD\n  Alpha -->");
  await expect
    .poll(() => page.locator(".monaco-editor .squiggly-error").count())
    .toBeGreaterThan(0);
  errors.assertNone();
});

test("the trusted Merman Benchmark entry cannot reach Monaco", async ({
  page,
}) => {
  const requests: string[] = [];
  const workers: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  page.on("worker", (worker) => workers.push(worker.url()));

  await page.goto("./benchmark.html", { waitUntil: "networkidle" });

  expect(
    requests.filter((url) => /monaco|editor\.worker|json\.worker/i.test(url)),
  ).toEqual([]);
  expect(
    workers.filter((url) => /monaco|editor\.worker|json\.worker/i.test(url)),
  ).toEqual([]);
});
