# Engineering Memory

## Portable Filenames

Keep repository-relative paths within 160 UTF-16 code units, including this directory prefix.
Use filename slugs of at most 40 ASCII characters for timestamped records; keep the full title,
session identifiers and narrative in frontmatter or the body. Preserve the timestamp and full
record ID when shortening an existing filename, and update any incoming links.

When a capture tool's default title slug is too long, use a concise title or its explicit `--path`
option. Run `python scripts/check_path_lengths.py` after staging new records or renames. This
protects Windows consumers that fetch the entire repository as a Cargo Git dependency.

## Core

* [Current State](current-state.md) - Short durable summary of the active engineering state.
* [Update Log](log.md) - Chronological history of meaningful memory updates.

## Concepts

* [Decisions](decisions/) - Durable engineering choices and rationale.
* [Verification](verification/) - Test, build, lint, benchmark, and manual evidence.
* [Conventions](conventions/) - Local repo rules and reusable agent contracts.
