import type { SvgBindingOptions } from "@mermanjs/web";

import { sourceWithConfig } from "../lib/mermaid-config.ts";
import type { MermanRenderOptions } from "./merman-core.ts";

export interface ConfiguredMermanOperationInput {
  readonly source: string;
  readonly bindingOptions: SvgBindingOptions;
}

export function configuredMermanOperationInput(
  code: string,
  theme: string,
  configJson: string,
  options: MermanRenderOptions | undefined,
): ConfiguredMermanOperationInput {
  return Object.freeze({
    source: sourceWithConfig(code, theme, configJson, {
      diagramFont: options?.diagramFont,
    }),
    bindingOptions: bindingOptionsForRender(options),
  });
}

function bindingOptionsForRender(
  options: MermanRenderOptions | undefined,
): SvgBindingOptions {
  const presentationProfileId = options?.presentationProfileId;
  const bindingOptions: SvgBindingOptions = { version: 2 };
  const presentationThemePresetId = options?.presentationThemePresetId;
  if (presentationProfileId || presentationThemePresetId) {
    bindingOptions.presentation = {
      ...(presentationProfileId
        ? { profile: presentationProfileId }
        : {}),
      ...(presentationThemePresetId
        ? { theme: { preset: presentationThemePresetId } }
        : {}),
    };
  }
  if (options?.svgPipeline) {
    bindingOptions.svg = { pipeline: options.svgPipeline };
  }
  if (options?.layoutEnvironment) {
    bindingOptions.layout = {
      container_width: options.layoutEnvironment.containerWidth,
      container_height: options.layoutEnvironment.containerHeight,
      ...(options.layoutEnvironment.screenAvailableWidth === undefined
        ? {}
        : {
            screen_available_width:
              options.layoutEnvironment.screenAvailableWidth,
          }),
    };
  }
  return bindingOptions;
}
