import type { PreviewDiagnosticTarget, PreviewDiagnostics } from "./preview-model.js";

const MERMAN_DIAGNOSTIC_SOURCE = "merman";

export interface PreviewDiagnosticInput {
  range: {
    start: {
      line: number;
      character: number;
    };
    end: {
      line: number;
      character: number;
    };
  };
  severity: number;
  source?: string;
  code?: string | number | { value: string | number };
  message: string;
}

export function collectMermanPreviewDiagnostics(
  diagnostics: readonly PreviewDiagnosticInput[],
  uri: string,
  diagnosticRange: { startLine: number; endLine: number },
): PreviewDiagnostics {
  const filtered = deduplicateDiagnostics(
    diagnostics
      .filter(isMermanDiagnostic)
      .filter((diagnostic) => isDiagnosticInRange(diagnostic, diagnosticRange))
      .sort(compareDiagnostics),
  );

  const counts = {
    error: filtered.filter((diagnostic) => diagnostic.severity === 0).length,
    warning: filtered.filter((diagnostic) => diagnostic.severity === 1).length,
    info: filtered.filter((diagnostic) => diagnostic.severity === 2).length,
    hint: filtered.filter((diagnostic) => diagnostic.severity === 3).length,
  };

  return {
    summary: `${counts.error} errors, ${counts.warning} warnings, ${counts.info} infos, ${counts.hint} hints`,
    totalCount: filtered.length,
    firstTarget: filtered[0] ? diagnosticTarget(uri, filtered[0]) : undefined,
  };
}

function isMermanDiagnostic(diagnostic: PreviewDiagnosticInput): boolean {
  return diagnostic.source?.toLowerCase() === MERMAN_DIAGNOSTIC_SOURCE;
}

function deduplicateDiagnostics(
  diagnostics: readonly PreviewDiagnosticInput[],
): PreviewDiagnosticInput[] {
  const seen = new Set<string>();
  return diagnostics.filter((diagnostic) => {
    const key = [
      diagnostic.range.start.line,
      diagnostic.range.start.character,
      diagnostic.range.end.line,
      diagnostic.range.end.character,
      diagnostic.severity,
      diagnostic.source ?? "",
      diagnosticCodeLabel(diagnostic.code) ?? "",
      diagnostic.message,
    ].join("\u0000");
    if (seen.has(key)) {
      return false;
    }
    seen.add(key);
    return true;
  });
}

function isDiagnosticInRange(
  diagnostic: PreviewDiagnosticInput,
  diagnosticRange: { startLine: number; endLine: number },
): boolean {
  const startLine = diagnostic.range.start.line;
  const endLine = diagnostic.range.end.line;
  return startLine <= diagnosticRange.endLine && endLine >= diagnosticRange.startLine;
}

function compareDiagnostics(a: PreviewDiagnosticInput, b: PreviewDiagnosticInput): number {
  return (
    diagnosticSeverityRank(a.severity) - diagnosticSeverityRank(b.severity) ||
    a.range.start.line - b.range.start.line ||
    a.range.start.character - b.range.start.character
  );
}

function diagnosticSeverityRank(severity: number): number {
  switch (severity) {
    case 0:
      return 0;
    case 1:
      return 1;
    case 2:
      return 2;
    case 3:
    default:
      return 3;
  }
}

function diagnosticCodeLabel(
  code: PreviewDiagnosticInput["code"],
): string | undefined {
  if (typeof code === "string" || typeof code === "number") {
    return String(code);
  }
  if (code && typeof code === "object" && "value" in code) {
    return String(code.value);
  }
  return undefined;
}

function diagnosticTarget(
  uri: string,
  diagnostic: PreviewDiagnosticInput,
): PreviewDiagnosticTarget {
  return {
    uri,
    startLine: diagnostic.range.start.line,
    startCharacter: diagnostic.range.start.character,
    endLine: diagnostic.range.end.line,
    endCharacter: diagnostic.range.end.character,
  };
}
