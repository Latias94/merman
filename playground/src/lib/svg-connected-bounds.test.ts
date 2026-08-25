import assert from "node:assert/strict";
import test from "node:test";

import {
  resolveConnectedBounds,
  type SvgBounds,
} from "./svg-connected-bounds.ts";

test("expands through a reverse-ordered chain without including disconnected bounds", () => {
  const root = bounds(0, 1, 0, 1);
  const chain = Array.from({ length: 100 }, (_, index) => {
    const step = 100 - index;
    return bounds(step, step + 1, 0, 1);
  });

  assert.deepEqual(
    resolveConnectedBounds(root, [
      bounds(1_000, 1_001, 1_000, 1_001),
      ...chain,
    ]),
    bounds(0, 101, 0, 1),
  );
});

test("treats touching edges as connected", () => {
  assert.deepEqual(
    resolveConnectedBounds(bounds(0, 1, 0, 1), [bounds(1, 2, 1, 2)]),
    bounds(0, 2, 0, 2),
  );
});

test("matches the existing fixed-point algorithm across deterministic samples", () => {
  let state = 0x5eed1234;
  const random = () => {
    state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
    return state / 0x1_0000_0000;
  };

  for (let sample = 0; sample < 250; sample += 1) {
    const root = randomBounds(random);
    const candidates = Array.from({ length: 40 }, () => randomBounds(random));
    assert.deepEqual(
      resolveConnectedBounds(root, candidates),
      resolveConnectedBoundsReference(root, candidates),
      `sample ${sample}`,
    );
  }
});

function resolveConnectedBoundsReference(
  root: SvgBounds,
  candidates: readonly SvgBounds[],
): SvgBounds {
  let connected = root;
  let pending = [...candidates];

  while (pending.length > 0) {
    const disconnected: SvgBounds[] = [];
    let foundConnection = false;
    for (const candidate of pending) {
      if (!boundsIntersect(connected, candidate)) {
        disconnected.push(candidate);
        continue;
      }
      connected = unionBounds(connected, candidate);
      foundConnection = true;
    }
    if (!foundConnection) break;
    pending = disconnected;
  }

  return connected;
}

function randomBounds(random: () => number): SvgBounds {
  const left = Math.floor(random() * 40) - 20;
  const top = Math.floor(random() * 40) - 20;
  return bounds(
    left,
    left + Math.floor(random() * 8),
    top,
    top + Math.floor(random() * 8),
  );
}

function bounds(
  left: number,
  right: number,
  top: number,
  bottom: number,
): SvgBounds {
  return { bottom, left, right, top };
}

function boundsIntersect(left: SvgBounds, right: SvgBounds): boolean {
  return (
    left.left <= right.right &&
    left.right >= right.left &&
    left.top <= right.bottom &&
    left.bottom >= right.top
  );
}

function unionBounds(left: SvgBounds, right: SvgBounds): SvgBounds {
  return {
    bottom: Math.max(left.bottom, right.bottom),
    left: Math.min(left.left, right.left),
    right: Math.max(left.right, right.right),
    top: Math.min(left.top, right.top),
  };
}
