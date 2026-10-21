//! Semantic-only parsing without resolving Mermaid themes or creating render state.

use crate::preprocess::preprocess_builtin_semantic_controlled;
use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, Result};

/// Configuration and source metadata needed to interpret a diagram's meaning.
#[derive(Debug, Clone)]
pub struct SemanticMetadata {
    pub diagram_type: String,
    /// Source-authored config projection used by Mermaid's public parse metadata. It follows
    /// upstream secure keys unless the host explicitly supplies its own `secure` policy.
    pub config: MermaidConfig,
    /// Effective semantic config after the site's secure-key and CSS-value admission, before
    /// theme materialization.
    pub semantic_config: MermaidConfig,
    pub title: Option<String>,
}

/// A typed semantic result. Variants are added as diagram families migrate to semantic parsing.
#[non_exhaustive]
#[derive(Debug)]
pub enum SemanticDiagramModel {
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    Flowchart(crate::diagrams::flowchart::FlowchartModel),
    #[cfg(feature = "diagram-sequence")]
    Sequence(crate::diagrams::sequence::SequenceDiagramModel),
    #[cfg(feature = "diagram-state")]
    State(crate::diagrams::state::StateDiagramModel),
    #[cfg(feature = "diagram-timeline")]
    Timeline(crate::diagrams::timeline::TimelineDiagramModel),
    #[cfg(feature = "diagram-cynefin")]
    Cynefin(crate::diagrams::cynefin::CynefinDiagramModel),
    #[cfg(feature = "diagram-info")]
    Info(crate::diagrams::info::InfoDiagramModel),
    #[cfg(feature = "diagram-journey")]
    Journey(crate::diagrams::journey::JourneyDiagramModel),
    #[cfg(feature = "diagram-pie")]
    Pie(crate::diagrams::pie::PieDiagramModel),
    #[cfg(feature = "diagram-packet")]
    Packet(crate::diagrams::packet::PacketDiagramModel),
    #[cfg(feature = "diagram-tree-view")]
    TreeView(crate::diagrams::tree_view::TreeViewDiagramModel),
    #[cfg(feature = "diagram-quadrant-chart")]
    QuadrantChart(crate::diagrams::quadrant_chart::QuadrantChartDiagramModel),
    #[cfg(feature = "diagram-radar")]
    Radar(crate::diagrams::radar::RadarDiagramModel),
    #[cfg(feature = "diagram-sankey")]
    Sankey(crate::diagrams::sankey::SankeyDiagramModel),
    #[cfg(feature = "diagram-venn")]
    Venn(crate::diagrams::venn::VennDiagramModel),
    #[cfg(feature = "diagram-requirement")]
    Requirement(crate::diagrams::requirement::RequirementDiagramModel),
    #[cfg(feature = "diagram-usecase")]
    Usecase(crate::diagrams::usecase::UsecaseDiagramModel),
    #[cfg(feature = "diagram-event-modeling")]
    EventModeling(crate::diagrams::eventmodeling::EventModelingDiagramModel),
    #[cfg(feature = "diagram-wardley")]
    Wardley(crate::diagrams::wardley::WardleyDiagramModel),
    #[cfg(feature = "diagram-railroad")]
    Railroad(crate::diagrams::railroad::RailroadDiagramModel),
    #[cfg(feature = "diagram-zenuml")]
    Zenuml(crate::diagrams::zenuml::ZenumlDiagramModel),
    #[cfg(feature = "diagram-er")]
    Er(crate::diagrams::er::ErDiagramModel),
    #[cfg(feature = "diagram-architecture")]
    Architecture(crate::diagrams::architecture::ArchitectureDiagramModel),
    #[cfg(feature = "diagram-gantt")]
    Gantt(crate::diagrams::gantt::GanttDiagramModel),
    #[cfg(feature = "diagram-c4")]
    C4(crate::diagrams::c4::C4DiagramModel),
    #[cfg(feature = "diagram-ishikawa")]
    Ishikawa(crate::diagrams::ishikawa::IshikawaDiagramModel),
    #[cfg(feature = "diagram-treemap")]
    Treemap(crate::diagrams::treemap::TreemapDiagramModel),
    #[cfg(feature = "diagram-block")]
    Block(crate::diagrams::block::BlockDiagramModel),
    #[cfg(feature = "diagram-kanban")]
    Kanban(crate::diagrams::kanban::KanbanDiagramModel),
    #[cfg(feature = "diagram-mindmap")]
    Mindmap(crate::diagrams::mindmap::MindmapDiagramModel),
    #[cfg(feature = "diagram-xychart")]
    XyChart(crate::diagrams::xychart::XyChartDiagramModel),
    #[cfg(feature = "diagram-agentflow")]
    Agentflow(crate::diagrams::agentflow::AgentflowDiagramModel),
    #[cfg(feature = "diagram-git-graph")]
    GitGraph(crate::diagrams::git_graph::GitGraphDiagramModel),
    #[cfg(feature = "diagram-class")]
    Class(crate::diagrams::class::ClassDiagramModel),
}

impl SemanticDiagramModel {
    fn remap_source_positions(
        &mut self,
        source: &crate::PreprocessedSource,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        control.checkpoint()?;
        #[allow(unreachable_patterns)]
        let facts: &mut [crate::DiagramWarningFact] = match self {
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow(model) => {
                let (line_offset, column_offset) = source.parser_position_offset();
                model.offset_diagnostic_positions(line_offset, column_offset);
                &mut model.warning_facts
            }
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(model) => &mut model.warning_facts,
            #[cfg(feature = "diagram-block")]
            Self::Block(model) => &mut model.warning_facts,
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(model) => &mut model.warning_facts,
            _ => &mut [],
        };
        for (index, fact) in facts.iter_mut().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            source.remap_warning_fact_spans(fact);
        }
        control.checkpoint()
    }
}

/// Metadata and canonical family model returned by [`SemanticParser`].
#[derive(Debug)]
pub struct ParsedSemanticDiagram {
    pub metadata: SemanticMetadata,
    pub model: SemanticDiagramModel,
}

/// Parses diagram meaning without requiring theme resolution, layout, or SVG rendering.
///
/// Enabled families publish their canonical parser through the family catalog.
/// Other enabled families return [`Error::UnsupportedDiagram`] until their semantic parsers migrate here.
#[derive(Debug, Clone)]
pub struct SemanticParser {
    site_config: MermaidConfig,
    site_config_overrides: MermaidConfig,
    runtime_policy: crate::runtime::RuntimePolicy,
}

impl Default for SemanticParser {
    fn default() -> Self {
        Self {
            site_config: crate::generated::default_site_config(),
            site_config_overrides: MermaidConfig::empty_object(),
            runtime_policy: crate::runtime::RuntimePolicy::deterministic(),
        }
    }
}

impl SemanticParser {
    /// Creates a parser with Mermaid defaults and Merman's hardened source-config policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the runtime policy used to begin future parse operations.
    pub fn with_runtime_policy(mut self, policy: crate::runtime::RuntimePolicy) -> Self {
        self.runtime_policy = policy;
        self
    }

    /// Replays a frozen runtime context without consulting system state.
    pub fn with_operation_context(self, context: crate::runtime::OperationContext) -> Self {
        self.with_runtime_policy(crate::runtime::RuntimePolicy::from_operation_context(
            context,
        ))
    }

    pub fn runtime_policy(&self) -> &crate::runtime::RuntimePolicy {
        &self.runtime_policy
    }

    pub fn begin_operation(
        &self,
    ) -> std::result::Result<crate::runtime::OperationContext, crate::runtime::RuntimePolicyError>
    {
        self.runtime_policy.begin_operation()
    }

    /// Applies host-level config before parsing. An explicit `secure` array replaces the
    /// hardened defaults, matching the full parser's site-policy behavior.
    pub fn with_site_config(mut self, mut overrides: MermaidConfig) -> Self {
        if overrides.is_empty_object() {
            return self;
        }
        crate::config::mirror_legacy_font_family_into_theme_variables(&mut overrides);
        normalize_global_theme(&mut overrides);
        let explicit_secure = overrides
            .as_value()
            .get("secure")
            .filter(|value| value.is_array())
            .map(crate::json_value::clone_value_nonrecursive);

        self.site_config_overrides.deep_merge(overrides.as_value());
        self.site_config.deep_merge(overrides.as_value());
        if let Some(secure) = explicit_secure {
            self.site_config_overrides
                .set_value("secure", secure.clone());
            self.site_config.set_value("secure", secure);
        }
        self
    }

    /// Detects and parses a built-in family supported by the semantic facade.
    pub fn parse_diagram_sync(&self, input: &str) -> Result<ParsedSemanticDiagram> {
        let control = OperationControl::new();
        self.parse_diagram_controlled_sync(input, &control)
            .expect("a private semantic parse control cannot be cancelled")
    }

    /// Controlled variant for callers that need to cancel detection or semantic parsing.
    pub fn parse_diagram_controlled_sync(
        &self,
        input: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        self.parse_input_controlled(input, None, control)
    }

    /// Parses a diagram when its Mermaid type is already known and supported by this facade.
    pub fn parse_diagram_with_type_sync(
        &self,
        diagram_type: &str,
        input: &str,
    ) -> Result<ParsedSemanticDiagram> {
        let control = OperationControl::new();
        self.parse_diagram_with_type_controlled_sync(diagram_type, input, &control)
            .expect("a private semantic parse control cannot be cancelled")
    }

    /// Controlled variant for callers that need to cancel semantic parsing.
    pub fn parse_diagram_with_type_controlled_sync(
        &self,
        diagram_type: &str,
        input: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        self.parse_input_controlled(input, Some(diagram_type), control)
    }

    fn parse_input_controlled(
        &self,
        input: &str,
        diagram_type: Option<&str>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        control.checkpoint()?;
        let context = match self.begin_operation() {
            Ok(context) => context,
            Err(error) => return Ok(Err(error.into())),
        };
        crate::runtime::with_operation_context(&context, || {
            let pre = match preprocess_builtin_semantic_controlled(input, diagram_type, control)? {
                Ok(pre) => pre,
                Err(error) => return Ok(Err(error)),
            };
            let diagram_type = match diagram_type {
                Some(diagram_type) => diagram_type,
                None => {
                    match crate::detect::detect_builtin_precleaned_controlled(pre.code(), control)?
                    {
                        Ok(fact) => fact.id,
                        Err(error) => return Ok(Err(error)),
                    }
                }
            };
            self.parse_preprocessed_controlled(diagram_type, pre, control)
        })
    }

    fn parse_preprocessed_controlled(
        &self,
        diagram_type: &str,
        pre: crate::PreprocessResult,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        control.checkpoint()?;
        if pre.code().trim_start().starts_with("---") {
            return Ok(Err(Error::MalformedFrontMatter));
        }

        let mut semantic_config = self.effective_semantic_config(&pre.config, control)?;
        crate::detect::builtin_config_effect(diagram_type).apply(&mut semantic_config);
        let upstream_defaults = crate::generated::upstream_default_config();
        let source_authority = if self
            .site_config_overrides
            .as_value()
            .get("secure")
            .is_some()
        {
            &self.site_config_overrides
        } else {
            &upstream_defaults
        };
        let config = source_authority.authored_source_overrides(pre.config, control)?;
        let title = pre
            .title
            .as_deref()
            .map(|title| crate::sanitize::sanitize_text(title, &semantic_config));
        let metadata = SemanticMetadata {
            diagram_type: diagram_type.to_string(),
            config,
            semantic_config,
            title,
        };
        let source = crate::preprocess::select_family_parser_source_controlled(
            pre.source,
            pre.with_comments,
            diagram_type,
            &metadata.semantic_config,
            control,
        )?;
        let parser_input = source.text();

        let Some(parser) = crate::family::canonical_parser(diagram_type) else {
            return Ok(Err(Error::UnsupportedDiagram {
                diagram_type: diagram_type.to_string(),
            }));
        };
        let mut model = match parser(parser_input, &metadata, control)? {
            Ok(model) => model,
            Err(error) => return Ok(Err(source.remap_parse_error(error))),
        };
        model.remap_source_positions(&source, control)?;
        control.checkpoint()?;
        Ok(Ok(ParsedSemanticDiagram { metadata, model }))
    }

    fn effective_semantic_config(
        &self,
        source: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<MermaidConfig> {
        let mut config = self.site_config.clone();
        if !source.is_empty_object() {
            let admitted = self
                .site_config
                .source_filtered_overrides(source, control)?;
            config.deep_merge(admitted.as_value());
        }
        Ok(config)
    }
}

fn normalize_global_theme(config: &mut MermaidConfig) {
    let invalid_theme = config.as_value().get("theme").is_some_and(|value| {
        !value.is_null()
            && !value
                .as_str()
                .is_some_and(|name| name == "null" || crate::MermaidThemeId::NAMES.contains(&name))
    });
    if invalid_theme
        && let Some(theme) = crate::generated::upstream_default_config()
            .as_value()
            .get("theme")
    {
        config.set_value("theme", crate::json_value::clone_value_nonrecursive(theme));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "diagram-git-graph")]
    fn semantic_gitgraph_ids(parser: &SemanticParser, known_type: bool) -> Vec<String> {
        let source = "gitGraph\ncommit\ncommit\n";
        let parsed = if known_type {
            parser.parse_diagram_with_type_sync("gitGraph", source)
        } else {
            parser.parse_diagram_sync(source)
        }
        .expect("GitGraph semantic parse");
        let SemanticDiagramModel::GitGraph(model) = parsed.model else {
            panic!("expected GitGraph semantics");
        };
        model.commits.into_iter().map(|commit| commit.id).collect()
    }

    #[cfg(feature = "diagram-git-graph")]
    #[test]
    fn semantic_parser_owns_its_runtime_inside_an_ambient_operation() {
        let parser = SemanticParser::new();
        let expected = semantic_gitgraph_ids(&parser, false);
        let outer = crate::runtime::RuntimePolicy::deterministic()
            .with_fixed_seed(1)
            .with_fixed_unix_millis(1_771_113_600_000)
            .begin_operation()
            .unwrap();
        crate::runtime::with_operation_context(&outer, || {
            assert_eq!(semantic_gitgraph_ids(&parser, false), expected);
            assert_eq!(semantic_gitgraph_ids(&parser, true), expected);
            assert_eq!(crate::runtime::today_local(), outer.today_local());
        });
    }

    #[cfg(feature = "diagram-git-graph")]
    #[test]
    fn semantic_runtime_seed_and_captured_context_control_commit_ids() {
        let policy = crate::runtime::RuntimePolicy::deterministic().with_fixed_seed(1);
        let first = SemanticParser::new().with_runtime_policy(policy.clone());
        let second = SemanticParser::new()
            .with_runtime_policy(crate::runtime::RuntimePolicy::deterministic().with_fixed_seed(2));
        let replay = SemanticParser::new().with_operation_context(first.begin_operation().unwrap());
        let expected = semantic_gitgraph_ids(&first, false);
        assert_eq!(first.runtime_policy(), &policy);
        assert_ne!(semantic_gitgraph_ids(&second, false), expected);
        for known_type in [false, true] {
            assert_eq!(semantic_gitgraph_ids(&first, known_type), expected);
            assert_eq!(semantic_gitgraph_ids(&replay, known_type), expected);
        }
        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .with_runtime_policy(policy)
                .parse_diagram_for_render_model_sync(
                    "gitGraph\ncommit\ncommit\n",
                    crate::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::GitGraph(model) = full.model() else {
                panic!("expected full GitGraph model");
            };
            assert_eq!(
                model.commits.iter().map(|c| &c.id).collect::<Vec<_>>(),
                expected.iter().collect::<Vec<_>>()
            );
        }
    }

    #[cfg(feature = "diagram-gantt")]
    #[test]
    fn semantic_runtime_clock_and_timezone_resolve_gantt_dates() {
        let source = "gantt\ndateFormat MM-DD\nMissing year: id1,03-01,1d\nMissing ref: id2,after missing,1d\n";
        for (offset, adjustment) in [(0, 0), (480, -28_800_000)] {
            let policy = crate::runtime::RuntimePolicy::deterministic()
                .with_fixed_unix_millis(1_771_113_600_000)
                .try_with_fixed_local_offset_minutes(offset)
                .unwrap();
            let parser = SemanticParser::new().with_runtime_policy(policy);
            for known_type in [false, true] {
                crate::diagrams::gantt::reset_gantt_syntax_construction_count();
                let parsed = if known_type {
                    parser.parse_diagram_with_type_sync("gantt", source)
                } else {
                    parser.parse_diagram_sync(source)
                }
                .unwrap();
                let SemanticDiagramModel::Gantt(model) = parsed.model else {
                    panic!("expected Gantt semantics");
                };
                assert_eq!(model.tasks[0].start_ms, 1_772_323_200_000 + adjustment);
                assert_eq!(model.tasks[1].start_ms, 1_771_113_600_000 + adjustment);
                assert_eq!(crate::diagrams::gantt::gantt_syntax_construction_count(), 1);
            }
        }
    }

    #[cfg(feature = "diagram-gantt")]
    #[test]
    fn semantic_runtime_restores_ambient_context_after_errors_and_cancellation() {
        let policy = crate::runtime::RuntimePolicy::deterministic()
            .with_fixed_unix_millis(1_771_113_600_000);
        let parser = SemanticParser::new().with_runtime_policy(policy);
        let outer = crate::runtime::RuntimePolicy::deterministic()
            .with_fixed_today(Some(crate::time::CivilDate::new(2030, 1, 2).unwrap()))
            .begin_operation()
            .unwrap();
        let valid = "gantt\ndateFormat MM-DD\nMissing year: id1,03-01,1d\n";
        let invalid = "gantt\nweekday someday\n";
        crate::runtime::with_operation_context(&outer, || {
            for known_type in [false, true] {
                let parse = |input: &str, control: &OperationControl| {
                    if known_type {
                        parser.parse_diagram_with_type_controlled_sync("gantt", input, control)
                    } else {
                        parser.parse_diagram_controlled_sync(input, control)
                    }
                };
                for _ in 0..2 {
                    let parsed = parse(valid, &OperationControl::new()).unwrap().unwrap();
                    let SemanticDiagramModel::Gantt(model) = parsed.model else {
                        panic!("expected Gantt semantics");
                    };
                    assert_eq!(model.tasks[0].start_ms, 1_772_323_200_000);
                    assert_eq!(crate::runtime::today_local(), outer.today_local());
                    let error = parse(invalid, &OperationControl::new())
                        .unwrap()
                        .unwrap_err();
                    let Error::DiagramParse { diagnostic, .. } = error else {
                        panic!("expected Gantt parse diagnostic");
                    };
                    let start = invalid.find("someday").unwrap();
                    assert_eq!(
                        diagnostic.span(),
                        Some(crate::SourceSpan::new(start, start + "someday".len()))
                    );
                    assert_eq!(crate::runtime::today_local(), outer.today_local());
                    let control = OperationControl::new();
                    control.cancel_after_checkpoints(2);
                    assert!(parse(valid, &control).is_err());
                    assert_eq!(crate::runtime::today_local(), outer.today_local());
                }
            }
        });
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn semantic_agentflow_preserves_quoted_comment_lines() {
        let source = "agentflow-beta\nA[\"One\n%% Two\nThree\"]\n";
        let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
        let model = match parsed.model {
            SemanticDiagramModel::Agentflow(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected Agentflow semantics"),
        };
        assert_eq!(
            model.vertices[0].label.as_deref(),
            Some("One\n%% Two\nThree")
        );
        #[cfg(feature = "presentation")]
        {
            let full = crate::Engine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let crate::RenderSemanticModel::Agentflow(full) = full.model() else {
                panic!("expected full Agentflow model");
            };
            assert_eq!(model.vertices[0].label, full.vertices[0].label);
        }
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn semantic_agentflow_preserves_comment_aware_diagnostic_positions() {
        let source = "---\r\ntitle: \"\u{4e2d}\u{6587}\u{1f600}\"\r\n---\r\nagentflow-beta\r\n%% comment\r\na[\"\u{4e2d}\u{6587}\u{1f600}\"]; b@{ shape: cloud }\r\n";
        let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
        let model = match parsed.model {
            SemanticDiagramModel::Agentflow(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected Agentflow semantics"),
        };
        let position = model.diagnostics[0].position.as_ref().unwrap();
        assert_eq!((position.start_line, position.start_column), (6, 11));
        assert_eq!((position.end_line, position.end_column), (6, 28));
        let span = model.warning_facts[0].span.unwrap();
        assert_eq!(&source[span.start..span.end], "b@{ shape: cloud }");
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn semantic_source_positions_map_parse_errors_to_original_input() {
        let source = "---\ntitle: Source\n---\nagentflow-beta\nA ==> B\n";
        let error = SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a structured Agentflow parse error");
        };
        let span = diagnostic.span().expect("exact error span");
        assert_eq!(&source[span.start..span.end], "A ==> B");
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn semantic_source_positions_map_warnings_to_original_input() {
        let source = "\u{feff}---\r\ntitle: Source\r\n---\r\nagentflow-beta\r\n A((Circle))\r\n";
        let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
        let SemanticDiagramModel::Agentflow(model) = parsed.model else {
            panic!("expected Agentflow semantics");
        };
        let warning = &model.warning_facts[0];
        let span = warning.span.expect("warning span");
        assert_eq!(&source[span.start..span.end], "A((Circle))");
        let position = model.diagnostics[0].position.as_ref().unwrap();
        assert_eq!((position.start_line, position.start_column), (5, 1));
    }

    #[cfg(feature = "diagram-timeline")]
    #[test]
    fn semantic_timeline_is_available_without_presentation_or_editor_work() {
        crate::editor::test_work::reset();
        let parsed = SemanticParser::new()
            .parse_diagram_sync("timeline\nsection Phase\n2026 : Launch\n")
            .expect("Timeline semantics");
        assert_eq!(parsed.metadata.diagram_type, "timeline");
        assert!(matches!(parsed.model, SemanticDiagramModel::Timeline(_)));
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
    #[cfg(feature = "diagram-state")]
    #[test]
    fn semantic_state_parses_without_presentation_or_editor_work() {
        crate::editor::test_work::reset();
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync(
                "stateDiagram",
                "stateDiagram-v2\nstate Outer {\n[*] --> A: ready\n}\nOuter --> Done\n",
            )
            .expect("State semantics are available without presentation");
        assert_eq!(parsed.metadata.diagram_type, "stateDiagram");
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[cfg(any(
        feature = "diagram-cynefin",
        feature = "diagram-flowchart",
        feature = "diagram-info",
        feature = "diagram-pie"
    ))]
    use serde_json::json;
    #[cfg(all(feature = "diagram-flowchart", feature = "presentation"))]
    use serde_json::to_value;

    #[cfg(all(feature = "diagram-cynefin", feature = "diagram-flowchart"))]
    #[test]
    fn semantic_cynefin_parse_matches_full_typed_model() {
        let source =
            "cynefin-beta\ncomplex\n\"Probe\"\nclear\n\"Classify\"\ncomplex --> clear: \"learn\"";
        let parsed = SemanticParser::new()
            .parse_diagram_sync(source)
            .expect("automatic semantic parse");
        assert_eq!(parsed.metadata.diagram_type, "cynefin");
        let SemanticDiagramModel::Cynefin(semantic_model) = parsed.model else {
            panic!("expected Cynefin model");
        };
        assert_eq!(semantic_model.domains.len(), 2);
        #[cfg(feature = "presentation")]
        {
            let full = crate::Engine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "cynefin",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("Cynefin result");
            let crate::diagram::RenderSemanticModel::Cynefin(full_model) = full.model() else {
                panic!("expected full Cynefin model");
            };
            assert_eq!(
                serde_json::to_value(semantic_model).unwrap(),
                serde_json::to_value(full_model).unwrap()
            );
        }
    }

    #[cfg(all(
        feature = "diagram-cynefin",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane"),
        not(feature = "diagram-sequence")
    ))]
    #[test]
    fn semantic_cynefin_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("cynefin-beta\ncomplex\n\"Probe\"")
            .expect("Cynefin-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Cynefin(_)));
    }

    #[cfg(feature = "diagram-cynefin")]
    #[test]
    fn semantic_cynefin_parse_does_not_resolve_invalid_visual_theme_values() {
        let input = "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\ncynefin-beta\ncomplex\n\"Probe\"";
        let parsed = SemanticParser::new()
            .parse_diagram_sync(input)
            .expect("semantic parsing does not evaluate theme colors");
        assert!(matches!(parsed.model, SemanticDiagramModel::Cynefin(_)));
        assert_eq!(
            parsed.metadata.semantic_config.as_value()["themeVariables"]["primaryColor"],
            json!("not-a-color")
        );
        #[cfg(feature = "presentation")]
        {
            let full = crate::Engine::new().parse_diagram_for_render_model_with_type_sync(
                "cynefin",
                input,
                crate::ParseOptions::strict(),
            );
            assert!(
                full.is_ok(),
                "Cynefin typed-model construction should not resolve theme variables: {full:?}"
            );
        }
    }

    #[cfg(feature = "diagram-info")]
    #[test]
    fn semantic_info_matches_full_typed_model_without_editor_or_compat_state() {
        crate::editor::test_work::reset();
        let source = "info\nshowInfo\ntitle Version\n";
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("info", source)
            .expect("semantic Info parse");
        assert_eq!(parsed.metadata.diagram_type, "info");
        let semantic_model = match parsed.model {
            SemanticDiagramModel::Info(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Info model"),
        };
        assert!(semantic_model.show_info);
        assert_eq!(
            serde_json::to_value(&semantic_model).unwrap(),
            json!({"showInfo": true})
        );
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            let full = crate::Engine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "info",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("Info result");
            let crate::diagram::RenderSemanticModel::Info(full_model) = full.model() else {
                panic!("expected full Info model");
            };
            assert_eq!(semantic_model.show_info, full_model.show_info);
        }
        crate::editor::test_work::reset();
    }

    #[cfg(all(
        feature = "diagram-info",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane"),
        not(feature = "diagram-sequence"),
        not(feature = "diagram-cynefin")
    ))]
    #[test]
    fn semantic_info_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("info\nshowInfo")
            .expect("Info-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Info(_)));
    }

    #[cfg(feature = "diagram-pie")]
    #[test]
    fn semantic_pie_matches_full_typed_model_without_editor_or_compat_state() {
        crate::editor::test_work::reset();
        let source = "pie showData\ntitle Revenue\n\"A\": 1\n\"B\": 2\n";
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("pie", source)
            .expect("semantic Pie parse");
        let semantic_model = match parsed.model {
            SemanticDiagramModel::Pie(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Pie model"),
        };
        assert_eq!(
            serde_json::to_value(&semantic_model).unwrap(),
            json!({
                "showData": true,
                "title": "Revenue",
                "accTitle": null,
                "accDescr": null,
                "sections": [
                    {"label": "A", "value": 1.0},
                    {"label": "B", "value": 2.0}
                ]
            })
        );
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            let full = crate::Engine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "pie",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("Pie result");
            let crate::diagram::RenderSemanticModel::Pie(full_model) = full.model() else {
                panic!("expected full Pie model");
            };
            assert_eq!(
                serde_json::to_value(&semantic_model).unwrap(),
                json!({
                    "showData": full_model.show_data,
                    "title": full_model.title,
                    "accTitle": full_model.acc_title,
                    "accDescr": full_model.acc_descr,
                    "sections": full_model.sections,
                })
            );
        }
        crate::editor::test_work::reset();
    }

    #[cfg(all(
        feature = "diagram-pie",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane"),
        not(feature = "diagram-sequence"),
        not(feature = "diagram-cynefin"),
        not(feature = "diagram-info")
    ))]
    #[test]
    fn semantic_pie_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("pie\n\"A\": 1")
            .expect("Pie-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Pie(_)));
    }

    #[cfg(feature = "diagram-journey")]
    #[test]
    fn semantic_journey_matches_full_typed_model_without_editor_or_compat_state() {
        crate::editor::test_work::reset();
        let source = concat!(
            "journey\n",
            "title Checkout\n",
            "section Payment\n",
            "Confirm order: 5: Alice, Bob\n",
        );
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("journey", source)
            .expect("semantic Journey parse");
        let semantic_model = match parsed.model {
            SemanticDiagramModel::Journey(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Journey model"),
        };
        assert_eq!(semantic_model.tasks.len(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        #[cfg(feature = "presentation")]
        {
            let full = crate::Engine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "journey",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("Journey result");
            let crate::diagram::RenderSemanticModel::Journey(full_model) = full.model() else {
                panic!("expected full Journey model");
            };
            assert_eq!(
                serde_json::to_value(&semantic_model).unwrap(),
                serde_json::to_value(full_model).unwrap()
            );
        }
        crate::editor::test_work::reset();
    }

    #[cfg(all(
        feature = "diagram-journey",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane"),
        not(feature = "diagram-sequence"),
        not(feature = "diagram-cynefin"),
        not(feature = "diagram-info"),
        not(feature = "diagram-pie")
    ))]
    #[test]
    fn semantic_journey_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("journey\nTask: 5")
            .expect("Journey-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Journey(_)));
    }

    #[cfg(all(feature = "diagram-sequence", feature = "diagram-flowchart"))]
    #[test]
    fn semantic_sequence_parse_matches_full_typed_model() {
        let source = "%%{init: {\"wrap\": false, \"sequence\": {\"wrap\": true}}}%%\nsequenceDiagram\naccTitle: Sequence title\nparticipant Alice\nparticipant Bob\nAlice->>Bob: wrap: Ready?\nBob-->>Alice: Yes";
        let semantic = SemanticParser::new()
            .parse_diagram_sync(source)
            .expect("automatic semantic parse");
        assert_eq!(semantic.metadata.diagram_type, "sequence");
        let SemanticDiagramModel::Sequence(semantic_model) = semantic.model else {
            panic!("expected Sequence model");
        };
        let known_type = SemanticParser::new()
            .parse_diagram_with_type_sync("sequence", source)
            .expect("known-type semantic parse");
        let SemanticDiagramModel::Sequence(known_type_model) = known_type.model else {
            panic!("expected known-type Sequence model");
        };
        assert_eq!(
            serde_json::to_value(&semantic_model).unwrap(),
            serde_json::to_value(&known_type_model).unwrap()
        );

        #[cfg(feature = "presentation")]
        {
            let full = crate::Engine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "sequence",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("sequence result");
            let crate::diagram::RenderSemanticModel::Sequence(full_model) = full.model() else {
                panic!("expected full Sequence model");
            };
            assert_eq!(
                serde_json::to_value(&semantic_model).unwrap(),
                serde_json::to_value(full_model).unwrap()
            );
        }
        assert!(!semantic_model.actors["Alice"].wrap);
        assert!(semantic_model.messages[0].wrap);
    }

    #[cfg(all(
        feature = "diagram-sequence",
        not(feature = "diagram-flowchart"),
        not(feature = "diagram-swimlane")
    ))]
    #[test]
    fn semantic_sequence_is_available_without_other_diagram_features() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("sequenceDiagram\nAlice->>Bob: Hi")
            .expect("Sequence-only semantic parse");
        assert!(matches!(parsed.model, SemanticDiagramModel::Sequence(_)));
        assert!(matches!(
            SemanticParser::new()
                .parse_diagram_with_type_sync("flowchart-v2", "flowchart TD\nA-->B"),
            Err(Error::UnsupportedDiagram { .. })
        ));
    }

    #[cfg(feature = "diagram-sequence")]
    #[test]
    fn semantic_sequence_parse_skips_visual_theme_evaluation() {
        let source = "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\nsequenceDiagram\nAlice->>Bob: Hi";
        assert!(SemanticParser::new().parse_diagram_sync(source).is_ok());
        #[cfg(feature = "presentation")]
        assert!(matches!(
            crate::Engine::new().parse_diagram_for_render_model_with_type_sync(
                "sequence",
                source,
                crate::ParseOptions::strict(),
            ),
            Err(crate::Error::ThemeColor(_))
        ));
    }

    #[cfg(feature = "diagram-sequence")]
    #[test]
    fn semantic_sequence_uses_nested_wrap_when_global_wrap_is_not_boolean() {
        let source = "%%{init: {\"wrap\": null, \"sequence\": {\"wrap\": true}}}%%\nsequenceDiagram\nAlice->>Bob: Hi";
        let parsed = SemanticParser::new()
            .parse_diagram_sync(source)
            .expect("semantic parse");
        match parsed.model {
            SemanticDiagramModel::Sequence(model) => assert!(model.actors["Alice"].wrap),
            #[allow(unreachable_patterns)]
            _ => panic!("expected Sequence model"),
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_flowchart_parse_does_not_resolve_invalid_visual_theme_values() {
        let input = "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\nflowchart TD\nA-->B";
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("flowchart-v2", input)
            .expect("semantic parsing does not evaluate theme colors");
        assert!(matches!(parsed.model, SemanticDiagramModel::Flowchart(_)));
        assert_eq!(
            parsed.metadata.config.as_value()["themeVariables"]["primaryColor"],
            json!("not-a-color")
        );
        assert_eq!(
            parsed.metadata.semantic_config.as_value()["themeVariables"]["primaryColor"],
            json!("not-a-color")
        );
        let host_config = MermaidConfig::from_value(json!({
            "theme": "default",
            "themeVariables": {"primaryColor": "not-a-color"}
        }));
        assert!(
            SemanticParser::new()
                .with_site_config(host_config)
                .parse_diagram_with_type_sync("flowchart-v2", "flowchart TD\nA-->B")
                .is_ok()
        );
        #[cfg(feature = "presentation")]
        assert!(matches!(
            crate::Engine::new().parse_diagram_for_render_model_with_type_sync(
                "flowchart-v2",
                input,
                crate::ParseOptions::strict(),
            ),
            Err(crate::Error::ThemeColor(_))
        ));
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_metadata_separates_upstream_source_projection_from_hardened_config() {
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync(
                "flowchart-v2",
                "%%{init: {themeCSS: '.node { color: red; }'}}%%\nflowchart TD\nA-->B",
            )
            .expect("semantic parse");
        assert_eq!(
            parsed.metadata.config.get_str("themeCSS"),
            Some(".node { color: red; }")
        );
        assert_eq!(
            parsed.metadata.semantic_config.get_str("themeCSS"),
            None,
            "Merman hardening rejects themeCSS from source semantic config"
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_flowchart_uses_secure_site_defaults_and_honors_host_policy() {
        let source =
            "%%{init: {securityLevel: 'loose', maxEdges: 1}}%%\nflowchart TD\nA-->B\nB-->C";
        let hardened = SemanticParser::new()
            .parse_diagram_with_type_sync("flowchart-v2", source)
            .expect("source cannot lower hardened security or edge limits");
        assert_eq!(
            hardened.metadata.semantic_config.get_str("securityLevel"),
            Some("strict")
        );
        assert_ne!(
            hardened.metadata.semantic_config.as_value()["maxEdges"],
            json!(1)
        );

        let host_policy =
            SemanticParser::new().with_site_config(MermaidConfig::from_value(json!({
                "secure": [],
                "maxEdges": 1
            })));
        let error = host_policy
            .parse_diagram_with_type_sync("flowchart-v2", source)
            .expect_err("explicit host policy allows the source maxEdges override");
        assert!(error.to_string().contains("maxEdges"));
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_model_matches_full_parser_flowchart_model() {
        let source = "flowchart LR\nA[Alpha] -->|next| B{Ready?}\nB -->|yes| C[Done]";
        let semantic = SemanticParser::new()
            .parse_diagram_with_type_sync("flowchart-v2", source)
            .expect("semantic parse");
        let semantic_model = match semantic.model {
            SemanticDiagramModel::Flowchart(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Flowchart model"),
        };
        assert_eq!(semantic_model.nodes.len(), 3);
        #[cfg(feature = "presentation")]
        {
            let full = crate::Engine::new()
                .parse_diagram_for_render_model_with_type_sync(
                    "flowchart-v2",
                    source,
                    crate::ParseOptions::strict(),
                )
                .expect("full parser")
                .expect("flowchart result");
            let crate::diagram::RenderSemanticModel::Flowchart(full_model) = full.model() else {
                panic!("expected full Flowchart model");
            };
            assert_eq!(
                to_value(semantic_model).unwrap(),
                to_value(full_model).unwrap()
            );
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_flowchart_applies_source_inherit_direction() {
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync(
                "flowchart-v2",
                "%%{init: {flowchart: {inheritDir: true}}}%%\nflowchart LR\nsubgraph Outer\nsubgraph Inner\nA\nend\nend",
            )
            .expect("semantic parse");
        let model = match parsed.model {
            SemanticDiagramModel::Flowchart(model) => model,
            #[allow(unreachable_patterns)]
            _ => panic!("expected semantic Flowchart model"),
        };
        let inner = model
            .subgraphs
            .iter()
            .find(|subgraph| subgraph.id == "Inner")
            .expect("nested subgraph");
        assert_eq!(inner.dir.as_deref(), Some("LR"));
        assert!(!inner.has_explicit_dir);
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_flowchart_elk_retains_detector_layout_selection() {
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("flowchart-elk", "flowchart-elk TD\nA-->B")
            .expect("semantic parse");
        assert_eq!(
            parsed.metadata.semantic_config.get_str("layout"),
            Some("elk")
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_parser_detects_flowchart_without_constructing_engine() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("flowchart LR\nA[Start]-->B[Finish]")
            .expect("automatic semantic parse");
        assert_eq!(parsed.metadata.diagram_type, "flowchart-v2");
        assert!(matches!(parsed.model, SemanticDiagramModel::Flowchart(_)));
    }

    #[cfg(feature = "diagram-swimlane")]
    #[test]
    fn semantic_parser_detects_canonical_swimlane_type() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("swimlane-beta LR\nA-->B")
            .expect("automatic semantic parse");
        assert_eq!(parsed.metadata.diagram_type, "swimlane");
        assert!(matches!(parsed.model, SemanticDiagramModel::Flowchart(_)));
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn semantic_auto_detection_retains_flowchart_elk_layout_mutation() {
        let parsed = SemanticParser::new()
            .parse_diagram_sync("flowchart-elk TD\nA-->B")
            .expect("automatic semantic parse");
        assert_eq!(parsed.metadata.diagram_type, "flowchart-elk");
        assert_eq!(
            parsed.metadata.semantic_config.get_str("layout"),
            Some("elk")
        );
    }

    #[cfg(all(feature = "diagram-swimlane", not(feature = "diagram-flowchart")))]
    #[test]
    fn swimlane_feature_enables_the_shared_semantic_flowchart_parser() {
        let parsed = SemanticParser::new()
            .parse_diagram_with_type_sync("swimlane", "swimlane-beta\nA-->B")
            .expect("shared Flowchart parser");
        assert!(matches!(parsed.model, SemanticDiagramModel::Flowchart(_)));
    }

    #[cfg(all(feature = "diagram-swimlane", not(feature = "diagram-flowchart")))]
    #[test]
    fn swimlane_only_feature_does_not_claim_flowchart_types() {
        assert!(matches!(
            SemanticParser::new()
                .parse_diagram_with_type_sync("flowchart-v2", "flowchart TD\nA-->B"),
            Err(Error::UnsupportedDiagram { .. })
        ));
    }

    #[cfg(all(feature = "diagram-flowchart", not(feature = "diagram-swimlane")))]
    #[test]
    fn flowchart_only_feature_does_not_claim_swimlane_type() {
        assert!(matches!(
            SemanticParser::new().parse_diagram_with_type_sync("swimlane", "swimlane-beta\nA-->B"),
            Err(Error::UnsupportedDiagram { .. })
        ));
    }
}
