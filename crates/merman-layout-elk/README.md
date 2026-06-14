# merman-layout-elk

`merman-layout-elk` is the optional ELK layout engine integration point for
`merman`.

The crate exists so ELK-specific dependencies and algorithms can be developed,
tested, and feature-gated outside `merman-render`. The first supported target is
Mermaid's default ELK behavior for `flowchart-elk` / `layout: elk`, which maps
to the layered ELK algorithm.

This crate currently exposes the package boundary and feature surface only. The
actual ELK algorithm integration will be added behind this boundary before the
renderer advertises full `flowchart-elk` support.
