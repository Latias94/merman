# Engineering Memory

## Portable Filenames

Keep repository-relative paths within 160 UTF-16 code units, including this directory prefix.
Use filename slugs of at most 40 ASCII characters for timestamped records; keep the full title,
session identifiers and narrative in frontmatter or the body. Preserve the timestamp and full
record ID when shortening an existing filename, and update any incoming links.

When a capture tool's default title slug is too long, use a concise title or its explicit `--path`
option. Run `python scripts/check_path_lengths.py` after staging new records or renames. This
protects Windows consumers that fetch the entire repository as a Cargo Git dependency.

## Current guidance

Use the [documentation index](../../README.md) for maintained guides and the
[alignment dashboard](../../alignment/STATUS.md) for the current Mermaid target. Dated records
below preserve the revision and scope of their observations; they do not select today's baseline
or advertise an active agent session.

## Concepts

* [Decisions](decisions/) - Durable engineering choices and rationale.
* [Verification](verification/) - Test, build, lint, benchmark, and manual evidence.
* [Implementation checkpoints](progress/) - Dated progress and integration evidence.
* [Investigations](subagents/) - Focused findings and source audits.
