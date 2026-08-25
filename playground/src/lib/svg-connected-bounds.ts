export interface SvgBounds {
  readonly bottom: number;
  readonly left: number;
  readonly right: number;
  readonly top: number;
}

interface BoundaryFrontiers {
  bottom: number;
  left: number;
  right: number;
  top: number;
}

export function resolveConnectedBounds(
  root: SvgBounds,
  candidates: readonly SvgBounds[],
): SvgBounds {
  if (candidates.length === 0) return root;

  const unmetConditions = new Uint8Array(candidates.length);
  unmetConditions.fill(4);

  const ascendingLeft = sortedCandidateIndexes(
    candidates,
    (candidate) => candidate.left,
  );
  const descendingRight = sortedCandidateIndexes(
    candidates,
    (candidate) => -candidate.right,
  );
  const ascendingTop = sortedCandidateIndexes(
    candidates,
    (candidate) => candidate.top,
  );
  const descendingBottom = sortedCandidateIndexes(
    candidates,
    (candidate) => -candidate.bottom,
  );
  const frontiers: BoundaryFrontiers = {
    bottom: 0,
    left: 0,
    right: 0,
    top: 0,
  };
  const ready: number[] = [];
  let connected = root;

  const satisfy = (candidateIndex: number) => {
    const remaining = unmetConditions[candidateIndex] - 1;
    unmetConditions[candidateIndex] = remaining;
    if (remaining === 0) ready.push(candidateIndex);
  };

  const advanceFrontiers = () => {
    while (
      frontiers.left < ascendingLeft.length &&
      candidates[ascendingLeft[frontiers.left]].left <= connected.right
    ) {
      satisfy(ascendingLeft[frontiers.left]);
      frontiers.left += 1;
    }
    while (
      frontiers.right < descendingRight.length &&
      candidates[descendingRight[frontiers.right]].right >= connected.left
    ) {
      satisfy(descendingRight[frontiers.right]);
      frontiers.right += 1;
    }
    while (
      frontiers.top < ascendingTop.length &&
      candidates[ascendingTop[frontiers.top]].top <= connected.bottom
    ) {
      satisfy(ascendingTop[frontiers.top]);
      frontiers.top += 1;
    }
    while (
      frontiers.bottom < descendingBottom.length &&
      candidates[descendingBottom[frontiers.bottom]].bottom >= connected.top
    ) {
      satisfy(descendingBottom[frontiers.bottom]);
      frontiers.bottom += 1;
    }
  };

  advanceFrontiers();
  for (let readyIndex = 0; readyIndex < ready.length; readyIndex += 1) {
    connected = unionBounds(connected, candidates[ready[readyIndex]]);
    advanceFrontiers();
  }

  return connected;
}

function sortedCandidateIndexes(
  candidates: readonly SvgBounds[],
  key: (candidate: SvgBounds) => number,
): number[] {
  return Array.from(candidates, (_, index) => index).sort(
    (left, right) => key(candidates[left]) - key(candidates[right]),
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
