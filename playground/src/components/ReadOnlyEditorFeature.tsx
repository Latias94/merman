import Editor from "@monaco-editor/react";

import { ensureLocalMonacoConfigured } from "@/src/editor/monaco";

ensureLocalMonacoConfigured();

export const LocalReadOnlyEditor = Editor;
