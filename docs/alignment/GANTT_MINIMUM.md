# Gantt Minimum Slice (Phase 1)

This document defines the initial, test-driven minimum slice for gantt parsing in `merman`.

Baseline: Mermaid `@11.12.2`.

## Supported (current)

- Header:
  - `gantt` (case-insensitive).
- Statements (case-insensitive):
  - `dateFormat <fmt>`
  - `title <text>`
  - `section <name>`
  - `inclusiveEndDates`
  - `topAxis`
  - `axisFormat <fmt>` (stored, not interpreted in Phase 1)
  - `tickInterval <interval>` (stored, not interpreted in Phase 1)
  - `includes <list>`
  - `excludes <list>` (supports `weekends`, weekday names, `YYYY-MM-DD`)
  - `weekday <name>` / `weekend <friday|saturday>`
  - accessibility:
    - `accTitle: ...`
    - `accDescr: ...`
    - `accDescr { ... }` (multi-line, ends at `}`)
    - Text sanitization matches Mermaid `commonDb.ts`:
      - `title` and `accDescr/accTitle` are passed through `sanitizeText(getConfig())`.
      - `accTitle` removes leading whitespace (`/^\s+/`).
      - `accDescr` collapses indentation after newlines (`/\n\s+/g -> "\n"`).
  - interactivity:
    - `click <id[,id...]> href "<url>"`
    - `click <id[,id...]> call <fn>(<args>)`
    - Behavior: `href` is URL-sanitized unless `securityLevel == "loose"`. Callback metadata is recorded only when `securityLevel == "loose"`.
- Tasks:
  - Line form: `<taskTxt>: <taskData>`
  - Task tags extracted from the start of `<taskData>`: `active`, `done`, `crit`, `milestone`, `vert`
  - `<taskData>` comma forms (after tags are removed):
    - `end` (implicit id, start at previous task end)
    - `start, end` (implicit id)
    - `id, start, end`
  - Relative references:
    - `after <id...>` selects the latest referenced `endTime` (fallback: today midnight)
    - `until <id...>` selects the earliest referenced `startTime` (fallback: today midnight)
    - Forward references are resolved via iterative compilation (max 10 passes), mirroring Mermaid.
- End expressions:
  - strict date parsing for `YYYY-MM-DD`, `YYYYMMDD`, `YYYY-MM-DD HH:mm:ss`, `ss`
  - timestamp formats:
    - `x` = Unix epoch milliseconds (Day.js `x`)
    - `X` = Unix epoch seconds (Day.js `X`)
  - duration parsing for `ms|s|m|h|d|w` (floats supported)
  - JS-like fallback parsing for date strings that fail strict parsing, including the “ridiculous year” rejection and common ISO-like inputs:
    - `YYYY-MM-DD` (UTC semantics like JS)
    - `YYYY-MM-DDTHH:mm:ss` / `YYYY-MM-DD HH:mm:ss` (local semantics like JS)
    - `YYYY/MM/DD` and `YYYY/MM/DD HH:mm:ss` (local semantics like JS)
    - timezone offsets like `+0800` / `+08:00`
  - Excludes adjustment:
    - For non-fixed end tasks (i.e. `manualEndTime == false`), `excludes` can extend `endTime` and populate `renderEndTime` (Mermaid `fixTaskDates` parity).

## Output shape (Phase 1)

- The semantic output is a headless snapshot of gantt DB state:
  - `tasks[*].startTime/endTime/renderEndTime` are epoch milliseconds (`i64`)
  - `tasks[*].raw` mirrors Mermaid’s compilation inputs (`raw.startTime/raw.endTime`)
  - `links` and `clickEvents` are emitted for integration layers
    - `clickEvents[*].function_args` is parsed like Mermaid (split on commas, ignoring commas inside double quotes; outer quotes are removed)
    - if no callback args are provided, defaults to `[taskId]` (Mermaid behavior)

## Not yet implemented (Mermaid-supported)

- Full `dayjs` format token parity for arbitrary `dateFormat` values (locale-specific tokens, escaping edge cases, and broader parsing/formatting coverage).
- Full `Date.parse()` compatibility for all JS date string variants.

## Alignment goal

This is an incremental slice. The ultimate goal is full Mermaid `gantt` grammar and DB behavior
compatibility at the pinned baseline tag.
