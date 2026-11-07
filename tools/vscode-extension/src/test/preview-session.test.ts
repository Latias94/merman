import * as assert from "node:assert/strict";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import { PreviewSession } from "../preview-session.js";
import { pairedThemeDiscovery } from "./theme-fixtures.js";

describe("preview session", () => {
  it("keeps configured and panel-owned theme files distinct across selection changes and reset", () => {
    const session = new PreviewSession({ diagramTheme: "file" });
    const editor = textEditor("file:///workspace/example.mmd", "example.mmd", "flowchart TD\nA --> B\n");
    const configured = {
      content: "configured-recipe", label: "configured.theme.json",
      supportedAppearances: ["light", "dark"] as Array<"light" | "dark">,
      defaultAppearance: "light" as const,
    };
    const imported = { ...configured, content: "panel-recipe", label: "panel.theme.json" };
    session.configureThemes(pairedThemeDiscovery(), "light");
    assert.equal(session.usesConfiguredThemeFile, true);
    assert.equal(session.setConfiguredThemeFile("workspace-key", configured), true);
    assert.equal(session.configuredThemeFileKey, "workspace-key");
    session.setDiagramTheme("source");
    assert.equal(session.usesConfiguredThemeFile, false);
    assert.equal(session.setConfiguredThemeFile("obsolete-key", imported), false);
    session.setDiagramTheme("file");
    assert.equal(session.usesConfiguredThemeFile, true);
    session.importTheme(imported);
    assert.equal(session.usesConfiguredThemeFile, false);
    assert.equal(session.configuredThemeFileKey, undefined);
    assert.equal(session.setConfiguredThemeFile("later-key", configured), false);
    session.setDiagramTheme("source");
    session.setDiagramTheme("file");
    assert.equal(session.createSnapshot(editor, [editor], emptyDiagnostics)?.resolvedTheme?.themeFileContent, "panel-recipe");
    session.reset();
    assert.equal(session.usesConfiguredThemeFile, true);
    assert.equal(session.configuredThemeFileKey, undefined);
  });

  it("peeks at configuration sources without consuming the explicit target before snapshot creation", () => {
    const session = new PreviewSession();
    const active = textEditor("file:///workspace-one/active.mmd", "active.mmd", "flowchart TD\nA --> B\n");
    const opening = textEditor("file:///workspace-two/opening.mmd", "opening.mmd", "sequenceDiagram\nA->>B: hi\n");
    session.rememberResource(opening.document.uri, { preferOnce: true });
    assert.equal(session.resolveConfigurationResource(active, [active]), active.document.uri.toString());
    assert.equal(session.resolveConfigurationResource(active, [active, opening]), opening.document.uri.toString());
    assert.equal(session.createSnapshot(active, [active, opening], emptyDiagnostics)?.documentUri, opening.document.uri.toString());
    assert.equal(session.resolveConfigurationResource(active, [active, opening]), active.document.uri.toString());
  });

  it("consumes an unavailable preferred source when a fallback snapshot is created", () => {
    for (const hidden of [false, true]) {
      const session = new PreviewSession();
      const fallback = textEditor("file:///workspace/active.mmd", "active.mmd", "flowchart TD\nA --> B\n");
      const invalid = textEditor("file:///workspace/target.mmd", "target.mmd", "", "mermaid", 0, 1);
      const valid = textEditor("file:///workspace/target.mmd", "target.mmd", "sequenceDiagram\nA->>B: hi\n", "mermaid", 0, 2);
      const visible = hidden ? [fallback] : [fallback, invalid];
      session.rememberResource(invalid.document.uri, { preferOnce: true });
      assert.equal(session.resolveConfigurationResource(fallback, visible), fallback.document.uri.toString());
      const snapshot = assertDefined(session.createSnapshot(fallback, visible, emptyDiagnostics));
      assert.equal(snapshot.documentUri, fallback.document.uri.toString());
      session.rememberSnapshot(snapshot);
      assert.equal(session.resolveConfigurationResource(fallback, [fallback, valid]), fallback.document.uri.toString());
      assert.equal(session.createSnapshot(fallback, [fallback, valid], emptyDiagnostics)?.documentUri, fallback.document.uri.toString());
    }
  });

  it("remembers editor appearance changes before a configured file has completed admission", () => {
    const session = new PreviewSession({ diagramTheme: "file" });
    const editor = textEditor("file:///workspace/example.mmd", "example.mmd", "flowchart TD\nA --> B\n");
    session.configureThemes(pairedThemeDiscovery(), "light");
    assert.equal(session.setHostAppearance("dark"), false);
    session.setConfiguredThemeFile("workspace-key", {
      content: "configured-recipe",
      label: "configured.theme.json",
      supportedAppearances: ["light", "dark"],
      defaultAppearance: "light",
    });
    assert.equal(session.createSnapshot(editor, [editor], emptyDiagnostics)?.resolvedTheme?.effectiveAppearance, "dark");
  });

  it("retains a pinned appearance while Source/default temporarily owns the diagram appearance", () => {
    const session = new PreviewSession();
    const editor = textEditor("file:///workspace/example.mmd", "example.mmd", "flowchart TD\nA --> B\n");
    session.configureThemes(pairedThemeDiscovery(), "light");
    session.setDiagramTheme("preset:editor");
    session.setAppearance("dark");

    session.setDiagramTheme("source");
    let snapshot = assertDefined(session.createSnapshot(editor, [editor], emptyDiagnostics));
    assert.equal(snapshot.appearancePreference, "dark");
    assert.equal(snapshot.resolvedTheme?.effectiveAppearance, undefined);
    assert.equal(snapshot.resolvedTheme?.theme, undefined);
    assert.equal(session.setHostAppearance("dark"), false);
    assert.equal(session.setHostAppearance("light"), false);

    session.setDiagramTheme("preset:editor");
    snapshot = assertDefined(session.createSnapshot(editor, [editor], emptyDiagnostics));
    assert.equal(snapshot.appearancePreference, "dark");
    assert.equal(snapshot.resolvedTheme?.effectiveAppearance, "dark");
  });

  it("retains a pinned appearance across an imported unclassified recipe and source changes", () => {
    const session = new PreviewSession();
    const first = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const second = textEditor("file:///other-workspace/two.mmd", "two.mmd", "sequenceDiagram\nA->>B: hi\n");
    const content = '{"schema_version":1,"kind":"design","design":{"canvas":{"base":"#fff"}}}';
    session.configureThemes(pairedThemeDiscovery(), "dark");
    session.setDiagramTheme("preset:editor");
    session.setAppearance("light");
    session.importTheme({ content, label: "unclassified.theme.json", supportedAppearances: [] });

    let snapshot = assertDefined(session.createSnapshot(first, [first], emptyDiagnostics));
    assert.equal(snapshot.appearancePreference, "light");
    assert.equal(snapshot.resolvedTheme?.effectiveAppearance, undefined);
    assert.equal(snapshot.resolvedTheme?.themeFileContent, content);
    session.rememberSnapshot(snapshot);
    snapshot = assertDefined(session.createSnapshot(second, [first, second], emptyDiagnostics));
    assert.equal(snapshot.documentUri, second.document.uri.toString());
    assert.equal(snapshot.resolvedTheme?.themeFileContent, content);
    assert.equal(session.setHostAppearance("light"), false);
    assert.equal(session.setHostAppearance("dark"), false);

    session.setDiagramTheme("preset:editor");
    snapshot = assertDefined(session.createSnapshot(second, [second], emptyDiagnostics));
    assert.equal(snapshot.appearancePreference, "light");
    assert.equal(snapshot.resolvedTheme?.effectiveAppearance, "light");
  });

  it("uses the latest open-document body for hidden locked sources during an appearance refresh", () => {
    const session = new PreviewSession();
    const original = textEditor("file:///workspace/notes.md", "notes.md", markdownWithTwoFences(), "markdown", 7, 1);
    const edited = textEditor("file:///workspace/notes.md", "notes.md", markdownWithEditedSecondFence(), "markdown", 7, 2);
    session.rememberResource(original.document.uri);
    const first = session.createSnapshot(original, [original], emptyDiagnostics);
    session.rememberSnapshot(assertDefined(first));
    session.setLocked(true);
    session.trackDocumentChange(documentChange(edited.document, 7, 7, "Alice->>Bob: hello"));
    const hidden = session.createSnapshot(undefined, [], emptyDiagnostics, true, [edited.document]);
    assert.equal(hidden?.input.source, "sequenceDiagram\nAlice->>Bob: hello");
    assert.equal(hidden?.documentVersion, 2);
    assert.equal(hidden?.input.sourceId, first?.input.sourceId);
    assert.notEqual(hidden?.sourceKey.sourceHash, first?.sourceKey.sourceHash);
    assert.equal(hidden?.locked, true);
  });

  it("uses configured preview defaults for new snapshots and reset", () => {
    const session = new PreviewSession({
      diagramTheme: "dark",
      displayMode: "unicode",
      background: "transparent",
    });
    const editor = textEditor("file:///workspace/example.mmd", "example.mmd", "flowchart TD\nA --> B\n");

    session.rememberResource(editor.document.uri);
    let snapshot = session.createSnapshot(undefined, [editor], emptyDiagnostics);

    assert.equal(snapshot?.diagramTheme, "dark");
    assert.equal(snapshot?.displayMode, "unicode");
    assert.equal(snapshot?.background, "transparent");

    session.setBackground("paper");
    session.reset();
    session.rememberResource(editor.document.uri);
    snapshot = session.createSnapshot(undefined, [editor], emptyDiagnostics);

    assert.equal(snapshot?.background, "transparent");
  });

  it("resolves a remembered Mermaid resource after the webview takes active editor focus", () => {
    const session = new PreviewSession();
    const editor = textEditor("file:///workspace/example.mmd", "example.mmd", "flowchart TD\nA --> B\n");

    session.rememberResource(editor.document.uri);

    const snapshot = session.createSnapshot(undefined, [editor], () => ({
      summary: "0 errors, 0 warnings, 0 infos, 0 hints",
      totalCount: 0,
    }));

    assert.equal(snapshot?.input.kind, "mermaid-file");
    assert.equal(snapshot?.input.sourceId, "document");
    assert.equal(snapshot?.documentUri, "file:///workspace/example.mmd");
    assert.equal(snapshot?.background, "theme");
  });

  it("clears an explicit source selection so preview can follow the cursor again", () => {
    const session = new PreviewSession();
    const reads = { lineAt: 0 };
    const editor = textEditor(
      "file:///workspace/notes.md",
      "notes.md",
      [
        "# Notes",
        "",
        "```mermaid",
        "flowchart TD",
        "A --> B",
        "```",
        "",
        "```mermaid",
        "sequenceDiagram",
        "Alice->>Bob: hi",
        "```",
      ].join("\n"),
      "markdown",
      3,
      1,
      reads,
    );

    session.rememberResource(editor.document.uri);
    assert.equal(session.selectSource(editor, [editor], "fence-2"), true);

    let snapshot = session.createSnapshot(editor, [editor], emptyDiagnostics);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.selected, true);

    session.clearSelectedSource();
    snapshot = session.createSnapshot(editor, [editor], emptyDiagnostics);

    assert.equal(snapshot?.input.sourceId, "fence-1");
    assert.equal(snapshot?.selected, false);
    assert.equal(reads.lineAt, editor.document.lineCount);
  });

  it("resolves editor, source facts, and cursor selection together without rescanning", () => {
    const session = new PreviewSession();
    const reads = { lineAt: 0 };
    const editor = textEditor(
      "file:///workspace/notes.md",
      "notes.md",
      markdownWithTwoFences(),
      "markdown",
      1,
      1,
      reads,
    );
    session.rememberResource(editor.document.uri);

    const source = session.resolveSource(editor, [editor]);
    assert.equal(source?.editor, editor);
    assert.equal(source?.input.sourceId, "fence-1");
    assert.equal(source?.selected, false);
    assert.equal(source?.inputs.length, 2);

    const movedCursor = {
      ...editor,
      selection: { active: { line: 7 } },
    } as unknown as vscode.TextEditor;
    const snapshot = session.createSnapshot(movedCursor, [movedCursor], emptyDiagnostics);

    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.selectionLine, 7);
    assert.equal(snapshot?.selected, false);
    assert.equal(snapshot?.sources, source?.inputs);
    assert.equal(reads.lineAt, editor.document.lineCount);
  });

  it("shares document facts across sessions without sharing panel state", () => {
    const lockedSession = new PreviewSession();
    const followSession = new PreviewSession();
    const reads = { lineAt: 0 };
    const editor = textEditor(
      "file:///workspace/notes.md",
      "notes.md",
      markdownWithTwoFences(),
      "markdown",
      1,
      1,
      reads,
    );
    lockedSession.rememberResource(editor.document.uri);
    const original = assertDefined(lockedSession.createSnapshot(editor, [editor], emptyDiagnostics));
    lockedSession.rememberSnapshot(original);
    lockedSession.setLocked(true);
    lockedSession.setBackground("dark");
    lockedSession.setDiagramTheme("forest");

    const movedCursor = {
      ...editor,
      selection: { active: { line: 7 } },
    } as unknown as vscode.TextEditor;
    const followed = assertDefined(followSession.createSnapshot(movedCursor, [movedCursor], emptyDiagnostics));
    const locked = assertDefined(lockedSession.createSnapshot(movedCursor, [movedCursor], emptyDiagnostics));

    assert.equal(locked.sources, followed.sources);
    assert.equal(locked.input.sourceId, "fence-1");
    assert.equal(locked.locked, true);
    assert.equal(locked.selected, true);
    assert.equal(locked.background, "dark");
    assert.equal(locked.diagramTheme, "forest");
    assert.equal(followed.input.sourceId, "fence-2");
    assert.equal(followed.locked, false);
    assert.equal(followed.selected, false);
    assert.equal(followed.background, "theme");
    assert.equal(followed.diagramTheme, "source");
    assert.equal(original.locked, false);
    assert.equal(original.background, "theme");
    assert.equal(reads.lineAt, editor.document.lineCount);
  });

  it("prefers an explicitly opened resource once without disabling follow mode", () => {
    const session = new PreviewSession();
    const first = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const second = textEditor("file:///workspace/two.mmd", "two.mmd", "sequenceDiagram\nA->>B: hi\n");

    session.rememberResource(second.document.uri, { preferOnce: true });

    let snapshot = session.createSnapshot(first, [first, second], emptyDiagnostics);
    assert.equal(snapshot?.documentUri, "file:///workspace/two.mmd");

    snapshot = session.createSnapshot(first, [first, second], emptyDiagnostics);
    assert.equal(snapshot?.documentUri, "file:///workspace/one.mmd");
  });

  it("prefers an explicitly opened Markdown source once without disabling follow mode", () => {
    const session = new PreviewSession();
    const active = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const target = textEditor(
      "file:///workspace/notes.md",
      "notes.md",
      markdownWithTwoFences(),
      "markdown",
      7,
    );

    session.rememberResource(target.document.uri, { preferOnce: true });
    assert.equal(session.selectSource(target, [active, target], "fence-2"), true);

    let snapshot = session.createSnapshot(active, [active, target], emptyDiagnostics);
    assert.equal(snapshot?.documentUri, "file:///workspace/notes.md");
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.selected, true);

    snapshot = session.createSnapshot(active, [active, target], emptyDiagnostics);
    assert.equal(snapshot?.documentUri, "file:///workspace/one.mmd");
    assert.equal(snapshot?.input.sourceId, "document");
    assert.equal(snapshot?.selected, false);
  });

  it("keeps a locked preview on the remembered source instead of following the active editor", () => {
    const session = new PreviewSession();
    const first = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const second = textEditor("file:///workspace/two.mmd", "two.mmd", "sequenceDiagram\nA->>B: hi\n");

    session.rememberResource(first.document.uri);
    const initial = session.createSnapshot(first, [first], emptyDiagnostics);
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const snapshot = session.createSnapshot(second, [first, second], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, "file:///workspace/one.mmd");
    assert.equal(snapshot?.locked, true);
  });

  it("keeps a locked preview on the last snapshot when the source editor is no longer visible", () => {
    const session = new PreviewSession();
    const first = textEditor("file:///workspace/one.mmd", "one.mmd", "flowchart TD\nA --> B\n");
    const second = textEditor("file:///workspace/two.mmd", "two.mmd", "sequenceDiagram\nA->>B: hi\n");

    session.rememberResource(first.document.uri);
    const initial = session.createSnapshot(first, [first], emptyDiagnostics);
    assert.ok(initial);
    session.rememberSnapshot(initial);
    assert.equal(session.setLocked(true), true);
    assert.equal(session.setDiagramTheme("dark"), true);

    const snapshot = session.createSnapshot(second, [second], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, "file:///workspace/one.mmd");
    assert.equal(snapshot?.input.source, "flowchart TD\nA --> B\n");
    assert.equal(snapshot?.diagramTheme, "dark");
    assert.equal(snapshot?.sourceKey.diagramTheme, "dark");
    assert.equal(snapshot?.locked, true);
  });

  it("keeps a locked Markdown fence on the last snapshot when the selected fence disappears", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7);
    const edited = textEditor(
      uri,
      "notes.md",
      ["```mermaid", "flowchart TD", "A --> B", "```"].join("\n"),
      "markdown",
      1,
    );

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi");
    assert.equal(snapshot?.locked, true);
  });

  it("keeps a locked Markdown fence on the original body after a new fence is inserted before it", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7);
    const edited = textEditor(uri, "notes.md", markdownWithInsertedFence(), "markdown", 1);

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-3");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi");
    assert.equal(snapshot?.locked, true);
  });

  it("keeps the locked snapshot when the selected fence is deleted but its old ordinal still exists", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7);
    const edited = textEditor(uri, "notes.md", markdownWithReplacementSecondFence(), "markdown", 7);

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi");
    assert.equal(snapshot?.locked, true);
  });

  it("updates a locked Markdown fence after a tracked body edit", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7, 1);
    const edited = textEditor(uri, "notes.md", markdownWithEditedSecondFence(), "markdown", 7, 2);

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    session.trackDocumentChange(documentChange(edited.document, 7, 7, "Alice->>Bob: hello"));
    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nAlice->>Bob: hello");
    assert.equal(snapshot?.documentVersion, 2);
    assert.equal(snapshot?.locked, true);
    assert.equal(snapshot?.selected, true);
  });

  it("keeps the locked snapshot when a tracked Markdown edit skips a document version", () => {
    const session = new PreviewSession();
    const followSession = new PreviewSession();
    const reads = { lineAt: 0 };
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7, 1);
    const edited = textEditor(uri, "notes.md", markdownWithEditedSecondFence(), "markdown", 7, 3, reads);

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    const followed = followSession.createSnapshot(edited, [edited], emptyDiagnostics);
    assert.equal(followed?.documentVersion, 3);
    assert.equal(followed?.input.source, "sequenceDiagram\nAlice->>Bob: hello");

    session.trackDocumentChange(documentChange(edited.document, 7, 7, "Alice->>Bob: hello"));
    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi");
    assert.equal(snapshot?.documentVersion, 1);
    assert.equal(snapshot?.locked, true);
    assert.notEqual(snapshot?.sources, followed?.sources);
    assert.equal(reads.lineAt, edited.document.lineCount);
  });

  it("updates a locked Markdown fence range after a tracked body line insertion", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7, 1);
    const edited = textEditor(uri, "notes.md", markdownWithInsertedSecondFenceLine(), "markdown", 7, 2);

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    session.trackDocumentChange(documentChange(edited.document, 8, 8, "Bob-->>Alice: ok\n"));
    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi\nBob-->>Alice: ok");
    assert.deepEqual(snapshot?.input.sourceRange, { startLine: 5, endLine: 9 });
    assert.equal(snapshot?.locked, true);
    assert.equal(snapshot?.selected, true);
  });

  it("tracks a locked Markdown fence across a preceding fence insertion before body edit", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7, 1);
    const inserted = textEditor(uri, "notes.md", markdownWithInsertedFence(), "markdown", 12, 2);
    const edited = textEditor(
      uri,
      "notes.md",
      markdownWithInsertedFenceAndEditedOriginalSecondFence(),
      "markdown",
      12,
      3,
    );

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    session.trackDocumentChange(
      documentChange(inserted.document, 5, 5, "```mermaid\nstateDiagram-v2\n[*] --> Inserted\n```\n\n"),
    );
    const afterInsert = session.createSnapshot(inserted, [inserted], emptyDiagnostics);
    assert.equal(afterInsert?.input.sourceId, "fence-3");
    assert.equal(afterInsert?.input.source, "sequenceDiagram\nA->>B: hi");
    session.rememberSnapshot(assertDefined(afterInsert));

    session.trackDocumentChange(documentChange(edited.document, 12, 12, "Alice->>Bob: hello"));
    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-3");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nAlice->>Bob: hello");
    assert.deepEqual(snapshot?.input.sourceRange, { startLine: 10, endLine: 13 });
    assert.equal(snapshot?.locked, true);
    assert.equal(snapshot?.selected, true);
  });

  it("keeps the locked snapshot when a tracked change replaces the fence delimiters", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 7, 1);
    const edited = textEditor(uri, "notes.md", markdownWithReplacementSecondFence(), "markdown", 7, 2);

    session.rememberResource(original.document.uri);
    assert.equal(session.selectSource(original, [original], "fence-2"), true);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    session.trackDocumentChange(
      documentChange(edited.document, 5, 8, "```mermaid\npie title Replacement\n  \"A\" : 1\n```"),
    );
    const snapshot = session.createSnapshot(edited, [edited], emptyDiagnostics);

    assert.equal(snapshot?.documentUri, uri);
    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.input.source, "sequenceDiagram\nA->>B: hi");
    assert.equal(snapshot?.documentVersion, 1);
    assert.equal(snapshot?.locked, true);
  });

  it("locks the current Markdown fence even when it was selected by cursor position", () => {
    const session = new PreviewSession();
    const uri = "file:///workspace/notes.md";
    const original = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 6);
    const editedOnOtherFence = textEditor(uri, "notes.md", markdownWithTwoFences(), "markdown", 1);

    session.rememberResource(original.document.uri);
    const initial = session.createSnapshot(original, [original], emptyDiagnostics);
    assert.equal(initial?.input.sourceId, "fence-2");
    session.rememberSnapshot(assertDefined(initial));
    assert.equal(session.setLocked(true), true);

    let snapshot = session.createSnapshot(editedOnOtherFence, [editedOnOtherFence], emptyDiagnostics);

    assert.equal(snapshot?.input.sourceId, "fence-2");
    assert.equal(snapshot?.locked, true);
    assert.equal(snapshot?.selected, true);

    assert.equal(session.setLocked(false), true);
    snapshot = session.createSnapshot(editedOnOtherFence, [editedOnOtherFence], emptyDiagnostics);

    assert.equal(snapshot?.input.sourceId, "fence-1");
    assert.equal(snapshot?.locked, false);
    assert.equal(snapshot?.selected, false);
  });
});

function emptyDiagnostics() {
  return {
    summary: "0 errors, 0 warnings, 0 infos, 0 hints",
    totalCount: 0,
  };
}

function textEditor(
  uri: string,
  fileName: string,
  text: string,
  languageId = "mermaid",
  activeLine = 0,
  version = 1,
  reads?: { lineAt: number },
): vscode.TextEditor {
  const lines = text.split(/\r?\n/);
  return {
    document: {
      uri: {
        fsPath: fileName,
        toString: () => uri,
      },
      languageId,
      fileName,
      version,
      lineCount: lines.length,
      getText: () => {
        assert.notEqual(languageId, "markdown", "Markdown extraction must use lineAt");
        return text;
      },
      lineAt: (lineIndex: number) => {
        if (reads) {
          reads.lineAt += 1;
        }
        return { text: lines[lineIndex] ?? "" };
      },
    },
    selection: {
      active: {
        line: activeLine,
      },
    },
  } as unknown as vscode.TextEditor;
}

function documentChange(
  document: vscode.TextDocument,
  startLine: number,
  endLine: number,
  text: string,
): vscode.TextDocumentChangeEvent {
  return {
    document,
    contentChanges: [
      {
        range: {
          start: { line: startLine, character: 0 },
          end: { line: endLine, character: 0 },
        },
        rangeOffset: 0,
        rangeLength: 0,
        text,
      },
    ],
  } as unknown as vscode.TextDocumentChangeEvent;
}

function markdownWithTwoFences(): string {
  return [
    "```mermaid",
    "flowchart TD",
    "A --> B",
    "```",
    "",
    "```mermaid",
    "sequenceDiagram",
    "A->>B: hi",
    "```",
  ].join("\n");
}

function markdownWithEditedSecondFence(): string {
  return [
    "```mermaid",
    "flowchart TD",
    "A --> B",
    "```",
    "",
    "```mermaid",
    "sequenceDiagram",
    "Alice->>Bob: hello",
    "```",
  ].join("\n");
}

function markdownWithInsertedSecondFenceLine(): string {
  return [
    "```mermaid",
    "flowchart TD",
    "A --> B",
    "```",
    "",
    "```mermaid",
    "sequenceDiagram",
    "A->>B: hi",
    "Bob-->>Alice: ok",
    "```",
  ].join("\n");
}

function markdownWithInsertedFence(): string {
  return [
    "```mermaid",
    "flowchart TD",
    "A --> B",
    "```",
    "",
    "```mermaid",
    "stateDiagram-v2",
    "[*] --> Inserted",
    "```",
    "",
    "```mermaid",
    "sequenceDiagram",
    "A->>B: hi",
    "```",
  ].join("\n");
}

function markdownWithInsertedFenceAndEditedOriginalSecondFence(): string {
  return [
    "```mermaid",
    "flowchart TD",
    "A --> B",
    "```",
    "",
    "```mermaid",
    "stateDiagram-v2",
    "[*] --> Inserted",
    "```",
    "",
    "```mermaid",
    "sequenceDiagram",
    "Alice->>Bob: hello",
    "```",
  ].join("\n");
}

function markdownWithReplacementSecondFence(): string {
  return [
    "```mermaid",
    "flowchart TD",
    "A --> B",
    "```",
    "",
    "```mermaid",
    "pie title Replacement",
    "  \"A\" : 1",
    "```",
  ].join("\n");
}

function assertDefined<T>(value: T | undefined): T {
  assert.ok(value);
  return value;
}
