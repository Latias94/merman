use crate::config::presentation::{DetectionBaseline, SourcePresentationConfig};
use crate::config::{AppearanceDecision, MaterializationPlan};
use crate::{
    MermaidConfig, MermaidEngine, OperationControl, OperationControlResult, Result, family, theme,
};
use std::sync::Arc;

/// Owns the configuration through detection and classic Mermaid appearance selection.
pub(super) struct OperationConfigBuilder {
    effective: SourcePresentationConfig,
    source: MermaidConfig,
    before_detection: Option<Arc<DetectionBaseline>>,
}

impl OperationConfigBuilder {
    pub(super) fn new(
        engine: &MermaidEngine,
        overrides: &MermaidConfig,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self>> {
        let effective = match engine.default_effective_config() {
            Ok(config) => config.with_shared_sections(),
            Err(error) => return Ok(Err(error)),
        };
        let source = if overrides.is_empty_object() {
            overrides.clone()
        } else {
            crate::config::source_filtered_overrides(effective.as_config(), overrides, control)?
        };
        Ok(Ok(Self {
            effective,
            source,
            before_detection: None,
        }))
    }

    pub(super) fn effective_for_detection(&mut self) -> &mut SourcePresentationConfig {
        if self.before_detection.is_none() {
            self.effective.deep_merge(self.source.as_value());
            self.before_detection = Some(self.effective.begin_detection());
        }
        &mut self.effective
    }

    #[cfg(feature = "editor-facts")]
    pub(super) const fn filtered_source(&self) -> &MermaidConfig {
        &self.source
    }

    pub(super) fn finalize(
        self,
        engine: &MermaidEngine,
        diagram_type: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<SourcePresentationConfig>> {
        let Self {
            mut effective,
            source,
            before_detection,
        } = self;
        let mut source_merged = before_detection.is_some();
        control.checkpoint()?;
        let defaults = crate::generated::upstream_default_config();
        let decision = AppearanceDecision::resolve(
            diagram_type,
            &source,
            engine.site_config_overrides.as_config(),
            &defaults,
        );
        if !source_merged && decision.selected_theme().is_none() {
            effective.deep_merge(source.as_value());
            source_merged = true;
        }
        match decision.materialization_plan(
            effective.get_str("theme"),
            engine.initialization.site_config.get_str("theme"),
        ) {
            MaterializationPlan::InheritInitialized => {
                if !source_merged {
                    effective.deep_merge(source.as_value());
                }
                effective.finish_detection();
                decision.apply_to_presentation(&mut effective);
            }
            MaterializationPlan::Materialize { selected } => {
                // Detection must observe initialized config. Keep that result only when its
                // assignments actually need replay onto a newly materialized theme.
                let source_is_empty = source.is_empty_object();
                // Cache only site-owned appearance. A disjoint source merge can follow the
                // materialization with its explicit assignments and provenance still intact.
                let projected = if let Some(section) =
                    family::config_namespace_for_diagram_type(diagram_type)
                    && (source_is_empty
                        || decision.can_merge_after_site_appearance(
                            &source,
                            engine.initialization.site_config.as_config(),
                        )) {
                    engine
                        .initialization
                        .materialize_appearance(section, selected, || {
                            Self::materialize_operation_theme(
                                engine,
                                &decision,
                                &MermaidConfig::empty_object(),
                                selected,
                            )
                        })
                        .map(|mut config| {
                            if !source_is_empty {
                                config.deep_merge(source.as_value());
                            }
                            config
                        })
                } else {
                    Self::materialize_operation_theme(engine, &decision, &source, selected)
                };
                let mut projected = match projected {
                    Ok(config) => config,
                    Err(error) => return Ok(Err(error.into())),
                };
                if let Some(before) = &before_detection {
                    projected.replay_detection(before, &effective, control)?;
                }
                // Detector writes cannot replace authored appearance. Scoped-key presence is
                // checked again because replay can create or remove a scoped setting.
                decision.apply_to_presentation(&mut projected);
                effective = projected;
            }
        }
        // The explicit keyword is the only detector-level layout selection retained in Mermaid 12.
        if diagram_type == "flowchart-elk" {
            effective.set_value("layout", serde_json::Value::String("elk".to_owned()));
        }
        Ok(Ok(effective))
    }

    pub(super) fn materialize_operation_theme(
        engine: &MermaidEngine,
        decision: &AppearanceDecision<'_>,
        source_config: &MermaidConfig,
        selected: crate::MermaidThemeId,
    ) -> std::result::Result<SourcePresentationConfig, theme::ThemeResolutionError> {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::ThemePrepare,
        );
        let mut raw_config = engine
            .initialization
            .site_config
            .clone()
            .with_shared_sections();
        // Every actual theme recalculation consumes raw initialization and allowed source inputs.
        // The inherited path instead keeps ordinary updates to initialized derivations.
        let initial_variables = engine
            .site_config_overrides
            .get("themeVariables")
            .filter(|value| theme::is_js_truthy(value))
            .map(crate::json_value::clone_value_nonrecursive)
            .unwrap_or_else(|| serde_json::json!({}));
        raw_config.set_computed_value("themeVariables", initial_variables);
        raw_config.deep_merge(source_config.as_value());
        decision.apply_to_presentation(&mut raw_config);
        theme::materialize_selected_theme(raw_config, selected)
    }
}
