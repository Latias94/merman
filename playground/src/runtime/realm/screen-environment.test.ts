import assert from "node:assert/strict";
import test from "node:test";

import { applyScreenAvailableWidth } from "./screen-environment.ts";

test("establishes the shared screen width or fails closed", () => {
  const previousWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
  try {
    const screen = { availWidth: 800 };
    defineWindow(screen);
    applyScreenAvailableWidth(1512);
    assert.equal(screen.availWidth, 1512);

    defineWindow(Object.freeze({ availWidth: 800 }));
    assert.throws(
      () => applyScreenAvailableWidth(1512),
      /could not be established/u,
    );
  } finally {
    if (previousWindow) {
      Object.defineProperty(globalThis, "window", previousWindow);
    } else {
      Reflect.deleteProperty(globalThis, "window");
    }
  }
});

function defineWindow(screen: object): void {
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: { screen },
    writable: true,
  });
}
