# Prereleases and channel backfills

Read this branch for a new alpha/beta/release-candidate graph or an explicitly requested
channel-only publication. Stable `0.x` versions without a prerelease suffix are regular releases.

## New prerelease graph

1. Use the Version Checklist's projection. Coupled workspace requirements are exact
   `=X.Y.Z-alpha.N`, `=X.Y.Z-beta.N`, or `=X.Y.Z-rc.N`; stable dependencies use compatible
   requirements. Validate with `scripts/release-version.py`.
2. Run `python scripts/verify_prerelease_compatibility.py --version <candidate> --previous-version <previous>`.
   It builds fresh candidate consumers and, when versions share a Cargo compatibility line,
   previous facades with candidate siblings available. The workspace lockfile does not prove that
   downstream resolution works.
3. Resolve a same-line previous-facade failure by restoring the compile boundary or selecting a
   new compatibility line. Use `--allow-missing-previous` only after verifying that no prior
   workspace tag exists.

Done: fresh downstream resolution and the required previous-facade lane compile.

Qualify the projected version at its frozen source with `scope=release`; unreleased
`scope=bindings` evidence does not admit a new prerelease. Use the version projection's npm
dist-tag and Python PEP 440 mapping. Main-line Flutter uses the workspace `v<version>` tag as the
native source anchor and Release destination. Push its `flutter-v<version>` authentication tag only
after that primary Release exists; this triggers publication without a separate Flutter Release.
The existing pub.dev publisher pattern and 0.9.x maintenance workflow remain unchanged. Only manual
publication recovery needs the optional pub.dev dispatch event and an authentication-tag workflow
ref; see Required Credentials.
Keep a new compatibility line's intentional breaks in migration notes; do not infer same-line
backward compatibility or universal theme qualification from an alpha label.

## Existing prerelease backfill or channel-only test

Keep already accepted tags and packages immutable. For a historical prerelease published before
the admission gate existed, record its compatibility exception and use owner checks for missing
artifacts; the historical exception does not admit a new prerelease.

For a requested channel-only test, preserve the exact channel package/runtime versions and leave
unselected workspace development at `[Unreleased]`. Bind the reviewed source commit into its
owner manifest. A newer source than the same-named workspace tag is a distinct publication
snapshot, so record that source mapping and provenance separately. Keep other publishers outside
the requested scope.

For a Flutter facade-only snapshot, follow [recovery](recovery.md): the newer reviewed facade
commit needs `native_run_id`, while the unchanged native manifest must still identify the workspace
tag's source. Requalify the facade against the reused bytes; a channel backfill cannot rewrite an
accepted package or historical native download URL.

Done: the channel artifact's source, version and provenance are verified, with any historical
exception or cross-channel source difference recorded in the publication receipt.
