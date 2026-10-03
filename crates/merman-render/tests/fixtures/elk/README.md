# ELK crossing fixtures

These family-local K3,3 graphs exercise the shared Mermaid 12 ELK line-hop contract.
`elk_cross_family_semantics_test.rs` renders each input with line hops disabled,
with arcs, with gaps, and with the default setting.

The tests require actual path changes while preserving layout geometry, node and
label transforms, relationship count, and marker references. Family-owned tests
also cover Neo masks, squeezed crossings, explicit styles, and label modes.
The inputs are kept outside the main SVG corpus because these assertions test
semantic invariants without requiring browser font measurements or new baseline
receipts.
