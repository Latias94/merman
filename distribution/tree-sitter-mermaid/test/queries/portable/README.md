# Portable Query Evidence

`applicability.json` is the executable 35-family by 9-surface contract for the
portable profile. Portable queries intentionally contain only the four shared
Tree-sitter surfaces:

- `highlights.scm` covers all 35 families.
- `injections.scm` covers typed Event Modeling data blocks and delimiter-free XY
  Chart Markdown content. Families whose Markdown node still includes its
  delimiters remain N/A until a portable offset contract exists.
- `locals.scm` covers only families with a CST-level declaration/reference
  distinction.
- `tags.scm` covers stable named declarations, including declaration-only
  hierarchy entries where tags are useful but locals are not.

Folds, indents, brackets, outline, and textobjects are editor-profile surfaces;
their family rows are explicit `not_applicable` entries here, with the rationale
required by KTD12. Every applicable row names its query and required captures and
is replayed against the family highlight fixture by the query golden runner.
