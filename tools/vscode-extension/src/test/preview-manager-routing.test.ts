import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  isActiveEditorSelectionChange,
  isTrackedPreviewDiagnosticsChange,
} from "../preview-manager-routing.js";

describe("preview manager routing", () => {
  it("routes selection changes only for the active editor", () => {
    const active = {};

    assert.equal(isActiveEditorSelectionChange(active, active), true);
    assert.equal(isActiveEditorSelectionChange({}, active), false);
    assert.equal(isActiveEditorSelectionChange(active, undefined), false);
  });

  it("routes diagnostics changes only for the tracked preview URI", () => {
    const trackedUri = "file:///workspace/diagram.mmd";

    assert.equal(
      isTrackedPreviewDiagnosticsChange(trackedUri, [
        uri("file:///workspace/other.mmd"),
        uri("file:///workspace/diagram.mmd"),
      ]),
      true,
    );
    assert.equal(
      isTrackedPreviewDiagnosticsChange(trackedUri, [uri("file:///workspace/other.mmd")]),
      false,
    );
    assert.equal(
      isTrackedPreviewDiagnosticsChange(undefined, [uri("file:///workspace/diagram.mmd")]),
      false,
    );
  });
});

function uri(value: string): { toString(): string } {
  return {
    toString: () => value,
  };
}
