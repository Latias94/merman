export interface PreviewRenderInputs {
  code: string;
  diagramTheme: string;
  mermaidConfig: string;
  hostThemePreset: string | null;
  textMeasurementMode: string;
  diagramFont: string;
  refreshNonce: number;
}

export interface RenderArtifact<T> {
  key: string;
  value: T;
}

export function createPreviewRenderKey(inputs: PreviewRenderInputs): string {
  return JSON.stringify([
    inputs.code,
    inputs.diagramTheme,
    inputs.mermaidConfig,
    inputs.hostThemePreset,
    inputs.textMeasurementMode,
    inputs.diagramFont,
    inputs.refreshNonce,
  ]);
}

export function bindRenderArtifact<T>(
  key: string,
  value: T
): RenderArtifact<T> {
  return { key, value };
}

export function isFreshRenderArtifact<T>(
  artifact: RenderArtifact<T> | null,
  currentKey: string
): artifact is RenderArtifact<T> {
  return artifact?.key === currentKey;
}

export function freshRenderArtifactValue<T>(
  artifact: RenderArtifact<T> | null,
  currentKey: string
): T | null {
  return isFreshRenderArtifact(artifact, currentKey) ? artifact.value : null;
}
