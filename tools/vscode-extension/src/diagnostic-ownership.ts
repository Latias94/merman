export interface DiagnosticOwnershipSettings {
  enabled: boolean;
}

export interface FullDocumentDiagnosticReport<T> {
  kind: "full";
  resultId?: string;
  items: readonly T[];
  relatedDocuments?: unknown;
}

export interface UnchangedDocumentDiagnosticReport {
  kind: "unChanged";
  resultId: string;
  relatedDocuments?: unknown;
}

export type DocumentDiagnosticReport<T> =
  | FullDocumentDiagnosticReport<T>
  | UnchangedDocumentDiagnosticReport;

export function projectOwnedDiagnostics<T>(
  diagnostics: readonly T[],
  settings: DiagnosticOwnershipSettings,
): T[] {
  return settings.enabled ? [...diagnostics] : [];
}

export function emptyDocumentDiagnosticReport<T>(): FullDocumentDiagnosticReport<T> {
  return { kind: "full", items: [] };
}

export function projectOwnedDocumentDiagnosticReport<T>(
  report: DocumentDiagnosticReport<T>,
  settings: DiagnosticOwnershipSettings,
): DocumentDiagnosticReport<T> {
  if (!settings.enabled) {
    return emptyDocumentDiagnosticReport();
  }
  if (report.kind === "full") {
    return { ...report, items: [...report.items] };
  }
  return report;
}
