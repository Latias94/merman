# Distribution sources

This directory owns source inputs that are assembled into published packages or registry entries.

- `typst/merman/` is the source package submitted to the Typst registry.
- `cli/registry-templates/` contains the Scoop and WinGet templates used by CLI release tooling.

Generated release archives and binaries do not belong here. They continue to be written under
the ignored `dist/` output directory.
