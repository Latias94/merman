use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

use serde::Deserialize;

// LSP maturity is an explicit product admission decision. Keep this independent from
// `supported_diagrams()`: adding a renderable public type must not silently advertise every
// language feature before its editor evidence has been reviewed.
const FIRST_CLASS_LSP_DIAGRAM_TYPES: &[&str] = &[
    "architecture",
    "block",
    "c4",
    "class",
    "cynefin",
    "er",
    "eventmodeling",
    "flowchart",
    "gantt",
    "gitgraph",
    "info",
    "ishikawa",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrantchart",
    "radar",
    "railroad",
    "railroadAbnf",
    "railroadEbnf",
    "railroadPeg",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "swimlane",
    "timeline",
    "treeView",
    "treemap",
    "venn",
    "wardley",
    "xychart",
    "zenuml",
];

// Keep public-but-not-yet-admitted types explicit. The current release has none, but retaining a
// separate category forces every future public catalog addition through an LSP maturity decision.
const NOT_YET_ADMITTED_PUBLIC_DIAGRAM_TYPES: &[&str] = &[];

#[derive(Deserialize)]
struct TokenEquivalenceEvidence {
    family_cases: Vec<TokenEquivalenceCase>,
}

#[derive(Deserialize)]
struct TokenEquivalenceCase {
    id: String,
}

fn capability_matrix() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/lsp/CAPABILITIES.md");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn token_equivalence_evidence() -> TokenEquivalenceEvidence {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/editor-language/token-equivalence-v1.json");
    let contents =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    serde_json::from_str(&contents).unwrap_or_else(|err| {
        panic!(
            "parse {} as token equivalence evidence: {err}",
            path.display()
        )
    })
}

fn section<'a>(contents: &'a str, start: &str, end: &str) -> &'a str {
    let start_offset = contents
        .find(start)
        .unwrap_or_else(|| panic!("capability matrix is missing section {start:?}"));
    let contents = &contents[start_offset + start.len()..];
    let end_offset = contents
        .find(end)
        .unwrap_or_else(|| panic!("capability matrix is missing section {end:?}"));
    &contents[..end_offset]
}

fn markdown_table(section: &str, expected_header: &[&str]) -> Vec<Vec<String>> {
    let lines = section.lines().map(str::trim).collect::<Vec<_>>();
    let header_index = lines
        .iter()
        .position(|line| {
            parse_markdown_table_row(line).is_some_and(|cells| {
                cells.len() == expected_header.len()
                    && cells
                        .iter()
                        .zip(expected_header)
                        .all(|(actual, expected)| actual == expected)
            })
        })
        .unwrap_or_else(|| panic!("capability matrix is missing table header {expected_header:?}"));
    let separator = lines
        .get(header_index + 1)
        .and_then(|line| parse_markdown_table_row(line))
        .unwrap_or_else(|| panic!("table {expected_header:?} is missing its separator row"));
    assert_eq!(
        separator.len(),
        expected_header.len(),
        "table {expected_header:?} separator must have one cell per column"
    );
    assert!(
        separator.iter().all(|cell| {
            let cell = cell.trim_matches(':');
            cell.len() >= 3 && cell.chars().all(|character| character == '-')
        }),
        "table {expected_header:?} has an invalid separator row: {separator:?}"
    );

    lines[header_index + 2..]
        .iter()
        .map_while(|line| parse_markdown_table_row(line))
        .collect()
}

fn parse_markdown_table_row(line: &str) -> Option<Vec<String>> {
    let line = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    let mut cells = vec![String::new()];
    let mut escaped = false;

    for character in line.chars() {
        if escaped {
            cells.last_mut()?.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '|' {
            cells.push(String::new());
        } else {
            cells.last_mut()?.push(character);
        }
    }
    if escaped {
        cells.last_mut()?.push('\\');
    }

    Some(
        cells
            .into_iter()
            .map(|cell| cell.trim().to_string())
            .collect(),
    )
}

#[test]
fn capability_matrix_matches_explicit_lsp_admission_and_evidence() {
    const HEADER: &[&str] = &[
        "Family",
        "Public diagram type",
        "Parser-backed facts",
        "Recoverable input",
        "Completion",
        "Hover / Symbols",
        "Semantic Tokens",
        "Definition / References / Rename",
        "Notes",
    ];

    let contents = capability_matrix();
    let family_coverage = section(&contents, "## Family Coverage", "## Coverage Boundary");
    let rows = markdown_table(family_coverage, HEADER);
    let documented = rows
        .iter()
        .map(|cells| {
            assert_eq!(
                cells.len(),
                HEADER.len(),
                "family coverage rows must keep the documented nine-column contract: {cells:?}"
            );
            let diagram_type = cells[1]
                .strip_prefix('`')
                .and_then(|value| value.strip_suffix('`'))
                .unwrap_or_else(|| {
                    panic!(
                        "family coverage rows must name their public diagram type in backticks: {cells:?}"
                    )
                });
            assert!(
                cells[2..8].iter().all(|value| value == "Yes"),
                "admitted row {diagram_type:?} must explicitly mark every language feature mature: {cells:?}"
            );
            diagram_type
        })
        .collect::<Vec<_>>();
    let documented_set = documented.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        documented.len(),
        documented_set.len(),
        "family coverage must not repeat a public diagram type"
    );
    assert_eq!(
        documented, FIRST_CLASS_LSP_DIAGRAM_TYPES,
        "family coverage must follow the independently admitted LSP product order"
    );

    let public_diagram_types = merman_core::supported_diagrams()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    assert!(
        documented_set.is_subset(&public_diagram_types),
        "LSP admission must not contain a diagram type outside the public product catalog"
    );
    let not_yet_admitted = public_diagram_types
        .difference(&documented_set)
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        not_yet_admitted,
        NOT_YET_ADMITTED_PUBLIC_DIAGRAM_TYPES
            .iter()
            .copied()
            .collect(),
        "every public diagram type must be explicitly classified as admitted or not yet admitted"
    );

    let family_capabilities = merman_core::diagram_family_capabilities();
    for diagram_type in FIRST_CLASS_LSP_DIAGRAM_TYPES {
        assert!(
            family_capabilities.iter().any(|capability| {
                capability.metadata_id == Some(*diagram_type)
                    && capability.has_semantic_parser
                    && capability.has_editor_parser
                    && capability.has_combined_parser
            }),
            "admitted LSP type {diagram_type:?} must have catalog-owned semantic and editor facts"
        );
    }

    let token_evidence = token_equivalence_evidence()
        .family_cases
        .into_iter()
        .map(|case| case.id)
        .collect::<Vec<_>>();
    let token_evidence_set = token_evidence
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        token_evidence.len(),
        token_evidence_set.len(),
        "semantic-token equivalence evidence must not repeat a public diagram type"
    );
    assert!(
        documented_set.is_subset(&token_evidence_set),
        "every admitted LSP type must retain exact semantic-token equivalence evidence"
    );
    let admitted_evidence_order = token_evidence
        .iter()
        .map(String::as_str)
        .filter(|diagram_type| documented_set.contains(diagram_type))
        .collect::<Vec<_>>();
    assert_eq!(
        admitted_evidence_order, FIRST_CLASS_LSP_DIAGRAM_TYPES,
        "admitted semantic-token evidence must follow the independently reviewed product order"
    );
}

#[test]
fn capability_matrix_derives_the_non_product_boundary_from_the_core_catalog() {
    const HEADER: &[&str] = &["Family", "Status", "Why"];

    let contents = capability_matrix();
    let coverage_boundary = section(
        &contents,
        "## Coverage Boundary",
        "## Semantic Fact Provenance",
    );
    let rows = markdown_table(coverage_boundary, HEADER);
    assert_eq!(
        rows,
        vec![vec![
            "Error".to_string(),
            "Internal only".to_string(),
            "Fallback diagram only; not a product-family commitment.".to_string(),
        ]],
        "the documented coverage boundary must describe the internal fallback precisely"
    );

    let mut logical_family_visibility = BTreeMap::<&str, bool>::new();
    for capability in merman_core::diagram_family_capabilities() {
        logical_family_visibility
            .entry(capability.logical_family_kind)
            .and_modify(|has_public_type| {
                *has_public_type |= capability.metadata_id.is_some();
            })
            .or_insert_with(|| capability.metadata_id.is_some());
    }
    let non_product_families = logical_family_visibility
        .into_iter()
        .filter_map(|(family, has_public_type)| (!has_public_type).then_some(family))
        .collect::<BTreeSet<_>>();
    let documented_families = rows
        .iter()
        .map(|cells| cells[0].to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        documented_families,
        non_product_families
            .into_iter()
            .map(str::to_string)
            .collect(),
        "the documented non-product boundary must follow the catalog's logical-family ownership"
    );
}
