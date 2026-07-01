export interface DiagnosticOwnershipSettings {
  enabled: boolean;
}

export function projectOwnedDiagnostics<T>(
  diagnostics: readonly T[],
  settings: DiagnosticOwnershipSettings,
): T[] {
  return settings.enabled ? [...diagnostics] : [];
}
