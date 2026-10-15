# C7a release-gate verification — qualification and artifact closure

This record captures the current release-gate checks for the presentation-theme-model candidate. It
is evidence for the gate implementation; it does not declare C7a eligible or promote public catalog
cells.

## Verification

At the current candidate source, the following checks passed:

```text
python3 -m unittest scripts/test_qualify_theme_presets.py scripts/test_verify_cli_release_archive.py
Ran 63 tests in 4.532s — OK

python3 scripts/verify_artifact_dependency_closures.py --representative-targets
All representative profiles passed, including CLI, Python, Typst, Node/Web, FFI, Flutter, and
native SDK closure checks.
```

The qualification tests cover clean-build identity binding, executable/archive digest matching,
actual CLI output replay, companion catalog projection, stale-record rejection, and preservation of
an intentionally unqualified production catalog. Artifact closure checks validate the descriptor-
owned dependency boundaries for the representative release profiles.

## Boundary

Public production catalogs remain `qualified_cells = []`. Qualified cells are emitted only in the
archive-bound companion after a fresh execution against the exact extracted artifact. These checks
therefore establish that the release gate fails closed and that its evidence is artifact-bound; they
do not provide the remaining first-party rollout, complete C5 primary-writer tranche, or contract
freeze required for C7a.
