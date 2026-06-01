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
