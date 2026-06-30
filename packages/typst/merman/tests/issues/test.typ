#import "@preview/merman:0.1.0": mermaid, show-mermaid-blocks

#show raw.where(lang: "mermaid-issue"): show-mermaid-blocks(
  width: 100%,
  error-mode: "placeholder",
)

#mermaid("", error-mode: "placeholder", width: 80%)

```mermaid-issue
flowchart LR
  First[First raw block] --> Shared[No fixed id]
```

```mermaid-issue
flowchart LR
  Second[Second raw block] --> Shared[No fixed id]
```

Issue fixture passed.
