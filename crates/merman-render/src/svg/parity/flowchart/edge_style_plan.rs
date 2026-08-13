//! Operation-local Flowchart edge source-style plan.

use super::*;

#[derive(Debug)]
struct PreparedEdgeStyles {
    emission: FlowchartCompiledStyles,
    swimlane_label: Option<FlowchartCompiledStyles>,
    projected_source_style_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct FlowchartEdgeStylePlan {
    edges: FxHashMap<String, PreparedEdgeStyles>,
    #[cfg(test)]
    parsed_class_declaration_count: usize,
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
        Self::prepare(
            &model.class_defs,
            &model.edges,
            default_edge_style,
            flowchart_config_look(effective_config) == "handDrawn",
            swimlane,
            work_meter,
        )
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
        let mut prepared_edges =
            FxHashMap::with_capacity_and_hasher(edges.len(), Default::default());

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
            .transpose()?;

        for edge in edges {
            let emission = flowchart_compile_prepared_styles(
                &class_styles,
                &edge.classes,
                default_edge_style,
                &edge.style,
                Some(work_meter),
            )?;
            let projected_source_style_bytes = emission.projected_edge_source_style_bytes(
                default_edge_style,
                &edge.style,
                hand_drawn,
            );
            let swimlane_label = if !swimlane {
                None
            } else if let Some(shared) = &shared_swimlane_label {
                Some(shared.clone())
            } else {
                edge.style
                    .first()
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
            };
            prepared_edges.insert(
                edge.id.clone(),
                PreparedEdgeStyles {
                    emission,
                    swimlane_label,
                    projected_source_style_bytes,
                },
            );
        }

        Ok(Self {
            edges: prepared_edges,
            #[cfg(test)]
            parsed_class_declaration_count,
        })
    }

    pub(in crate::svg::parity::flowchart) fn edge(
        &self,
        edge_id: &str,
    ) -> crate::Result<&FlowchartCompiledStyles> {
        self.edges
            .get(edge_id)
            .map(|styles| &styles.emission)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Flowchart edge style for `{edge_id}`"),
            })
    }

    pub(in crate::svg::parity::flowchart) fn swimlane_label(
        &self,
        edge_id: &str,
    ) -> crate::Result<Option<&FlowchartCompiledStyles>> {
        self.edges
            .get(edge_id)
            .map(|styles| styles.swimlane_label.as_ref())
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Swimlane edge label style for `{edge_id}`"),
            })
    }

    pub(crate) fn edge_label_text_style<'a>(
        &self,
        edge_id: &str,
        base: &'a crate::text::TextStyle,
    ) -> crate::Result<crate::flowchart::FlowchartTextStyleResolution<'a>> {
        Ok(self
            .edge(edge_id)?
            .effective_edge_label_text_style_with_provenance(base))
    }

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

    pub(in crate::svg::parity::flowchart) fn marker_color(
        &self,
        edge_id: &str,
        hand_drawn: bool,
    ) -> Option<&str> {
        self.edges
            .get(edge_id)
            .and_then(|styles| styles.emission.edge_marker_color(hand_drawn))
    }

    pub(in crate::svg::parity::flowchart) fn admit_edge_source_style_svg_bytes(
        &self,
        edge_id: &str,
        current_svg_bytes: usize,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<()> {
        let projected = self
            .edges
            .get(edge_id)
            .map(|styles| styles.projected_source_style_bytes)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Flowchart edge style for `{edge_id}`"),
            })?;
        work_meter.policy().check_svg_byte_count(
            current_svg_bytes.saturating_add(projected),
            crate::resources::ResourceLimitPhase::SvgOutput,
        )?;
        work_meter.charge_svg_bytes(projected)?;
        Ok(())
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
        assert_eq!(plan.marker_color("e1", false), Some("#ef4444"));
        assert_eq!(plan.marker_color("e2", false), Some("#ef4444"));
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

        assert_eq!(plan.marker_color("e1", false), Some("#ef4444"));
        assert_eq!(
            plan.edge("e1").expect("prepared edge").stroke.as_deref(),
            Some("#ef4444")
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

        assert_eq!(plan.marker_color("e1", false), Some("#111827"));
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
    fn edge_source_style_svg_budget_is_charged_before_emission() {
        let class_defs = IndexMap::from([(
            "shared".to_string(),
            vec!["stroke:#ef4444,stroke-width:3px".to_string()],
        )]);
        let edges = vec![edge("e1", &["shared"], &["opacity:0.5"])];
        let preparation_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = FlowchartEdgeStylePlan::prepare(
            &class_defs,
            &edges,
            &[],
            false,
            false,
            &preparation_meter,
        )
        .expect("prepare edge styles");
        let projected = plan
            .edges
            .get("e1")
            .expect("prepared edge")
            .projected_source_style_bytes;
        assert!(projected > 0);

        let exact_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxSvgBytes,
                    37 + projected,
                )
                .expect("exact SVG limit"),
        );
        plan.admit_edge_source_style_svg_bytes("e1", 37, &exact_meter)
            .expect("exact SVG style budget must succeed");
        assert_eq!(exact_meter.projected_svg_bytes(), projected);

        let short_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxSvgBytes,
                    37 + projected - 1,
                )
                .expect("short SVG limit"),
        );
        let error = plan
            .admit_edge_source_style_svg_bytes("e1", 37, &short_meter)
            .expect_err("short SVG style budget must fail before emission");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected structured SVG resource rejection");
        };
        assert_eq!(error.limit, "max_svg_bytes");
        assert_eq!(error.actual, 37 + projected);
        assert_eq!(error.max, 37 + projected - 1);
        assert_eq!(short_meter.projected_svg_bytes(), 0);
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
