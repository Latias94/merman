# merman-markup

Shared Mermaid markup safety, URL normalization, structural CSS predicates, and
Khroma-compatible color operations. Source ingestion and SVG output use the same
implementation while retaining their own admission, resource budgets, and error
conversion.

Default features are empty. Select only the shared work a consumer needs:

| Feature | Module | Responsibility |
| --- | --- | --- |
| Always available | `style` | Structural CSS declaration, paint, and typography predicates |
| `html` | `sanitize` | HTML5 recovery, DOMPurify-compatible rules, representation-aware URI normalization, and controlled HTML/XHTML output |
| `url` | `url` | Mermaid-compatible navigation URL normalization |
| `color` | `theme_color` | Pinned Khroma color parsing, conversion, and adjustment |

The sanitizer accepts `merman_model::config::MermaidConfig` and a caller-owned
`SanitizeOutputSink`. The sink retains control of parsing work, temporary storage,
output allocation, and cancellation. `SanitizeFailure<E>` preserves the caller's
error channel. Color operations use the local `ColorError` type.

The package contains only sanitizer-owned DOMPurify defaults. Mermaid source
parsers, source configuration authorization, theme evaluation, and site defaults
remain with their source owner. Render retains its stricter token-aware CSS
policy, SVG sealing, and output resource accounting.
