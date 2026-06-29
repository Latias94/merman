export interface Example {
  id: string;
  name: string;
  category: string;
  code: string;
  asciiSupported?: boolean;
}

export const examples: Example[] = [
  {
    id: "flowchart-basic",
    name: "Basic Flowchart",
    category: "Flowchart",
    code: `flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D`,
  },
  {
    id: "flowchart-complex",
    name: "Complex Flowchart",
    category: "Flowchart",
    code: `flowchart LR
    subgraph Input
        A[User Request] --> B[API Gateway]
    end
    subgraph Processing
        B --> C{Auth Check}
        C -->|Pass| D[Business Logic]
        C -->|Fail| E[Return 401]
        D --> F[Data Processing]
    end
    subgraph Storage
        F --> G[(Database)]
        F --> H[(Cache)]
    end
    G --> I[Response]
    H --> I
    E --> I`,
  },
  {
    id: "flowchart-datastore",
    name: "Flowchart Data Store",
    category: "Flowchart",
    code: `flowchart LR
    Source@{ shape: lean-r, label: "Event stream" } --> Load[Normalize]
    Load --> Store@{ shape: datastore, label: "Warehouse" }
    Store --> Report@{ shape: doc, label: "Daily report" }`,
  },
  {
    id: "sequence-basic",
    name: "Basic Sequence",
    category: "Sequence",
    code: `sequenceDiagram
    participant U as User
    participant B as Browser
    participant S as Server
    U->>B: Click Login
    B->>S: POST /login
    S-->>B: Return Token
    B-->>U: Show Success`,
  },
  {
    id: "sequence-async",
    name: "Async Sequence",
    category: "Sequence",
    code: `sequenceDiagram
    autonumber
    participant C as Client
    participant S as Server
    participant D as Database
    participant Q as Message Queue

    C->>+S: Submit Order
    S->>D: Save Order
    D-->>S: Confirm
    S->>Q: Send Notification
    S-->>-C: Return Order ID

    Note over Q: Async Processing
    Q->>S: Consume Message
    S->>C: Push Notification`,
  },
  {
    id: "sequence-decimal-autonumber",
    name: "Decimal Autonumber",
    category: "Sequence",
    code: `sequenceDiagram
    autonumber 1.1 0.1
    participant App
    participant API
    participant DB
    App->>API: Submit request
    API->>DB: Persist command
    DB-->>API: Stored
    API-->>App: Accepted`,
  },
  {
    id: "sequence-control-flow",
    name: "Control Flow",
    category: "Sequence",
    code: `sequenceDiagram
    participant Client
    participant API
    participant Worker
    Client->>API: Submit job
    alt Valid request
      API->>Worker: Queue work
      loop Poll status
        Client->>API: GET /jobs/123
        API-->>Client: Running
      end
    else Invalid request
      API-->>Client: 400 Bad Request
    end`,
  },
  {
    id: "class-basic",
    name: "Basic Class Diagram",
    category: "Class",
    code: `classDiagram
    class Animal {
        +String name
        +int age
        +makeSound()
    }
    class Dog {
        +String breed
        +bark()
    }
    class Cat {
        +String color
        +meow()
    }
    Animal <|-- Dog
    Animal <|-- Cat`,
  },
  {
    id: "class-nested-namespace",
    name: "Nested Namespace",
    category: "Class",
    asciiSupported: false,
    code: `classDiagram
    namespace Platform["Platform Layer"] {
      namespace FFI {
        class DartBinding
        class PythonBinding
      }
      namespace Core {
        class Renderer
      }
    }
    Platform.FFI.DartBinding --> Platform.Core.Renderer : calls
    Platform.FFI.PythonBinding --> Platform.Core.Renderer : calls`,
  },
  {
    id: "class-generics-interfaces",
    name: "Generics and Interfaces",
    category: "Class",
    code: `classDiagram
    direction TB
    class Repository~T~
    <<interface>> Repository~T~
    class Service~T~ {
      +get(id: String) T
    }
    class SqlRepo~T~ {
      +get(id: String) T
    }
    Repository~T~ <|.. SqlRepo~T~
    Service~T~ ..> Repository~T~ : depends`,
  },
  {
    id: "state-basic",
    name: "State Diagram",
    category: "State",
    code: `stateDiagram-v2
    [*] --> Pending
    Pending --> Processing: Start
    Processing --> Completed: Success
    Processing --> Failed: Error
    Failed --> Pending: Retry
    Completed --> [*]`,
  },
  {
    id: "state-composite-concurrency",
    name: "Composite State",
    category: "State",
    code: `stateDiagram-v2
    [*] --> Active
    state Active {
      [*] --> Idle
      Idle --> Saving: change
      Saving --> Idle: saved
      --
      [*] --> Online
      Online --> Offline: lost connection
      Offline --> Online: reconnect
    }
    Active --> [*]: close`,
  },
  {
    id: "er-basic",
    name: "ER Diagram",
    category: "ER",
    code: `erDiagram
    USER ||--o{ ORDER : places
    USER {
        int id PK
        string name
        string email
    }
    ORDER ||--|{ ORDER_ITEM : contains
    ORDER {
        int id PK
        date created_at
        string status
    }
    ORDER_ITEM {
        int id PK
        int quantity
        float price
    }
    PRODUCT ||--o{ ORDER_ITEM : "ordered in"
    PRODUCT {
        int id PK
        string name
        float price
    }`,
  },
  {
    id: "er-relationships-and-styles",
    name: "Relationships and Styles",
    category: "ER",
    code: `erDiagram
    CAR ||--o{ DRIVER : "insured for"
    CAR }o--|| PERSON : "owned by"
    NODE ||--o{ NODE : "leads to"
    BOOK["Book"]:::core {
      string *title PK "Title"
      string[] author-ref[name](1) FK "Author ref"
    }
    BOOK ||--o{ PAGE : has
    PAGE {
      int number PK
    }
    classDef core fill:#f96,stroke:#333,stroke-width:2px,color:#fff
    class BOOK core`,
  },
  {
    id: "gantt-basic",
    name: "Gantt Chart",
    category: "Gantt",
    code: `gantt
    title Project Development Plan
    dateFormat  YYYY-MM-DD
    section Design
    Requirements    :a1, 2024-01-01, 7d
    UI Design       :a2, after a1, 10d
    section Development
    Frontend Dev    :b1, after a2, 15d
    Backend Dev     :b2, after a2, 15d
    section Testing
    Integration     :c1, after b1, 7d
    User Testing    :c2, after c1, 5d`,
  },
  {
    id: "gantt-tags-and-calendar",
    name: "Tags and Calendar",
    category: "Gantt",
    code: `gantt
    title Tagged Release Plan
    dateFormat YYYY-MM-DD
    excludes weekends 2026-02-16,friday
    section Build
    Parser Freeze  :done, parser, 2026-02-09, 2d
    Renderer Pass  :active, render, after parser, 4d
    section Validation
    Browser Matrix :crit, qa, after render, 3d`,
  },
  {
    id: "pie-basic",
    name: "Pie Chart",
    category: "Pie",
    code: `pie showData
    title Tech Stack Usage
    "React" : 45
    "Vue" : 30
    "Angular" : 15
    "Svelte" : 10`,
  },
  {
    id: "pie-decimal-values",
    name: "Decimal Values",
    category: "Pie",
    code: `pie
    title Build Time Share
    "Parse" : 42.5
    "Layout" : 31.25
    "Render" : 26.25`,
  },
  {
    id: "mindmap-basic",
    name: "Mindmap",
    category: "Mindmap",
    code: `mindmap
  root((Merman))
    Parser
      Lexer
      Syntax Analysis
      AST Builder
    Renderer
      SVG Output
      PNG Output
      PDF Output
    Themes
      default
      dark
      forest`,
  },
  {
    id: "mindmap-shapes-icons",
    name: "Shapes and Icons",
    category: "Mindmap",
    code: `mindmap
  root((Merman))
    Parser[Parser]
      :::urgent large
      ::icon(fa fa-code)
      Lexer
      AST
    Renderer(Renderer)
      ::icon(fa fa-image)
      SVG[SVG output]
      PNG(PNG output)
    Themes
      default
      dark`,
  },
  {
    id: "gitgraph-basic",
    name: "Git Graph",
    category: "Git",
    code: `gitGraph
    commit
    commit
    branch develop
    checkout develop
    commit
    commit
    checkout main
    merge develop
    commit
    branch feature
    checkout feature
    commit
    checkout main
    merge feature`,
  },
  {
    id: "gitgraph-tags-cherrypick",
    name: "Tags and Cherry-pick",
    category: "Git",
    code: `gitGraph
    commit id: "base"
    branch feature
    checkout feature
    commit id: "parser-fix"
    checkout main
    commit id: "release" tag: "v1.0"
    cherry-pick id: "parser-fix" tag: "backport"`,
  },
  {
    id: "timeline-basic",
    name: "Timeline",
    category: "Timeline",
    code: `timeline
    title Merman Development
    section 2024
        Q1 : Project Started
           : Core Parser Dev
        Q2 : SVG Renderer
           : Flowchart Support
        Q3 : Full Diagram Support
           : WASM Compilation
    section 2025
        Q1 : Playground Release
           : Community Feedback`,
  },
  {
    id: "timeline-stacked-events",
    name: "Stacked Events",
    category: "Timeline",
    code: `timeline
    title Runtime Capability Timeline
    section Core
      Parser parity : Flowchart
                    : Sequence
      Renderer parity : SVG
                      : ASCII
    section Bindings
      Web package : WASM API
      Playground : Examples`,
  },
  {
    id: "journey-working-day",
    name: "Working Day Journey",
    category: "Journey",
    code: `journey
    title My working day
    section Go to work
      Make tea: 5: Me
      Go upstairs: 3: Me
      Do work: 1: Me, Cat
    section Go home
      Go downstairs: 5: Me
      Sit down: 5: Me`,
  },
  {
    id: "journey-review-flow",
    name: "Review Flow",
    category: "Journey",
    code: `journey
    title Review flow
    section Authoring
      Write diagram: 5: Author
      Request review: 3: Author, Reviewer
    section Validation
      Run smoke tests: 4: CI, Reviewer
      Merge change: 5: Maintainer`,
  },
  {
    id: "info-show-info",
    name: "Info Diagram",
    category: "Info",
    code: `info showInfo`,
  },
  {
    id: "zenuml-basic",
    name: "Basic ZenUML",
    category: "ZenUML",
    code: `zenuml
  Alice->Bob: Hello
  Bob-->Alice: Reply`,
  },
  {
    id: "zenuml-conditional-flow",
    name: "Conditional Flow",
    category: "ZenUML",
    code: `zenuml
  Client->API: Submit order
  if(valid) {
    API->Worker: Queue fulfillment
  } else {
    API->Client: Reject request
  }`,
  },
  {
    id: "xychart-render-timing",
    name: "Render Timing",
    category: "XY Chart",
    code: `xychart
    title "Render timings"
    x-axis ["Parse", "Layout", "SVG", "PNG"]
    y-axis "ms" 0 --> 120
    bar [12, 34, 58, 96]
    line [10, 28, 50, 85]`,
  },
  {
    id: "xychart-negative-values",
    name: "Negative Values",
    category: "XY Chart",
    code: `xychart
    title "Error budget delta"
    y-axis -2.4 --> 3.5
    line [+1.3, .6, 2.4, -.34]`,
  },
  {
    id: "architecture-binding-stack",
    name: "Binding Stack",
    category: "Architecture",
    code: `architecture-beta
    group edge(cloud)[Edge]
    group core(server)[Core]
    service browser(internet)[Browser] in edge
    service api(server)[API] in core
    service renderer(server)[Renderer] in core
    service cache(database)[Cache] in core
    browser:R --> L:api
    api:R --> L:renderer
    renderer:B --> T:cache`,
  },
  {
    id: "architecture-junctions",
    name: "Junction Fanout",
    category: "Architecture",
    code: `architecture-beta
    service browser(internet)[Browser]
    service api(server)[API]
    service db(database)[Database]
    service queue(server)[Queue]
    junction fanout
    browser:R --> L:fanout
    fanout:R --> L:api
    fanout:B --> T:db
    fanout:T --> B:queue`,
  },
  {
    id: "c4-system-context",
    name: "System Context",
    category: "C4",
    code: `C4Context
    title System Context diagram
    Person(customerA, "Customer", "A customer")
    System(sys, "Banking System", "Does banking")
    Rel(customerA, sys, "Uses")`,
  },
  {
    id: "c4-container-view",
    name: "Container View",
    category: "C4",
    code: `C4Container
    title Playground container view
    Person(user, "Diagram Author", "Edits Mermaid source")
    System_Ext(cdn, "Mermaid CDN", "Optional compare engine")
    Container_Boundary(playground, "Merman Playground") {
      Container(ui, "Web UI", "React", "Editor and preview shell")
      Container(wasm, "WASM Renderer", "Rust", "Parses and renders diagrams")
      ContainerDb(cache, "Settings Cache", "Browser storage", "Stores local preferences")
    }
    Rel(user, ui, "Uses")
    Rel(ui, wasm, "Renders with")
    Rel(ui, cdn, "Compares against")
    Rel(wasm, cache, "Reads config")`,
  },
  {
    id: "c4-dynamic-flow",
    name: "Dynamic Flow",
    category: "C4",
    code: `C4Dynamic
    title Render request flow
    Container(ui, "Web UI", "React", "Collects source and config")
    Container_Boundary(engine, "Render Engine") {
      Component(parser, "Parser", "Rust", "Builds diagram model")
      Component(renderer, "SVG Renderer", "Rust", "Produces SVG")
    }
    ContainerDb(cache, "Theme Cache", "Browser storage", "Stores selected theme")
    Rel(ui, parser, "Submits source")
    Rel(parser, renderer, "Passes layout model")
    Rel(renderer, cache, "Reads theme")`,
  },
  {
    id: "block-render-pipeline",
    name: "Render Pipeline",
    category: "Block",
    code: `block-beta
    columns 3
    source["Source"] parser["Parser"] layout["Layout"]
    source --> parser
    parser --> layout
    layout --> svg["SVG"]`,
  },
  {
    id: "block-shapes-and-styles",
    name: "Shapes and Styles",
    category: "Block",
    code: `block-beta
    columns 4
    input>"Input"]
    parser(("Parser"))
    ast{{"AST"}}
    output["SVG"]
    input --> parser
    parser --> ast
    ast --> output
    classDef focus color:#ffffff,fill:#0f766e
    class parser,ast focus`,
  },
  {
    id: "block-nested-groups",
    name: "Nested Groups",
    category: "Block",
    code: `block-beta
    block
      editor["Editor"]
      preview["Preview"]
    end
    block
      parser["Parser"]
      renderer["Renderer"]
    end`,
  },
  {
    id: "packet-ipv4-header",
    name: "IPv4 Header",
    category: "Packet",
    code: `packet
    +4: "Version"
    +4: "IHL"
    +8: "DSCP"
    +16: "Total Length"
    +16: "Identification"
    +3: "Flags"
    +13: "Fragment Offset"`,
  },
  {
    id: "packet-tcp-header",
    name: "TCP Header",
    category: "Packet",
    code: `packet
    0-15: "Source Port"
    16-31: "Destination Port"
    32-63: "Sequence Number"
    64-95: "Acknowledgment Number"
    96-99: "Data Offset"
    100-105: "Reserved"
    106: "URG"
    107: "ACK"
    108: "PSH"
    109: "RST"
    110: "SYN"
    111: "FIN"
    112-127: "Window"
    128-143: "Checksum"`,
  },
  {
    id: "kanban-release-work",
    name: "Release Work",
    category: "Kanban",
    code: `kanban
    backlog[Backlog]
      api[Define FFI API]@{ assigned: "Core", priority: "High" }
      docs[Write README]@{ assigned: "Docs", priority: "Low" }
    progress[In Progress]
      flutter[Flutter packaging]@{ assigned: "Mobile", priority: "High" }
    done[Done]
      ci[CI matrix]@{ assigned: "Infra", priority: "Very Low" }`,
  },
  {
    id: "kanban-ticket-metadata",
    name: "Ticket Metadata",
    category: "Kanban",
    code: `kanban
    todo[Todo]
      spec[Write parity spec]@{ ticket: "MER-101", assigned: "Docs", priority: "High" }
      audit[Audit fixtures]@{ ticket: "MER-102", assigned: "Core", priority: "Low" }
    progress[In Progress]
      render[Implement renderer]@{ ticket: "MER-103", assigned: "Rust", priority: "Very High" }
    done[Done]
      smoke[Browser smoke test]@{ ticket: "MER-104", assigned: "QA", priority: "Very Low" }`,
  },
  {
    id: "quadrant-integration-priority",
    name: "Integration Priority",
    category: "Quadrant",
    code: `quadrantChart
    title Integration Priority
    x-axis Low Effort --> High Effort
    y-axis Low Impact --> High Impact
    quadrant-1 Strategic
    quadrant-2 Quick Wins
    quadrant-3 Backlog
    quadrant-4 Expensive
    Flutter: [0.35, 0.82]
    Python: [0.28, 0.68]
    Event Modeling: [0.72, 0.45]`,
  },
  {
    id: "quadrant-styled-points",
    name: "Styled Points",
    category: "Quadrant",
    code: `quadrantChart
    title Feature Risk
    x-axis Low Effort --> High Effort
    y-axis Low Risk --> High Risk
    quadrant-1 Major Bets
    quadrant-2 Guarded Bets
    quadrant-3 Safe Wins
    quadrant-4 Risky Work
    Parser: [0.20, 0.80] radius: 8, color: #22c55e
    Renderer: [0.62, 0.70] radius: 10, color: #f97316, stroke-color: #9a3412
    Export: [0.35, 0.32] radius: 6, color: #3b82f6`,
  },
  {
    id: "sankey-render-flow",
    name: "Render Flow",
    category: "Sankey",
    code: `sankey
    Editor,Parser,8
    Parser,Layout,7
    Layout,SVG,6
    Layout,Diagnostics,3
    SVG,Export,4`,
  },
  {
    id: "sankey-shared-nodes",
    name: "Shared Nodes",
    category: "Sankey",
    code: `sankey
    In,A,10
    In,B,8
    In,C,6
    A,X,5
    A,Y,5
    B,Y,3
    B,Z,5
    C,X,2
    C,Z,4
    X,Out 1,7
    Y,Out 1,6
    Z,Out 2,7`,
  },
  {
    id: "radar-binding-coverage",
    name: "Binding Coverage",
    category: "Radar",
    code: `radar-beta
    title Binding Coverage
    axis Rust, WASM, Dart, Python, Swift
    curve Current{90, 80, 60, 55, 45}
    curve Target{95, 90, 85, 80, 75}
    max 100
    min 0`,
  },
  {
    id: "radar-named-values",
    name: "Named Values",
    category: "Radar",
    code: `radar-beta
    title Parser Readiness
    axis Syntax, Errors, Layout
    curve Current{ Syntax: 4, Errors: 3, Layout: 2 }
    curve Target{ Syntax: 5, Errors: 4, Layout: 4 }`,
  },
  {
    id: "treemap-package-surface",
    name: "Package Surface",
    category: "Treemap",
    code: `treemap-beta
    "Bindings"
      "Rust Core": 40
      "Web WASM": 25
      "Flutter": 15
      "Python": 12
      "Swift": 8`,
  },
  {
    id: "treemap-styled-sections",
    name: "Styled Sections",
    category: "Treemap",
    code: `treemap-beta
"Runtime"
  "Parser": 35
  "Renderer":::hot
    "SVG": 25
    "ASCII": 10
"Bindings"
  "Web": 20:::hot
  "CLI": 10

classDef hot fill:#fecaca,color:#7f1d1d,stroke:#f87171;`,
  },
  {
    id: "treeview-package-tree",
    name: "Package Tree",
    category: "TreeView",
    code: `treeView-beta
    "packages"
        "merman"
            "src"
        "merman-core"
            "src"
        "merman-ascii"
            "src"
        "web"
            "src"`,
  },
  {
    id: "requirement-ffi-api",
    name: "FFI API Requirement",
    category: "Requirement",
    code: `requirementDiagram
    requirement ffi_api {
      id: 1
      text: Stable parse layout and render API
      risk: medium
      verifymethod: test
    }
    element wasm {
      type: library
    }
    wasm - satisfies -> ffi_api`,
  },
  {
    id: "requirement-styled-elements",
    name: "Styled Elements",
    category: "Requirement",
    code: `requirementDiagram
    requirement cache_req:::important {
      id: 2
      text: "Cache selected theme"
      risk: low
      verifymethod: inspection
    }
    element local_storage {
      type: database
    }
    local_storage - satisfies -> cache_req
    classDef important font-weight:bold
    class local_storage important
    style local_storage fill:#dbeafe,stroke:#1d4ed8`,
  },
];

export const categories = [
  "All",
  ...Array.from(new Set(examples.map((e) => e.category))),
];

export function getExampleById(id: string): Example | undefined {
  return examples.find((e) => e.id === id);
}

export function getExamplesByCategory(category: string): Example[] {
  if (category === "All") return examples;
  return examples.filter((e) => e.category === category);
}
