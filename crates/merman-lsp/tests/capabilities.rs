use std::{fs, path::PathBuf};

use merman_analysis::FenceTextIndexSource;
use merman_lsp::document_store::DocumentStore;
use tower_lsp::lsp_types::Url;

#[test]
fn product_families_are_parser_backed_and_role_aware() {
    struct CapabilityCase<'a> {
        label: &'a str,
        expected_source: FenceTextIndexSource,
        snippet: &'a str,
        required_ids: &'a [&'a str],
        required_outline: &'a [&'a str],
        required_prefixes: &'a [&'a str],
        forbidden_ids: &'a [&'a str],
    }

    let cases = [
        CapabilityCase {
            label: "flowchart",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "flowchart TD\nA-->B\n",
            required_ids: &["A", "B"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "flowchart-recovered",
            expected_source: FenceTextIndexSource::ParserRecovered,
            snippet: "flowchart TD\nsubgraph group\nA-->B\nC-->",
            required_ids: &["A", "C"],
            required_outline: &["group"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "sequence",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "sequenceDiagram\nAlice->>Bob: Hi\n",
            required_ids: &["Alice", "Bob"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "sequence-recovered",
            expected_source: FenceTextIndexSource::ParserRecovered,
            snippet: "sequenceDiagram\nAlice->>Bob: Hi\nBob->>",
            required_ids: &["Alice", "Bob"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "state",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "stateDiagram-v2\n",
                "Idle --> Running\n",
                "state \"Paused State\" as Paused\n",
                "classDef activeStyle fill:#0f0,border:#333\n",
                "class Idle, Running activeStyle\n",
                "style Running fill:#f00\n",
            ),
            required_ids: &["Idle", "Running"],
            required_outline: &["activeStyle"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "class",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "classDiagram\n",
                "class User {\n",
                "  +login()\n",
                "}\n",
                "class User:::service\n",
                "classDef service fill:#eee\n",
                "User <|-- Admin\n",
            ),
            required_ids: &["User", "Admin"],
            required_outline: &["+login()", "service"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "er",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "erDiagram\nCUSTOMER ||--o{ ORDER : places\n",
            required_ids: &["CUSTOMER", "ORDER"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "gantt",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "gantt\n",
                "title Roadmap\n",
                "accTitle: Roadmap chart\n",
                "accDescr: Shows release tasks\n",
                "section Demo\n",
                "Task 1: id1,2014-01-01,1d\n",
                "Task 2: id2,after id1,2d\n",
            ),
            required_ids: &["id1", "id2"],
            required_outline: &["Demo"],
            required_prefixes: &["title", "accTitle", "accDescr", "section"],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "mindmap",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "mindmap\nroot(Root Node)\n child1(Child 1)\n child2\n",
            required_ids: &["root", "child1", "child2"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &["Root", "Node", "Child", "1"],
        },
        CapabilityCase {
            label: "gitGraph",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "gitGraph\n",
                "commit id:\"C1\"\n",
                "branch feature\n",
                "checkout feature\n",
                "merge main id:\"M1\"\n",
            ),
            required_ids: &["C1", "feature", "main", "M1"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "radar",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "radar-beta\n",
                "title Radar diagram\n",
                "accTitle: Radar accTitle\n",
                "accDescr: Radar accDescription\n",
                "axis A[\"Axis A\"], B[\"Axis B\"], C[\"Axis C\"]\n",
                "curve mycurve[\"My Curve\"]{1,2,3}\n",
            ),
            required_ids: &["A", "B", "C", "mycurve"],
            required_outline: &[],
            required_prefixes: &["title", "accTitle", "accDescr"],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "kanban",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "kanban\n    root\n      child1\n",
            required_ids: &["child1"],
            required_outline: &["root"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "treemap",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "treemap\n",
                "title Treemap Title\n",
                "accTitle: Treemap accTitle\n",
                "accDescr: Treemap accDescr\n",
                "\"Root\"\n",
                "  \"Leaf\": 42 :::highlight\n",
                "classDef highlight fill:#f00\n",
            ),
            required_ids: &["Root", "Leaf"],
            required_outline: &["highlight"],
            required_prefixes: &["title", "accTitle", "accDescr"],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "block",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "block\n",
                "  columns 2\n",
                "  block:group[\"Group label\"]\n",
                "    A[\"Start\"] -- \"edge label\" --> B[\"End\"]\n",
                "  end\n",
                "  classDef hot fill:#f00\n",
                "  class A,B hot\n",
                "  style B stroke:#333\n",
            ),
            required_ids: &["group", "A", "B"],
            required_outline: &["hot"],
            required_prefixes: &["classDef", "class", "style"],
            forbidden_ids: &["Start", "End", "edge", "label"],
        },
        CapabilityCase {
            label: "c4",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "C4Context\n",
                "title Banking Context\n",
                "accTitle: Banking accessibility title\n",
                "accDescr: Banking accessibility description\n",
                "Boundary(bank, \"Bank\") {\n",
                "Person(customer, \"Customer\", \"Uses the system\")\n",
                "System(system, \"Internet Banking\", \"Core system\")\n",
                "}\n",
                "Rel(customer, system, \"Uses\", \"HTTPS\")\n",
                "UpdateElementStyle(system, $bgColor=\"red\")\n",
                "UpdateRelStyle(customer, system, $lineColor=\"blue\")\n",
            ),
            required_ids: &["bank", "customer", "system"],
            required_outline: &[],
            required_prefixes: &["title", "accTitle", "accDescr"],
            forbidden_ids: &[
                "Banking", "Context", "Bank", "Customer", "Uses", "Internet", "Core", "red", "blue",
            ],
        },
        CapabilityCase {
            label: "zenuml",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "zenuml\n",
                "title Login Flow\n",
                "accTitle Login accessibility title\n",
                "accDescr Login accessibility description\n",
                "Alice\n",
                "Bob\n",
                "A as API\n",
                "Alice->Bob: Login\n",
                "SomeType result = A.SyncMessage()\n",
                "new Session(with, params)\n",
            ),
            required_ids: &["Alice", "Bob", "A", "Session"],
            required_outline: &[],
            required_prefixes: &["title", "accTitle", "accDescr"],
            forbidden_ids: &[
                "Login",
                "Flow",
                "accessibility",
                "description",
                "API",
                "SyncMessage",
                "result",
                "params",
            ],
        },
    ];

    for case in cases {
        let mut store = DocumentStore::new();
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, case.snippet.to_string());
        let index = &snapshot.fences[0].text_index;

        assert_eq!(
            index.source(),
            case.expected_source,
            "unexpected provenance for {}",
            case.label
        );
        for id in case.required_ids {
            assert!(
                index.node_ids().any(|candidate| candidate == id),
                "missing node id {id:?} for {}",
                case.label
            );
        }
        for name in case.required_outline {
            assert!(
                index.outline_items().iter().any(|item| item.name == *name),
                "missing outline item {name:?} for {}",
                case.label
            );
        }
        for prefix in case.required_prefixes {
            assert!(
                index.has_directive_prefix(prefix),
                "missing directive prefix {prefix:?} for {}",
                case.label
            );
        }
        for forbidden in case.forbidden_ids {
            assert!(
                !index.node_ids().any(|candidate| candidate == forbidden),
                "forbidden node id {forbidden:?} leaked for {}",
                case.label
            );
        }
    }
}

#[test]
fn capability_matrix_matches_parser_backed_family_expectations() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/capability-matrix.mmd").unwrap();

    for (label, snippet) in [
        ("info", "info showInfo\n"),
        ("timeline", "timeline\nsection Alpha\ntask1\n"),
        ("packet", "packet\n0-10: \"first\"\n"),
        ("sankey", "sankey\nsource,target,1\n"),
        ("treeView", "treeView-beta\n\"Root\"\n    \"Child\"\n"),
        (
            "ishikawa",
            "ishikawa-beta\n  Problem\nCause A\n  Subcause A1\n",
        ),
        (
            "eventmodeling",
            concat!(
                "eventmodeling\n",
                "tf 01 cmd AddItem { productId: 7 }\n",
                "tf 02 evt ItemAdded [[ItemAddedData]] ->> 01\n",
                "\n",
                "data ItemAddedData {\n",
                "  productId: 7\n",
                "}\n",
            ),
        ),
        ("gitGraph", "gitGraph\ncommit id:\"C1\"\nbranch feature\n"),
        ("kanban", "kanban\n    root\n      child1\n"),
        (
            "architecture",
            concat!(
                "architecture-beta\n",
                "group platform(cloud)[Platform]\n",
                "service api(server)[API] in platform\n",
                "junction hub in platform\n",
                "api:R -- L:hub\n",
            ),
        ),
        (
            "radar",
            concat!(
                "radar-beta\n",
                "title Radar diagram\n",
                "accTitle: Radar accTitle\n",
                "accDescr: Radar accDescription\n",
                "axis A[\"Axis A\"], B[\"Axis B\"], C[\"Axis C\"]\n",
                "curve mycurve[\"My Curve\"]{1,2,3}\n",
            ),
        ),
        (
            "treemap",
            concat!(
                "treemap\n",
                "title Treemap Title\n",
                "accTitle: Treemap accTitle\n",
                "accDescr: Treemap accDescr\n",
                "\"Root\"\n",
                "  \"Leaf\": 42 :::highlight\n",
                "classDef highlight fill:#f00\n",
            ),
        ),
        (
            "block",
            concat!(
                "block\n",
                "  A[\"Start\"] -- \"edge label\" --> B[\"End\"]\n",
                "  classDef hot fill:#f00\n",
                "  class A,B hot\n",
            ),
        ),
        (
            "c4",
            concat!(
                "C4Context\n",
                "Person(customer, \"Customer\")\n",
                "System(system, \"System\")\n",
                "Rel(customer, system, \"Uses\")\n",
            ),
        ),
        (
            "zenuml",
            concat!("zenuml\n", "Alice\n", "Bob\n", "Alice->Bob: Hi\n",),
        ),
        ("venn", "venn-beta\nset A\nset B\nunion A,B\n"),
    ] {
        let snapshot = store.upsert(uri.clone(), 1, snippet.to_string());
        assert_eq!(
            snapshot.fences[0].text_index.source(),
            FenceTextIndexSource::ParserComplete,
            "unexpected parser provenance for {label}"
        );
    }

    let snapshot = store.upsert(uri, 1, "flowchart TD\nA-->B\n".to_string());
    let index = &snapshot.fences[0].text_index;
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "A"));
    assert!(index.node_ids().any(|id| id == "B"));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "A" && item.role == merman_analysis::FenceSemanticRole::Entity)
    );
}

#[test]
fn capability_matrix_document_marks_first_class_families_mature() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/lsp/CAPABILITIES.md");
    let contents =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));

    for expected in [
        "| Flowchart | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Sequence | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| State | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Class | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| ER | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Mindmap | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Gantt | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Architecture | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| GitGraph | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Kanban | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Radar | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Treemap | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Block | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| C4 | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| ZenUML | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Ishikawa | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Journey | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Info | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Timeline | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Pie | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Packet | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Sankey | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Tree View | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Event Modeling | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Quadrant Chart | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Requirement | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| Venn | Yes | Yes | Yes | Yes | Yes | Yes |",
        "| XY Chart | Yes | Yes | Yes | Yes | Yes | Yes |",
    ] {
        assert!(
            contents.contains(expected),
            "capability matrix is missing mature row: {expected}"
        );
    }
}

#[test]
fn capability_matrix_document_marks_partial_families_outside_first_class_contract() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/lsp/CAPABILITIES.md");
    let contents =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));

    assert!(
        contents.contains("## Coverage Boundary"),
        "capability matrix is missing the coverage boundary section"
    );

    for expected in ["| Error | Internal only |"] {
        assert!(
            contents.contains(expected),
            "capability matrix is missing partial-family row: {expected}"
        );
    }
}
