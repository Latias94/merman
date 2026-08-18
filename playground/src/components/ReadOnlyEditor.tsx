import { lazy } from "react";
import type { EditorProps } from "@monaco-editor/react";

import { LazyFeatureBoundary } from "@/src/components/LazyFeatureBoundary";

const LocalReadOnlyEditor = lazy(() =>
  import("./ReadOnlyEditorFeature").then((module) => ({
    default: module.LocalReadOnlyEditor,
  })),
);

interface ReadOnlyEditorProps
  extends Omit<
    EditorProps,
    "defaultLanguage" | "defaultPath" | "defaultValue" | "keepCurrentModel" | "onChange"
  > {
  readonly feature: string;
  readonly value: string;
}

export function ReadOnlyEditor({
  feature,
  options,
  ...props
}: ReadOnlyEditorProps) {
  return (
    <LazyFeatureBoundary
      feature={feature}
      presentation={{ kind: "panel" }}
    >
      <LocalReadOnlyEditor
        {...props}
        options={{
          ...options,
          domReadOnly: true,
          occurrencesHighlight: "off",
          readOnly: true,
        }}
      />
    </LazyFeatureBoundary>
  );
}
