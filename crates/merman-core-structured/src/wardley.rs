//! Wardley source grammar and controlled construction of the authored model.

use indexmap::IndexMap;
use merman_core_preprocess::scan::{
    langium::{
        LangiumCommonDbFields, LangiumCommonFacts, parse_langium_common, parse_langium_string,
        strip_langium_inline_comment,
    },
    physical_line_at,
};
use merman_model::families::wardley::{
    WardleyAccelerator, WardleyAnnotation, WardleyAxes, WardleyDeaccelerator, WardleyDiagramModel,
    WardleyFlowDirection, WardleyLink, WardleyNode, WardleyNote, WardleyPipeline, WardleyPoint,
    WardleySize, WardleySourceStrategy, WardleyTrend,
};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

#[cfg(feature = "editor-facts")]
mod observations;
#[cfg(feature = "editor-facts")]
pub use observations::{WardleyFieldKind, WardleyObservation};
#[cfg(feature = "editor-facts")]
use observations::{
    WardleyObserver, observe_component, observe_entity, observe_number, observe_outline,
    observe_payload, observe_pipeline_component, observe_reference,
};

const HEADER: &str = "wardley-beta";

/// Parses one preprocessed Wardley source and builds its typed, unsanitized model.
pub fn parse_wardley_source_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<WardleyDiagramModel, WardleyParseProblem>> {
    parse_wardley_source(
        code,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

/// Parses Wardley once and reports editor observations in preprocessed source coordinates.
#[cfg(feature = "editor-facts")]
pub fn parse_wardley_source_with_observer_controlled(
    code: &str,
    observe: &mut dyn for<'source> FnMut(WardleyObservation<'source>),
    control: &OperationControl,
) -> OperationControlResult<Result<WardleyDiagramModel, WardleyParseProblem>> {
    parse_wardley_source(code, Some(observe), control)
}

fn parse_wardley_source(
    code: &str,
    #[cfg(feature = "editor-facts")] observe: Option<&mut WardleyObserver<'_>>,
    control: &OperationControl,
) -> OperationControlResult<Result<WardleyDiagramModel, WardleyParseProblem>> {
    let ast = match parse_wardley_ast(
        code,
        #[cfg(feature = "editor-facts")]
        observe,
        control,
    )? {
        Ok(ast) => ast,
        Err(problem) => return Ok(Err(problem)),
    };
    control.checkpoint()?;
    let model = match build_wardley_model_controlled(&ast, control)? {
        Ok(model) => model,
        Err(problem) => return Ok(Err(problem)),
    };
    control.checkpoint()?;
    Ok(Ok(model))
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    selection: SourceSpan,
    #[cfg(feature = "editor-facts")]
    quoted: bool,
}

#[derive(Debug, Clone, Copy)]
struct SpannedNumber {
    value: f64,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct WardleySizeAst {
    width: SpannedNumber,
    height: SpannedNumber,
}

#[derive(Debug, Clone)]
struct WardleyEvolutionStageAst {
    name: SpannedText,
    boundary: Option<SpannedNumber>,
    second_name: Option<SpannedText>,
}

#[derive(Debug, Clone)]
struct WardleyEvolutionAst {
    stages: Vec<WardleyEvolutionStageAst>,
}

#[derive(Debug, Clone)]
struct WardleyPositionedNodeAst {
    name: SpannedText,
    visibility: SpannedNumber,
    evolution: SpannedNumber,
}

#[derive(Debug, Clone)]
struct WardleyLabelAst {
    offset_x: SpannedNumber,
    offset_y: SpannedNumber,
}

#[derive(Debug, Clone)]
struct WardleyComponentAst {
    positioned: WardleyPositionedNodeAst,
    label: Option<WardleyLabelAst>,
    source_strategy: Option<WardleySourceStrategy>,
    #[cfg(feature = "editor-facts")]
    source_strategy_span: Option<SourceSpan>,
    inertia: bool,
    #[cfg(feature = "editor-facts")]
    inertia_span: Option<SourceSpan>,
}

#[derive(Debug, Clone)]
struct WardleyPipelineComponentAst {
    name: SpannedText,
    evolution: SpannedNumber,
    label: Option<WardleyLabelAst>,
}

#[derive(Debug, Clone)]
struct WardleyPipelineAst {
    parent: SpannedText,
    components: Vec<WardleyPipelineComponentAst>,
}

#[derive(Debug, Clone)]
struct WardleyLinkAst {
    from: SpannedText,
    to: SpannedText,
    arrow: Option<String>,
    from_port: Option<String>,
    to_port: Option<String>,
    label: Option<SpannedText>,
}

#[derive(Debug, Clone)]
struct WardleyEvolveAst {
    component: SpannedText,
    target: SpannedNumber,
}

#[derive(Debug, Clone)]
struct WardleyNoteAst {
    text: SpannedText,
    visibility: SpannedNumber,
    evolution: SpannedNumber,
}

#[derive(Debug, Clone)]
struct WardleyAnnotationsBoxAst {
    x: SpannedNumber,
    y: SpannedNumber,
}

#[derive(Debug, Clone)]
struct WardleyAnnotationAst {
    number: SpannedNumber,
    x: SpannedNumber,
    y: SpannedNumber,
    text: SpannedText,
}

#[derive(Debug, Clone)]
struct WardleyForceAst {
    name: SpannedText,
    x: SpannedNumber,
    y: SpannedNumber,
}

#[derive(Debug, Default)]
struct WardleyAst {
    common: LangiumCommonFacts,
    size: Option<WardleySizeAst>,
    evolution: Option<WardleyEvolutionAst>,
    anchors: Vec<WardleyPositionedNodeAst>,
    components: Vec<WardleyComponentAst>,
    links: Vec<WardleyLinkAst>,
    evolves: Vec<WardleyEvolveAst>,
    pipelines: Vec<WardleyPipelineAst>,
    notes: Vec<WardleyNoteAst>,
    annotations_boxes: Vec<WardleyAnnotationsBoxAst>,
    annotations: Vec<WardleyAnnotationAst>,
    accelerators: Vec<WardleyForceAst>,
    deaccelerators: Vec<WardleyForceAst>,
}

#[derive(Debug, Clone)]
struct PendingWardleyNode {
    id: String,
    label: String,
    x: f64,
    y: f64,
    class_name: Option<String>,
    label_offset_x: Option<f64>,
    label_offset_y: Option<f64>,
    in_pipeline: bool,
    is_pipeline_parent: bool,
    inertia: Option<bool>,
    source_strategy: Option<WardleySourceStrategy>,
}

#[derive(Debug, Default)]
struct WardleyBuilder {
    nodes: IndexMap<String, PendingWardleyNode>,
    links: Vec<WardleyLink>,
    trends: IndexMap<String, WardleyTrend>,
    pipelines: IndexMap<String, WardleyPipeline>,
    annotations: Vec<WardleyAnnotation>,
    notes: Vec<WardleyNote>,
    accelerators: Vec<WardleyAccelerator>,
    deaccelerators: Vec<WardleyDeaccelerator>,
    annotations_box: Option<WardleyPoint>,
    axes: WardleyAxes,
    size: Option<WardleySize>,
}

impl WardleyBuilder {
    fn add_node(&mut self, node: PendingWardleyNode) {
        if let Some(existing) = self.nodes.get_mut(&node.id) {
            existing.label = node.label;
            existing.x = node.x;
            existing.y = node.y;
            existing.class_name = node.class_name.or_else(|| existing.class_name.clone());
            existing.label_offset_x = node.label_offset_x.or(existing.label_offset_x);
            existing.label_offset_y = node.label_offset_y.or(existing.label_offset_y);
            existing.inertia = node.inertia;
            existing.source_strategy = node.source_strategy;
        } else {
            self.nodes.insert(node.id.clone(), node);
        }
    }

    fn start_pipeline(&mut self, node_id: &str) {
        self.pipelines.insert(
            node_id.to_string(),
            WardleyPipeline {
                node_id: node_id.to_string(),
                component_ids: Vec::new(),
            },
        );
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.is_pipeline_parent = true;
        }
    }

    fn add_pipeline_component(&mut self, pipeline_node_id: &str, component_id: &str) {
        if let Some(pipeline) = self.pipelines.get_mut(pipeline_node_id) {
            pipeline.component_ids.push(component_id.to_string());
        }
        if let Some(node) = self.nodes.get_mut(component_id) {
            node.in_pipeline = true;
        }
    }

    fn resolve_node_id_controlled(
        &self,
        name: &str,
        control: &OperationControl,
    ) -> OperationControlResult<String> {
        if self.nodes.contains_key(name) {
            return Ok(name.to_string());
        }
        for (index, (id, node)) in self.nodes.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if node.label == name {
                return Ok(id.clone());
            }
        }
        Ok(name.to_string())
    }

    fn finish_controlled(
        self,
        common: LangiumCommonDbFields,
        control: &OperationControl,
    ) -> OperationControlResult<WardleyDiagramModel> {
        let mut nodes = Vec::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.into_values().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            nodes.push(WardleyNode {
                id: node.id,
                label: node.label,
                x: node.x,
                y: node.y,
                class_name: node.class_name,
                label_offset_x: node.label_offset_x,
                label_offset_y: node.label_offset_y,
                in_pipeline: node.in_pipeline,
                is_pipeline_parent: node.is_pipeline_parent,
                inertia: node.inertia,
                source_strategy: node.source_strategy,
            });
        }
        let mut trends = Vec::with_capacity(self.trends.len());
        for (index, trend) in self.trends.into_values().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            trends.push(trend);
        }
        let mut pipelines = Vec::with_capacity(self.pipelines.len());
        for (index, pipeline) in self.pipelines.into_values().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            pipelines.push(pipeline);
        }
        control.checkpoint()?;
        Ok(WardleyDiagramModel {
            title: common.title,
            acc_title: common.acc_title,
            acc_descr: common.acc_descr,
            nodes,
            links: self.links,
            trends,
            pipelines,
            annotations: self.annotations,
            notes: self.notes,
            accelerators: self.accelerators,
            deaccelerators: self.deaccelerators,
            annotations_box: self.annotations_box,
            axes: self.axes,
            size: self.size,
        })
    }
}

/// The first syntax or model issue in the supplied preprocessed source.
#[derive(Debug)]
pub struct WardleyParseProblem {
    pub message: String,
    pub span: SourceSpan,
}

impl WardleyParseProblem {
    fn new(message: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

fn parse_wardley_ast(
    code: &str,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut WardleyObserver<'_>>,
    control: &OperationControl,
) -> OperationControlResult<Result<WardleyAst, WardleyParseProblem>> {
    control.checkpoint()?;
    let mut ast = WardleyAst::default();
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe.as_deref_mut() {
        observe(WardleyObservation::Started);
    }
    let mut first_problem = None;
    let mut offset = 0usize;
    let mut header_decided = false;
    let mut saw_header = false;

    while offset < code.len() {
        control.checkpoint()?;
        let line_start = offset;
        let (line, next_offset) = physical_line_at(code, offset);
        offset = next_offset;
        let visible = strip_langium_inline_comment(line);
        let trimmed = visible.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (statement, statement_start) = if !header_decided {
            header_decided = true;
            let leading = visible.len() - visible.trim_start().len();
            let header_start = line_start + leading;
            let Some(rest) = visible.trim_start().strip_prefix(HEADER) else {
                remember_wardley_problem(
                    &mut first_problem,
                    WardleyParseProblem::new(
                        "expected wardley-beta header",
                        trimmed_span(visible, line_start),
                    ),
                );
                continue;
            };
            if rest
                .chars()
                .next()
                .is_some_and(|ch| !ch.is_ascii_whitespace())
            {
                remember_wardley_problem(
                    &mut first_problem,
                    WardleyParseProblem::new(
                        "expected whitespace or end of line after wardley-beta header",
                        SourceSpan::new(header_start, header_start + HEADER.len()),
                    ),
                );
                continue;
            }
            saw_header = true;
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut() {
                observe(WardleyObservation::Directive(HEADER));
            }
            let body_start = header_start + HEADER.len();
            (rest, body_start)
        } else {
            let leading = visible.len() - visible.trim_start().len();
            (&visible[leading..], line_start + leading)
        };

        if statement.trim().is_empty() {
            continue;
        }

        let leading = statement.len() - statement.trim_start().len();
        let statement_start = statement_start + leading;
        let statement = &statement[leading..];
        if let Some(parsed) = parse_langium_common(code, statement_start) {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut() {
                observe(WardleyObservation::Common(&parsed.fact));
            }
            if let Some(diagnostic) = parsed.diagnostic {
                remember_wardley_problem(
                    &mut first_problem,
                    WardleyParseProblem::new(diagnostic.message, diagnostic.span),
                );
            } else {
                ast.common.push(parsed.fact);
            }
            offset = statement_start + parsed.consumed;
            continue;
        }

        if keyword_body(statement, "pipeline").is_some() {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut() {
                observe(WardleyObservation::Directive("pipeline"));
            }
            let pipeline = parse_pipeline(
                code,
                statement,
                statement_start,
                offset,
                #[cfg(feature = "editor-facts")]
                &mut observe,
            );
            offset = pipeline.next_offset;
            if let Some(problem) = pipeline.first_problem {
                remember_wardley_problem(&mut first_problem, problem);
            } else if let Some(pipeline) = pipeline.pipeline {
                ast.pipelines.push(pipeline);
            }
            continue;
        }

        if let Err(problem) = parse_wardley_statement(
            statement,
            statement_start,
            &mut ast,
            #[cfg(feature = "editor-facts")]
            &mut observe,
        ) {
            remember_wardley_problem(&mut first_problem, problem);
        }
    }

    if !header_decided || !saw_header {
        remember_wardley_problem(
            &mut first_problem,
            WardleyParseProblem::new("expected wardley-beta header", SourceSpan::new(0, 0)),
        );
    }

    control.checkpoint()?;
    Ok(match first_problem {
        Some(problem) => Err(problem),
        None => Ok(ast),
    })
}

fn remember_wardley_problem(
    first_problem: &mut Option<WardleyParseProblem>,
    problem: WardleyParseProblem,
) {
    if first_problem.is_none() {
        *first_problem = Some(problem);
    }
}

fn parse_wardley_statement(
    statement: &str,
    statement_start: usize,
    ast: &mut WardleyAst,
    #[cfg(feature = "editor-facts")] observe: &mut Option<&mut WardleyObserver<'_>>,
) -> std::result::Result<(), WardleyParseProblem> {
    if let Some(body) = keyword_body(statement, "size") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("size"));
        }
        let size = parse_size(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_number(observe, size.width, "wardley canvas width");
            observe_number(observe, size.height, "wardley canvas height");
        }
        ast.size = Some(size);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "evolution") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("evolution"));
        }
        let evolution = parse_evolution(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            for stage in &evolution.stages {
                observe_payload(observe, &stage.name, "wardley evolution stage");
                if let Some(boundary) = stage.boundary {
                    observe_number(observe, boundary, "wardley evolution boundary");
                }
                if let Some(second_name) = &stage.second_name {
                    observe_payload(
                        observe,
                        second_name,
                        "wardley evolution stage secondary label",
                    );
                }
            }
        }
        ast.evolution = Some(evolution);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "anchor") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("anchor"));
        }
        let anchor = parse_positioned_node(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_entity(observe, &anchor.name, "wardley anchor");
            observe_number(observe, anchor.visibility, "wardley anchor visibility");
            observe_number(observe, anchor.evolution, "wardley anchor evolution");
        }
        ast.anchors.push(anchor);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "component") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("component"));
        }
        let component = parse_component(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_component(observe, &component);
        }
        ast.components.push(component);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "evolve") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("evolve"));
        }
        let evolve = parse_evolve(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_reference(observe, &evolve.component, "wardley evolved component");
            observe_number(observe, evolve.target, "wardley evolution target");
        }
        ast.evolves.push(evolve);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "note") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("note"));
        }
        let note = parse_note(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_payload(observe, &note.text, "wardley note");
            observe_number(observe, note.visibility, "wardley note visibility");
            observe_number(observe, note.evolution, "wardley note evolution");
        }
        ast.notes.push(note);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "annotations") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("annotations"));
        }
        let annotations =
            parse_annotations_box(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_number(observe, annotations.x, "wardley annotations visibility");
            observe_number(observe, annotations.y, "wardley annotations evolution");
        }
        ast.annotations_boxes.push(annotations);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "annotation") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("annotation"));
        }
        let annotation = parse_annotation(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_number(observe, annotation.number, "wardley annotation number");
            observe_number(observe, annotation.x, "wardley annotation visibility");
            observe_number(observe, annotation.y, "wardley annotation evolution");
            observe_payload(observe, &annotation.text, "wardley annotation text");
        }
        ast.annotations.push(annotation);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "accelerator") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("accelerator"));
        }
        let force = parse_force(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_outline(observe, &force.name, "wardley accelerator");
            observe_number(observe, force.x, "wardley accelerator visibility");
            observe_number(observe, force.y, "wardley accelerator evolution");
        }
        ast.accelerators.push(force);
        return Ok(());
    }
    if let Some(body) = keyword_body(statement, "deaccelerator") {
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("deaccelerator"));
        }
        let force = parse_force(body, body_start(statement, statement_start, body))?;
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe_outline(observe, &force.name, "wardley deaccelerator");
            observe_number(observe, force.x, "wardley deaccelerator visibility");
            observe_number(observe, force.y, "wardley deaccelerator evolution");
        }
        ast.deaccelerators.push(force);
        return Ok(());
    }

    let link = parse_link(statement, statement_start)?;
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe.as_deref_mut() {
        observe_reference(observe, &link.from, "wardley link source");
        observe_reference(observe, &link.to, "wardley link target");
        if let Some(label) = &link.label {
            observe_payload(observe, label, "wardley link label");
        }
    }
    ast.links.push(link);
    Ok(())
}

struct WardleyPipelineOutcome {
    pipeline: Option<WardleyPipelineAst>,
    next_offset: usize,
    first_problem: Option<WardleyParseProblem>,
}

fn parse_pipeline(
    code: &str,
    statement: &str,
    statement_start: usize,
    offset: usize,
    #[cfg(feature = "editor-facts")] observe: &mut Option<&mut WardleyObserver<'_>>,
) -> WardleyPipelineOutcome {
    let mut offset = offset;
    let mut first_problem = None;
    let body = keyword_body(statement, "pipeline").expect("pipeline dispatch checked keyword");
    let pipeline_body_start = body_start(statement, statement_start, body);
    let trimmed = body.trim();
    let Some(open_brace) = find_unquoted_char(trimmed, '{') else {
        return WardleyPipelineOutcome {
            pipeline: None,
            next_offset: offset,
            first_problem: Some(WardleyParseProblem::new(
                "expected '{' after wardley pipeline parent",
                trimmed_span(body, pipeline_body_start),
            )),
        };
    };
    let leading = body.len() - body.trim_start().len();
    let opening_start = pipeline_body_start + leading + open_brace;
    if !trimmed[open_brace + 1..].trim().is_empty() {
        remember_wardley_problem(
            &mut first_problem,
            WardleyParseProblem::new(
                "unexpected tokens after wardley pipeline opening brace",
                SourceSpan::new(opening_start + 1, pipeline_body_start + body.len()),
            ),
        );
    }
    let parent_raw = &trimmed[..open_brace];
    let parent = match parse_name(parent_raw, pipeline_body_start + leading) {
        Ok(parent) => {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut() {
                observe_reference(observe, &parent, "wardley pipeline parent");
            }
            Some(parent)
        }
        Err(problem) => {
            remember_wardley_problem(&mut first_problem, problem);
            None
        }
    };
    let mut components = Vec::new();

    loop {
        if offset >= code.len() {
            remember_wardley_problem(
                &mut first_problem,
                WardleyParseProblem::new(
                    "unterminated wardley pipeline block",
                    SourceSpan::new(code.len(), code.len()),
                ),
            );
            return WardleyPipelineOutcome {
                pipeline: None,
                next_offset: offset,
                first_problem,
            };
        }
        let line_start = offset;
        let (line, next_offset) = physical_line_at(code, offset);
        offset = next_offset;
        let visible = strip_langium_inline_comment(line);
        let trimmed_line = visible.trim();
        if trimmed_line.is_empty() {
            continue;
        }
        let leading = visible.len() - visible.trim_start().len();
        let absolute = line_start + leading;
        if let Some(after_closing) = trimmed_line.strip_prefix('}') {
            if !after_closing.is_empty() {
                let trailing_start = absolute + 1 + after_closing.len()
                    - after_closing.trim_start_matches([' ', '\t']).len();
                remember_wardley_problem(
                    &mut first_problem,
                    WardleyParseProblem::new(
                        "unexpected tokens after wardley pipeline closing brace",
                        SourceSpan::new(trailing_start, absolute + trimmed_line.len()),
                    ),
                );
            }
            if components.is_empty() {
                remember_wardley_problem(
                    &mut first_problem,
                    WardleyParseProblem::new(
                        "wardley pipeline requires at least one component",
                        SourceSpan::new(absolute, absolute + 1),
                    ),
                );
            }
            let pipeline = if first_problem.is_none() {
                parent.map(|parent| parent_with_components(parent, components))
            } else {
                None
            };
            return WardleyPipelineOutcome {
                pipeline,
                next_offset: offset,
                first_problem,
            };
        }
        let Some(component_body) = keyword_body(trimmed_line, "component") else {
            let span = SourceSpan::new(absolute, absolute + trimmed_line.len());
            remember_wardley_problem(
                &mut first_problem,
                WardleyParseProblem::new("expected component or '}' inside wardley pipeline", span),
            );
            continue;
        };
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(WardleyObservation::Directive("component"));
        }
        let component_start = body_start(trimmed_line, absolute, component_body);
        match parse_pipeline_component(component_body, component_start) {
            Ok(component) => {
                #[cfg(feature = "editor-facts")]
                if let Some(observe) = observe.as_deref_mut() {
                    observe_pipeline_component(observe, &component);
                }
                components.push(component);
            }
            Err(problem) => remember_wardley_problem(&mut first_problem, problem),
        }
    }
}

fn parent_with_components(
    parent: SpannedText,
    components: Vec<WardleyPipelineComponentAst>,
) -> WardleyPipelineAst {
    WardleyPipelineAst { parent, components }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WardleyNumberKind {
    Decimal,
    IntegerOrDecimal,
    Integer,
}

struct WardleyCursor<'input> {
    input: &'input str,
    base: usize,
    pos: usize,
}

impl<'input> WardleyCursor<'input> {
    fn new(input: &'input str, base: usize) -> Self {
        Self {
            input,
            base,
            pos: 0,
        }
    }

    fn remaining(&self) -> &'input str {
        &self.input[self.pos..]
    }

    fn absolute(&self) -> usize {
        self.base + self.pos
    }

    fn skip_ws(&mut self) {
        self.pos += self
            .remaining()
            .bytes()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
    }

    fn consume_char(
        &mut self,
        expected: char,
        context: &str,
    ) -> std::result::Result<(), WardleyParseProblem> {
        self.skip_ws();
        let start = self.absolute();
        if self.remaining().starts_with(expected) {
            self.pos += expected.len_utf8();
            Ok(())
        } else {
            Err(WardleyParseProblem::new(
                format!("expected '{expected}' {context}"),
                SourceSpan::new(start, start),
            ))
        }
    }

    fn take_number(
        &mut self,
        kind: WardleyNumberKind,
        context: &str,
    ) -> std::result::Result<SpannedNumber, WardleyParseProblem> {
        self.skip_ws();
        let token_start = self.absolute();
        let bytes = self.remaining().as_bytes();
        let integer_len = bytes
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if integer_len == 0 {
            return Err(WardleyParseProblem::new(
                format!("expected {context}"),
                SourceSpan::new(token_start, token_start),
            ));
        }

        let mut consumed = integer_len;
        let has_dot = bytes.get(consumed) == Some(&b'.');
        if has_dot {
            consumed += 1;
            let fractional = bytes[consumed..]
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
            if fractional == 0 {
                return Err(WardleyParseProblem::new(
                    format!("expected digits after decimal point in {context}"),
                    SourceSpan::new(token_start, token_start + consumed),
                ));
            }
            consumed += fractional;
        }

        if kind == WardleyNumberKind::Decimal && !has_dot {
            return Err(WardleyParseProblem::new(
                format!("expected decimal {context}"),
                SourceSpan::new(token_start, token_start + consumed),
            ));
        }
        if kind == WardleyNumberKind::Integer && has_dot {
            return Err(WardleyParseProblem::new(
                format!("expected integer {context}"),
                SourceSpan::new(token_start, token_start + consumed),
            ));
        }
        let token = &self.remaining()[..consumed];
        let token_span = SourceSpan::new(token_start, token_start + consumed);
        if !has_dot && token.len() > 1 && token.starts_with('0') {
            return Err(WardleyParseProblem::new(
                format!("invalid leading zero in {context}"),
                SourceSpan::new(token_start, token_start + consumed),
            ));
        }
        if self.remaining()[consumed..]
            .chars()
            .next()
            .is_some_and(|ch| ch == '.' || ch.is_ascii_digit())
        {
            return Err(WardleyParseProblem::new(
                format!("invalid {context}"),
                SourceSpan::new(token_start, token_start + consumed + 1),
            ));
        }

        let value = token.parse::<f64>().map_err(|_| {
            WardleyParseProblem::new(
                format!("invalid {context}"),
                SourceSpan::new(token_start, token_start + consumed),
            )
        })?;
        self.pos += consumed;
        Ok(SpannedNumber {
            value,
            span: token_span,
        })
    }

    fn take_signed_integer(
        &mut self,
        context: &str,
    ) -> std::result::Result<SpannedNumber, WardleyParseProblem> {
        self.skip_ws();
        let start = self.absolute();
        let negative = self.remaining().starts_with('-');
        if negative {
            self.pos += 1;
        }
        let number = self.take_number(WardleyNumberKind::Integer, context)?;
        Ok(SpannedNumber {
            value: if negative {
                -number.value
            } else {
                number.value
            },
            span: SourceSpan::new(start, number.span.end),
        })
    }

    fn expect_end(&mut self, context: &str) -> std::result::Result<(), WardleyParseProblem> {
        self.skip_ws();
        if self.remaining().is_empty() {
            Ok(())
        } else {
            let span = SourceSpan::new(self.absolute(), self.base + self.input.len());
            Err(WardleyParseProblem::new(
                format!("unexpected trailing tokens in {context}"),
                span,
            ))
        }
    }
}

fn parse_size(body: &str, base: usize) -> std::result::Result<WardleySizeAst, WardleyParseProblem> {
    let mut cursor = WardleyCursor::new(body, base);
    cursor.consume_char('[', "after wardley size")?;
    let width = cursor.take_number(WardleyNumberKind::Integer, "wardley canvas width")?;
    cursor.consume_char(',', "between wardley canvas dimensions")?;
    let height = cursor.take_number(WardleyNumberKind::Integer, "wardley canvas height")?;
    cursor.consume_char(']', "after wardley canvas dimensions")?;
    cursor.expect_end("wardley size")?;
    Ok(WardleySizeAst { width, height })
}

fn parse_evolution(
    body: &str,
    base: usize,
) -> std::result::Result<WardleyEvolutionAst, WardleyParseProblem> {
    let ranges = split_unquoted_token(body, "->", base)?;
    if ranges.len() < 2 {
        return Err(WardleyParseProblem::new(
            "wardley evolution requires at least two stages",
            trimmed_span(body, base),
        ));
    }
    let mut stages = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        stages.push(parse_evolution_stage(&body[start..end], base + start)?);
    }
    Ok(WardleyEvolutionAst { stages })
}

fn parse_evolution_stage(
    input: &str,
    base: usize,
) -> std::result::Result<WardleyEvolutionStageAst, WardleyParseProblem> {
    let (trimmed, trimmed_start) = trim_horizontal(input, base);
    if trimmed.is_empty() {
        return Err(WardleyParseProblem::new(
            "expected wardley evolution stage",
            SourceSpan::new(trimmed_start, trimmed_start),
        ));
    }
    let slash = find_unquoted_char(trimmed, '/');
    let primary_end = slash.unwrap_or(trimmed.len());
    let primary = &trimmed[..primary_end];
    let boundary_at = find_unquoted_char(primary, '@');
    let name_end = boundary_at.unwrap_or(primary.len());
    let name = parse_name(&primary[..name_end], trimmed_start)?;
    let boundary = if let Some(at) = boundary_at {
        let mut cursor = WardleyCursor::new(&primary[at + 1..], trimmed_start + at + 1);
        let number = cursor.take_number(
            WardleyNumberKind::Decimal,
            "wardley evolution stage boundary",
        )?;
        cursor.expect_end("wardley evolution stage boundary")?;
        Some(number)
    } else {
        None
    };
    let second_name = if let Some(slash) = slash {
        Some(parse_name(
            &trimmed[slash + 1..],
            trimmed_start + slash + 1,
        )?)
    } else {
        None
    };
    Ok(WardleyEvolutionStageAst {
        name,
        boundary,
        second_name,
    })
}

fn parse_positioned_node(
    body: &str,
    base: usize,
) -> std::result::Result<WardleyPositionedNodeAst, WardleyParseProblem> {
    let bracket = find_unquoted_char(body, '[').ok_or_else(|| {
        WardleyParseProblem::new(
            "expected coordinates after wardley node name",
            trimmed_span(body, base),
        )
    })?;
    let name = parse_name(&body[..bracket], base)?;
    let mut cursor = WardleyCursor::new(&body[bracket..], base + bracket);
    let (visibility, evolution) = take_position_pair(
        &mut cursor,
        WardleyNumberKind::Decimal,
        "wardley node coordinates",
    )?;
    cursor.expect_end("wardley node")?;
    Ok(WardleyPositionedNodeAst {
        name,
        visibility,
        evolution,
    })
}

fn parse_component(
    body: &str,
    base: usize,
) -> std::result::Result<WardleyComponentAst, WardleyParseProblem> {
    let bracket = find_unquoted_char(body, '[').ok_or_else(|| {
        WardleyParseProblem::new(
            "expected coordinates after wardley component name",
            trimmed_span(body, base),
        )
    })?;
    let name = parse_name(&body[..bracket], base)?;
    let mut cursor = WardleyCursor::new(&body[bracket..], base + bracket);
    let (visibility, evolution) = take_position_pair(
        &mut cursor,
        WardleyNumberKind::Decimal,
        "wardley component coordinates",
    )?;
    let label = parse_optional_label(&mut cursor)?;
    let source_strategy = parse_optional_strategy(&mut cursor)?;
    let inertia_span = parse_optional_inertia(&mut cursor)?;
    cursor.expect_end("wardley component")?;
    Ok(WardleyComponentAst {
        positioned: WardleyPositionedNodeAst {
            name,
            visibility,
            evolution,
        },
        label,
        source_strategy: source_strategy.map(|(strategy, _)| strategy),
        #[cfg(feature = "editor-facts")]
        source_strategy_span: source_strategy.map(|(_, span)| span),
        inertia: inertia_span.is_some(),
        #[cfg(feature = "editor-facts")]
        inertia_span,
    })
}

fn parse_pipeline_component(
    body: &str,
    base: usize,
) -> std::result::Result<WardleyPipelineComponentAst, WardleyParseProblem> {
    let bracket = find_unquoted_char(body, '[').ok_or_else(|| {
        WardleyParseProblem::new(
            "expected evolution coordinate after wardley pipeline component name",
            trimmed_span(body, base),
        )
    })?;
    let name = parse_name(&body[..bracket], base)?;
    let mut cursor = WardleyCursor::new(&body[bracket..], base + bracket);
    cursor.consume_char('[', "before wardley pipeline component evolution")?;
    let evolution = cursor.take_number(
        WardleyNumberKind::Decimal,
        "wardley pipeline component evolution",
    )?;
    cursor.consume_char(']', "after wardley pipeline component evolution")?;
    let label = parse_optional_label(&mut cursor)?;
    cursor.expect_end("wardley pipeline component")?;
    Ok(WardleyPipelineComponentAst {
        name,
        evolution,
        label,
    })
}

fn take_position_pair(
    cursor: &mut WardleyCursor<'_>,
    kind: WardleyNumberKind,
    context: &str,
) -> std::result::Result<(SpannedNumber, SpannedNumber), WardleyParseProblem> {
    cursor.consume_char('[', &format!("before {context}"))?;
    let first = cursor.take_number(kind, context)?;
    cursor.consume_char(',', &format!("between {context}"))?;
    let second = cursor.take_number(kind, context)?;
    cursor.consume_char(']', &format!("after {context}"))?;
    Ok((first, second))
}

fn parse_optional_label(
    cursor: &mut WardleyCursor<'_>,
) -> std::result::Result<Option<WardleyLabelAst>, WardleyParseProblem> {
    cursor.skip_ws();
    let Some(rest) = keyword_body(cursor.remaining(), "label") else {
        return Ok(None);
    };
    cursor.pos += cursor.remaining().len() - rest.len();
    cursor.consume_char('[', "after wardley label")?;
    let offset_x = cursor.take_signed_integer("wardley label X offset")?;
    cursor.consume_char(',', "between wardley label offsets")?;
    let offset_y = cursor.take_signed_integer("wardley label Y offset")?;
    cursor.consume_char(']', "after wardley label offsets")?;
    Ok(Some(WardleyLabelAst { offset_x, offset_y }))
}

fn parse_optional_strategy(
    cursor: &mut WardleyCursor<'_>,
) -> std::result::Result<Option<(WardleySourceStrategy, SourceSpan)>, WardleyParseProblem> {
    cursor.skip_ws();
    if !cursor.remaining().starts_with('(') {
        return Ok(None);
    }
    let start = cursor.absolute();
    let Some(close) = cursor.remaining().find(')') else {
        return Err(WardleyParseProblem::new(
            "unterminated wardley component decorator",
            SourceSpan::new(start, cursor.base + cursor.input.len()),
        ));
    };
    let token = cursor.remaining()[1..close].trim();
    let token_leading =
        cursor.remaining()[1..close].len() - cursor.remaining()[1..close].trim_start().len();
    let token_span = SourceSpan::new(
        start + 1 + token_leading,
        start + 1 + token_leading + token.len(),
    );
    if token == "inertia" {
        return Ok(None);
    }
    let strategy = match token {
        "build" => WardleySourceStrategy::Build,
        "buy" => WardleySourceStrategy::Buy,
        "outsource" => WardleySourceStrategy::Outsource,
        "market" => WardleySourceStrategy::Market,
        _ => {
            return Err(WardleyParseProblem::new(
                "expected build, buy, outsource, or market wardley source strategy",
                SourceSpan::new(start + 1, start + close),
            ));
        }
    };
    cursor.pos += close + 1;
    Ok(Some((strategy, token_span)))
}

fn parse_optional_inertia(
    cursor: &mut WardleyCursor<'_>,
) -> std::result::Result<Option<SourceSpan>, WardleyParseProblem> {
    cursor.skip_ws();
    let start = cursor.absolute();
    if let Some(rest) = keyword_body(cursor.remaining(), "inertia") {
        cursor.pos += cursor.remaining().len() - rest.len();
        let span = SourceSpan::new(start, start + "inertia".len());
        return Ok(Some(span));
    }
    if !cursor.remaining().starts_with('(') {
        return Ok(None);
    }

    cursor.pos += 1;
    cursor.skip_ws();
    let keyword_start = cursor.absolute();
    let Some(after_keyword) = cursor.remaining().strip_prefix("inertia") else {
        return Err(WardleyParseProblem::new(
            "expected inertia inside wardley component inertia annotation",
            SourceSpan::new(keyword_start, keyword_start),
        ));
    };
    if after_keyword
        .chars()
        .next()
        .is_some_and(|ch| !matches!(ch, ' ' | '\t' | ')'))
    {
        return Err(WardleyParseProblem::new(
            "expected inertia inside wardley component inertia annotation",
            SourceSpan::new(keyword_start, keyword_start + "inertia".len()),
        ));
    }
    cursor.pos += "inertia".len();
    let keyword = SourceSpan::new(keyword_start, keyword_start + "inertia".len());
    cursor.skip_ws();
    let close = cursor.absolute();
    if !cursor.remaining().starts_with(')') {
        return Err(WardleyParseProblem::new(
            "expected ')' after wardley component inertia annotation",
            SourceSpan::new(close, close),
        ));
    }
    cursor.pos += 1;
    Ok(Some(keyword))
}

fn parse_evolve(
    body: &str,
    base: usize,
) -> std::result::Result<WardleyEvolveAst, WardleyParseProblem> {
    let (trimmed, trimmed_start) = trim_horizontal(body, base);
    let target_start = trimmed
        .rfind([' ', '\t'])
        .map(|index| {
            index
                + trimmed[index..]
                    .bytes()
                    .take_while(|byte| matches!(byte, b' ' | b'\t'))
                    .count()
        })
        .ok_or_else(|| {
            WardleyParseProblem::new(
                "expected component name and target after wardley evolve",
                trimmed_span(body, base),
            )
        })?;
    let component = parse_name(&trimmed[..target_start], trimmed_start)?;
    let mut cursor = WardleyCursor::new(&trimmed[target_start..], trimmed_start + target_start);
    let target = cursor.take_number(
        WardleyNumberKind::Decimal,
        "wardley component evolution target",
    )?;
    cursor.expect_end("wardley evolve")?;
    Ok(WardleyEvolveAst { component, target })
}

fn parse_note(body: &str, base: usize) -> std::result::Result<WardleyNoteAst, WardleyParseProblem> {
    let (trimmed, trimmed_start) = trim_horizontal(body, base);
    let (text, consumed) =
        parse_wardley_quoted_text(trimmed, trimmed_start, "expected quoted wardley note text")?;
    let mut cursor = WardleyCursor::new(&trimmed[consumed..], trimmed_start + consumed);
    let (visibility, evolution) = take_position_pair(
        &mut cursor,
        WardleyNumberKind::Decimal,
        "wardley note coordinates",
    )?;
    cursor.expect_end("wardley note")?;
    Ok(WardleyNoteAst {
        text,
        visibility,
        evolution,
    })
}

fn parse_annotations_box(
    body: &str,
    base: usize,
) -> std::result::Result<WardleyAnnotationsBoxAst, WardleyParseProblem> {
    let mut cursor = WardleyCursor::new(body, base);
    let (x, y) = take_position_pair(
        &mut cursor,
        WardleyNumberKind::IntegerOrDecimal,
        "wardley annotations box coordinates",
    )?;
    cursor.expect_end("wardley annotations box")?;
    Ok(WardleyAnnotationsBoxAst { x, y })
}

fn parse_annotation(
    body: &str,
    base: usize,
) -> std::result::Result<WardleyAnnotationAst, WardleyParseProblem> {
    let mut cursor = WardleyCursor::new(body, base);
    let number = cursor.take_number(WardleyNumberKind::Integer, "wardley annotation number")?;
    cursor.consume_char(',', "after wardley annotation number")?;
    let (x, y) = take_position_pair(
        &mut cursor,
        WardleyNumberKind::IntegerOrDecimal,
        "wardley annotation coordinates",
    )?;
    cursor.skip_ws();
    let text_start = cursor.absolute();
    let (text, consumed) = parse_wardley_quoted_text(
        cursor.remaining(),
        text_start,
        "expected quoted wardley annotation text",
    )?;
    cursor.pos += consumed;
    cursor.expect_end("wardley annotation")?;
    Ok(WardleyAnnotationAst { number, x, y, text })
}

fn parse_force(
    body: &str,
    base: usize,
) -> std::result::Result<WardleyForceAst, WardleyParseProblem> {
    let positioned = parse_positioned_node(body, base)?;
    Ok(WardleyForceAst {
        name: positioned.name,
        x: positioned.visibility,
        y: positioned.evolution,
    })
}

fn parse_link(
    statement: &str,
    statement_start: usize,
) -> std::result::Result<WardleyLinkAst, WardleyParseProblem> {
    let (link_body, label) = split_link_label(statement, statement_start)?;
    let Some((operator_start, first_operator)) = find_link_operator(link_body) else {
        let (from, to, to_port) = parse_operatorless_link(link_body, statement_start)?;
        return Ok(WardleyLinkAst {
            from,
            to,
            arrow: None,
            from_port: None,
            to_port,
            label,
        });
    };
    let from = parse_name(&link_body[..operator_start], statement_start)?;
    let mut cursor = operator_start + first_operator.len();
    while link_body[cursor..].starts_with([' ', '\t']) {
        cursor += 1;
    }

    let mut from_port = None;
    let mut arrow = None;
    if is_link_port(first_operator) {
        from_port = Some(first_operator.to_string());
        if let Some(second) = link_operator_at(link_body, cursor)
            && !is_link_port(second)
        {
            arrow = Some(second.to_string());
            cursor += second.len();
        }
    } else {
        arrow = Some(first_operator.to_string());
    }

    let target_raw = &link_body[cursor..];
    let (target_without_port, to_port) = split_trailing_link_port(target_raw);
    let target_base = statement_start + cursor;
    let to = parse_name(target_without_port, target_base)?;

    Ok(WardleyLinkAst {
        from,
        to,
        arrow,
        from_port,
        to_port: to_port.map(|(port, _)| port.to_string()),
        label,
    })
}

fn split_link_label(
    statement: &str,
    statement_start: usize,
) -> std::result::Result<(&str, Option<SpannedText>), WardleyParseProblem> {
    let Some(index) = find_unquoted_char(statement, ';') else {
        return Ok((statement, None));
    };
    let raw = &statement[index + 1..];
    if raw.is_empty() {
        return Err(WardleyParseProblem::new(
            "expected wardley link label after ';'",
            SourceSpan::new(statement_start + index, statement_start + index + 1),
        ));
    }
    let (trimmed, _start) = trim_horizontal(raw, statement_start + index + 1);
    let label = SpannedText {
        text: trimmed.to_string(),
        span: SourceSpan::new(statement_start + index, statement_start + statement.len()),
        #[cfg(feature = "editor-facts")]
        selection: SourceSpan::new(_start, _start + trimmed.len()),
        #[cfg(feature = "editor-facts")]
        quoted: false,
    };
    Ok((&statement[..index], Some(label)))
}

fn find_link_operator(input: &str) -> Option<(usize, &str)> {
    let mut quote = None;
    let mut escaped = false;
    let mut index = 0usize;
    while index < input.len() {
        let ch = input[index..].chars().next()?;
        if escaped {
            escaped = false;
            index += ch.len_utf8();
            continue;
        }
        if quote.is_some() && ch == '\\' {
            escaped = true;
            index += ch.len_utf8();
            continue;
        }
        if matches!(ch, '\'' | '"') {
            match quote {
                Some(open) if open == ch => quote = None,
                None => quote = Some(ch),
                _ => {}
            }
            index += ch.len_utf8();
            continue;
        }
        if quote.is_none()
            && let Some(operator) = link_operator_at(input, index)
        {
            if is_link_port(operator) && input[index + operator.len()..].trim().is_empty() {
                index += operator.len();
                continue;
            }
            return Some((index, operator));
        }
        index += ch.len_utf8();
    }
    None
}

fn link_operator_at(input: &str, index: usize) -> Option<&str> {
    let rest = input.get(index..)?;
    if let Some(label_body) = rest.strip_prefix("+'")
        && let Some(close) = label_body.find('\'')
    {
        let suffix_start = 2 + close + 1;
        let suffix = &rest[suffix_start..];
        let suffix_len = if suffix.starts_with("<>") {
            2
        } else if suffix.starts_with(['<', '>']) {
            1
        } else {
            0
        };
        if suffix_len > 0 {
            return Some(&rest[..suffix_start + suffix_len]);
        }
    }
    ["+<>", "-.->", "-->", "->", "+>", "+<", ">"]
        .into_iter()
        .find(|operator| rest.starts_with(operator))
}

fn is_link_port(operator: &str) -> bool {
    matches!(operator, "+<>" | "+>" | "+<")
}

fn split_trailing_link_port(input: &str) -> (&str, Option<(&str, usize)>) {
    let trimmed_end = input.trim_end_matches([' ', '\t']);
    for port in ["+<>", "+>", "+<"] {
        if let Some(prefix) = trimmed_end.strip_suffix(port)
            && !prefix.trim().is_empty()
        {
            return (prefix, Some((port, prefix.len())));
        }
    }
    (input, None)
}

fn parse_operatorless_link(
    input: &str,
    base: usize,
) -> std::result::Result<(SpannedText, SpannedText, Option<String>), WardleyParseProblem> {
    let (trimmed, start) = trim_horizontal(input, base);
    let Some(first_len) = wardley_lexical_name_len(trimmed) else {
        let span = trimmed_span(input, base);
        return Err(WardleyParseProblem::new(
            "expected wardley statement or link operator",
            span,
        ));
    };
    let from = parse_name(&trimmed[..first_len], start)?;
    if first_len == trimmed.len() {
        return Err(WardleyParseProblem::new(
            "expected a second wardley link endpoint",
            SourceSpan::new(start + first_len, start + first_len),
        ));
    }
    let remainder = &trimmed[first_len..];
    let remainder_leading = remainder.len() - remainder.trim_start_matches([' ', '\t']).len();
    let target_start = start + first_len + remainder_leading;
    let target_raw = &remainder[remainder_leading..];
    let (target, to_port) = split_trailing_link_port(target_raw);
    let to = parse_name(target, target_start)?;
    Ok((from, to, to_port.map(|(port, _)| port.to_string())))
}

fn wardley_lexical_name_len(input: &str) -> Option<usize> {
    let first = input.chars().next()?;
    if matches!(first, '\'' | '"') {
        return parse_langium_string(input, 0).map(|parsed| parsed.consumed);
    }
    if first.is_ascii_alphabetic() {
        let mut cursor = first.len_utf8();
        cursor += input[cursor..]
            .bytes()
            .take_while(|byte| is_name_continuation(*byte as char))
            .count();
        loop {
            let whitespace = input[cursor..]
                .bytes()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count();
            if whitespace == 0 {
                break;
            }
            let word_start = cursor + whitespace;
            let Some(next) = input[word_start..].chars().next() else {
                break;
            };
            if !next.is_ascii_alphabetic() && next != '(' {
                break;
            }
            cursor = word_start + next.len_utf8();
            cursor += input[cursor..]
                .bytes()
                .take_while(|byte| is_name_continuation(*byte as char))
                .count();
        }
        return Some(cursor);
    }
    if first.is_ascii_digit() || first == '_' {
        let mut len = input
            .bytes()
            .take_while(|byte| is_id_continuation(*byte as char))
            .count();
        while input[..len].ends_with('-') {
            len -= 1;
        }
        return (len > 0).then_some(len);
    }
    None
}

fn parse_name(input: &str, base: usize) -> std::result::Result<SpannedText, WardleyParseProblem> {
    let (trimmed, start) = trim_horizontal(input, base);
    if trimmed.is_empty() {
        return Err(WardleyParseProblem::new(
            "expected wardley name",
            SourceSpan::new(start, start),
        ));
    }
    if matches!(trimmed.chars().next(), Some('\'' | '"')) {
        let Some(parsed) = parse_langium_string(trimmed, start) else {
            return Err(WardleyParseProblem::new(
                "unterminated quoted wardley name",
                SourceSpan::new(start, start + trimmed.len()),
            ));
        };
        let text = SpannedText {
            text: parsed.value,
            span: parsed.raw_span,
            #[cfg(feature = "editor-facts")]
            selection: parsed.value_span,
            #[cfg(feature = "editor-facts")]
            quoted: true,
        };
        if parsed.consumed != trimmed.len() {
            return Err(WardleyParseProblem::new(
                "unexpected tokens after quoted wardley name",
                SourceSpan::new(start + parsed.consumed, start + trimmed.len()),
            ));
        }
        return Ok(text);
    }
    if !is_valid_wardley_bare_name(trimmed) {
        return Err(WardleyParseProblem::new(
            format!("invalid unquoted wardley name: {trimmed}"),
            SourceSpan::new(start, start + trimmed.len()),
        ));
    }
    let text = SpannedText {
        text: trimmed.to_string(),
        span: SourceSpan::new(start, start + trimmed.len()),
        #[cfg(feature = "editor-facts")]
        selection: SourceSpan::new(start, start + trimmed.len()),
        #[cfg(feature = "editor-facts")]
        quoted: false,
    };
    Ok(text)
}

fn parse_wardley_quoted_text(
    input: &str,
    start: usize,
    context: &str,
) -> std::result::Result<(SpannedText, usize), WardleyParseProblem> {
    let Some(_) = input
        .chars()
        .next()
        .filter(|quote| matches!(quote, '\'' | '"'))
    else {
        return Err(WardleyParseProblem::new(
            context,
            SourceSpan::new(start, start + input.len()),
        ));
    };
    let Some(parsed) = parse_langium_string(input, start) else {
        return Err(WardleyParseProblem::new(
            context,
            SourceSpan::new(start, start + input.len()),
        ));
    };
    let consumed = parsed.consumed;
    let text = SpannedText {
        text: parsed.value,
        span: parsed.raw_span,
        #[cfg(feature = "editor-facts")]
        selection: parsed.value_span,
        #[cfg(feature = "editor-facts")]
        quoted: true,
    };
    Ok((text, consumed))
}

fn is_valid_wardley_bare_name(name: &str) -> bool {
    const RESERVED: &[&str] = &[
        "wardley-beta",
        "size",
        "evolution",
        "anchor",
        "component",
        "label",
        "inertia",
        "evolve",
        "pipeline",
        "note",
        "annotations",
        "annotation",
        "accelerator",
        "deaccelerator",
        "build",
        "buy",
        "outsource",
        "market",
        "title",
        "accTitle",
        "accDescr",
    ];
    if RESERVED.contains(&name) {
        return false;
    }
    if name.contains([' ', '\t'])
        && ["title", "accTitle", "accDescr"].into_iter().any(|prefix| {
            name.strip_prefix(prefix)
                .and_then(|rest| rest.chars().next())
                .is_some_and(|ch| matches!(ch, ' ' | '\t'))
        })
    {
        return false;
    }
    if !name.contains([' ', '\t']) {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        let id = (first.is_ascii_alphanumeric() || first == '_')
            && chars.clone().all(is_id_continuation)
            && !name.ends_with('-');
        let spaced_name_atom = first.is_ascii_alphabetic() && chars.all(is_name_continuation);
        return id || spaced_name_atom;
    }

    let mut words = name.split([' ', '\t']).filter(|word| !word.is_empty());
    let Some(first_word) = words.next() else {
        return false;
    };
    if first_word.is_empty()
        || !first_word
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        || !first_word.chars().all(is_name_continuation)
    {
        return false;
    }
    words.all(|word| {
        word.chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '(')
            && word.chars().all(is_name_continuation)
    })
}

fn is_id_continuation(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-')
}

fn is_name_continuation(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '(' | ')' | '&' | '-')
}

fn split_unquoted_token(
    input: &str,
    token: &str,
    base: usize,
) -> std::result::Result<Vec<(usize, usize)>, WardleyParseProblem> {
    let mut ranges = Vec::new();
    let mut quote = None;
    let mut escaped = false;
    let mut segment_start = 0usize;
    let mut index = 0usize;
    while index < input.len() {
        let ch = input[index..]
            .chars()
            .next()
            .expect("index stays on char boundary");
        if escaped {
            escaped = false;
            index += ch.len_utf8();
            continue;
        }
        if quote.is_some() && ch == '\\' {
            escaped = true;
            index += ch.len_utf8();
            continue;
        }
        if matches!(ch, '\'' | '"') {
            match quote {
                Some(open) if open == ch => quote = None,
                None => quote = Some(ch),
                _ => {}
            }
            index += ch.len_utf8();
            continue;
        }
        if quote.is_none() && input[index..].starts_with(token) {
            ranges.push((segment_start, index));
            index += token.len();
            segment_start = index;
            continue;
        }
        index += ch.len_utf8();
    }
    if quote.is_some() {
        return Err(WardleyParseProblem::new(
            "unterminated quoted wardley value",
            SourceSpan::new(base, base + input.len()),
        ));
    }
    ranges.push((segment_start, input.len()));
    Ok(ranges)
}

fn find_unquoted_char(input: &str, needle: char) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote.is_some() && ch == '\\' {
            escaped = true;
            continue;
        }
        if matches!(ch, '\'' | '"') {
            match quote {
                Some(open) if open == ch => quote = None,
                None => quote = Some(ch),
                _ => {}
            }
            continue;
        }
        if quote.is_none() && ch == needle {
            return Some(index);
        }
    }
    None
}

fn keyword_body<'a>(input: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = input.strip_prefix(keyword)?;
    (rest.is_empty()
        || rest
            .chars()
            .next()
            .is_some_and(|ch| matches!(ch, ' ' | '\t' | '[' | '{')))
    .then_some(rest)
}

fn body_start(statement: &str, statement_start: usize, body: &str) -> usize {
    statement_start + statement.len() - body.len()
}

fn trim_horizontal(input: &str, base: usize) -> (&str, usize) {
    let leading = input.len() - input.trim_start_matches([' ', '\t']).len();
    (input.trim_matches([' ', '\t']), base + leading)
}

fn trimmed_span(input: &str, base: usize) -> SourceSpan {
    let (trimmed, start) = trim_horizontal(input, base);
    SourceSpan::new(start, start + trimmed.len())
}

fn build_wardley_model_controlled(
    ast: &WardleyAst,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<WardleyDiagramModel, WardleyParseProblem>> {
    macro_rules! wardley_try {
        ($expression:expr) => {
            match $expression {
                Ok(value) => value,
                Err(problem) => return Ok(Err(problem)),
            }
        };
    }

    control.checkpoint()?;
    let mut builder = WardleyBuilder::default();
    if let Some(size) = &ast.size {
        builder.size = Some(WardleySize {
            width: size.width.value,
            height: size.height.value,
        });
    }
    if let Some(evolution) = &ast.evolution {
        builder.axes.stages.reserve(evolution.stages.len());
        builder
            .axes
            .stage_boundaries
            .reserve(evolution.stages.len());
        for (index, stage) in evolution.stages.iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            builder.axes.stages.push(match &stage.second_name {
                Some(second) => format!("{} / {}", stage.name.text.trim(), second.text.trim()),
                None => stage.name.text.trim().to_string(),
            });
            if let Some(boundary) = stage.boundary {
                builder.axes.stage_boundaries.push(boundary.value);
            }
        }
    }

    for (index, anchor) in ast.anchors.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let point = wardley_try!(to_coordinates(
            anchor.visibility,
            anchor.evolution,
            &format!("Anchor \"{}\"", anchor.name.text),
        ));
        builder.add_node(PendingWardleyNode {
            id: anchor.name.text.clone(),
            label: anchor.name.text.clone(),
            x: point.x,
            y: point.y,
            class_name: Some("anchor".to_string()),
            label_offset_x: None,
            label_offset_y: None,
            in_pipeline: false,
            is_pipeline_parent: false,
            inertia: None,
            source_strategy: None,
        });
    }

    for (index, component) in ast.components.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let point = wardley_try!(to_coordinates(
            component.positioned.visibility,
            component.positioned.evolution,
            &format!("Component \"{}\"", component.positioned.name.text),
        ));
        builder.add_node(PendingWardleyNode {
            id: component.positioned.name.text.clone(),
            label: component.positioned.name.text.clone(),
            x: point.x,
            y: point.y,
            class_name: Some("component".to_string()),
            label_offset_x: component.label.as_ref().map(|label| label.offset_x.value),
            label_offset_y: component.label.as_ref().map(|label| label.offset_y.value),
            in_pipeline: false,
            is_pipeline_parent: false,
            inertia: Some(component.inertia),
            source_strategy: component.source_strategy,
        });
    }

    for (index, note) in ast.notes.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let point = wardley_try!(to_coordinates(
            note.visibility,
            note.evolution,
            &format!("Note \"{}\"", note.text.text),
        ));
        builder.notes.push(WardleyNote {
            text: note.text.text.clone(),
            x: point.x,
            y: point.y,
        });
    }

    for (pipeline_index, pipeline) in ast.pipelines.iter().enumerate() {
        if pipeline_index % 128 == 0 {
            control.checkpoint()?;
        }
        let Some(parent_y) = builder.nodes.get(&pipeline.parent.text).map(|node| node.y) else {
            return Ok(Err(WardleyParseProblem::new(
                format!(
                    "Pipeline \"{}\" must reference an existing component with coordinates.",
                    pipeline.parent.text
                ),
                pipeline.parent.span,
            )));
        };
        builder.start_pipeline(&pipeline.parent.text);
        for (component_index, component) in pipeline.components.iter().enumerate() {
            if component_index % 128 == 0 {
                control.checkpoint()?;
            }
            let component_id = format!("{}_{}", pipeline.parent.text, component.name.text);
            let x = wardley_try!(to_percent(
                component.evolution,
                &format!("Pipeline component \"{}\" evolution", component.name.text),
            ));
            builder.add_node(PendingWardleyNode {
                id: component_id.clone(),
                label: component.name.text.clone(),
                x,
                y: parent_y,
                class_name: Some("pipeline-component".to_string()),
                label_offset_x: component.label.as_ref().map(|label| label.offset_x.value),
                label_offset_y: component.label.as_ref().map(|label| label.offset_y.value),
                in_pipeline: false,
                is_pipeline_parent: false,
                inertia: None,
                source_strategy: None,
            });
            builder.add_pipeline_component(&pipeline.parent.text, &component_id);
        }
    }

    for (index, link) in ast.links.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let dashed = link
            .arrow
            .as_deref()
            .is_some_and(|arrow| arrow.contains("-.->") || arrow.contains(".-."));
        let mut flow = link
            .from_port
            .as_deref()
            .and_then(flow_from_port)
            .or_else(|| link.to_port.as_deref().and_then(flow_from_port));
        let (arrow_flow, flow_label) = link
            .arrow
            .as_deref()
            .map(flow_from_arrow)
            .unwrap_or_default();
        if flow.is_none() {
            flow = arrow_flow;
        }
        let label = flow_label.or_else(|| link.label.as_ref().map(|label| label.text.clone()));
        builder.links.push(WardleyLink {
            source: builder.resolve_node_id_controlled(&link.from.text, control)?,
            target: builder.resolve_node_id_controlled(&link.to.text, control)?,
            dashed,
            label,
            flow,
        });
    }

    for (index, evolve) in ast.evolves.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let Some(node_y) = builder.nodes.get(&evolve.component.text).map(|node| node.y) else {
            continue;
        };
        let target_x = wardley_try!(to_percent(
            evolve.target,
            &format!("Evolve target for \"{}\"", evolve.component.text),
        ));
        builder.trends.insert(
            evolve.component.text.clone(),
            WardleyTrend {
                node_id: evolve.component.text.clone(),
                target_x,
                target_y: node_y,
            },
        );
    }

    if let Some(annotations_box) = ast.annotations_boxes.first() {
        builder.annotations_box = Some(wardley_try!(to_coordinates(
            annotations_box.x,
            annotations_box.y,
            "Annotations box",
        )));
    }
    for (index, annotation) in ast.annotations.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let point = wardley_try!(to_coordinates(
            annotation.x,
            annotation.y,
            &format!(
                "Annotation {}",
                ryu_js::Buffer::new().format(annotation.number.value)
            ),
        ));
        builder.annotations.push(WardleyAnnotation {
            number: annotation.number.value,
            coordinates: vec![point],
            text: Some(annotation.text.text.clone()),
        });
    }
    for (index, accelerator) in ast.accelerators.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let point = wardley_try!(to_coordinates(
            accelerator.x,
            accelerator.y,
            &format!("Accelerator \"{}\"", accelerator.name.text),
        ));
        builder.accelerators.push(WardleyAccelerator {
            name: accelerator.name.text.clone(),
            x: point.x,
            y: point.y,
        });
    }
    for (index, deaccelerator) in ast.deaccelerators.iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let point = wardley_try!(to_coordinates(
            deaccelerator.x,
            deaccelerator.y,
            &format!("Deaccelerator \"{}\"", deaccelerator.name.text),
        ));
        builder.deaccelerators.push(WardleyDeaccelerator {
            name: deaccelerator.name.text.clone(),
            x: point.x,
            y: point.y,
        });
    }

    let common = LangiumCommonDbFields::from_facts(&ast.common);
    Ok(Ok(builder.finish_controlled(common, control)?))
}

fn to_coordinates(
    visibility: SpannedNumber,
    evolution: SpannedNumber,
    context: &str,
) -> std::result::Result<WardleyPoint, WardleyParseProblem> {
    Ok(WardleyPoint {
        x: to_percent(evolution, &format!("{context} evolution"))?,
        y: to_percent(visibility, &format!("{context} visibility"))?,
    })
}

fn to_percent(
    value: SpannedNumber,
    context: &str,
) -> std::result::Result<f64, WardleyParseProblem> {
    let normalized = if value.value <= 1.0 {
        value.value * 100.0
    } else {
        value.value
    };
    if !(0.0..=100.0).contains(&normalized) {
        return Err(WardleyParseProblem::new(
            format!(
                "{context} must be between 0-1 (decimal) or 0-100 (percentage). Received: {}",
                ryu_js::Buffer::new().format(value.value)
            ),
            value.span,
        ));
    }
    Ok(normalized)
}

fn flow_from_port(port: &str) -> Option<WardleyFlowDirection> {
    match port {
        "+<>" => Some(WardleyFlowDirection::Bidirectional),
        "+<" => Some(WardleyFlowDirection::Backward),
        "+>" => Some(WardleyFlowDirection::Forward),
        _ => None,
    }
}

fn flow_from_arrow(arrow: &str) -> (Option<WardleyFlowDirection>, Option<String>) {
    if !arrow.starts_with('+') {
        return (None, None);
    }
    let label = arrow
        .strip_prefix("+'")
        .and_then(|rest| rest.find('\'').map(|end| rest[..end].to_string()));
    let flow = if arrow.contains("<>") {
        Some(WardleyFlowDirection::Bidirectional)
    } else if arrow.contains('<') {
        Some(WardleyFlowDirection::Backward)
    } else if arrow.contains('>') {
        Some(WardleyFlowDirection::Forward)
    } else {
        None
    };
    (flow, label)
}

#[cfg(test)]
mod tests;
