import assert from "node:assert/strict";
import test from "node:test";

import {
  DEFAULT_WORKSPACE_SNAPSHOT,
  selectWorkspaceSnapshot,
  useAppStore,
  type WorkspaceSnapshot,
} from "./index.ts";
import type { StartupShareHydration } from "../lib/share-view.ts";

test("native theme selection clears its legacy host base", () => {
  useAppStore.setState({
    diagramTheme: "forest",
    themePresetId: null,
    svgPipeline: "parity",
  });

  useAppStore.getState().selectTheme({ kind: "preset", presetId: "future-theme" });
  assert.deepEqual(renderingState(), {
    diagramTheme: "auto",
    themePresetId: "future-theme",
    svgPipeline: "parity",
  });

  useAppStore.getState().setSvgPipeline("readable");
  assert.deepEqual(renderingState(), {
    diagramTheme: "auto",
    themePresetId: "future-theme",
    svgPipeline: "readable",
  });

  useAppStore.getState().selectTheme({ kind: "mermaid", theme: "dark" });
  assert.deepEqual(renderingState(), {
    diagramTheme: "dark",
    themePresetId: null,
    svgPipeline: "readable",
  });
});

test("applies one complete workspace snapshot with one coherent notification", () => {
  const next: WorkspaceSnapshot = {
    code: "sequenceDiagram\nA->>B: hello",
    mermaidConfig: '{"look":"neo"}',
    diagramTheme: "forest",
    themePresetId: "future-theme",
    themeDocumentJson: null,
    svgPipeline: "readable",
    textMeasurementMode: "headless",
    diagramFont: "arial",
  };
  const notifications: WorkspaceSnapshot[] = [];
  const unsubscribe = useAppStore.subscribe((state) => {
    notifications.push(selectWorkspaceSnapshot(state));
  });

  useAppStore.getState().applyWorkspaceSnapshot(next);
  unsubscribe();

  assert.deepEqual(notifications, [next]);
  assert.deepEqual(selectWorkspaceSnapshot(useAppStore.getState()), next);
});

test("document replacement advances the preview identity even with identical source", () => {
  const initial = useAppStore.getState();
  const revision = initial.documentRevision;
  initial.setCode("flowchart LR\nA --> B");
  const edited = useAppStore.getState();
  assert.equal(edited.documentRevision, revision);

  edited.replaceDocument(edited.code);
  const reopened = useAppStore.getState();
  assert.equal(reopened.documentRevision, revision + 1);
  assert.equal(reopened.code, edited.code);
  assert.ok(!("documentRevision" in selectWorkspaceSnapshot(reopened)));

  reopened.applyWorkspaceSnapshot(selectWorkspaceSnapshot(reopened));
  assert.equal(useAppStore.getState().documentRevision, revision + 2);
});

test("stores Preview-owned SVG presentation preferences outside workspace snapshots", () => {
  useAppStore.setState({
    showSvgBounds: false,
    svgPresentationMode: "infinite",
  });

  useAppStore.getState().setShowSvgBounds(true);
  useAppStore.getState().setSvgPresentationMode("viewbox");
  useAppStore.getState().setPreviewBacking("transparency");

  assert.equal(useAppStore.getState().showSvgBounds, true);
  assert.equal(useAppStore.getState().svgPresentationMode, "viewbox");
  assert.equal(useAppStore.getState().previewBacking, "transparency");
  assert.equal("previewBacking" in selectWorkspaceSnapshot(useAppStore.getState()), false);
  assert.equal("showSvgBounds" in selectWorkspaceSnapshot(useAppStore.getState()), false);
  assert.equal(
    "svgPresentationMode" in selectWorkspaceSnapshot(useAppStore.getState()),
    false,
  );
});

test("applies startup workspace, view preferences, and warning in one store transition", () => {
  const hydration: StartupShareHydration = {
    workspace: {
      code: "flowchart TD\nA --> B",
      mermaidConfig: '{"look":"neo"}',
      diagramTheme: "forest",
      diagramFont: "arial",
      themePresetId: "future-theme",
      themeDocumentJson: null,
      svgPipeline: "readable",
      textMeasurementMode: "headless",
    },
    view: {
      workspacePane: "preview",
      editorMode: "config",
      previewMode: "compare",
      showSvgBounds: true,
      svgPresentationMode: "viewbox",
    },
    warning: {
      code: "share-view-not-restored",
      message: "warning",
    },
  };
  const notifications: Array<Record<string, unknown>> = [];
  const unsubscribe = useAppStore.subscribe((state) => {
    notifications.push({
      workspace: selectWorkspaceSnapshot(state),
      workspacePane: state.workspacePane,
      editorMode: state.editorMode,
      previewMode: state.previewMode,
      showSvgBounds: state.showSvgBounds,
      svgPresentationMode: state.svgPresentationMode,
      shareViewWarning: state.shareViewWarning,
    });
  });

  useAppStore.getState().applyStartupShareHydration(hydration);
  unsubscribe();

  assert.deepEqual(notifications, [
    {
      workspace: hydration.workspace,
      workspacePane: hydration.view.workspacePane,
      editorMode: hydration.view.editorMode,
      previewMode: hydration.view.previewMode,
      showSvgBounds: hydration.view.showSvgBounds,
      svgPresentationMode: hydration.view.svgPresentationMode,
      shareViewWarning: hydration.warning,
    },
  ]);
});

function renderingState() {
  const state = useAppStore.getState();
  return {
    diagramTheme: state.diagramTheme,
    themePresetId: state.themePresetId,
    svgPipeline: state.svgPipeline,
  };
}

test("preset and recipe choices replace one another atomically and preserve names", () => {
  const recipe = '{"schema_version":2,"extends":"studio"}';
  useAppStore.getState().applyWorkspaceSnapshot({ ...DEFAULT_WORKSPACE_SNAPSHOT });
  const notifications: Array<[string | null, string | null]> = [];
  const unsubscribe = useAppStore.subscribe((state) => notifications.push([state.themePresetId, state.themeDocumentJson]));
  useAppStore.getState().selectTheme({ kind: "document", documentJson: recipe, label: { name: "Nightfall", source: "exploration" } });
  assert.equal(selectWorkspaceSnapshot(useAppStore.getState()).themeDocumentLabel?.name, "Nightfall");
  useAppStore.getState().selectTheme({ kind: "preset", presetId: "spotless" });
  assert.equal(useAppStore.getState().themeDocumentLabel, null);
  useAppStore.getState().selectTheme({ kind: "mermaid", theme: "forest" });
  unsubscribe();
  assert.deepEqual(notifications, [[null, recipe], ["spotless", null], [null, null]]);
  assert.equal(useAppStore.getState().diagramTheme, "forest");
});

test("applying and selecting a custom workspace retains its recipe", () => {
  const snapshot: WorkspaceSnapshot = {
    ...selectWorkspaceSnapshot(useAppStore.getState()),
    themePresetId: null,
    themeDocumentJson: '{"schema_version":2,"extends":"studio"}',
  };
  useAppStore.getState().applyWorkspaceSnapshot(snapshot);
  assert.deepEqual(selectWorkspaceSnapshot(useAppStore.getState()), snapshot);
});

test("closing appearance keeps the selected theme and subsequent source edits", () => {
  useAppStore.getState().applyWorkspaceSnapshot({ ...DEFAULT_WORKSPACE_SNAPSHOT });
  useAppStore.getState().openThemePanel();
  useAppStore.getState().selectTheme({ kind: "preset", presetId: "cyberpunk" });
  useAppStore.getState().setCode("sequenceDiagram\nAlice->>Bob: Keep this edit");
  useAppStore.getState().closeThemePanel();
  assert.equal(useAppStore.getState().themePanelOpen, false);
  assert.equal(useAppStore.getState().themePresetId, "cyberpunk");
  assert.equal(selectWorkspaceSnapshot(useAppStore.getState()).code, "sequenceDiagram\nAlice->>Bob: Keep this edit");
  assert.equal(useAppStore.getState().undoAppearance(), true);
  assert.equal(useAppStore.getState().themePresetId, null);
  assert.equal(useAppStore.getState().code, "sequenceDiagram\nAlice->>Bob: Keep this edit");
});

test("workspace replacement clears stale appearance identity, history and panel state", () => {
  useAppStore.getState().selectTheme({ kind: "document", documentJson: "{}", label: { name: "My theme", source: "imported" } });
  useAppStore.getState().openThemePanel();
  useAppStore.getState().applyWorkspaceSnapshot({ ...DEFAULT_WORKSPACE_SNAPSHOT });
  assert.equal(useAppStore.getState().themeDocumentLabel, null);
  assert.equal(useAppStore.getState().appearanceHistory.length, 0);
  assert.equal(useAppStore.getState().themePanelOpen, false);
});

test("no-op choices create no history and appearance history is bounded", () => {
  useAppStore.getState().applyWorkspaceSnapshot({ ...DEFAULT_WORKSPACE_SNAPSHOT });
  useAppStore.getState().setDiagramFont(DEFAULT_WORKSPACE_SNAPSHOT.diagramFont);
  assert.equal(useAppStore.getState().appearanceHistory.length, 0);
  for (let i = 0; i < 40; i++) useAppStore.getState().selectTheme({ kind: "preset", presetId: `theme-${i}` });
  assert.equal(useAppStore.getState().appearanceHistory.length, 32);
});

test("appearance preference changes are undoable while application appearance changes are not", () => {
  useAppStore.getState().applyWorkspaceSnapshot(DEFAULT_WORKSPACE_SNAPSHOT);
  useAppStore.getState().selectTheme({ kind: "preset", presetId: "editor", appearance: "follow" });
  const history = useAppStore.getState().appearanceHistory;
  useAppStore.getState().setUITheme("light");
  useAppStore.getState().setUITheme("dark");
  assert.equal(useAppStore.getState().appearanceHistory, history);
  assert.equal(useAppStore.getState().themeAppearance, "follow");
  useAppStore.getState().setThemeAppearance("light");
  assert.equal(selectWorkspaceSnapshot(useAppStore.getState()).themeAppearance, "light");
  useAppStore.getState().setCode("later authored source");
  useAppStore.getState().setMermaidConfig('{"look":"neo"}');
  assert.equal(useAppStore.getState().undoAppearance(), true);
  assert.equal(useAppStore.getState().themeAppearance, "follow");
  assert.equal(useAppStore.getState().code, "later authored source");
  assert.equal(useAppStore.getState().mermaidConfig, '{"look":"neo"}');
});

test("document selection and appearance update together", () => {
  const initial = { ...DEFAULT_WORKSPACE_SNAPSHOT, diagramTheme: "forest" as const, themePresetId: "editor", themeAppearance: "dark" as const };
  useAppStore.getState().applyWorkspaceSnapshot(initial);
  useAppStore.getState().selectTheme({
    kind: "document", documentJson: '{"schema_version":2,"extends":"studio"}', appearance: "dark",
    label: { name: "Editor", source: "imported", derivedFrom: "preset:editor" },
  });
  assert.equal(useAppStore.getState().diagramTheme, "auto");
  assert.equal(useAppStore.getState().themeAppearance, "dark");
  assert.equal(useAppStore.getState().appearanceHistory.length, 1);
  useAppStore.getState().setCode("a later source edit");
  useAppStore.getState().setMermaidConfig('{"flowchart":{"look":"classic"}}');
  useAppStore.getState().undoAppearance();
  assert.equal(useAppStore.getState().themePresetId, "editor");
  assert.equal(useAppStore.getState().themeDocumentJson, null);
  assert.equal(useAppStore.getState().diagramTheme, "forest");
  assert.equal(useAppStore.getState().themeAppearance, "dark");
  assert.equal(useAppStore.getState().code, "a later source edit");
  assert.equal(useAppStore.getState().mermaidConfig, '{"flowchart":{"look":"classic"}}');
});

test("a new single-appearance choice updates identity and appearance atomically", () => {
  useAppStore.getState().applyWorkspaceSnapshot({ ...DEFAULT_WORKSPACE_SNAPSHOT, themePresetId: "editor", themeAppearance: "dark" });
  const changes: WorkspaceSnapshot[] = [];
  const dispose = useAppStore.subscribe((state) => changes.push(selectWorkspaceSnapshot(state)));
  useAppStore.getState().selectTheme({ kind: "preset", presetId: "brutalist", appearance: "light" });
  dispose();
  assert.equal(changes.length, 1);
  assert.equal(changes[0].themePresetId, "brutalist");
  assert.equal(changes[0].themeAppearance, "light");
});
