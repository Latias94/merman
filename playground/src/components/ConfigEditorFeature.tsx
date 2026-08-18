import { ensureLocalMonacoConfigured } from "@/src/editor/monaco";
import { activateLocalMonacoJson } from "@/src/editor/monaco-json";

import { ConfigEditor } from "./ConfigEditor";

ensureLocalMonacoConfigured();
activateLocalMonacoJson();

export { ConfigEditor };
