//! Semantic-only parsing without resolving Mermaid themes or creating render state.

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
}

/// Metadata and canonical family model returned by [`SemanticParser`].
#[derive(Debug)]
pub struct ParsedSemanticDiagram {
    pub metadata: SemanticMetadata,
    pub model: SemanticDiagramModel,
}

/// Parses diagram meaning without requiring theme resolution, layout, or SVG rendering.
///
/// The semantic facade currently handles Flowchart, Swimlane, Sequence, State, Cynefin, Info,
/// Journey, Pie, and Timeline.
/// Other enabled families return [`Error::UnsupportedDiagram`] until their semantic parsers migrate here.
#[derive(Debug, Clone)]
pub struct SemanticParser {
    site_config: MermaidConfig,
    site_config_overrides: MermaidConfig,
}

impl Default for SemanticParser {
    fn default() -> Self {
        Self {
            site_config: crate::generated::default_site_config(),
            site_config_overrides: MermaidConfig::empty_object(),
        }
    }
}

impl SemanticParser {
    /// Creates a parser with Mermaid defaults and Merman's hardened source-config policy.
    pub fn new() -> Self {
        Self::default()
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
            .map(crate::compatibility_json::clone_value_nonrecursive);

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
        control.checkpoint()?;
        let registry = builtin_detector_registry();
        let pre = match preprocess(input, registry, None, control)? {
            Ok(pre) => pre,
            Err(error) => return Ok(Err(error)),
        };
        let semantic_config = self.effective_semantic_config(&pre.config, control)?;
        let mut detector_config = crate::PresentationConfig::from_config(self.site_config.clone());
        detector_config.deep_merge(semantic_config.as_value());
        let diagram_type = match registry.detect_type_precleaned_controlled(
            pre.code(),
            &mut detector_config,
            control,
        )? {
            Ok(diagram_type) => diagram_type,
            Err(error) => return Ok(Err(error)),
        };
        self.parse_preprocessed_controlled(diagram_type, pre, control)
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
        control.checkpoint()?;
        let pre = match preprocess(
            input,
            builtin_detector_registry(),
            Some(diagram_type),
            control,
        )? {
            Ok(pre) => pre,
            Err(error) => return Ok(Err(error)),
        };
        self.parse_preprocessed_controlled(diagram_type, pre, control)
    }

    fn parse_preprocessed_controlled(
        &self,
        diagram_type: &str,
        pre: crate::PreprocessResult,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedSemanticDiagram>> {
        control.checkpoint()?;
        let source = pre.source;
        let parser_input = source.text();
        if parser_input.trim_start().starts_with("---") {
            return Ok(Err(Error::MalformedFrontMatter));
        }

        let mut semantic_config = self.effective_semantic_config(&pre.config, control)?;
        if diagram_type == "flowchart-elk" {
            semantic_config.set_value("layout", serde_json::json!("elk"));
        }
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
        let _metadata = SemanticMetadata {
            diagram_type: diagram_type.to_string(),
            config,
            semantic_config,
            title,
        };

        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        {
            let supports_flowchart = cfg!(feature = "diagram-flowchart")
                && matches!(diagram_type, "flowchart" | "flowchart-v2" | "flowchart-elk");
            let supports_swimlane = cfg!(feature = "diagram-swimlane")
                && matches!(diagram_type, "swimlane" | "swimlane-beta");
            if supports_flowchart || supports_swimlane {
                let model = match crate::diagrams::flowchart::parse_semantic_model_controlled(
                    parser_input,
                    &_metadata,
                    control,
                )? {
                    Ok(model) => model,
                    Err(error) => return Ok(Err(error)),
                };
                control.checkpoint()?;
                return Ok(Ok(ParsedSemanticDiagram {
                    metadata: _metadata,
                    model: SemanticDiagramModel::Flowchart(model),
                }));
            }
        }

        #[cfg(feature = "diagram-sequence")]
        if diagram_type == "sequence" {
            let model = match crate::diagrams::sequence::parse_sequence_semantic_model_controlled(
                parser_input,
                &_metadata,
                control,
            )? {
                Ok(model) => model,
                Err(error) => return Ok(Err(error)),
            };
            control.checkpoint()?;
            return Ok(Ok(ParsedSemanticDiagram {
                metadata: _metadata,
                model: SemanticDiagramModel::Sequence(model),
            }));
        }

        #[cfg(feature = "diagram-cynefin")]
        if diagram_type == "cynefin" {
            let model = match crate::diagrams::cynefin::parse_cynefin_semantic_model_controlled(
                parser_input,
                &_metadata,
                control,
            )? {
                Ok(model) => model,
                Err(error) => return Ok(Err(error)),
            };
            control.checkpoint()?;
            return Ok(Ok(ParsedSemanticDiagram {
                metadata: _metadata,
                model: SemanticDiagramModel::Cynefin(model),
            }));
        }

        #[cfg(feature = "diagram-info")]
        if diagram_type == "info" {
            let model = match crate::diagrams::info::parse_info_semantic_model_controlled(
                parser_input,
                diagram_type,
                control,
            )? {
                Ok(model) => model,
                Err(error) => return Ok(Err(error)),
            };
            control.checkpoint()?;
            return Ok(Ok(ParsedSemanticDiagram {
                metadata: _metadata,
                model: SemanticDiagramModel::Info(model),
            }));
        }

        #[cfg(feature = "diagram-pie")]
        if diagram_type == "pie" {
            let model = match crate::diagrams::pie::parse_pie_semantic_model_controlled(
                parser_input,
                diagram_type,
                &_metadata.semantic_config,
                control,
            )? {
                Ok(model) => model,
                Err(error) => return Ok(Err(error)),
            };
            control.checkpoint()?;
            return Ok(Ok(ParsedSemanticDiagram {
                metadata: _metadata,
                model: SemanticDiagramModel::Pie(model),
            }));
        }

        #[cfg(feature = "diagram-journey")]
        if diagram_type == "journey" {
            let model = match crate::diagrams::journey::parse_journey_semantic_model_controlled(
                parser_input,
                diagram_type,
                &_metadata.semantic_config,
                control,
            )? {
                Ok(model) => model,
                Err(error) => return Ok(Err(error)),
            };
            control.checkpoint()?;
            return Ok(Ok(ParsedSemanticDiagram {
                metadata: _metadata,
                model: SemanticDiagramModel::Journey(model),
            }));
        }

        #[cfg(feature = "diagram-state")]
        if matches!(diagram_type, "stateDiagram" | "stateDiagram-v2") {
            let model = match crate::diagrams::state::parse_state_semantic_model_controlled(
                parser_input,
                diagram_type,
                &_metadata.semantic_config,
                control,
            )? {
                Ok(model) => model,
                Err(error) => return Ok(Err(error)),
            };
            control.checkpoint()?;
            return Ok(Ok(ParsedSemanticDiagram {
                metadata: _metadata,
                model: SemanticDiagramModel::State(model),
            }));
        }

        #[cfg(feature = "diagram-timeline")]
        if diagram_type == "timeline" {
            let model = match crate::diagrams::timeline::parse_timeline_semantic_model_controlled(
                parser_input,
                diagram_type,
                &_metadata.semantic_config,
                control,
            )? {
                Ok(model) => model,
                Err(error) => return Ok(Err(error)),
            };
            control.checkpoint()?;
            return Ok(Ok(ParsedSemanticDiagram {
                metadata: _metadata,
                model: SemanticDiagramModel::Timeline(model),
            }));
        }

        Ok(Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        }))
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

fn builtin_detector_registry() -> &'static crate::DetectorRegistry {
    static REGISTRY: std::sync::OnceLock<crate::DetectorRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(crate::DetectorRegistry::pinned_mermaid_baseline)
}

fn preprocess(
    input: &str,
    registry: &crate::DetectorRegistry,
    diagram_type: Option<&str>,
    control: &OperationControl,
) -> OperationControlResult<Result<crate::PreprocessResult>> {
    crate::preprocess::preprocess_mermaid_public_parse_pipeline_with_directive_recovery_controlled(
        input,
        registry,
        diagram_type,
        crate::preprocess::DirectiveRecoveryMode::Strict,
        control,
    )
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
        config.set_value(
            "theme",
            crate::compatibility_json::clone_value_nonrecursive(theme),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                "stateDiagram-v2",
                "stateDiagram-v2\nstate Outer {\n[*] --> A: ready\n}\nOuter --> Done\n",
            )
            .expect("State semantics are available without presentation");
        assert_eq!(parsed.metadata.diagram_type, "stateDiagram-v2");
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[cfg(any(
        feature = "diagram-cynefin",
        feature = "diagram-flowchart",
        feature = "diagram-info",
        feature = "diagram-pie"
    ))]
    use serde_json::json;
    #[cfg(feature = "diagram-flowchart")]
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
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-sequence",
                feature = "diagram-cynefin",
                feature = "diagram-journey",
                feature = "diagram-pie",
                feature = "diagram-state",
                feature = "diagram-timeline"
            ))]
            _ => panic!("expected semantic Info model"),
        };
        assert!(semantic_model.show_info);
        assert_eq!(
            serde_json::to_value(&semantic_model).unwrap(),
            json!({"showInfo": true})
        );
        assert_eq!(crate::editor::test_work::read(), Default::default());

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
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-sequence",
                feature = "diagram-cynefin",
                feature = "diagram-info",
                feature = "diagram-journey",
                feature = "diagram-state",
                feature = "diagram-timeline"
            ))]
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
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-sequence",
                feature = "diagram-cynefin",
                feature = "diagram-info",
                feature = "diagram-pie",
                feature = "diagram-state",
                feature = "diagram-timeline"
            ))]
            _ => panic!("expected semantic Journey model"),
        };
        assert_eq!(crate::editor::test_work::read(), Default::default());

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
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-cynefin",
                feature = "diagram-info",
                feature = "diagram-state",
                feature = "diagram-timeline",
                feature = "diagram-journey",
                feature = "diagram-pie"
            ))]
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
        let full = crate::Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-v2",
                source,
                crate::ParseOptions::strict(),
            )
            .expect("full parser")
            .expect("flowchart result");
        let semantic_model = match semantic.model {
            SemanticDiagramModel::Flowchart(model) => model,
            #[cfg(feature = "diagram-sequence")]
            SemanticDiagramModel::Sequence(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-cynefin")]
            SemanticDiagramModel::Cynefin(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-info")]
            SemanticDiagramModel::Info(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-pie")]
            SemanticDiagramModel::Pie(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-journey")]
            SemanticDiagramModel::Journey(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-state")]
            SemanticDiagramModel::State(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-timeline")]
            SemanticDiagramModel::Timeline(_) => panic!("expected semantic Flowchart model"),
        };
        let crate::diagram::RenderSemanticModel::Flowchart(full_model) = full.model() else {
            panic!("expected full Flowchart model");
        };
        assert_eq!(
            to_value(semantic_model).unwrap(),
            to_value(full_model).unwrap()
        );
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
            #[cfg(feature = "diagram-sequence")]
            SemanticDiagramModel::Sequence(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-cynefin")]
            SemanticDiagramModel::Cynefin(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-info")]
            SemanticDiagramModel::Info(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-pie")]
            SemanticDiagramModel::Pie(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-journey")]
            SemanticDiagramModel::Journey(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-state")]
            SemanticDiagramModel::State(_) => panic!("expected semantic Flowchart model"),
            #[cfg(feature = "diagram-timeline")]
            SemanticDiagramModel::Timeline(_) => panic!("expected semantic Flowchart model"),
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
