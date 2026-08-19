//! Operation-local Flowchart edge source-style plan.

use super::*;
use std::sync::Arc;

#[derive(Debug)]
struct FlowchartEdgeStyleArtifact {
    emission: FlowchartCompiledStyles,
    projected_source_style_bytes: usize,
    source_stroke_width_status: crate::flowchart::FlowchartSourceFacetStatus,
    source_stroke_width_value: Option<f32>,
}

#[derive(Debug, Clone)]
struct PreparedEdgeStyles {
    artifact: Arc<FlowchartEdgeStyleArtifact>,
    animation: FlowchartEdgeAnimationResolution,
    swimlane_label: Option<Arc<FlowchartCompiledStyles>>,
}

#[derive(Debug)]
pub(crate) struct FlowchartEdgeStylePlan {
    #[cfg(test)]
    edges: FxHashMap<String, PreparedEdgeStyles>,
    edge_occurrences: Vec<PreparedEdgeStyles>,
    edge_stroke_width_config_override: bool,
    #[cfg(test)]
    parsed_class_declaration_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity::flowchart) struct FlowchartEdgeStrokeWidthResolution {
    precedence: crate::flowchart::FlowchartFacetPrecedence,
    typed_value: Option<f32>,
    paint_value: Option<f32>,
}

impl FlowchartEdgeStrokeWidthResolution {
    pub(in crate::svg::parity::flowchart) const fn precedence(
        self,
    ) -> crate::flowchart::FlowchartFacetPrecedence {
        self.precedence
    }

    pub(in crate::svg::parity::flowchart) const fn typed_value(self) -> Option<f32> {
        self.typed_value
    }

    pub(in crate::svg::parity::flowchart) fn paint_outset(self) -> Option<f64> {
        self.paint_value.map(|value| f64::from(value) / 2.0)
    }
}

impl FlowchartEdgeStylePlan {
    pub(crate) fn prepare_for_model(
        model: &crate::flowchart::FlowchartModel,
        effective_config: &merman_core::MermaidConfig,
        swimlane: bool,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let default_edge_style = model
            .edge_defaults
            .as_ref()
            .map_or(&[][..], |defaults| defaults.style.as_slice());
        let mut plan = Self::prepare(
            &model.class_defs,
            &model.edges,
            default_edge_style,
            flowchart_config_diagram_look(effective_config).is_hand_drawn(),
            swimlane,
            work_meter,
        )?;
        plan.edge_stroke_width_config_override =
            merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.strokeWidth",
            );
        Ok(plan)
    }

    fn prepare(
        class_defs: &IndexMap<String, Vec<String>>,
        edges: &[crate::flowchart::FlowEdge],
        default_edge_style: &[String],
        hand_drawn: bool,
        swimlane: bool,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let class_styles = FlowchartPreparedClassStyles::prepare(
            class_defs,
            edges
                .iter()
                .flat_map(|edge| edge.classes.iter().map(String::as_str)),
            Some(work_meter),
        )?;
        #[cfg(test)]
        let parsed_class_declaration_count = class_styles.parsed_declaration_count();
        #[cfg(test)]
        let mut prepared_edges =
            FxHashMap::with_capacity_and_hasher(edges.len(), Default::default());
        let mut prepared_edge_occurrences = Vec::with_capacity(edges.len());
        let mut shared_edge_artifacts: FxHashMap<
            (&[String], &[String]),
            Arc<FlowchartEdgeStyleArtifact>,
        > = FxHashMap::default();

        let empty_class_styles = FlowchartPreparedClassStyles::default();
        let shared_swimlane_label = swimlane
            .then(|| default_edge_style.first())
            .flatten()
            .map(|style| {
                flowchart_compile_prepared_styles(
                    &empty_class_styles,
                    &[],
                    std::slice::from_ref(style),
                    &[],
                    Some(work_meter),
                )
            })
            .transpose()?
            .map(FlowchartCompiledStyles::into_swimlane_edge_label_artifact)
            .map(Arc::new);
        let mut shared_swimlane_labels: FxHashMap<&str, Arc<FlowchartCompiledStyles>> =
            FxHashMap::default();

        for edge in edges {
            let cache_key = (edge.classes.as_slice(), edge.style.as_slice());
            let artifact = if let Some(artifact) = shared_edge_artifacts.get(&cache_key) {
                Arc::clone(artifact)
            } else {
                let emission = flowchart_compile_prepared_styles(
                    &class_styles,
                    &edge.classes,
                    default_edge_style,
                    &edge.style,
                    Some(work_meter),
                )?;
                let Some(projected_source_style_bytes) = emission
                    .projected_edge_source_style_bytes(default_edge_style, &edge.style, hand_drawn)
                else {
                    work_meter.check_svg_append(usize::MAX, 1)?;
                    unreachable!("overflowing edge source-style projection must be rejected")
                };
                let source_stroke_width_status = if hand_drawn {
                    crate::flowchart::FlowchartSourceFacetStatus::Absent
                } else {
                    emission.source_stroke_width_status()
                };
                let source_stroke_width_value = (!hand_drawn)
                    .then(|| emission.admitted_stroke_width_value())
                    .flatten();
                let artifact = Arc::new(FlowchartEdgeStyleArtifact {
                    source_stroke_width_status,
                    source_stroke_width_value,
                    emission: emission.into_edge_artifact(hand_drawn),
                    projected_source_style_bytes,
                });
                shared_edge_artifacts.insert(cache_key, Arc::clone(&artifact));
                artifact
            };
            let swimlane_label = if !swimlane {
                None
            } else if let Some(shared) = &shared_swimlane_label {
                Some(Arc::clone(shared))
            } else if let Some(style) = edge.style.first() {
                if let Some(shared) = shared_swimlane_labels.get(style.as_str()) {
                    Some(Arc::clone(shared))
                } else {
                    let compiled = flowchart_compile_prepared_styles(
                        &empty_class_styles,
                        &[],
                        std::slice::from_ref(style),
                        &[],
                        Some(work_meter),
                    )?
                    .into_swimlane_edge_label_artifact();
                    let compiled = Arc::new(compiled);
                    shared_swimlane_labels.insert(style.as_str(), Arc::clone(&compiled));
                    Some(compiled)
                }
            } else {
                None
            };
            let prepared = PreparedEdgeStyles {
                animation: FlowchartEdgeAnimationResolution::resolve(edge, &artifact.emission),
                artifact,
                swimlane_label,
            };
            #[cfg(test)]
            prepared_edges.insert(edge.id.clone(), prepared.clone());
            prepared_edge_occurrences.push(prepared);
        }

        Ok(Self {
            #[cfg(test)]
            edges: prepared_edges,
            edge_occurrences: prepared_edge_occurrences,
            edge_stroke_width_config_override: false,
            #[cfg(test)]
            parsed_class_declaration_count,
        })
    }

    #[cfg(test)]
    pub(in crate::svg::parity::flowchart) fn edge(
        &self,
        edge_id: &str,
    ) -> crate::Result<&FlowchartCompiledStyles> {
        self.edges
            .get(edge_id)
            .map(|styles| &styles.artifact.emission)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Flowchart edge style for `{edge_id}`"),
            })
    }

    fn occurrence(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> crate::Result<&PreparedEdgeStyles> {
        self.edge_occurrences
            .get(key.semantic_index())
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!(
                    "missing prepared Flowchart edge occurrence {}",
                    key.semantic_index()
                ),
            })
    }

    pub(in crate::svg::parity::flowchart) fn edge_for(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> crate::Result<&FlowchartCompiledStyles> {
        Ok(&self.occurrence(key)?.artifact.emission)
    }

    #[cfg(test)]
    pub(in crate::svg::parity::flowchart) fn edge_source_stroke_width_status_for(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> crate::Result<crate::flowchart::FlowchartSourceFacetStatus> {
        Ok(self.occurrence(key)?.artifact.source_stroke_width_status)
    }

    pub(in crate::svg::parity::flowchart) fn resolve_edge_stroke_width_for(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
        edge: &crate::flowchart::FlowEdge,
        edge_theme: &crate::flowchart::FlowchartEdgeThemeStyle,
        mermaid_stroke_width: f32,
        hand_drawn: bool,
    ) -> crate::Result<FlowchartEdgeStrokeWidthResolution> {
        let artifact = &self.occurrence(key)?.artifact;
        let precedence = crate::flowchart::FlowchartFacetPrecedence::new(
            artifact.source_stroke_width_status,
            self.edge_stroke_width_config_override,
        );
        let typed_value = edge_theme.stroke_width_value(precedence, !hand_drawn);
        let class_value = if hand_drawn {
            1.0
        } else {
            match edge.stroke.as_deref() {
                Some("thick") => 3.5,
                Some("invisible") => 0.0,
                _ => mermaid_stroke_width,
            }
        };
        let paint_value = match artifact.source_stroke_width_status {
            crate::flowchart::FlowchartSourceFacetStatus::Admitted => {
                artifact.source_stroke_width_value
            }
            // Dynamic or invalid source CSS still owns precedence, but it has no numeric paint
            // extent that the headless viewport can safely claim.
            crate::flowchart::FlowchartSourceFacetStatus::Unverified => None,
            crate::flowchart::FlowchartSourceFacetStatus::Absent => {
                typed_value.or(Some(class_value))
            }
        };
        Ok(FlowchartEdgeStrokeWidthResolution {
            precedence,
            typed_value,
            paint_value,
        })
    }

    #[cfg(test)]
    pub(in crate::svg::parity::flowchart) fn swimlane_label(
        &self,
        edge_id: &str,
    ) -> crate::Result<Option<&FlowchartCompiledStyles>> {
        self.edges
            .get(edge_id)
            .map(|styles| styles.swimlane_label.as_deref())
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Swimlane edge label style for `{edge_id}`"),
            })
    }

    #[cfg(test)]
    pub(in crate::svg::parity::flowchart) fn animation(
        &self,
        edge_id: &str,
    ) -> crate::Result<FlowchartEdgeAnimationResolution> {
        self.edges
            .get(edge_id)
            .map(|styles| styles.animation)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Flowchart edge animation for `{edge_id}`"),
            })
    }

    pub(in crate::svg::parity::flowchart) fn animation_for(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> crate::Result<FlowchartEdgeAnimationResolution> {
        Ok(self.occurrence(key)?.animation)
    }

    #[cfg(test)]
    pub(crate) fn edge_label_text_style<'a>(
        &self,
        edge_id: &str,
        base: &'a crate::text::TextStyle,
    ) -> crate::Result<crate::flowchart::FlowchartTextStyleResolution<'a>> {
        Ok(self
            .edge(edge_id)?
            .effective_edge_label_text_style_with_provenance(base))
    }

    pub(crate) fn edge_label_text_style_for<'a>(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
        base: &'a crate::text::TextStyle,
    ) -> crate::Result<crate::flowchart::FlowchartTextStyleResolution<'a>> {
        Ok(self
            .edge_for(key)?
            .effective_edge_label_text_style_with_provenance(base))
    }

    #[cfg(test)]
    pub(crate) fn swimlane_edge_label_text_style<'a>(
        &self,
        edge_id: &str,
        base: &'a crate::text::TextStyle,
    ) -> crate::Result<crate::flowchart::FlowchartTextStyleResolution<'a>> {
        let Some(styles) = self.swimlane_label(edge_id)? else {
            return Ok(crate::flowchart::FlowchartTextStyleResolution::borrowed(
                base,
            ));
        };
        Ok(styles.effective_edge_label_text_style_with_provenance(base))
    }

    pub(crate) fn swimlane_edge_label_text_style_for<'a>(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
        base: &'a crate::text::TextStyle,
    ) -> crate::Result<crate::flowchart::FlowchartTextStyleResolution<'a>> {
        let Some(styles) = self.occurrence(key)?.swimlane_label.as_deref() else {
            return Ok(crate::flowchart::FlowchartTextStyleResolution::borrowed(
                base,
            ));
        };
        Ok(styles.effective_edge_label_text_style_with_provenance(base))
    }

    #[cfg(test)]
    pub(in crate::svg::parity::flowchart) fn edge_source_style_svg_bytes(
        &self,
        edge_id: &str,
    ) -> crate::Result<usize> {
        self.edges
            .get(edge_id)
            .map(|styles| styles.artifact.projected_source_style_bytes)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Flowchart edge style for `{edge_id}`"),
            })
    }

    pub(in crate::svg::parity::flowchart) fn edge_source_style_svg_bytes_for(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> crate::Result<usize> {
        Ok(self.occurrence(key)?.artifact.projected_source_style_bytes)
    }

    pub(in crate::svg::parity::flowchart) fn swimlane_label_for(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> crate::Result<Option<&FlowchartCompiledStyles>> {
        Ok(self.occurrence(key)?.swimlane_label.as_deref())
    }

    #[cfg(test)]
    pub(super) const fn parsed_class_declaration_count(&self) -> usize {
        self.parsed_class_declaration_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn edge(id: &str, classes: &[&str], style: &[&str]) -> crate::flowchart::FlowEdge {
        crate::flowchart::FlowEdge {
            id: id.to_string(),
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            label_type: None,
            edge_type: Some("arrow_point".to_string()),
            arrow: "normal".to_string(),
            is_user_defined_id: true,
            stroke: None,
            interpolate: None,
            classes: classes.iter().map(|value| (*value).to_string()).collect(),
            style: style.iter().map(|value| (*value).to_string()).collect(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    #[test]
    fn shared_class_declarations_are_parsed_once_for_all_edges() {
        let class_defs = IndexMap::from([(
            "shared".to_string(),
            vec!["stroke:#ef4444,stroke-width:3px,color:#fff".to_string()],
        )]);
        let edges = vec![edge("e1", &["shared"], &[]), edge("e2", &["shared"], &[])];
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        let plan = FlowchartEdgeStylePlan::prepare(&class_defs, &edges, &[], false, false, &meter)
            .expect("prepare edge styles");

        assert_eq!(plan.parsed_class_declaration_count(), 3);
        assert_eq!(
            plan.edge("e1")
                .expect("prepared edge")
                .edge_marker_color(false),
            Some("#ef4444")
        );
        assert_eq!(
            plan.edge("e2")
                .expect("prepared edge")
                .edge_marker_color(false),
            Some("#ef4444")
        );
        assert!(std::sync::Arc::ptr_eq(
            &plan.edges.get("e1").expect("first edge").artifact,
            &plan.edges.get("e2").expect("second edge").artifact,
        ));
        let base = crate::text::TextStyle::default();
        let resolution = plan
            .edge_label_text_style_for(crate::flowchart::FlowchartEdgeKey::new(0), &base)
            .expect("compiled edge label style");
        let foreground = resolution
            .terminal_foreground()
            .expect("compiled class color winner");
        assert_eq!(foreground.value(), "#fff");
        assert_eq!(
            foreground.provenance(),
            crate::flowchart::FlowchartTerminalForegroundProvenance::AssignedClass
        );
    }

    #[test]
    fn duplicate_raw_edge_ids_keep_occurrence_scoped_styles() {
        let edges = vec![
            edge("duplicate", &[], &["stroke:#ef4444"]),
            edge("duplicate", &[], &["stroke:#2563eb"]),
        ];
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan =
            FlowchartEdgeStylePlan::prepare(&IndexMap::new(), &edges, &[], false, false, &meter)
                .expect("prepare duplicate-id edge styles");

        assert_eq!(
            plan.edge_for(crate::flowchart::FlowchartEdgeKey::new(0))
                .expect("first occurrence")
                .edge_marker_color(false),
            Some("#ef4444")
        );
        assert_eq!(
            plan.edge_for(crate::flowchart::FlowchartEdgeKey::new(1))
                .expect("second occurrence")
                .edge_marker_color(false),
            Some("#2563eb")
        );
        assert_eq!(
            plan.edge("duplicate")
                .expect("legacy raw-id projection")
                .edge_marker_color(false),
            Some("#2563eb")
        );
    }

    #[test]
    fn marker_color_uses_the_same_important_winner_as_edge_evidence() {
        let class_defs = IndexMap::from([(
            "important".to_string(),
            vec!["stroke:#ef4444 !important".to_string()],
        )]);
        let edges = vec![edge("e1", &["important"], &["stroke:#2563eb"])];
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = FlowchartEdgeStylePlan::prepare(&class_defs, &edges, &[], false, false, &meter)
            .expect("prepare edge styles");

        assert_eq!(
            plan.edge("e1")
                .expect("prepared edge")
                .edge_marker_color(false),
            Some("#ef4444")
        );
        assert_eq!(
            plan.edge("e1")
                .expect("prepared edge")
                .emitted_edge_source_stroke_status(false),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );
    }

    #[test]
    fn edge_plan_preserves_classic_stroke_width_precedence_and_fails_closed_for_hand_drawn() {
        let class_defs =
            IndexMap::from([("wide".to_string(), vec!["stroke-width:8px".to_string()])]);
        let edges = vec![edge("e1", &["wide"], &[])];
        let classic_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let classic =
            FlowchartEdgeStylePlan::prepare(&class_defs, &edges, &[], false, false, &classic_meter)
                .expect("prepare classic edge styles");

        assert_eq!(
            classic
                .edge_source_stroke_width_status_for(crate::flowchart::FlowchartEdgeKey::new(0))
                .expect("classic edge stroke-width status"),
            crate::flowchart::FlowchartSourceFacetStatus::Admitted
        );

        let hand_drawn_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let hand_drawn = FlowchartEdgeStylePlan::prepare(
            &class_defs,
            &edges,
            &[],
            true,
            false,
            &hand_drawn_meter,
        )
        .expect("prepare hand-drawn edge styles");

        assert_eq!(
            hand_drawn
                .edge_source_stroke_width_status_for(crate::flowchart::FlowchartEdgeKey::new(0))
                .expect("hand-drawn edge stroke-width status"),
            crate::flowchart::FlowchartSourceFacetStatus::Absent
        );
    }

    #[test]
    fn animation_resolution_uses_parsed_properties_instead_of_raw_substrings() {
        let class_defs = IndexMap::from([
            ("disabled".to_string(), vec!["animation:none".to_string()]),
            (
                "custom-token".to_string(),
                vec!["--animation-token:dash 2s linear".to_string()],
            ),
        ]);
        let mut explicit_speed = edge("explicit-speed", &[], &[]);
        explicit_speed.animate = Some(false);
        explicit_speed.animation = Some("fast".to_string());
        let mut generated_but_disabled = edge("generated-but-disabled", &[], &["animation:none"]);
        generated_but_disabled.animate = Some(true);
        let inline_animated = edge("inline-animated", &[], &["animation:dash 2s linear"]);
        let class_disabled = edge("class-disabled", &["disabled"], &[]);
        let custom_token = edge("custom-token", &["custom-token"], &[]);
        let shorthand_then_longhand = edge(
            "shorthand-then-longhand",
            &[],
            &["animation:dash 2s linear", "animation-name:none"],
        );
        let longhand_then_shorthand = edge(
            "longhand-then-shorthand",
            &[],
            &["animation-name:none", "animation:dash 2s linear"],
        );
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        let plan = FlowchartEdgeStylePlan::prepare(
            &class_defs,
            &[
                explicit_speed,
                generated_but_disabled,
                inline_animated,
                class_disabled,
                custom_token,
                shorthand_then_longhand,
                longhand_then_shorthand,
            ],
            &[],
            false,
            false,
            &meter,
        )
        .expect("prepare edge animations");

        let explicit_speed = plan.animation("explicit-speed").expect("explicit speed");
        assert_eq!(explicit_speed.class(), Some("edge-animation-fast"));
        assert!(explicit_speed.is_active());

        let generated_but_disabled = plan
            .animation("generated-but-disabled")
            .expect("source animation:none overrides generated class");
        assert_eq!(generated_but_disabled.class(), Some("edge-animation-fast"));
        assert!(!generated_but_disabled.is_active());

        let inline_animated = plan.animation("inline-animated").expect("inline animation");
        assert_eq!(inline_animated.class(), None);
        assert!(inline_animated.is_active());

        let class_disabled = plan
            .animation("class-disabled")
            .expect("disabled class animation");
        assert_eq!(class_disabled.class(), None);
        assert!(!class_disabled.is_active());

        let custom_token = plan
            .animation("custom-token")
            .expect("custom property is not animation");
        assert_eq!(custom_token.class(), None);
        assert!(!custom_token.is_active());

        assert!(
            !plan
                .animation("shorthand-then-longhand")
                .expect("later animation-name")
                .is_active()
        );
        assert!(
            plan.animation("longhand-then-shorthand")
                .expect("later animation shorthand")
                .is_active()
        );
    }

    #[test]
    fn animation_resolution_obeys_default_inline_order_and_important_priority() {
        let class_defs = IndexMap::from([(
            "important-disabled".to_string(),
            vec!["animation:none !important".to_string()],
        )]);
        let edges = [
            edge("default-active", &[], &[]),
            edge("inline-disabled", &[], &["animation:none"]),
            edge(
                "important-disabled",
                &["important-disabled"],
                &["animation:dash 2s linear"],
            ),
            edge(
                "important-reactivated",
                &["important-disabled"],
                &["animation-name:dash !important"],
            ),
        ];
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        let plan = FlowchartEdgeStylePlan::prepare(
            &class_defs,
            &edges,
            &["animation:default-edge 2s linear".to_string()],
            false,
            false,
            &meter,
        )
        .expect("prepare edge animation cascade");

        assert!(
            plan.animation("default-active")
                .expect("default animation")
                .is_active()
        );
        assert!(
            !plan
                .animation("inline-disabled")
                .expect("inline animation:none")
                .is_active()
        );
        assert!(
            !plan
                .animation("important-disabled")
                .expect("important class animation:none")
                .is_active()
        );
        assert!(
            plan.animation("important-reactivated")
                .expect("later important animation-name")
                .is_active()
        );
    }

    #[test]
    fn semantic_winner_does_not_hide_an_emitted_dynamic_declaration_residual() {
        let edges = vec![edge("e1", &[], &["stroke:var(--marker),stroke:#111827"])];
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan =
            FlowchartEdgeStylePlan::prepare(&IndexMap::new(), &edges, &[], false, false, &meter)
                .expect("prepare edge styles");

        assert_eq!(
            plan.edge("e1")
                .expect("prepared edge")
                .edge_marker_color(false),
            Some("#111827")
        );
        let residuals = plan
            .edge("e1")
            .expect("prepared edge")
            .emitted_edge_source_residuals("e1", false);
        assert!(residuals.iter().any(|residual| {
            residual.raw() == "stroke:var(--marker)"
                && residual.reason()
                    == crate::diagram_theme::SourceStyleResidualReason::InvalidValue
        }));
    }

    #[test]
    fn edge_style_work_budget_accepts_exact_and_rejects_short_one() {
        let class_defs = IndexMap::from([(
            "shared".to_string(),
            vec!["stroke:#ef4444,stroke-width:3px,color:#fff".to_string()],
        )]);
        let edges = vec![
            edge("e1", &["shared"], &["stroke:#2563eb"]),
            edge("e2", &["shared"], &["stroke:#16a34a"]),
        ];
        let unbounded = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        FlowchartEdgeStylePlan::prepare(&class_defs, &edges, &[], false, false, &unbounded)
            .expect("measure exact edge style work");
        let exact = unbounded.used();
        assert!(exact > 0);

        let exact_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, exact)
                .expect("exact work limit"),
        );
        FlowchartEdgeStylePlan::prepare(&class_defs, &edges, &[], false, false, &exact_meter)
            .expect("exact work budget must succeed");
        assert_eq!(exact_meter.used(), exact);

        let short_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxLayoutWorkUnits,
                    exact - 1,
                )
                .expect("short work limit"),
        );
        let error =
            FlowchartEdgeStylePlan::prepare(&class_defs, &edges, &[], false, false, &short_meter)
                .expect_err("short work budget must fail");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured resource rejection");
        };
        assert_eq!(error.limit, "max_layout_work_units");
        assert_eq!(error.actual, exact);
        assert_eq!(error.max, exact - 1);
        assert_eq!(short_meter.used(), exact - 1);
    }

    #[test]
    fn edge_and_swimlane_artifacts_share_identical_compiled_payloads() {
        let class_defs = IndexMap::from([(
            "shared".to_string(),
            vec!["stroke:#ef4444,stroke-width:3px".to_string()],
        )]);
        let edges = vec![
            edge("e1", &["shared"], &["opacity:0.5"]),
            edge("e2", &["shared"], &["opacity:0.5"]),
            edge("e3", &["shared"], &["opacity:0.75"]),
        ];
        let preparation_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = FlowchartEdgeStylePlan::prepare(
            &class_defs,
            &edges,
            &["color:#111827".to_string()],
            false,
            true,
            &preparation_meter,
        )
        .expect("prepare edge styles");

        let first = plan.edges.get("e1").expect("first edge");
        let second = plan.edges.get("e2").expect("second edge");
        let third = plan.edges.get("e3").expect("third edge");
        assert!(Arc::ptr_eq(&first.artifact, &second.artifact));
        assert!(!Arc::ptr_eq(&first.artifact, &third.artifact));
        assert!(Arc::ptr_eq(
            first.swimlane_label.as_ref().expect("first Swimlane label"),
            second
                .swimlane_label
                .as_ref()
                .expect("second Swimlane label"),
        ));
        assert!(Arc::ptr_eq(
            first.swimlane_label.as_ref().expect("first Swimlane label"),
            third.swimlane_label.as_ref().expect("third Swimlane label"),
        ));

        let projected = plan
            .edge_source_style_svg_bytes("e1")
            .expect("projected source style bytes");
        assert!(projected > 0);
        assert_eq!(
            projected,
            plan.edge_source_style_svg_bytes("e2")
                .expect("shared source style projection")
        );
    }

    #[test]
    fn repeated_invalid_class_residuals_share_the_raw_payload() {
        let class_defs = IndexMap::from([(
            "unsafe".to_string(),
            vec!["filter:url(#unsafe)".to_string()],
        )]);
        let edges = vec![edge("e1", &["unsafe"], &[]), edge("e2", &["unsafe"], &[])];
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = FlowchartEdgeStylePlan::prepare(&class_defs, &edges, &[], false, false, &meter)
            .expect("prepare edge styles");

        let first = plan
            .edge("e1")
            .expect("first prepared edge")
            .emitted_edge_source_residuals("e1", false);
        let second = plan
            .edge("e2")
            .expect("second prepared edge")
            .emitted_edge_source_residuals("e2", false);
        assert_eq!(first[0].raw(), "filter:url(#unsafe)");
        assert_eq!(second[0].raw(), "filter:url(#unsafe)");
        assert!(Arc::ptr_eq(&first[0].raw_arc(), &second[0].raw_arc()));
    }
}
