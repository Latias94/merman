# Historical theme-retirement gate removal

The October 9, 2026 theme-path refactor removes the executable KTD23 migration witness.
Before removal, it authorized a frozen inventory of 80 retired projection routes using
160 transparent/solid probes. Its bridge implementation emitted no assignments and
existed only under unit-test or workspace-private acceptance configuration. It did not
lower production themes.

Historical snapshot: revision `0224a679d4b4deb5f273f64a7388b28bfad253af`, manifest version 9. The frozen
SHA-256 boundaries below identify the removed witness; they are archival values, not
new release requirements.

- `EXPECTED_HISTORICAL_WITNESS_DIGEST`: `56b791337c5684dfd065c37a2be3b8c018effdc8278cf2a1edec93ed58ec2fae`
- `EXPECTED_PRODUCTION_INVENTORY_DIGEST`: `03c93f853e6af360df9738135d54a347b6a2efb3462940515c1336551332162a`
- `EXPECTED_RECEIPT_REPORT_DIGEST`: `08ddee7a01bc4b4fe205e24ff279da70338f51b11a94396566b8a4fef9c1d526`

The historical renderer inventories, tombstone identities, acceptance authorization,
the self-testing cutover authorization manifest, fixed CSS projection fixtures, private
exports, and their dedicated CI/release commands
were removed together. The original evidence remains in Git history and in
[the October 8 inventory](../../../performance/theme_retirement_and_deduplication_2026-10-08.md)
and [the earlier release verification](2026-09-15-theme-retirement-release-head.md).

The separate cutover authorization manifest (version 90) had no caller outside its own
unit tests. Its archived SHA-256 was
`73ed3fec108884181557db1b560040fcf2401e4c5c0107b23bc78bbaddf7190c`.
Removing it does not remove observations used by current renderer receipt tests.

The private PNG route-pair exporter also had no remaining consumer. Its facade pair
and digest metadata, document-only retained route/binding receipts, and exporter-side
`raster_paint_cutover` implementation were removed together. The exporter no longer
needs a direct `sha2` dependency for that private witness. Normal PNG encoding and
native admission still use their existing implementation and checks.

The corresponding renderer `theme_raster_paint` receipt chain had no other consumer.
Its family report retention, Sequence-only terminal geometry bookkeeping, private
exports, and self-tests were removed with the orphan exporter. This chain was
independent of current route receipt sealing. Sequence continues to record winning
styles and actual emitted line/effect counts for its current runtime evidence.

Current guarantees remain with current consumers:

- `family_mechanism_matrix` retains its narrow assertion that no route is classified as
  a legacy compatibility route. It does not compare a frozen historical digest.
- Preset qualification and support-discovery tests still run in CI and release preflight.
- Block text, Class label-background, and Flowchart marker tests still compare actual SVG
  terminals and native pixels. Their historical filenames do not imply historical-only
  behavior; they remain current behavior checks.
- The current upstream Class label-background selector check moved into the Class
  label-background test. Its fixture checks survive; the historical CSS digest does not.
- Current renderer cutover observations and receipts remain available to renderer tests;
  this removal does not change renderer/native admission or resource policy.

Source/reference and workflow-security checks establish the deletion boundary. Runtime
validation belongs to the serial owner checks for the complete refactor and must not be
inferred from this historical record.
