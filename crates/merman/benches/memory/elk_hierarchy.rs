use super::{ALLOCATOR, Mode, Probe, ProbeError, SemanticReport, sha256_bytes};
use merman::{
    MermaidEngine, OperationControl, ParseOptions, RenderOutput, RenderRequest, RenderTarget,
    Renderer, SvgRequest,
};
use merman::{model::runtime::RuntimePolicy, svg::SvgRenderOptions};
use serde::Serialize;
use std::collections::HashSet;
use std::fmt::Write as _;

pub(super) const LANE_ID: &str = "flowchart-elk-separate-children-memory";
const PUBLIC_OPERATION: &str = "render-flowchart-elk-hierarchy-svg";
const DEPTH_UNITS_PER_SCALE: u32 = 2;
const SEMANTIC_KIND: &str = "flowchart-elk-hierarchy-render-v2";
const DIAGRAM_TYPE: &str = "flowchart-elk";
const TERMINAL_NODE_ID: &str = "terminal";

#[derive(Debug, Serialize)]
struct SemanticProjection {
    kind: &'static str,
    metadata_diagram_type_matches: bool,
    metadata_layout_elk: bool,
    semantic_kind_flowchart: bool,
    prepared_family_flowchart: bool,
    svg_role_matches: bool,
    svg_contains_no_nan: bool,
    terminal_node_count: u32,
    scope_cluster_ids_unique: bool,
    scope_cluster_ids_complete: bool,
    view_box_finite_positive: bool,
    strict_nested_geometry: bool,
    rendered_edge_count: u32,
}

impl SemanticProjection {
    const fn zero() -> Self {
        Self {
            kind: SEMANTIC_KIND,
            metadata_diagram_type_matches: false,
            metadata_layout_elk: false,
            semantic_kind_flowchart: false,
            prepared_family_flowchart: false,
            svg_role_matches: false,
            svg_contains_no_nan: false,
            terminal_node_count: 0,
            scope_cluster_ids_unique: false,
            scope_cluster_ids_complete: false,
            view_box_finite_positive: false,
            strict_nested_geometry: false,
            rendered_edge_count: 0,
        }
    }
}

#[derive(Debug, Serialize)]
struct SemanticOutput {
    #[serde(flatten)]
    projection: SemanticProjection,
    scope_cluster_count: u32,
}

#[derive(Debug)]
struct StagedRenderObservation {
    metadata_diagram_type_matches: bool,
    metadata_layout_elk: bool,
    semantic_kind_flowchart: bool,
    prepared_family_flowchart: bool,
    svg: String,
}

#[derive(Debug)]
struct SvgObservation {
    svg_role_matches: bool,
    svg_contains_no_nan: bool,
    terminal_node_count: u32,
    scope_cluster_count: u32,
    scope_cluster_ids_unique: bool,
    scope_cluster_ids_complete: bool,
    view_box_finite_positive: bool,
    strict_nested_geometry: bool,
    rendered_edge_count: u32,
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn is_finite_positive(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.width > 0.0
            && self.height.is_finite()
            && self.height > 0.0
    }

    fn strictly_contains(self, nested: Self) -> bool {
        self.is_finite_positive()
            && nested.is_finite_positive()
            && self.x < nested.x
            && self.y < nested.y
            && self.x + self.width > nested.x + nested.width
            && self.y + self.height > nested.y + nested.height
    }
}

fn hierarchy_depth(scale: u32) -> Result<u32, ProbeError> {
    scale
        .checked_mul(DEPTH_UNITS_PER_SCALE)
        .ok_or_else(|| ProbeError::new("hierarchy depth overflow"))
}

fn build_flowchart(depth: u32) -> Result<String, ProbeError> {
    let mut source = String::with_capacity(depth as usize * 64);
    source.push_str("flowchart-elk TD\n");
    for level in 0..depth {
        writeln!(&mut source, "subgraph scope{level:03}[\"scope{level:03}\"]")
            .map_err(|_| ProbeError::new("failed to construct flowchart"))?;
        source.push_str("direction TD\n");
    }
    source.push_str("terminal[\"Leaf\"]\n");
    for _ in 0..depth {
        source.push_str("end\n");
    }
    Ok(source)
}

fn has_class(node: roxmltree::Node<'_, '_>, expected: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|class| class == expected)
    })
}

fn parse_finite(value: Option<&str>) -> Option<f64> {
    value?.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn direct_rect(node: roxmltree::Node<'_, '_>) -> Option<Rect> {
    let rect = node.children().find(|child| child.has_tag_name("rect"))?;
    Some(Rect {
        x: parse_finite(rect.attribute("x"))?,
        y: parse_finite(rect.attribute("y"))?,
        width: parse_finite(rect.attribute("width"))?,
        height: parse_finite(rect.attribute("height"))?,
    })
}

fn translate(node: roxmltree::Node<'_, '_>) -> Option<(f64, f64)> {
    let transform = node.attribute("transform")?;
    let values = transform
        .strip_prefix("translate(")?
        .strip_suffix(')')?
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    match values.as_slice() {
        [x] if x.is_finite() => Some((*x, 0.0)),
        [x, y] if x.is_finite() && y.is_finite() => Some((*x, *y)),
        _ => None,
    }
}

fn translated_rect(node: roxmltree::Node<'_, '_>) -> Option<Rect> {
    let rect = direct_rect(node)?;
    let (translate_x, translate_y) = translate(node)?;
    Some(Rect {
        x: rect.x + translate_x,
        y: rect.y + translate_y,
        ..rect
    })
}

fn rendered_scope_index(node: roxmltree::Node<'_, '_>) -> Option<usize> {
    // Read the source scope from the visible label so this benchmark remains independent of the
    // renderer's diagram-scoped DOM id format.
    let label_node = node
        .children()
        .find(|child| child.has_tag_name("g") && has_class(*child, "cluster-label"))?;
    let mut label = String::new();
    for text in label_node
        .descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
    {
        label.push_str(text);
    }
    label.trim().strip_prefix("scope")?.parse::<usize>().ok()
}

fn view_box_is_finite_positive(root: roxmltree::Node<'_, '_>) -> bool {
    let Some(view_box) = root.attribute("viewBox") else {
        return false;
    };
    let Ok(values) = view_box
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
    else {
        return false;
    };
    matches!(values.as_slice(), [x, y, width, height]
        if x.is_finite()
            && y.is_finite()
            && width.is_finite()
            && *width > 0.0
            && height.is_finite()
            && *height > 0.0)
}

fn observe_svg(svg: &str, expected_depth: u32) -> Result<SvgObservation, ProbeError> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| ProbeError::new(format!("invalid rendered SVG: {error}")))?;
    let root = document.root_element();
    let svg_role_matches = root.tag_name().name() == "svg"
        && root.attribute("aria-roledescription") == Some(DIAGRAM_TYPE);
    let svg_contains_no_nan = !svg.contains("NaN");
    let clusters = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && has_class(*node, "cluster"))
        .collect::<Vec<_>>();
    let scope_cluster_count = u32::try_from(clusters.len()).unwrap_or(u32::MAX);
    let mut scope_ids = HashSet::with_capacity(clusters.len());
    let mut scope_rects = vec![None; expected_depth as usize];
    let mut every_scope_id_valid = true;
    for cluster in clusters {
        let Some(scope_index) = rendered_scope_index(cluster) else {
            every_scope_id_valid = false;
            continue;
        };
        if !scope_ids.insert(scope_index) {
            every_scope_id_valid = false;
        }
        let Some(slot) = scope_rects.get_mut(scope_index) else {
            every_scope_id_valid = false;
            continue;
        };
        let Some(rect) = direct_rect(cluster) else {
            every_scope_id_valid = false;
            continue;
        };
        if slot.replace(rect).is_some() {
            every_scope_id_valid = false;
        }
    }
    let scope_cluster_ids_unique =
        every_scope_id_valid && scope_ids.len() == scope_cluster_count as usize;
    let ordered_scope_rects = scope_rects.into_iter().collect::<Option<Vec<_>>>();
    let scope_cluster_ids_complete = scope_cluster_count == expected_depth
        && scope_cluster_ids_unique
        && ordered_scope_rects.is_some();

    let terminal_nodes = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && has_class(*node, "node")
                && node.attribute("data-id") == Some(TERMINAL_NODE_ID)
        })
        .collect::<Vec<_>>();
    let terminal_node_count = u32::try_from(terminal_nodes.len()).unwrap_or(u32::MAX);
    let terminal_rect = match terminal_nodes.as_slice() {
        [terminal] => translated_rect(*terminal),
        _ => None,
    };
    let strict_nested_geometry = ordered_scope_rects
        .as_ref()
        .filter(|rects| !rects.is_empty())
        .is_some_and(|rects| {
            rects
                .windows(2)
                .all(|pair| pair[0].strictly_contains(pair[1]))
                && terminal_rect.is_some_and(|terminal| {
                    rects
                        .last()
                        .is_some_and(|deepest| deepest.strictly_contains(terminal))
                })
        });
    let rendered_edge_count = u32::try_from(
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .count(),
    )
    .unwrap_or(u32::MAX);

    Ok(SvgObservation {
        svg_role_matches,
        svg_contains_no_nan,
        terminal_node_count,
        scope_cluster_count,
        scope_cluster_ids_unique,
        scope_cluster_ids_complete,
        view_box_finite_positive: view_box_is_finite_positive(root),
        strict_nested_geometry,
        rendered_edge_count,
    })
}

fn render_hierarchy(
    renderer: &Renderer,
    source: &str,
) -> Result<StagedRenderObservation, ProbeError> {
    // The format-neutral artifact owns the operation control and runtime context. Consuming it
    // into the SVG target therefore keeps parse, ELK layout, emission, and postprocessing within
    // one operation rather than creating a hidden replacement session.
    let output = renderer
        .render(RenderRequest::presentation(source, OperationControl::new()))
        .map_err(|error| {
            ProbeError::new(format!("flowchart semantic preparation failed: {error}"))
        })?;
    let RenderOutput::Presentation(Some(semantic)) = output else {
        return Err(ProbeError::new(
            "flowchart semantic preparation returned no diagram",
        ));
    };
    let metadata_diagram_type_matches = semantic.metadata().diagram_type == DIAGRAM_TYPE;
    let metadata_layout_elk = semantic.metadata().effective_config.get_str("layout") == Some("elk");
    let semantic_kind_flowchart = semantic.semantic_kind() == "flowchart";
    let prepared_family_flowchart =
        semantic.family_id() == Some(merman::model::family::DiagramFamilyId::FLOWCHART);
    let output = semantic
        .render(RenderTarget::Svg(SvgRequest {
            options: SvgRenderOptions {
                diagram_id: Some("elk-hierarchy-memory".to_string()),
                ..Default::default()
            },
            ..Default::default()
        }))
        .map_err(|error| ProbeError::new(format!("flowchart SVG render failed: {error}")))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err(ProbeError::new("flowchart SVG render returned no diagram"));
    };

    Ok(StagedRenderObservation {
        metadata_diagram_type_matches,
        metadata_layout_elk,
        semantic_kind_flowchart,
        prepared_family_flowchart,
        svg: svg.into_parts().0,
    })
}

pub(super) fn measure(probe: &Probe) -> Result<(), ProbeError> {
    let request = &probe.request;
    let depth = hierarchy_depth(request.scale)?;
    let source = build_flowchart(depth)?;
    let renderer = Renderer::new()
        .with_presentation_engine(
            MermaidEngine::new()
                .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(request.seed)),
        )
        .with_parse_options(ParseOptions::strict());

    let snapshot_live_bytes = ALLOCATOR.begin_measurement();
    let render_result = match request.mode {
        Mode::Operation => Some(render_hierarchy(&renderer, &source)),
        Mode::Zero => None,
    };
    let metrics = ALLOCATOR.finish_measurement(snapshot_live_bytes);

    let empty_sha256 = sha256_bytes(&[]);
    let (output_sha256, semantic_output) = match render_result {
        Some(Ok(staged)) => {
            let svg = observe_svg(&staged.svg, depth)?;
            let projection = SemanticProjection {
                kind: SEMANTIC_KIND,
                metadata_diagram_type_matches: staged.metadata_diagram_type_matches,
                metadata_layout_elk: staged.metadata_layout_elk,
                semantic_kind_flowchart: staged.semantic_kind_flowchart,
                prepared_family_flowchart: staged.prepared_family_flowchart,
                svg_role_matches: svg.svg_role_matches,
                svg_contains_no_nan: svg.svg_contains_no_nan,
                terminal_node_count: svg.terminal_node_count,
                scope_cluster_ids_unique: svg.scope_cluster_ids_unique,
                scope_cluster_ids_complete: svg.scope_cluster_ids_complete,
                view_box_finite_positive: svg.view_box_finite_positive,
                strict_nested_geometry: svg.strict_nested_geometry,
                rendered_edge_count: svg.rendered_edge_count,
            };
            let projection_bytes = serde_json::to_vec(&projection).map_err(|error| {
                ProbeError::new(format!("failed to serialize semantic projection: {error}"))
            })?;
            (
                sha256_bytes(&projection_bytes),
                SemanticOutput {
                    projection,
                    scope_cluster_count: svg.scope_cluster_count,
                },
            )
        }
        Some(Err(error)) => return Err(error),
        None => {
            let projection = SemanticProjection::zero();
            (
                empty_sha256,
                SemanticOutput {
                    projection,
                    scope_cluster_count: 0,
                },
            )
        }
    };

    probe.report(
        PUBLIC_OPERATION,
        output_sha256,
        SemanticReport {
            workload_units: depth,
            semantic_output,
        },
        metrics,
        None,
    );
    Ok(())
}
