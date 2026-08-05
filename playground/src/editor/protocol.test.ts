import assert from "node:assert/strict";
import test from "node:test";
import {
  EDITOR_SCHEMA_VERSION,
  EDITOR_WORKER_PROTOCOL,
  EditorWorkerProtocolProjectionError,
  projectEditorDocumentIdentity,
  requestIdFromEditorWorkerMessage,
  projectEditorWorkerQuery,
  projectEditorWorkerQueryResult,
  projectEditorWorkerRequest,
  projectEditorWorkerResponse,
  type EditorWorkerQuery,
} from "./protocol.ts";

const position = { line: 1, character: 2 };
const range = { start: position, end: { line: 1, character: 5 } };
const factSource = "parser_complete";
const location = { uri: "inmemory://model/1", factSource, range };
const textEdit = { range, newText: "replacement", factSource };
const workspaceEdit = {
  factSource,
  changes: { "inmemory://model/1": [textEdit] },
};
const diagnostic = {
  range,
  severity: "warning",
  code: "rule-id",
  source: "merman",
  message: "A diagnostic",
  related: [{ message: "Related", range }],
  data: {
    id: "rule-id",
    code: 7,
    codeName: "MERMAN_RULE",
    category: "correctness",
    diagramType: "flowchart",
    help: "Fix it",
    fixes: [
      {
        title: "Apply fix",
        is_preferred: true,
        edits: [
          {
            replacement: "replacement",
            span: {
              byte_start: 0,
              byte_end: 3,
              line: 1,
              column: 1,
              end_line: 1,
              end_column: 4,
              lsp_range: range,
            },
          },
        ],
      },
    ],
  },
};
const diagnostics = {
  version: EDITOR_SCHEMA_VERSION,
  valid: false,
  summary: { errors: 0, warnings: 1, infos: 0, hints: 0 },
  source: {
    kind: "diagram",
    path: null,
    diagram_index: null,
    language: "mermaid",
  },
  diagnostics: [diagnostic],
};
const completionItem = {
  label: "graph",
  kind: "keyword",
  detail: null,
  data: { kind: "diagram_header", label: "graph" },
  insert_text: "graph",
  insert_text_format: "plain_text",
  text_edit: { range, new_text: "graph" },
  label_details: { description: "Flowchart", detail: null },
};
const documentSymbol = {
  name: "node",
  detail: null,
  kind: "variable",
  factSource,
  range,
  selectionRange: range,
  children: [],
};

interface QueryProjectionCase {
  readonly query: EditorWorkerQuery;
  readonly valid: unknown;
  readonly malformed: unknown;
}

const queryProjectionCases: readonly QueryProjectionCase[] = [
  {
    query: { kind: "diagnostics" },
    valid: diagnostics,
    malformed: { ...diagnostics, diagnostics: {} },
  },
  {
    query: { kind: "diagramDetection" },
    valid: {
      status: "available",
      validity: "valid",
      diagramType: "flowchart",
      syntaxId: "flowchart-v2",
      effectiveLayoutId: "dagre",
    },
    malformed: {
      status: "available",
      validity: "unknown",
      diagramType: "flowchart",
      syntaxId: "flowchart-v2",
      effectiveLayoutId: "dagre",
    },
  },
  {
    query: { kind: "codeActions" },
    valid: [
      {
        title: "Apply fix",
        kind: "quickfix",
        diagnostics: [diagnostic],
        edit: workspaceEdit,
        isPreferred: true,
      },
    ],
    malformed: [{ title: "Apply fix", kind: "quickfix" }],
  },
  {
    query: { kind: "completions", position },
    valid: {
      is_incomplete: false,
      fact_source: factSource,
      items: [completionItem],
    },
    malformed: { is_incomplete: false, items: {} },
  },
  {
    query: { kind: "hover", position },
    valid: {
      contents: { kind: "markdown", value: "**node**" },
      factSource,
      range,
    },
    malformed: {
      contents: { kind: "plaintext", value: "node" },
      factSource,
    },
  },
  {
    query: { kind: "documentSymbols" },
    valid: [documentSymbol],
    malformed: [{ ...documentSymbol, children: {} }],
  },
  {
    query: { kind: "definition", position },
    valid: location,
    malformed: { ...location, factSource: "guessed" },
  },
  {
    query: { kind: "references", position, includeDeclaration: true },
    valid: [location],
    malformed: location,
  },
  {
    query: { kind: "prepareRename", position },
    valid: { factSource, range, placeholder: "node" },
    malformed: { factSource, range, placeholder: 3 },
  },
  {
    query: { kind: "rename", position, newName: "renamed" },
    valid: workspaceEdit,
    malformed: { factSource, changes: [] },
  },
  {
    query: { kind: "semanticTokens" },
    valid: new Uint32Array([0, 0, 3, 1, 0]),
    malformed: [0, 0, 3, 1, 0],
  },
];

test("document identity projection owns the URI/version query boundary", () => {
  assert.deepEqual(
    projectEditorDocumentIdentity({ uri: "file:///diagram.mmd", version: 3 }),
    { uri: "file:///diagram.mmd", version: 3 },
  );
  assert.throws(
    () => projectEditorDocumentIdentity({ uri: "", version: 3 }),
    EditorWorkerProtocolProjectionError,
  );
  assert.throws(
    () =>
      projectEditorDocumentIdentity({ uri: "file:///diagram.mmd", version: 0 }),
    EditorWorkerProtocolProjectionError,
  );
});

test("all eleven editor queries and their precise result shapes project", () => {
  for (const { query, valid, malformed } of queryProjectionCases) {
    assert.deepEqual(projectEditorWorkerQuery(query), query);
    assert.throws(
      () => projectEditorWorkerQuery({ ...query, unexpected: true }),
      EditorWorkerProtocolProjectionError,
      `${query.kind} request`,
    );
    assert.doesNotThrow(() => projectEditorWorkerQueryResult(query, valid));
    assert.throws(
      () => projectEditorWorkerQueryResult(query, malformed),
      EditorWorkerProtocolProjectionError,
      query.kind,
    );
  }
});

test("result projections discard unknown extension fields", () => {
  const projected = projectEditorWorkerQueryResult(
    { kind: "diagnostics" },
    {
      ...diagnostics,
      futureTopLevel: true,
      summary: { ...diagnostics.summary, futureSummary: 1 },
      source: { ...diagnostics.source, futureSource: "value" },
      diagnostics: [
        {
          ...diagnostic,
          futureDiagnostic: true,
          range: {
            ...range,
            futureRange: true,
            start: { ...range.start, futurePosition: true },
          },
        },
      ],
    },
  );

  assert.deepEqual(projected, diagnostics);
});

test("diagram detection rejects unknown families and blank identifiers", () => {
  const query = { kind: "diagramDetection" } as const;
  const available = {
    status: "available",
    validity: "valid",
    diagramType: "flowchart",
    syntaxId: "flowchart-v2",
    effectiveLayoutId: "dagre",
  };

  assert.throws(
    () =>
      projectEditorWorkerQueryResult(query, {
        ...available,
        diagramType: "invented-family",
      }),
    /type is invalid/,
  );
  for (const field of ["syntaxId", "effectiveLayoutId"] as const) {
    assert.throws(
      () =>
        projectEditorWorkerQueryResult(query, {
          ...available,
          [field]: "   ",
        }),
      /must not be blank/,
    );
  }
});

test("semantic token projection retains the typed-array transport value", () => {
  const input = new Uint32Array([0, 0, 3, 1, 0]);
  const projected = projectEditorWorkerQueryResult(
    { kind: "semanticTokens" },
    input,
  );

  assert.ok(projected instanceof Uint32Array);
  assert.deepEqual(projected, input);
  assert.equal(projected, input);
});

test("nullable editor query results remain valid without inventing data", () => {
  for (const query of [
    { kind: "hover", position },
    { kind: "definition", position },
    { kind: "prepareRename", position },
    { kind: "rename", position, newName: "renamed" },
  ] as const) {
    assert.equal(projectEditorWorkerQueryResult(query, null), null);
  }
});

test("workspace edit URI keys remain inert data properties", () => {
  const changes = Object.create(null) as Record<string, unknown>;
  Object.defineProperty(changes, "__proto__", {
    enumerable: true,
    value: [textEdit],
  });

  const projected = projectEditorWorkerQueryResult(
    { kind: "rename", position, newName: "renamed" },
    { changes },
  );

  assert(projected);
  assert.equal(Object.getPrototypeOf(projected.changes), Object.prototype);
  assert.deepEqual(Object.keys(projected.changes), ["__proto__"]);
  assert.deepEqual(projected.changes.__proto__, [textEdit]);
});

test("optional undefined fields project as absent instead of failing the boundary", () => {
  const projected = projectEditorWorkerQueryResult(
    { kind: "completions", position },
    {
      is_incomplete: false,
      fact_source: undefined,
      items: [{ label: "graph", kind: "keyword", detail: undefined }],
    },
  );

  assert.deepEqual(projected, {
    is_incomplete: false,
    items: [{ label: "graph", kind: "keyword" }],
  });
});

test("request projection validates every envelope and query request shape", () => {
  const document = {
    uri: "inmemory://model/1",
    version: 1,
    source: "flowchart LR\nA-->B",
  };
  const requests: unknown[] = [
    {
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: 1,
      type: "initialize",
    },
    {
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: 2,
      type: "didOpen",
      document,
    },
    {
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: 3,
      type: "didChange",
      document: { ...document, version: 2 },
    },
    ...queryProjectionCases.map(({ query }, index) => ({
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: index + 4,
      type: "query",
      uri: document.uri,
      version: 2,
      legendDigest: "legend-digest",
      query,
    })),
    { protocol: EDITOR_WORKER_PROTOCOL, type: "dispose" },
  ];

  for (const request of requests) {
    assert.deepEqual(projectEditorWorkerRequest(request), request);
  }

  assert.throws(
    () =>
      projectEditorWorkerRequest({
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId: 1,
        type: "query",
        uri: document.uri,
        version: 2,
        legendDigest: "legend-digest",
        query: { kind: "hover", position: { line: -1, character: 0 } },
      }),
    /non-negative safe integer/,
  );
  assert.throws(
    () =>
      projectEditorWorkerRequest({
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId: 1,
        type: "initialize",
        extra: true,
      }),
    /unexpected or missing fields/,
  );
  assert.throws(
    () =>
      projectEditorWorkerRequest({
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId: 2,
        type: "query",
        uri: document.uri,
        version: 2,
        legendDigest: "legend-digest",
        query: {
          kind: "hover",
          position: { line: 0, character: 0, extra: true },
        },
      }),
    /unexpected or missing fields/,
  );
});

test("response projection binds positive request IDs and null synchronization acks", () => {
  const ready = projectEditorWorkerResponse({
    protocol: EDITOR_WORKER_PROTOCOL,
    requestId: 1,
    type: "ready",
    transportApiVersion: 3,
    editorSchema: EDITOR_SCHEMA_VERSION,
    legendDigest: "legend-digest",
    legend: { tokenTypes: ["keyword"], tokenModifiers: [] },
  });
  assert.equal(ready.type, "ready");

  assert.deepEqual(
    projectEditorWorkerResponse({
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: 2,
      type: "result",
      result: null,
    }),
    {
      protocol: EDITOR_WORKER_PROTOCOL,
      requestId: 2,
      type: "result",
      result: null,
    },
  );
  assert.throws(
    () =>
      projectEditorWorkerResponse({
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId: 2,
        type: "result",
        result: undefined,
      }),
    /must be null/,
  );
  assert.throws(
    () =>
      projectEditorWorkerResponse({
        protocol: EDITOR_WORKER_PROTOCOL,
        requestId: 0,
        type: "result",
        result: null,
      }),
    /positive safe integer/,
  );
});

test("query response projection validates the result against its request kind", () => {
  const response = {
    protocol: EDITOR_WORKER_PROTOCOL,
    requestId: 3,
    type: "queryResult",
    uri: "inmemory://model/1",
    version: 2,
    legendDigest: "legend-digest",
    result: new Uint32Array([0, 0, 3, 1, 0]),
  };
  const projected = projectEditorWorkerResponse(response);
  assert.equal(projected.type, "queryResult");
  assert.ok(
    projectEditorWorkerQueryResult(
      { kind: "semanticTokens" },
      projected.result,
    ) instanceof Uint32Array,
  );
  assert.throws(
    () => {
      const malformed = projectEditorWorkerResponse({
        ...response,
        result: [0, 0, 3, 1, 0],
      });
      assert.equal(malformed.type, "queryResult");
      projectEditorWorkerQueryResult(
        { kind: "semanticTokens" },
        malformed.result,
      );
    },
    /Uint32Array/,
  );
});

test("malformed-message request ID recovery only accepts positive safe integers", () => {
  assert.equal(requestIdFromEditorWorkerMessage({ requestId: 17 }), 17);
  assert.equal(requestIdFromEditorWorkerMessage({ requestId: 0 }), null);
  assert.equal(requestIdFromEditorWorkerMessage({ requestId: -1 }), null);
  assert.equal(requestIdFromEditorWorkerMessage({ requestId: "17" }), null);
  assert.equal(requestIdFromEditorWorkerMessage(null), null);
});
