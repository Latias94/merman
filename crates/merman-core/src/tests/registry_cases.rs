//! Private parser catalog cases shared with renderer integration tests.

pub(super) const MALFORMED_SOURCE: &str = "not-a-mermaid-diagram\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CharacterizedCapabilities {
    pub(super) semantic: bool,
    pub(super) editor: bool,
    pub(super) combined: bool,
    pub(super) typed: bool,
}

const COMBINED_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: true,
    editor: true,
    combined: true,
    typed: true,
};
const ERROR_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: true,
    editor: false,
    combined: false,
    typed: true,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MalformedContract {
    StrictAcceptsEditorAvailable,
    StrictAcceptsEditorUnavailable,
    StrictRejectsEditorAvailable,
    StrictRejectsEditorUnavailable,
    Unsupported,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FamilyCharacterization {
    pub(super) variant_id: &'static str,
    pub(super) logical_family: &'static str,
    pub(super) representative_source: &'static str,
    pub(super) malformed_source: &'static str,
    pub(super) capabilities: CharacterizedCapabilities,
    pub(super) malformed_contract: MalformedContract,
}

macro_rules! combined_family {
    ($variant_id:literal, $logical_family:literal, $representative_source:expr) => {
        FamilyCharacterization {
            variant_id: $variant_id,
            logical_family: $logical_family,
            representative_source: $representative_source,
            malformed_source: MALFORMED_SOURCE,
            capabilities: COMBINED_CAPABILITIES,
            malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
        }
    };
}

macro_rules! combined_family_accepting_malformed_source {
    ($variant_id:literal, $logical_family:literal, $representative_source:expr) => {
        FamilyCharacterization {
            variant_id: $variant_id,
            logical_family: $logical_family,
            representative_source: $representative_source,
            malformed_source: MALFORMED_SOURCE,
            capabilities: COMBINED_CAPABILITIES,
            malformed_contract: MalformedContract::StrictAcceptsEditorAvailable,
        }
    };
}

// This is deliberately one matrix. A Mermaid baseline is a single language catalog, so every
// family gets the same parser/editor/typed-render admission contract regardless of Cargo features.
pub(super) const FAMILY_CHARACTERIZATION_MATRIX: &[FamilyCharacterization] = &[
    FamilyCharacterization {
        variant_id: "error",
        logical_family: "error",
        representative_source: "error\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: ERROR_CAPABILITIES,
        malformed_contract: MalformedContract::StrictAcceptsEditorUnavailable,
    },
    combined_family!("flowchart-elk", "flowchart", "flowchart-elk TD\nA-->B\n"),
    combined_family!("flowchart-v2", "flowchart", "flowchart TD\nA-->B\n"),
    combined_family!("flowchart", "flowchart", "graph TD\nA-->B\n"),
    combined_family!("swimlane", "swimlane", "swimlane-beta LR\nA-->B\n"),
    combined_family!("mindmap", "mindmap", "mindmap\n  root\n    child\n"),
    combined_family!(
        "architecture",
        "architecture",
        "architecture-beta\n  service api(server)[API]\n"
    ),
    combined_family!("zenuml", "zenuml", "zenuml\n  Alice->Bob: Hello\n"),
    combined_family!(
        "sequence",
        "sequence",
        "sequenceDiagram\nAlice->>Bob: Hello\n"
    ),
    combined_family!("c4", "c4", "C4Context\nPerson(user, \"User\")\n"),
    combined_family!("kanban", "kanban", "kanban\n  Todo\n    item1\n"),
    combined_family!("classDiagram", "class", "classDiagram\nclass Animal\n"),
    combined_family!("class", "class", "classDiagram\nclass Animal\n"),
    combined_family!("er", "er", "erDiagram\nCUSTOMER\n"),
    combined_family!("erDiagram", "er", "erDiagram\nCUSTOMER\n"),
    combined_family!(
        "gantt",
        "gantt",
        "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2024-01-01, 1d\n"
    ),
    combined_family_accepting_malformed_source!("info", "info", "info\n"),
    combined_family_accepting_malformed_source!("pie", "pie", "pie\n\"A\": 1\n"),
    combined_family!(
        "requirement",
        "requirement",
        "requirementDiagram\nrequirement req1 {\n  id: 1\n  text: Test\n  risk: low\n  verifymethod: analysis\n}\n"
    ),
    combined_family!("timeline", "timeline", "timeline\n2024 : Event\n"),
    combined_family!("gitGraph", "gitGraph", "gitGraph\ncommit id:\"first\"\n"),
    combined_family!("stateDiagram", "state", "stateDiagram-v2\n[*] --> Idle\n"),
    combined_family!("state", "state", "stateDiagram\n[*] --> Idle\n"),
    combined_family!("journey", "journey", "journey\nsection Work\nTask: 5\n"),
    combined_family!(
        "quadrantChart",
        "quadrantChart",
        "quadrantChart\nx-axis Low --> High\ny-axis Low --> High\nA: [0.5, 0.5]\n"
    ),
    combined_family!("sankey", "sankey", "sankey\nA,B,1\n"),
    combined_family!("packet", "packet", "packet-beta\n0-7: \"A\"\n"),
    combined_family!("xychart", "xychart", "xychart-beta\nline [10, 30, 20]\n"),
    combined_family!("block", "block", "block\n  a b c\n"),
    combined_family!(
        "eventmodeling",
        "eventmodeling",
        "eventmodeling\ntf 01 ui Shop.Cart\n"
    ),
    combined_family!("treeView", "treeView", "treeView-beta\n  root\n    child\n"),
    combined_family!(
        "radar",
        "radar",
        "radar-beta\naxis A,B,C\ncurve sample{1,2,3}\n"
    ),
    combined_family!(
        "ishikawa",
        "ishikawa",
        "ishikawa-beta\n  Effect\n    Cause\n"
    ),
    combined_family!(
        "treemap",
        "treemap",
        "treemap-beta\n\"Root\"\n  \"Child\": 1\n"
    ),
    combined_family!(
        "railroad",
        "railroad",
        "railroad-beta\nrule = terminal(\"a\") ;\n"
    ),
    combined_family!(
        "railroadEbnf",
        "railroad",
        "railroad-ebnf-beta\nrule = \"a\" ;\n"
    ),
    combined_family!(
        "railroadAbnf",
        "railroad",
        "railroad-abnf-beta\nrule = \"a\" ;\n"
    ),
    combined_family!(
        "railroadPeg",
        "railroad",
        "railroad-peg-beta\nrule <- \"a\" ;\n"
    ),
    combined_family!(
        "venn",
        "venn",
        "venn-beta\nset Frontend\nset Backend\nunion Frontend,Backend[\"API\"]\n"
    ),
    combined_family!(
        "wardley",
        "wardley",
        "wardley-beta\ncomponent API [0.6, 0.7]\n"
    ),
    combined_family!("cynefin", "cynefin", "cynefin-beta\n  complex\n"),
    combined_family!("agentflow", "agentflow", "agentflow-beta\nA --> B\n"),
    combined_family!(
        "usecase",
        "usecase",
        "usecase-beta\nactor Customer(\"Customer\")\nCheckout(\"Place order\")\nCustomer --> Checkout\n"
    ),
];
