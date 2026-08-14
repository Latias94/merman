# Family Edit Traces

Each `<family>.json` file is an array of exact UTF-8 replacement traces:

```json
[
  {
    "name": "descriptive unique name",
    "source": "complete Mermaid source",
    "old": "one unique source substring",
    "replacement": "replacement text"
  }
]
```

The runner applies each edit to an existing tree, reparses with reuse, and compares
the complete named tree, fields, byte ranges, and points with a fresh parse. The
`old` substring must occur exactly once so the edit location cannot drift silently.
