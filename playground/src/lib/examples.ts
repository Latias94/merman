export interface Example {
  id: string;
  name: string;
  category: string;
  code: string;
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
    id: "eventmodeling-full-syntax",
    name: "Event Modeling Full Syntax",
    category: "EventModeling",
    code: `eventmodeling
timeframe 01 event Start`,
  },
  {
    id: "eventmodeling-qualified-names",
    name: "Event Modeling Qualified Names",
    category: "EventModeling",
    code: `eventmodeling

timeframe 02 ui UI
tf 01 evt Product.PriceChanged
tf 03 evt Cart.ItemAdded`,
  },
  {
    id: "eventmodeling-resetframe",
    name: "Event Modeling Reset Frame",
    category: "EventModeling",
    code: `eventmodeling

tf 02 ui UI
resetframe 01 evt Product.PriceChanged
tf 03 evt Cart.ItemAdded`,
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
