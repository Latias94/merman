//! Fixed model/source boundary consumers; original M0 fixture is retained at 2e71bfa33. This is fixture-local construction, not a model import API.
use serde_json::{Value, json};
use std::error::Error;
use std::io::{self, Write};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn field<'a>(input: &'a Value, name: &str) -> Result<&'a str> {
    input[name]
        .as_str()
        .ok_or_else(|| format!("missing fixture field {name}").into())
}

#[cfg(feature = "model")]
mod models {
    use super::*;
    use merman_model::families::class::ClassDiagramModel;
    use merman_model::families::flowchart::FlowchartModel;
    use merman_model::families::state::{StateDiagramModel, StateEdge, StateNode};

    pub struct Models {
        pub flowchart: FlowchartModel,
        pub class: ClassDiagramModel,
        pub state: StateDiagramModel,
    }

    impl Models {
        pub fn new(input: &Value) -> Result<Self> {
            let a = field(input, "id_a")?;
            let b = field(input, "id_b")?;
            let label_a = field(input, "label_a")?;
            let label_b = field(input, "label_b")?;
            let edge_id = field(input, "edge_id")?;
            let edge_label = field(input, "edge_label")?;
            // Use the public typed serde already supported by these two families.
            let flowchart = serde_json::from_value(json!({
                "keyword": "flowchart", "direction": "LR",
                "nodes": [
                    {"id": a, "label": label_a, "layoutShape": "rect"},
                    {"id": b, "label": label_b, "layoutShape": "rect"}
                ],
                "edges": [{"id": edge_id, "from": a, "to": b, "label": edge_label,
                    "arrow": "-->", "endMarker": "point", "length": 1}]
            }))?;
            let class = serde_json::from_value(json!({
                "type": "class", "direction": "LR",
                "classes": {
                    (a): {"id": a, "label": label_a, "text": label_a},
                    (b): {"id": b, "label": label_b, "text": label_b}
                },
                "relations": [{"id": edge_id, "id1": a, "id2": b, "title": edge_label,
                    "relation": {"type1": -1, "type2": 3, "lineType": 0}}],
                "constants": {
                    "lineType": {"line": 0, "dottedLine": 1},
                    "relationType": {"none": -1, "aggregation": 0, "extension": 1,
                        "composition": 2, "dependency": 3, "lollipop": 4}
                }
            }))?;
            let node = |id: &str, label: &str| -> Result<StateNode> {
                Ok(serde_json::from_value(json!({
                    "id": id, "label": label, "description": [label], "isGroup": false,
                    "type": null, "parentId": null, "cssClasses": "",
                    "cssCompiledStyles": [], "cssStyles": [], "dir": null,
                    "explicitDir": null, "shape": "rect", "position": null
                }))?)
            };
            // State's flat document has no public builder at BASE. Construct its public
            // graph fields directly and retain an explicitly empty document/record store.
            let state = StateDiagramModel {
                direction: "LR".to_owned(),
                nodes: vec![node(a, label_a)?, node(b, label_b)?],
                edges: vec![StateEdge {
                    id: edge_id.to_owned(),
                    start: a.to_owned(),
                    end: b.to_owned(),
                    classes: "transition".to_owned(),
                    label: edge_label.to_owned(),
                }],
                ..StateDiagramModel::default()
            };
            Ok(Self {
                flowchart,
                class,
                state,
            })
        }

        pub fn observations(&self) -> Value {
            json!({
                "flowchart": {
                    "direction": self.flowchart.direction,
                    "nodes": self.flowchart.nodes.iter().map(|n| json!([n.id, n.label])).collect::<Vec<_>>(),
                    "edges": self.flowchart.edges.iter().map(|e| json!([e.id, e.from, e.to, e.label,
                        e.start_marker, e.end_marker, e.stroke_kind, e.visibility])).collect::<Vec<_>>()
                },
                "class": {
                    "direction": self.class.direction,
                    "nodes": self.class.classes.iter().map(|(id, n)| json!([id, n.label, n.text])).collect::<Vec<_>>(),
                    "edges": self.class.relations.iter().map(|e| json!([e.id, e.id1, e.id2, e.title, e.relation])).collect::<Vec<_>>()
                },
                "state": {
                    "direction": self.state.direction, "flat_documents": self.state.document.len(),
                    "nodes": self.state.nodes.iter().map(|n| json!([n.id, n.label, n.description])).collect::<Vec<_>>(),
                    "edges": self.state.edges.iter().map(|e| json!([e.id, e.start, e.end, e.label])).collect::<Vec<_>>()
                }
            })
        }
    }
}

#[cfg(feature = "model-ascii")]
fn ascii(input: &Value) -> Result<Value> {
    use merman_ascii::{
        AsciiDiagram, AsciiOutput, AsciiOutputOutcome, AsciiRenderOptions, AsciiRenderer,
        AsciiResourcePolicy, AsciiSourceContext, AsciiViewportPolicy, OverflowPolicy,
    };
    use merman_model::{MermaidConfig, OperationControl};
    let renderer = AsciiRenderer::new(AsciiRenderOptions::ascii())?;
    let context = merman_model::runtime::RuntimePolicy::deterministic().begin_operation()?;
    let config = MermaidConfig::empty_object();
    #[cfg(not(feature = "source-ascii"))]
    let models = models::Models::new(input)?;
    let mut reports = serde_json::Map::new();
    for family in ["flowchart", "class", "state"] {
        #[cfg(feature = "source-ascii")]
        let parsed =
            merman_core::SemanticParser::new().parse_diagram_sync(field(input, family)?)?;
        let render = |viewport| -> Result<AsciiOutput> {
            let control = OperationControl::new();
            #[cfg(feature = "source-ascii")]
            {
                Ok(renderer.render_parsed_report(
                    &parsed,
                    viewport,
                    &control,
                    &context,
                    AsciiResourcePolicy::default(),
                )?)
            }
            #[cfg(not(feature = "source-ascii"))]
            {
                let view = match family {
                    "flowchart" => AsciiDiagram::Flowchart(&models.flowchart),
                    "class" => {
                        AsciiDiagram::Class(Box::new(models.class.topology(&config, &control)?))
                    }
                    "state" => AsciiDiagram::State(
                        merman_model::families::state::StateTerminalView::from_semantic(
                            &models.state,
                        ),
                    ),
                    _ => unreachable!(),
                };
                Ok(renderer.render_model_report(
                    &view,
                    AsciiSourceContext {
                        diagram_type: view.kind(),
                        config: &config,
                        semantic_config: &config,
                        title: None,
                    },
                    viewport,
                    &control,
                    &context,
                    AsciiResourcePolicy::default(),
                )?)
            }
        };
        let primary = render(AsciiViewportPolicy::unrestricted())?;
        let fallback =
            render(AsciiViewportPolicy::with_max_width(12).overflow(OverflowPolicy::Fallback))?;
        if fallback.outcome != AsciiOutputOutcome::Fallback {
            return Err(format!("{family} did not exercise complete fallback").into());
        }
        for output in [&primary, &fallback] {
            let flattened: String = output
                .as_text()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            for name in ["label_a", "label_b", "edge_label"] {
                if !flattened.contains(field(input, name)?) {
                    return Err(format!("{family} output lost runtime {name}").into());
                }
            }
        }
        reports.insert(
            family.to_owned(),
            json!({"primary": primary.report(), "fallback": fallback.report()}),
        );
    }
    Ok(Value::Object(reports))
}

#[cfg(feature = "presentation-svg")]
fn svg(input: &Value) -> Result<String> {
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_render::environment::RenderEnvironment;
    use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
    // The frozen M0 fixture uses the closed source result directly. This adapter borrows its
    // shared immutable data, retaining the same source, ID, measurement, and SVG contract.
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(field(input, "flowchart")?, ParseOptions::strict())?
        .ok_or("expected a graphical Flowchart")?;
    let artifact = merman_render::family::prepare(
        parsed.presentation(),
        &merman_render::LayoutOptions::default(),
        RenderEnvironment::deterministic().begin_session()?,
    )?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(field(input, "diagram_id")?.to_owned()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    let output = rendered.svg();
    for name in ["diagram_id", "label_a", "label_b", "edge_label"] {
        if !output.contains(field(input, name)?) {
            return Err(format!("SVG lost runtime {name}").into());
        }
    }
    Ok(output.to_owned())
}

#[cfg(feature = "analysis-diagnostics")]
fn analysis(input: &Value) -> Result<Value> {
    use merman_analysis::{AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, Analyzer};
    let rules = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended);
    let options = AnalysisOptions::default()
        .with_rule_config(rules)
        .with_source(merman_analysis::source_descriptor_for_markdown_path(Some(
            field(input, "analysis_path")?,
        )));
    #[cfg(feature = "analysis-fixes")]
    let options = options.with_fix_generation(merman_analysis::FixGeneration::Generate);
    let analyzer = Analyzer::with_options(options);
    let source = field(input, "analysis")?;
    let generation = analyzer
        .analyze_generation(source)
        .into_ready()
        .map_err(|error| format!("analysis rejected: {error:?}"))?;
    let mut payload = generation.project(analyzer.options().diagnostic_policy());
    if !payload.valid || payload.diagnostics.is_empty() || generation.diagrams().len() != 1 {
        return Err("expected one valid diagram with authoring diagnostics".into());
    }
    let diagram = &generation.diagrams()[0];
    let mut node_ids: Vec<_> = diagram.syntax().text_index.node_ids().cloned().collect();
    node_ids.sort();
    for name in ["id_a", "id_b"] {
        if !node_ids
            .iter()
            .any(|id| id == field(input, name).unwrap_or_default())
        {
            return Err(format!("analysis facts lost runtime {name}").into());
        }
    }
    let mut result = json!({"source_id": diagram.source_id(), "node_ids": node_ids});
    #[cfg(feature = "analysis-fixes")]
    {
        let fix = payload
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == "merman.authoring.config.prefer_frontmatter_config")
            .and_then(|diagnostic| diagnostic.fixes.first())
            .ok_or("expected the YAML frontmatter migration fix")?;
        let mut edits: Vec<_> = fix.edits.iter().collect();
        edits.sort_by_key(|edit| (edit.span.byte_start, edit.span.byte_end));
        let mut previous_end = 0;
        for edit in &edits {
            let (start, end) = (edit.span.byte_start, edit.span.byte_end);
            if start < previous_end
                || start > end
                || !source.is_char_boundary(start)
                || !source.is_char_boundary(end)
                || end > source.len()
            {
                return Err("fix edits are not a valid original-document transaction".into());
            }
            previous_end = end;
        }
        let mut patched = source.to_owned();
        for edit in edits.into_iter().rev() {
            patched.replace_range(edit.span.byte_start..edit.span.byte_end, &edit.replacement);
        }
        if !patched.contains("config:")
            || patched.contains("%%{init:")
            || !patched.contains(field(input, "label_a")?)
            || !patched.contains(field(input, "label_b")?)
        {
            return Err("frontmatter fix did not preserve the runtime diagram".into());
        }
        result["applied_source"] = json!(patched);
    }
    #[cfg(not(feature = "analysis-fixes"))]
    {
        // BASE eagerly builds fixes. Clear only the consumed DTO projection here; this does
        // not claim to omit serialization work or its dependency before the M4 change.
        for diagnostic in &mut payload.diagnostics {
            diagnostic.fixes.clear();
        }
    }
    result["diagnostics"] = serde_json::to_value(payload)?;
    Ok(result)
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or("expected a runtime JSON path or --families")?;
    if path == "--families" {
        // These are exercised consumers, not a source-parser capability declaration.
        #[cfg(feature = "model")]
        println!("class\nflowchart\nstate");
        #[cfg(not(feature = "model"))]
        println!("flowchart");
        return Ok(());
    }
    let input: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    #[cfg(feature = "presentation-svg")]
    let bytes = svg(&input)?.into_bytes();
    #[cfg(not(feature = "presentation-svg"))]
    let bytes = {
        #[cfg(all(feature = "model", not(feature = "model-ascii")))]
        let observations = models::Models::new(&input)?.observations();
        #[cfg(feature = "model-ascii")]
        let observations = ascii(&input)?;
        #[cfg(feature = "analysis-diagnostics")]
        let observations = analysis(&input)?;
        // Object keys are sorted only in the observable JSON envelope. Text, array order,
        // original-document spans, fix replacements and SVG bytes are never normalized.
        fn sorted(value: Value) -> Value {
            match value {
                Value::Object(map) => Value::Object(
                    map.into_iter()
                        .map(|(key, value)| (key, sorted(value)))
                        .collect::<std::collections::BTreeMap<_, _>>()
                        .into_iter()
                        .collect(),
                ),
                Value::Array(array) => Value::Array(array.into_iter().map(sorted).collect()),
                value => value,
            }
        }
        serde_json::to_vec(&sorted(
            json!({"identity": field(&input, "identity")?, "observations": observations}),
        ))?
    };
    io::stdout().lock().write_all(&bytes)?;
    Ok(())
}
