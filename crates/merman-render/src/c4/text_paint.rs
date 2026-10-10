use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Owns inherited text paint and the independently selected source colors of C4 terminals.
#[derive(Debug)]
pub(crate) struct C4TextPaintPlan {
    fill: Option<DirectStaticPaint>,
    title: Option<Box<str>>,
    evidence: FamilyThemeEvidence,
    pending: Vec<C4PendingTextMechanism>,
    terminal: OnceLock<C4TextPaintTerminalSeal>,
    source_paint: OnceLock<C4SourceTerminalPaint>,
}

#[derive(Debug)]
struct C4SourceTerminalPaint {
    elements: BTreeMap<String, C4ElementTerminalPaint>,
    relations: BTreeMap<String, BTreeMap<String, C4RelationTerminalPaint>>,
}

#[derive(Debug)]
pub(crate) struct C4ElementTerminalPaint {
    pub(crate) background: String,
    pub(crate) border: String,
    pub(crate) text: String,
}

#[derive(Debug)]
pub(crate) struct C4RelationTerminalPaint {
    pub(crate) text: String,
    pub(crate) line: String,
}

#[derive(Debug)]
struct C4PendingTextMechanism {
    key: FamilyThemeMechanismKey,
    residual: Option<FamilyThemeResidualReason>,
    inherited_fill_only: bool,
}

#[derive(Debug)]
struct C4TextPaintTerminalSeal {
    title_seen: bool,
    text_seen: bool,
    source_colors_static: bool,
}

#[derive(Debug)]
pub(crate) struct C4TextPaintReceipt<'a> {
    expected_fill: Option<&'a str>,
    expected_title: Option<&'a str>,
    css_seen: bool,
    title_seen: bool,
    text_seen: bool,
    source_colors_static: bool,
    valid: bool,
}

impl C4TextPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        title: Option<&str>,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(Into::into);
        let mut plan = Self {
            fill: None,
            title,
            evidence: FamilyThemeEvidence::from_theme(theme),
            pending: Vec::new(),
            terminal: OnceLock::new(),
            source_paint: OnceLock::new(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        let config_owned = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.textColor",
        );
        let style =
            theme.style_with_work_meter(ThemeTarget::Text, ThemeVariant::Default, None, work)?;
        let winners = style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        if !config_owned {
            plan.fill = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            );
        }
        // A rule key covers all its facets. A proved fill must not certify an unsupported sibling.
        let mut observations = BTreeMap::new();
        for route in theme.family_mechanism_routes() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Text,
                facet,
                selector,
            } = route.mechanism()
            else {
                continue;
            };
            work.charge(1)?;
            let entry = observations
                .entry(theme.family_mechanism_key(*route))
                .or_insert((false, None, true));
            let property = resolved_style_property_for_facet(facet);
            // C4 has no declared logical Text ordinal ordering. Only a later global winner
            // proves that an ordinal assignment is shadowed on every possible terminal.
            let unproved_ordinal = matches!(
                selector,
                FamilyThemeSelectorShape::Ordinal {
                    variant: None | Some(ThemeVariant::Default),
                    ..
                }
            ) && !winners.iter().any(|(winner_index, winner_property)| {
                *winner_property == property && *winner_index > rule_index
            });
            if (!winners.contains(&(rule_index, property)) && !unproved_ordinal)
                || (property == ResolvedStyleProperty::Fill && config_owned)
            {
                continue;
            }
            entry.2 &= property == ResolvedStyleProperty::Fill;
            if route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                && plan
                    .fill
                    .as_ref()
                    .is_some_and(|fill| fill.rule_index() == rule_index)
            {
                entry.0 = true;
            } else {
                entry.1.get_or_insert(unsupported_residual_for_facet(facet));
            }
        }
        for (key, (pending, residual, inherited_fill_only)) in observations {
            if pending || residual.is_some() {
                // Source-directed inheritance is only known after the writer emits its labels.
                plan.pending.push(C4PendingTextMechanism {
                    key,
                    residual,
                    inherited_fill_only,
                });
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        let mut fallbacks = FamilyThemeEvidence::from_theme(Some(theme));
        reconcile_unsupported_terminal_domains(
            theme,
            &mut fallbacks,
            &[UnsupportedTerminalDomain::static_fallbacks_only(
                ThemeTarget::Text,
                ThemeVariant::Default,
            )
            .with_source_owned_fill(&[config_owned])],
            work,
        )?;
        // Resolve fallback winners once, but defer their visible domain to the terminal receipt.
        for key in fallbacks.not_applicable_mechanisms() {
            plan.evidence.mark_not_applicable(key.clone());
        }
        for residual in fallbacks.residuals() {
            plan.pending.push(C4PendingTextMechanism {
                key: residual.key().clone(),
                residual: Some(residual.reason()),
                inherited_fill_only: residual.reason()
                    == FamilyThemeResidualReason::UnsupportedOrdinalPalette,
            });
        }
        Ok(plan)
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill.as_ref().map(DirectStaticPaint::css)
    }

    pub(crate) fn bind_source_terminal_paint(
        &self,
        model: &merman_core::diagrams::c4::C4DiagramRenderModel,
        layout: &crate::model::C4DiagramLayout,
        typography: &super::C4TypographyThemePlan,
    ) -> crate::Result<()> {
        // The writer historically selected the last source metadata for an alias, while
        // defaults came from the actual alias-keyed layout type. Preserve both owners.
        let source_shapes: BTreeMap<_, _> = model
            .shapes
            .iter()
            .map(|shape| (shape.alias.as_str(), shape))
            .collect();
        let mut elements = BTreeMap::new();
        for shape in &layout.shapes {
            let source = source_shapes.get(shape.alias.as_str()).ok_or_else(|| {
                crate::Error::InvalidModel {
                    message: format!("c4: missing model shape {}", shape.alias),
                }
            })?;
            let defaults = typography
                .element_style(&shape.type_c4_shape)
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: format!("c4: missing prepared element style {}", shape.type_c4_shape),
                })?;
            elements.insert(
                shape.alias.clone(),
                C4ElementTerminalPaint {
                    background: source
                        .bg_color
                        .as_deref()
                        .unwrap_or(&defaults.background)
                        .to_owned(),
                    border: source
                        .border_color
                        .as_deref()
                        .unwrap_or(&defaults.border)
                        .to_owned(),
                    text: source.font_color.as_deref().unwrap_or("#FFFFFF").to_owned(),
                },
            );
        }
        let source_relations: BTreeMap<_, _> = model
            .rels
            .iter()
            .map(|relation| {
                (
                    (relation.from_alias.as_str(), relation.to_alias.as_str()),
                    relation,
                )
            })
            .collect();
        let mut relations: BTreeMap<String, BTreeMap<String, C4RelationTerminalPaint>> =
            BTreeMap::new();
        for relation in &layout.rels {
            let source = source_relations.get(&(relation.from.as_str(), relation.to.as_str()));
            relations
                .entry(relation.from.clone())
                .or_default()
                .entry(relation.to.clone())
                .or_insert_with(|| C4RelationTerminalPaint {
                    text: source
                        .and_then(|source| source.text_color.as_deref())
                        .unwrap_or("#444444")
                        .to_owned(),
                    line: source
                        .and_then(|source| source.line_color.as_deref())
                        .unwrap_or("#444444")
                        .to_owned(),
                });
        }
        self.source_paint
            .set(C4SourceTerminalPaint {
                elements,
                relations,
            })
            .expect("C4 source paint prepared once per family artifact");
        Ok(())
    }

    pub(crate) fn element_paint(&self, alias: &str) -> Option<&C4ElementTerminalPaint> {
        self.source_paint
            .get()
            .expect("C4 source paint prepared before SVG emission")
            .elements
            .get(alias)
    }

    pub(crate) fn relation_paint(&self, from: &str, to: &str) -> Option<&C4RelationTerminalPaint> {
        self.source_paint
            .get()
            .expect("C4 source paint prepared before SVG emission")
            .relations
            .get(from)?
            .get(to)
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<C4TextPaintReceipt<'_>> {
        (!self.pending.is_empty()).then(|| C4TextPaintReceipt {
            expected_fill: self.fill_css(),
            expected_title: self.title.as_deref(),
            css_seen: false,
            title_seen: false,
            text_seen: false,
            source_colors_static: true,
            valid: true,
        })
    }

    pub(crate) fn record_terminal(&self, receipt: C4TextPaintReceipt<'_>) -> bool {
        receipt.valid
            && receipt.css_seen
            && receipt.title_seen == receipt.expected_title.is_some()
            && self
                .terminal
                .set(C4TextPaintTerminalSeal {
                    title_seen: receipt.title_seen,
                    text_seen: receipt.text_seen,
                    source_colors_static: receipt.source_colors_static,
                })
                .is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        for pending in &self.pending {
            let key = &pending.key;
            let residual = pending.residual;
            match self.terminal.get() {
                Some(receipt)
                    if !receipt.text_seen
                        || (pending.inherited_fill_only
                            && receipt.source_colors_static
                            && !receipt.title_seen) =>
                {
                    evidence.mark_not_applicable(key.clone());
                }
                Some(receipt) if receipt.source_colors_static && residual.is_none() => {
                    if let Some(fill) = &self.fill {
                        evidence.mark_applied_with_capabilities(key.clone(), [fill.capability()]);
                    } else {
                        evidence.mark_residual(
                            key.clone(),
                            FamilyThemeResidualReason::UnsupportedPaint,
                        );
                    }
                }
                _ => {
                    evidence.mark_residual(
                        key.clone(),
                        residual.unwrap_or(FamilyThemeResidualReason::UnsupportedPaint),
                    );
                }
            }
        }
        evidence
    }
}

impl C4TextPaintReceipt<'_> {
    pub(crate) fn record_css(&mut self, fill: Option<&str>) {
        self.valid &= !self.css_seen && fill == self.expected_fill;
        self.css_seen = true;
    }

    pub(crate) fn record_title(&mut self, title: &str, fill: Option<&str>) {
        self.valid &=
            !self.title_seen && Some(title) == self.expected_title && fill == self.expected_fill;
        self.title_seen = true;
        self.text_seen = true;
    }

    /// Source-directed inheritance and browser-dependent colors need their own terminal proof.
    pub(crate) fn record_owned_color(&mut self, color: &str) {
        self.text_seen = true;
        self.source_colors_static =
            self.source_colors_static && crate::mermaid_style::is_supported_css_color_value(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_terminal_binding_preserves_actual_layout_defaults_and_last_metadata() {
        let model = serde_json::from_value(serde_json::json!({
            "shapes": [
                {"alias": "duplicate", "typeC4Shape": "person", "bgColor": "red", "fontColor": "blue"},
                {"alias": "duplicate", "typeC4Shape": "custom", "bgColor": "", "fontColor": "var(--label)"}
            ],
            "rels": [
                {"from": "duplicate", "to": "duplicate", "type": "rel", "lineColor": "red"},
                {"from": "duplicate", "to": "duplicate", "type": "rel", "textColor": ""}
            ]
        })).expect("C4 source metadata fixture");
        let block =
            serde_json::json!({"text": "", "y": 0.0, "width": 0.0, "height": 0.0, "line_count": 0});
        let shape = serde_json::json!({
            "alias": "duplicate", "parent_boundary": "", "type_c4_shape": "person",
            "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0, "margin": 0.0,
            "image": {"width": 0.0, "height": 0.0, "y": 0.0}, "type_block": block, "label": block
        });
        let relation = serde_json::json!({
            "from": "duplicate", "to": "duplicate", "rel_type": "rel",
            "start_point": {"x": 0.0, "y": 0.0}, "end_point": {"x": 1.0, "y": 1.0}, "label": block
        });
        let layout = serde_json::from_value(serde_json::json!({
            "bounds": null, "width": 10.0, "height": 10.0,
            "container_width": 100.0, "container_height": 100.0, "c4_type": "C4Context", "title": null,
            "boundaries": [], "shapes": [shape, shape], "rels": [relation, relation]
        })).expect("C4 actual layout fixture");
        let config = MermaidConfig::from_value(serde_json::json!({"c4": {
            "person_border_color": "var(--actual-border)", "custom_border_color": "red"
        }}));
        let typography = super::super::C4TypographyThemePlan::resolve(None, &config, None, &model);
        let plan = C4TextPaintPlan::resolve(
            None,
            &config,
            None,
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .expect("C4 text plan");
        plan.bind_source_terminal_paint(&model, &layout, &typography)
            .expect("C4 terminal binding");
        let paint = plan.element_paint("duplicate").expect("bound alias");
        assert_eq!(paint.background, "");
        assert_eq!(paint.border, "var(--actual-border)");
        assert_eq!(paint.text, "var(--label)");
        let paint = plan
            .relation_paint("duplicate", "duplicate")
            .expect("bound endpoint pair");
        assert_eq!(paint.text, "");
        assert_eq!(paint.line, "#444444");
        assert!(plan.begin_terminal_receipt().is_none());
        let mut receipt = title_receipt();
        assert!(!receipt.text_seen);
        assert!(receipt.source_colors_static);
        receipt.record_owned_color("var(--label)");
        assert!(receipt.text_seen);
        assert!(!receipt.source_colors_static);
    }

    fn title_receipt() -> C4TextPaintReceipt<'static> {
        C4TextPaintReceipt {
            expected_fill: Some("#123456"),
            expected_title: Some("Title"),
            css_seen: false,
            title_seen: false,
            text_seen: false,
            source_colors_static: true,
            valid: true,
        }
    }

    #[test]
    fn receipt_rejects_wrong_or_duplicate_terminal_facts() {
        let mut wrong_css = title_receipt();
        wrong_css.record_css(Some("#654321"));
        assert!(!wrong_css.valid);

        let mut wrong_title = title_receipt();
        wrong_title.record_title("Other", Some("#123456"));
        assert!(!wrong_title.valid);

        let mut wrong_fill = title_receipt();
        wrong_fill.record_title("Title", None);
        assert!(!wrong_fill.valid);

        let mut duplicate = title_receipt();
        duplicate.record_css(Some("#123456"));
        duplicate.record_css(Some("#123456"));
        assert!(!duplicate.valid);

        let mut duplicate = title_receipt();
        duplicate.record_title("Title", Some("#123456"));
        duplicate.record_title("Title", Some("#123456"));
        assert!(!duplicate.valid);
    }

    #[test]
    fn sealing_requires_css_and_the_expected_title_exactly_once() {
        let plan = C4TextPaintPlan {
            fill: None,
            title: Some("Title".into()),
            evidence: FamilyThemeEvidence::default(),
            pending: Vec::new(),
            terminal: OnceLock::new(),
            source_paint: OnceLock::new(),
        };
        assert!(!plan.record_terminal(title_receipt()));
        let mut missing_title = title_receipt();
        missing_title.record_css(Some("#123456"));
        assert!(!plan.record_terminal(missing_title));
        for expected_success in [true, false] {
            let mut complete = title_receipt();
            complete.record_css(Some("#123456"));
            complete.record_title("Title", Some("#123456"));
            assert_eq!(plan.record_terminal(complete), expected_success);
        }
    }
}
