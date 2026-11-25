import * as path from "node:path";
import * as vscode from "vscode";

import { discardCliMetadata, readCliMetadata } from "./cli-capabilities.js";
import { errorMessage } from "./error-message.js";
import { RenderProcessExitError, runRenderProcess } from "./render-process.js";
import { resolveCliInvocation } from "./renderer.js";
import {
  CUSTOM_FILE_THEME,
  editorThemeAppearance,
  isDiagramTheme,
  isThemeAppearance,
  SOURCE_THEME,
  type ImportedTheme,
  type ThemeAppearances,
  type ThemeAppearancePreference,
  type ThemeChoice,
  type ThemeDiscovery,
} from "./theme-selection.js";

const discoveries = new WeakMap<Promise<unknown>, Promise<ThemeDiscovery>>();
const MAX_THEME_FILE_BYTES = 8 * 1024 * 1024;

export async function discoverThemes(context: vscode.ExtensionContext): Promise<ThemeDiscovery> {
  try {
    const invocation = resolveCliInvocation(context, ["theme", "list", "--json"]);
    const catalog = readCliMetadata(invocation);
    let pending = discoveries.get(catalog);
    if (!pending) {
      pending = catalog.then(parseThemeCatalog).catch((error: unknown) => {
        discardCliMetadata(invocation, catalog);
        throw error;
      });
      discoveries.set(catalog, pending);
    }
    return await pending;
  } catch (error) {
    try {
      const capabilities = await readCliMetadata(resolveCliInvocation(context, ["capabilities", "--json"]));
      const choices = legacyClassicChoices(capabilities);
      if (choices.length > 0) {
        return { choices: [SOURCE_THEME, ...choices], notice: "Update the CLI to use Merman v2 presets and custom files." };
      }
    } catch {
      // Preserve the original discovery error when the fallback is unavailable.
    }
    return { choices: [SOURCE_THEME], notice: `Theme discovery failed. ${errorMessage(error)}` };
  }
}

export function parseThemeCatalog(value: unknown): ThemeDiscovery {
  const catalog = object(value);
  if (catalog.schema_version !== 2) {
    throw new Error("Update the CLI to use the Merman v2 theme catalog.");
  }
  if (!Array.isArray(catalog.presets) || !Array.isArray(catalog.mermaid_themes)) {
    throw new Error("The CLI returned an incomplete theme catalog.");
  }

  const choices: ThemeChoice[] = [SOURCE_THEME];
  const ids = new Set([SOURCE_THEME.id, CUSTOM_FILE_THEME.id]);
  for (const raw of catalog.presets) {
    const preset = object(raw);
    if (typeof preset.name !== "string" || !preset.name.trim()) {
      throw new Error("The CLI returned a preset without a name.");
    }
    choices.push({
      id: readCatalogId(preset.id, ids),
      label: preset.name,
      origin: "preset",
      appearances: readThemeAppearances(preset.appearances, preset.default_appearance),
    });
  }
  for (const raw of catalog.mermaid_themes) {
    choices.push(classicThemeChoice(readCatalogId(raw, ids)));
  }
  choices.push(CUSTOM_FILE_THEME);
  return { choices };
}

function legacyClassicChoices(capabilities: unknown): ThemeChoice[] {
  const catalog = object(object(capabilities).theme_presets);
  if (catalog.schema_version !== 1) return [];
  const designs = catalog.mermaid_theme_designs;
  if (designs === undefined) {
    return ["default", "dark", "forest", "neutral", "base"].map(classicThemeChoice);
  }
  if (!Array.isArray(designs)) return [];

  const ids = new Set<string>();
  for (const design of designs) {
    const appearances = object(design).appearances;
    if (!Array.isArray(appearances)) continue;
    for (const variant of appearances) {
      const id = object(variant).theme_id;
      if (isDiagramTheme(id) && id !== SOURCE_THEME.id && id !== CUSTOM_FILE_THEME.id) ids.add(id);
    }
  }
  return Array.from(ids, classicThemeChoice);
}

function classicThemeChoice(id: string): ThemeChoice {
  const label = id.split("-").map((word) => word.charAt(0).toUpperCase() + word.slice(1)).join(" ");
  return { id, label, origin: "mermaid" };
}

export async function checkThemeFile(
  context: vscode.ExtensionContext,
  uri: vscode.Uri,
  discovery: ThemeDiscovery,
  preference: ThemeAppearancePreference = "follow-editor",
): Promise<ImportedTheme> {
  if (!discovery.choices.some((choice) => choice.id === CUSTOM_FILE_THEME.id)) {
    throw new Error("Custom theme files require a CLI with `theme check` support.");
  }
  const stat = await vscode.workspace.fs.stat(uri);
  if (stat.size > MAX_THEME_FILE_BYTES) {
    throw new Error("Theme JSON exceeds the editor's 8 MiB file limit.");
  }
  const bytes = await vscode.workspace.fs.readFile(uri);
  if (bytes.byteLength > MAX_THEME_FILE_BYTES) {
    throw new Error("Theme JSON exceeds the editor's 8 MiB file limit.");
  }
  const content = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
  let document: Record<string, unknown> = {};
  try {
    document = object(JSON.parse(content));
  } catch {
    // The CLI reports syntax errors against the original text.
  }
  const inherited = discovery.choices.find((choice) => choice.origin === "preset" && choice.id === document.extends);
  const supportedAppearances = new Set([
    ...Object.keys(object(document.appearances)).filter(isThemeAppearance),
    ...(inherited?.appearances?.supportedAppearances ?? []),
  ]);
  const args = ["theme", "check", "-", "--json"];
  if (!Object.hasOwn(document, "default_appearance") && supportedAppearances.size === 2) {
    const appearance = preference === "follow-editor"
      ? editorThemeAppearance(vscode.window.activeColorTheme.kind)
      : preference;
    args.push("--appearance", appearance);
  }
  const result = await runRenderProcess({
    invocation: resolveCliInvocation(context, args),
    source: content,
    maxStdoutBytes: 8 * 1024 * 1024,
  }).catch((error: unknown) => {
    if (error instanceof RenderProcessExitError) {
      throw themeCheckError(error.stdout) ?? error;
    }
    throw error;
  });
  const checked = object(JSON.parse(result.stdout.toString("utf8")));
  const defaultAppearance = object(checked.resolved).appearance;
  if (checked.ok !== true || !isThemeAppearance(defaultAppearance)) {
    throw new Error("The CLI returned an invalid theme check result.");
  }
  supportedAppearances.add(defaultAppearance);
  return {
    content,
    label: path.basename(uri.fsPath),
    supportedAppearances: [...supportedAppearances],
    defaultAppearance,
  };
}

function themeCheckError(stdout: Buffer): Error | undefined {
  let diagnostic: Record<string, unknown>;
  try {
    diagnostic = object(JSON.parse(stdout.toString("utf8")));
  } catch {
    return undefined;
  }
  if (diagnostic.ok !== false || typeof diagnostic.message !== "string") {
    return undefined;
  }
  const location = [
    typeof diagnostic.path === "string" ? diagnostic.path : undefined,
    typeof diagnostic.line === "number" && typeof diagnostic.column === "number"
      ? `line ${diagnostic.line}, column ${diagnostic.column}`
      : undefined,
  ].filter(Boolean).join(", ");
  return new Error(location ? `${diagnostic.message} (${location})` : diagnostic.message);
}

function readCatalogId(value: unknown, ids: Set<string>): string {
  if (!isDiagramTheme(value) || ids.has(value)) {
    throw new Error("The CLI returned an invalid or duplicate catalog theme ID.");
  }
  ids.add(value);
  return value;
}

function readThemeAppearances(appearances: unknown, fallback: unknown): ThemeAppearances {
  if (!Array.isArray(appearances) || !appearances.every(isThemeAppearance) ||
    new Set(appearances).size !== appearances.length) {
    throw new Error("The CLI returned invalid theme appearances.");
  }
  const defaultAppearance = fallback ?? (appearances.length === 1 ? appearances[0] : undefined);
  if (!isThemeAppearance(defaultAppearance) || !appearances.includes(defaultAppearance)) {
    throw new Error("The CLI returned a theme without a supported default appearance.");
  }
  return { supportedAppearances: appearances, defaultAppearance };
}

function object(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}
