export function encodeJsonInput(value: unknown, label: string): string {
  const encoded = JSON.stringify(value, (_key, child) => {
    if (typeof child === "number" && !Number.isFinite(child)) {
      throw new TypeError(`${label} contains a non-finite number`);
    }
    return child;
  });
  if (encoded === undefined) {
    throw new TypeError(`${label} must be JSON-serializable`);
  }
  return encoded;
}
