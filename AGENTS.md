# Agent Instructions

## Open Source Collaboration

This repository is public. Keep agent-facing instructions clear, respectful, and safe for external contributors.

- Prefer feature branches for implementation work, but a local `main` worktree is acceptable when explicitly requested by the maintainer.
- Before syncing with `main`, fetch the remote and verify whether the update is a fast-forward, merge, or conflict.
- Do not discard local work with `git reset`, `git restore`, `git checkout`, `git clean`, or `git stash` unless the maintainer explicitly asks for that exact operation.
- When unrelated local changes or untracked files are present, leave them untouched and stage only the files required for the current task.
- If a branch pointer needs to be moved without checking it out, verify first that the move is a fast-forward and that the branch has no local-only commits.

## Mermaid Parity Strategy

For Mermaid parity work, prefer source-backed semantic and structural convergence over forced
pixel-perfect matching. Do not introduce brittle hacks, broad magic-number tuning, or model
distortions only to make a fixture match.

Prioritize:

- parser, model, layout, and render semantics from the pinned Mermaid source;
- stable SVG DOM structure and config/theme behavior;
- family-local evidence before main-matrix admission.

Treat browser-dependent behavior as a bounded residual unless there is a robust source-backed fix:

- text measurement;
- `getBBox()` floats;
- `foreignObject` and HTML labels;
- font rendering;
- D3 wrapper noise;
- RoughJS and hand-drawn output.

Comparator normalization must be narrow and non-semantic. Accepted residuals should be documented
rather than hidden.

When refreshing Mermaid baselines, align DOM-id assertions with the current upstream SVG shape
instead of historical forms. Mindmap, Flowchart, and related diagrams may use diagram-prefixed
node ids such as `fixture-name-node_1`; tests and debug tools should read the current baseline or
rendered SVG before hard-coding node selectors.
