# SDK consumer examples

Date: 2026-10-24. Scope: main and the existing release/0.9.x SDK feature branches.

## Outcome and boundaries

Deliver runnable Go/wazero and Python/wasmtime WASI examples and a C++ native C
ABI consumer alongside the existing C examples. Package the source examples with
their SDK and execute them against the actual built/extracted SDK in the existing
qualification owner. No new public Go/Python package, browser runtime, business
API, dynamic plugin layer, version bump or remote publication is implied.

Use the unchanged WASI ABI 1 and native ABI 3. Match each branch's options schema
through its loaded catalog rather than translating main's schema into maintenance.
Keep examples small, with explicit memory/result ownership, copied font bytes,
structured ordinary errors, SVG/PNG output, scale/matte and serial reuse. Document
instance replacement after traps or interrupted calls; do not call the smoke
harness a production language SDK or promise complete CLI argument compatibility.

## Units and ownership

- Go: `distribution/sdk/examples/go-wasi/`, using the existing pinned wazero
  dependency and a small reusable serial host plus a runnable consumer.
- Python: `distribution/sdk/examples/python-wasi/`, using the existing pinned
  wasmtime dependency, bounded store fuel/memory and a runnable serial consumer.
- Native: `distribution/sdk/examples/render.cpp` and CMake shared/static targets.
- Integration: `scripts/sdk_archive.py`, `scripts/sdk_qualify.py`, their focused
  tests, SDK documentation, and the existing release owner as needed.
- Acceptance: actual WASI artifacts on both lines, extracted native libraries,
  ordinary error recovery, caller fonts, scale/matte, and direct matched CLI/WASI
  comparison where the same feature/font/config inputs can be supplied.

The integration owner runs authoritative tests and all Cargo work serially.
Language authoring may use isolated scratch directories; workers may not touch
the canonical working tree or Git index. Existing uncommitted changes are from
the preceding SDK work and are preserved. No default-branch worktree is created.

## Evidence and completion

First strengthen archive/qualification tests and observe the missing example
payload/verification. Execute source examples against existing qualified bytes
while development is dirty. Use the existing clean-source qualification command
for a final archive only after a source commit is available. A test/mock alone is
not evidence of actual module or installed-library execution. Record exact
artifact/source identities and any unexecuted target lanes honestly.

Check Python contracts, Go tests, CMake/CTest, workflow lint when touched, local
path lengths and whitespace. Require an independent read-only review before
handoff. Preserve previous work; do not publish, push, merge or claim remote CI
success in this task.
