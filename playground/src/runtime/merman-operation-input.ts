import type { SvgBindingOptions } from "@mermanjs/web";

import { diagramFontStack } from "../lib/diagram-font.ts";
import { sourceWithConfig } from "../lib/mermaid-config.ts";
import type { MermanRenderOptions } from "./merman-core.ts";

export interface ConfiguredMermanOperationInput {
  readonly source: string;
  readonly bindingOptions: SvgBindingOptions | undefined;
}

export function configuredMermanOperationInput(
  code: string,
  theme: string,
  configJson: string,
  options: MermanRenderOptions | undefined,
): ConfiguredMermanOperationInput {
  return Object.freeze({
    source: sourceWithConfig(
      code,
      options?.hostThemePreset ? "default" : theme,
      configJson,
    ),
    bindingOptions: bindingOptionsForRender(options),
  });
}

function bindingOptionsForRender(
  options: MermanRenderOptions | undefined,
): SvgBindingOptions | undefined {
  const fontFamily = options?.diagramFont
    ? diagramFontStack(options.diagramFont)
    : undefined;
  if (!options?.pipeline && !options?.hostThemePreset && !fontFamily) {
    return undefined;
  }

  const bindingOptions: SvgBindingOptions = {};
  if (options?.hostThemePreset) {
    bindingOptions.host_theme = {
      preset: options.hostThemePreset,
      ...(fontFamily ? { font_family: fontFamily } : {}),
    };
  } else if (fontFamily) {
    bindingOptions.site_config = {
      fontFamily,
      themeVariables: { fontFamily },
    };
  }
  if (options?.pipeline) {
    bindingOptions.svg = { pipeline: options.pipeline };
  }
  return bindingOptions;
}
