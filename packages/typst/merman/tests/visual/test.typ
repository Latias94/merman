#import "@preview/merman:0.1.0": mermaid

#set page(width: 16cm, margin: 12mm)

= Visual smoke gallery

#mermaid("flowchart TD
  A[Flowchart] --> B[Rendered SVG]
", width: 100%, alt: "Flowchart visual smoke")

#mermaid("sequenceDiagram
  participant User
  participant Typst
  User->>Typst: Compile document
  Typst-->>User: Embed SVG
", width: 100%, alt: "Sequence visual smoke")

#mermaid("classDiagram
  class Animal
  Animal : +name
  Animal <|-- Dog
", width: 100%, alt: "Class visual smoke")

#mermaid("erDiagram
  CUSTOMER ||--o{ ORDER : places
  ORDER ||--|{ LINE-ITEM : contains
", width: 100%, alt: "ER visual smoke")

#mermaid("stateDiagram-v2
  [*] --> Still
  Still --> Moving
  Moving --> Still
", width: 100%, alt: "State visual smoke")

#mermaid("gitGraph
  commit
  branch develop
  checkout develop
  commit
", width: 100%, alt: "Git graph visual smoke")

Visual fixture passed.
