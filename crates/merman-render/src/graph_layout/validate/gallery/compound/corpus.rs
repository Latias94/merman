use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const FOCUSED: &[&str] = &[
    "fixtures/flowchart/stress_flowchart_nested_subgraphs_titles_001.mmd",
    "fixtures/flowchart/stress_flowchart_subgraph_cross_edges_between_siblings_048.mmd",
    "fixtures/flowchart/stress_flowchart_subgraph_edges_to_subgraph_and_internal_edges_041.mmd",
    "fixtures/flowchart/stress_flowchart_subgraph_sibling_titles_size_contrast_040.mmd",
    "crates/merman/benches/fixtures/flowchart_long_edge_labels_dagre_classic.mmd",
    "fixtures/flowchart/local_flowchart_elk_hardening_compound_parent_child_edges_001.mmd",
    "fixtures/agentflow/basic.mmd",
    "fixtures/agentflow/container-and-shapes.mmd",
    "fixtures/agentflow/local_nested_collapse_routes.mmd",
    "crates/merman-render/tests/fixtures/shoal-compound/empty-own.mmd",
    "crates/merman-render/tests/fixtures/shoal-compound/collapsed-reference.mmd",
];

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

fn inventory() -> Vec<PathBuf> {
    let output = Command::new("git")
        .current_dir(workspace())
        .args(["ls-files", "--cached", "-z", "--", "*.mmd"])
        .output()
        .expect("enumerate repository Mermaid fixture sources");
    assert!(
        output.status.success(),
        "git ls-files: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut paths: Vec<_> = std::str::from_utf8(&output.stdout)
        .unwrap()
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect();
    paths.extend(FOCUSED.iter().map(PathBuf::from));
    paths.sort();
    paths.dedup();
    paths
}

#[derive(Default)]
struct Suite {
    admitted: usize,
    drawn: usize,
    passed: usize,
    native: BTreeMap<String, usize>,
    rejects: Vec<String>,
    failures: Vec<String>,
    counts: Counts,
}

impl Suite {
    fn replay(&mut self, path: &Path, focused: bool) {
        let source = fs::read_to_string(workspace().join(path)).unwrap();
        let name = path.display().to_string();
        let lower = source.to_ascii_lowercase();
        if !lower.contains("subgraph") && !lower.contains("agentflow") {
            return;
        }
        let engine = MermaidEngine::new();
        let parsed = match engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        {
            Ok(Some(parsed)) => parsed.into_presentation(),
            other => {
                let rejection = format!("{name}: {other:?}");
                let golden = workspace().join(path).with_extension("golden.json");
                if matches!(
                    engine.parse_diagram_sync(&source, ParseOptions::strict()),
                    Ok(Some(_))
                ) {
                    self.failures.push(format!(
                        "semantic parser accepted but presentation rejected: {rejection}"
                    ));
                    return;
                }
                if focused
                    || golden.is_file()
                        && serde_json::from_slice::<serde_json::Value>(&fs::read(golden).unwrap())
                            .unwrap()["diagramType"]
                            != "error"
                {
                    self.failures.push(format!(
                        "positive fixture rejected by source parser: {rejection}"
                    ));
                }
                eprintln!("SOURCE-REJECT {rejection}");
                self.rejects.push(rejection);
                return;
            }
        };
        let family = parsed.metadata().diagram_type.as_str();
        match parsed.model() {
            RenderSemanticModel::Flowchart(model) if !model.subgraphs.is_empty() => {}
            RenderSemanticModel::Agentflow(_) => {}
            _ => return,
        }
        *self.native.entry(family.to_string()).or_default() += 1;
        if !matches!(
            family,
            "flowchart" | "flowchart-v2" | "flowchart-elk" | "agentflow"
        ) {
            eprintln!(
                "OTHER-FAMILY {name} type={family} family={:?}",
                parsed.family_id()
            );
            return;
        }
        self.admitted += 1;
        let input = if family == "flowchart-elk" {
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let original = family::plan_render(&parsed, &session).unwrap();
            eprintln!(
                "ELK-HEADER {name} source_layout={:?} source_capabilities={:?} replay=flowchart-v2/shoal",
                parsed.metadata().effective_config.get_str("layout"),
                original.required_capabilities()
            );
            let mut input = source.clone();
            let header = source
                .match_indices("flowchart-elk")
                .find_map(|(index, word)| {
                    let line = source[..index].rsplit('\n').next().unwrap_or_default();
                    (line.trim().is_empty()
                        && source[index + word.len()..].starts_with(char::is_whitespace))
                    .then_some(index)
                })
                .expect("explicit ELK fixture header");
            input.replace_range(header..header + "flowchart-elk".len(), "flowchart    ");
            input
        } else {
            source
        };
        let requested = engine
            .parse_diagram_for_render_model_sync(&shoal_source(&input), ParseOptions::strict())
            .map_err(|e| e.to_string())
            .and_then(|p| p.ok_or_else(|| "missing Shoal replay model".to_owned()));
        let result = requested.and_then(|p| {
            let diagram = p.into_presentation();
            if family == "flowchart-elk" {
                let semantics = |diagram: &PresentationDiagram| {
                    let RenderSemanticModel::Flowchart(model) = diagram.model() else {
                        panic!("ELK fixture lost Flowchart family")
                    };
                    let mut authored = model.clone();
                    authored.nodes.retain(|node| {
                        node.provenance
                            == merman_model::families::flowchart::FlowNodeProvenance::Authored
                    });
                    let mut json = serde_json::to_value(authored).unwrap();
                    json.as_object_mut().unwrap().remove("keyword");
                    json
                };
                assert!(
                    semantics(&parsed) == semantics(&diagram),
                    "{name}: ELK header rebind changed the authored graph"
                );
            }
            prepare(&diagram)
        });
        let drawing = match result {
            Ok(drawing) => drawing,
            Err(error) => {
                eprintln!("PREPARE-FAIL {name} type={family}: {error}");
                self.failures.push(format!("{name} [{family}] {error}"));
                return;
            }
        };
        self.drawn += 1;
        if let Some(directory) = std::env::var_os("MERMAN_SHOAL_COMPOUND_SVG_DIR") {
            let output = Path::new(&directory).join(path).with_extension("svg");
            fs::create_dir_all(output.parent().unwrap()).unwrap();
            fs::write(output, &drawing.svg).unwrap();
        }
        let doc = Document::parse(&drawing.svg).unwrap();
        let capture = drawing.capture(&doc);
        let Counts {
            nodes,
            groups,
            titles,
            node_labels,
            edge_labels,
            shafts,
            stroke_passes,
            rough_subpaths,
            contacts,
            normals,
            markers,
            stitches,
        } = capture.counts;
        self.counts.nodes += nodes;
        self.counts.groups += groups;
        self.counts.titles += titles;
        self.counts.node_labels += node_labels;
        self.counts.edge_labels += edge_labels;
        self.counts.shafts += shafts;
        self.counts.stroke_passes += stroke_passes;
        self.counts.rough_subpaths += rough_subpaths;
        self.counts.contacts += contacts;
        self.counts.normals += normals;
        self.counts.markers += markers;
        self.counts.stitches += stitches;
        let nfail = capture.failures.len();
        if !capture.rough_contacts.is_empty() {
            eprintln!(
                "HAND-DRAWN-INK {name} subpaths={rough_subpaths} stroke_passes={stroke_passes} max_px={:?}",
                capture.ink
            );
        }
        eprintln!(
            "{} {name} type={family} groups={groups} nodes={nodes} titles={titles} node_labels={node_labels} edge_labels={edge_labels} shafts={shafts} contacts={contacts} normals={normals} markers={markers} stitches={stitches} failures={nfail}",
            if nfail == 0 { "PASS" } else { "GEOMETRY-FAIL" }
        );
        if nfail == 0 {
            self.passed += 1;
        } else {
            self.failures
                .push(format!("{name}\n  {}", capture.failures.join("\n  ")));
        }
    }
}

fn run(paths: &[PathBuf], focused: bool) {
    let mut suite = Suite::default();
    for path in paths {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| suite.replay(path, focused)));
        if let Err(cause) = result {
            let message = cause
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| cause.downcast_ref::<&str>().copied())
                .unwrap_or("capture panic");
            suite
                .failures
                .push(format!("{}: {message}", path.display()));
        }
    }
    eprintln!(
        "compound sources={} family_inventory={:?} source_rejects={} admitted={} rendered={} passed={} {:?}",
        paths.len(),
        suite.native,
        suite.rejects.len(),
        suite.admitted,
        suite.drawn,
        suite.passed,
        suite.counts
    );
    assert!(
        suite.failures.is_empty(),
        "{} failed fixtures\n{}",
        suite.failures.len(),
        suite.failures.join("\n")
    );
    assert_eq!(suite.passed, suite.admitted);
    if focused {
        assert_eq!(suite.passed, FOCUSED.len());
    } else {
        assert!(
            suite
                .native
                .get("flowchart-v2")
                .copied()
                .unwrap_or_default()
                >= 272
        );
        assert!(suite.native.get("agentflow").copied().unwrap_or_default() >= 3);
        assert!(suite.native.contains_key("flowchart-elk"));
    }
    assert!(suite.counts.stitches > 0 && suite.counts.titles > 0 && suite.counts.groups > 0);
    assert_eq!(suite.counts.contacts, 2 * suite.counts.stroke_passes);
    assert_eq!(suite.counts.normals, suite.counts.contacts);
}

#[test]
fn replay_compound_examples() {
    run(&FOCUSED.iter().map(PathBuf::from).collect::<Vec<_>>(), true);
}

#[test]
#[ignore = "explicit replay of every repository compound Flowchart and Agentflow source"]
fn replay_entire_compound_corpus() {
    run(&inventory(), false);
}
