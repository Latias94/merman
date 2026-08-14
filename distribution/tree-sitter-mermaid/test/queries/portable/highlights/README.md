# Portable Highlight Golden Files

Each family golden consists of two files with the same slug:

- `<family>.mmd` is the UTF-8 source input.
- `<family>.captures.json` records the normalized capture list.

The JSON shape is:

```json
{
  "schemaVersion": 1,
  "profile": "portable",
  "surface": "highlights",
  "source": "<family>.mmd",
  "captures": [
    {
      "name": "keyword",
      "text": "flowchart",
      "startByte": 0,
      "endByte": 9
    }
  ]
}
```

Capture order is normalized by start byte, end byte, capture name, and text so the
golden is stable across the C, Rust, Node, and WASM bindings. A family golden must
contain at least one family-owned named capture; an empty query result is not valid
evidence.
