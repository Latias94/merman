interface EditorLike {
  readonly document: unknown;
}

interface UriLike {
  toString(): string;
}

export function isActiveEditorSelectionChange(
  eventEditor: unknown,
  activeEditor: unknown,
): boolean {
  return activeEditor !== undefined && eventEditor === activeEditor;
}

export function isTrackedPreviewDocumentChange(
  trackedEditor: EditorLike | undefined,
  changedDocument: unknown,
): boolean {
  return trackedEditor !== undefined && changedDocument === trackedEditor.document;
}

export function isTrackedPreviewDiagnosticsChange(
  trackedUri: UriLike | undefined,
  changedUris: readonly UriLike[],
): boolean {
  return trackedUri !== undefined && changedUris.some((uri) => uri.toString() === trackedUri.toString());
}
