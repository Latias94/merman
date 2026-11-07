import * as path from "node:path";
import * as vscode from "vscode";

import { runRenderProcess } from "./render-process.js";
import { resolveCliInvocation } from "./renderer.js";
import {
  importedThemeInspection,
  isDiagramTheme,
  isThemeAppearance,
  legacyThemeDiscovery,
  object,
  SOURCE_THEME,
  type ImportedTheme,
  type ThemeChoice,
  type ThemeDiscovery,
} from "./theme-selection.js";

const discoveries = new Map<string, Promise<ThemeDiscovery>>();
const MAX_THEME_FILE_BYTES = 8 * 1024 * 1024;

export async function discoverThemes(context: vscode.ExtensionContext): Promise<ThemeDiscovery> {
  try {
    const invocation = resolveCliInvocation(context, ["capabilities", "--json"]);
    const key = JSON.stringify(invocation);
    let pending = discoveries.get(key);
    if (!pending) {
      pending = runRenderProcess({
        invocation,
        source: "",
        timeoutMs: 15000,
        maxStdoutBytes: 8 * 1024 * 1024,
      }).then((result) => parseThemeCapabilities(JSON.parse(result.stdout.toString("utf8"))))
        .catch((error: unknown) => {
          discoveries.delete(key);
          return legacyThemeDiscovery(`Theme discovery failed: ${error instanceof Error ? error.message : String(error)}. Source / default remains available.`);
        });
      discoveries.set(key, pending);
    }
    return await pending;
  } catch (error) {
    return legacyThemeDiscovery(`Theme discovery is unavailable: ${error instanceof Error ? error.message : String(error)}.`);
  }
}

export function parseThemeCapabilities(value: unknown): ThemeDiscovery {
  const capabilities = object(value);
  const catalog = object(capabilities.theme_presets);
  if (!(typeof capabilities.cli_contract_version === "number" && capabilities.cli_contract_version >= 7) ||
    catalog.schema_version !== 1) {
    return legacyThemeDiscovery();
  }
  if (!Array.isArray(catalog.presets) || !Array.isArray(catalog.mermaid_theme_designs)) {
    throw new Error("The CLI returned an incomplete grouped theme catalog.");
  }
  const choices: ThemeChoice[] = [SOURCE_THEME];
  const legacyChoices: ThemeChoice[] = [];
  for (const raw of catalog.mermaid_theme_designs) {
    const design = object(raw);
    if (!Array.isArray(design.appearances) || typeof design.id !== "string" ||
      typeof design.display_name !== "string" || !isThemeAppearance(design.default_appearance)) {
      throw new Error("The CLI returned invalid Mermaid theme metadata.");
    }
    const variants = design.appearances.map((rawVariant: unknown) => {
      const variant = object(rawVariant);
      if (!isThemeAppearance(variant.appearance) || typeof variant.theme_id !== "string" ||
        !isDiagramTheme(variant.theme_id) || variant.theme_id.includes(":") || variant.theme_id === "file" || variant.theme_id === "source") {
        throw new Error("The CLI returned an unsupported Mermaid theme ID.");
      }
      return { appearance: variant.appearance, themeId: variant.theme_id };
    });
    const defaultVariant = variants.find((variant: { appearance: string }) => variant.appearance === design.default_appearance);
    if (!defaultVariant) {
      throw new Error("The CLI returned a Mermaid theme without its default appearance.");
    }
    const id = variants.length > 1 ? `mermaid:${design.id}` : defaultVariant.themeId;
    if (!isDiagramTheme(id)) {
      throw new Error("The CLI returned an unsupported Mermaid theme design ID.");
    }
    const choice: ThemeChoice = {
      id,
      label: `Mermaid · ${design.display_name}`,
      origin: "mermaid",
      available: true,
      supportedAppearances: variants.map((variant: { appearance: "light" | "dark" }) => variant.appearance),
      defaultAppearance: design.default_appearance,
      fixedAppearance: variants.length === 1 ? design.default_appearance : undefined,
      variants,
    };
    choices.push(choice);
    for (const variant of variants) {
      if (variant.themeId !== choice.id) {
        legacyChoices.push({
          ...choice,
          id: variant.themeId,
          label: `${choice.label} · ${variant.appearance} (fixed)`,
          supportedAppearances: [variant.appearance],
          defaultAppearance: variant.appearance,
          fixedAppearance: variant.appearance,
        });
      }
    }
  }
  for (const raw of catalog.presets) {
    const preset = object(raw);
    if (typeof preset.id !== "string" || typeof preset.display_name !== "string" ||
      !Array.isArray(preset.supported_appearances) || !preset.supported_appearances.every(isThemeAppearance) ||
      !isThemeAppearance(preset.default_appearance) ||
      !preset.supported_appearances.includes(preset.default_appearance) ||
      typeof preset.available !== "boolean" || !Array.isArray(preset.appearances) ||
      !isDiagramTheme(`preset:${preset.id}`)) {
      throw new Error("The CLI returned invalid preset appearance metadata.");
    }
    const choice: ThemeChoice = {
      id: `preset:${preset.id}`,
      label: `Merman · ${preset.display_name}`,
      origin: "preset",
      available: preset.available,
      supportedAppearances: preset.supported_appearances,
      defaultAppearance: preset.default_appearance,
    };
    choices.push(choice);
    for (const rawAppearance of preset.appearances) {
      const variant = object(rawAppearance);
      if (!isThemeAppearance(variant.appearance) ||
        !preset.supported_appearances.includes(variant.appearance) ||
        typeof variant.concrete_preset_id !== "string" ||
        !isDiagramTheme(`preset:${variant.concrete_preset_id}`)) {
        throw new Error("The CLI returned invalid concrete preset metadata.");
      }
      const id = `preset:${variant.concrete_preset_id}` as const;
      if (id !== choice.id) {
        legacyChoices.push({
          ...choice,
          id,
          label: `${choice.label} · ${variant.appearance} (fixed)`,
          supportedAppearances: [variant.appearance],
          defaultAppearance: variant.appearance,
          fixedAppearance: variant.appearance,
        });
      }
    }
  }
  return {
    choices,
    legacyChoices,
    paired: true,
    inspect: catalog.recipe_schema_version === 1 &&
      catalog.authoring_request_schema_version === 1 &&
      catalog.authoring_result_schema_version === 1 &&
      Array.isArray(capabilities.commands) && capabilities.commands.includes("theme"),
  };
}

export async function inspectThemeFile(
  context: vscode.ExtensionContext,
  uri: vscode.Uri,
  discovery: ThemeDiscovery,
): Promise<ImportedTheme> {
  if (!discovery.inspect) {
    throw new Error("Theme recipe import requires a CLI with `theme inspect` support. Update the CLI; Source / default keeps its original behavior.");
  }
  const stat = await vscode.workspace.fs.stat(uri);
  if (stat.size > MAX_THEME_FILE_BYTES) {
    throw new Error("Theme recipe JSON exceeds the editor's 8 MiB import limit.");
  }
  const bytes = await vscode.workspace.fs.readFile(uri);
  if (bytes.byteLength > MAX_THEME_FILE_BYTES) {
    throw new Error("Theme recipe JSON exceeds the editor's 8 MiB import limit.");
  }
  const content = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes);
  const result = await runRenderProcess({
    invocation: resolveCliInvocation(context, ["theme", "inspect", "-"]),
    source: content,
    maxStdoutBytes: 8 * 1024 * 1024,
  });
  return importedThemeInspection(content, path.basename(uri.fsPath), JSON.parse(result.stdout.toString("utf8")));
}
