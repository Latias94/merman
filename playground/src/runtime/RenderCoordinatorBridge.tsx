import { useEffect, useMemo } from "react";
import { useShallow } from "zustand/react/shallow";

import { useAppStore } from "../store";
import { setRenderCoordinatorInput } from "./render-coordinator-browser.ts";
import {
  selectMermanFacade,
  useMermanRuntime,
} from "./use-merman-runtime.ts";

export function RenderCoordinatorBridge() {
  const {
    code,
    diagramFont,
    diagramTheme,
    mermaidConfig,
    presentationProfileId,
    presentationThemePresetId,
    svgPipeline,
    textMeasurementMode,
  } = useAppStore(
    useShallow((state) => ({
      code: state.code,
      diagramFont: state.diagramFont,
      diagramTheme: state.diagramTheme,
      mermaidConfig: state.mermaidConfig,
      presentationProfileId: state.presentationProfileId,
      presentationThemePresetId: state.presentationThemePresetId,
      svgPipeline: state.svgPipeline,
      textMeasurementMode: state.textMeasurementMode,
    }))
  );
  const facade = useMermanRuntime(selectMermanFacade);
  const options = useMemo(
    () => ({
      diagramFont,
      presentationProfileId,
      presentationThemePresetId,
      svgPipeline,
      textMeasurementMode,
    }),
    [
      diagramFont,
      presentationProfileId,
      presentationThemePresetId,
      svgPipeline,
      textMeasurementMode,
    ]
  );

  useEffect(() => {
    setRenderCoordinatorInput({
      facade,
      source: code,
      theme: diagramTheme,
      configJson: mermaidConfig,
      options,
    });
  }, [code, diagramTheme, facade, mermaidConfig, options]);

  return null;
}
