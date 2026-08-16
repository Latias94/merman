import {
  RealmProtocolError,
  validateScreenAvailableWidth,
} from "./channel-protocol.ts";

export function applyScreenAvailableWidth(value: number): void {
  const width = validateScreenAvailableWidth(value);
  const screen = window.screen;
  if (!screen) {
    throw new RealmProtocolError("Realm screen is unavailable.");
  }
  try {
    Object.defineProperty(screen, "availWidth", {
      configurable: true,
      enumerable: true,
      value: width,
    });
  } catch {
    throw new RealmProtocolError(
      "Realm screen width could not be established.",
    );
  }
  if (screen.availWidth !== width) {
    throw new RealmProtocolError(
      "Realm screen width does not match the shared environment.",
    );
  }
}
