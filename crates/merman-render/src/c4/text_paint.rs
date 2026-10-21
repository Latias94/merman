use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Owns inherited text paint and the independently selected source colors of C4 terminals.
#[derive(Debug)]
pub(crate) struct C4TextPaintPlan {
    fill: Option<DirectStaticPaint>,
    title: Option<Box<str>>,
    evidence: FamilyThemeOutcomes,
    pending: Vec<C4PendingTextMechanism>,
    terminal: OnceLock<C4TextPaintTerminalSeal>,
    source_paint: OnceLock<C4SourceTerminalPaint>,
    theme: Option<ResolvedDiagramTheme>,
}

#[derive(Debug)]
struct C4SourceTerminalPaint {
    elements: Vec<C4ElementTerminalPaint>,
    relations: Vec<C4RelationTerminalPaint>,
    boundaries: Vec<Option<C4LabelTerminalPaint>>,
    role_evidence: FamilyThemeOutcomes,
    typed_roles: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
}

#[derive(Debug)]
pub(crate) struct C4ElementTerminalPaint {
    pub(crate) background: String,
    pub(crate) border: String,
    pub(crate) text: String,
    typed_text: bool,
}

#[derive(Debug)]
pub(crate) struct C4RelationTerminalPaint {
    pub(crate) text: String,
    pub(crate) line: String,
    pub(crate) themed_markers: bool,
    pub(crate) marker_fill: String,
    pub(crate) marker_stroke: String,
    typed_text: bool,
}

#[derive(Debug)]
struct C4LabelTerminalPaint {
    text: String,
    typed_text: bool,
    visible: bool,
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
    typed_fill_seen: bool,
    roles_complete: bool,
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
    typed_fill_seen: bool,
    expected_source: Option<&'a C4SourceTerminalPaint>,
    elements_seen: BTreeSet<usize>,
    boundaries_seen: BTreeSet<usize>,
    relations_seen: BTreeSet<usize>,
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
            evidence: FamilyThemeOutcomes::default(),
            pending: Vec::new(),
            terminal: OnceLock::new(),
            source_paint: OnceLock::new(),
            theme: theme.cloned(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        let config_owned = merman_core::__private::config_paint_overrides_typed_default(
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
        let mut fallbacks = FamilyThemeOutcomes::default();
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
        typography: &mut super::C4TypographyThemePlan,
        config: &MermaidConfig,
        work: &OperationWorkMeter,
    ) -> crate::Result<()> {
        // The writer historically selected the last source metadata for an alias, while
        // defaults came from each actual layout occurrence's type. Preserve both owners.
        let source_shapes: BTreeMap<_, _> = model
            .shapes
            .iter()
            .map(|shape| (shape.alias.as_str(), shape))
            .collect();
        let view = super::C4ConfigView::new(config.as_value());
        let mut elements = Vec::with_capacity(layout.shapes.len());
        let mut typed_roles = BTreeMap::new();
        let mut element_text_owned = Vec::new();
        let mut element_fill_owned = Vec::new();
        let mut element_stroke_owned = Vec::new();
        for shape in &layout.shapes {
            let source = source_shapes.get(shape.alias.as_str()).ok_or_else(|| {
                crate::Error::InvalidModel {
                    message: format!("c4: missing model shape {}", shape.alias),
                }
            })?;
            let defaults = typography
                .prepare_element_colors(&shape.type_c4_shape, &view)
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: format!("c4: missing prepared element style {}", shape.type_c4_shape),
                })?;
            let explicit_text = source
                .font_color
                .as_deref()
                .or_else(|| config.get_str(&format!("c4.{}_font_color", shape.type_c4_shape)));
            let fill_owned = source.bg_color.is_some()
                || merman_core::__private::config_paint_overrides_typed_default(
                    config,
                    &format!("c4.{}_bg_color", shape.type_c4_shape),
                );
            let stroke_owned = source.border_color.is_some()
                || merman_core::__private::config_paint_overrides_typed_default(
                    config,
                    &format!("c4.{}_border_color", shape.type_c4_shape),
                );
            let (background_paint, border_paint) = self.role_paints(
                ThemeTarget::Node,
                element_fill_owned.len() + 1,
                fill_owned && stroke_owned,
                work,
            )?;
            let background_paint = background_paint.filter(|_| !fill_owned);
            let border_paint = border_paint.filter(|_| !stroke_owned);
            element_fill_owned.push(fill_owned);
            element_stroke_owned.push(stroke_owned);
            record_role_paint(
                &mut typed_roles,
                ThemeTarget::Node,
                background_paint.as_ref(),
            );
            record_role_paint(&mut typed_roles, ThemeTarget::Node, border_paint.as_ref());
            let visible = text_is_visible(&shape.label.text)
                || text_is_visible(&shape.type_block.text)
                || shape
                    .descr
                    .as_ref()
                    .is_some_and(|text| text_is_visible(&text.text));
            let (text_paint, _) = self.role_paints(
                ThemeTarget::NodeLabel,
                element_text_owned.len() + 1,
                explicit_text.is_some(),
                work,
            )?;
            if visible {
                element_text_owned.push(explicit_text.is_some());
                record_role_paint(
                    &mut typed_roles,
                    ThemeTarget::NodeLabel,
                    text_paint.as_ref(),
                );
            }
            elements.push(C4ElementTerminalPaint {
                background: source
                    .bg_color
                    .as_deref()
                    .or_else(|| background_paint.as_ref().map(DirectStaticPaint::css))
                    .unwrap_or(&defaults.background)
                    .to_owned(),
                border: source
                    .border_color
                    .as_deref()
                    .or_else(|| border_paint.as_ref().map(DirectStaticPaint::css))
                    .unwrap_or(&defaults.border)
                    .to_owned(),
                text: explicit_text
                    .or_else(|| text_paint.as_ref().map(DirectStaticPaint::css))
                    .unwrap_or(&defaults.text)
                    .to_owned(),
                typed_text: visible && self.uses_inherited_fill(text_paint.as_ref()),
            });
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
        let mut relations = Vec::with_capacity(layout.rels.len());
        let mut relation_text_owned = Vec::new();
        let mut relation_line_owned = Vec::new();
        for (index, relation) in layout.rels.iter().enumerate() {
            let source = source_relations.get(&(relation.from.as_str(), relation.to.as_str()));
            let explicit_text = source
                .and_then(|source| source.text_color.as_deref())
                .or_else(|| config.get_str("c4.relationship_text_color"));
            let explicit_line = source
                .and_then(|source| source.line_color.as_deref())
                .or_else(|| config.get_str("c4.relationship_line_color"));
            let label_visible = text_is_visible(&relation.label.text)
                || relation
                    .techn
                    .as_ref()
                    .is_some_and(|text| !text.text.trim().is_empty());
            let (text_paint, _) = self.role_paints(
                ThemeTarget::EdgeLabel,
                relation_text_owned.len() + 1,
                explicit_text.is_some(),
                work,
            )?;
            let (_, line_paint) = self.role_paints(ThemeTarget::Edge, index + 1, false, work)?;
            let (marker_fill, marker_stroke) =
                self.role_paints(ThemeTarget::Marker, index + 1, false, work)?;
            // Mermaid's source colors retain the shared, unpainted markers. A requested
            // host paint activates colored markers even when source colors override it.
            let themed_markers =
                line_paint.is_some() || marker_fill.is_some() || marker_stroke.is_some();
            let line_paint = line_paint.filter(|_| explicit_line.is_none());
            let marker_fill = marker_fill.filter(|_| explicit_line.is_none());
            let marker_stroke = marker_stroke.filter(|_| explicit_line.is_none());
            if label_visible {
                relation_text_owned.push(explicit_text.is_some());
                record_role_paint(
                    &mut typed_roles,
                    ThemeTarget::EdgeLabel,
                    text_paint.as_ref(),
                );
            }
            relation_line_owned.push(explicit_line.is_some());
            record_role_paint(&mut typed_roles, ThemeTarget::Edge, line_paint.as_ref());
            record_role_paint(&mut typed_roles, ThemeTarget::Marker, marker_fill.as_ref());
            record_role_paint(
                &mut typed_roles,
                ThemeTarget::Marker,
                marker_stroke.as_ref(),
            );
            let line = explicit_line
                .or_else(|| line_paint.as_ref().map(DirectStaticPaint::css))
                .unwrap_or(typography.relationship_line_color())
                .to_owned();
            relations.push(C4RelationTerminalPaint {
                text: source
                    .and_then(|source| source.text_color.as_deref())
                    .or_else(|| config.get_str("c4.relationship_text_color"))
                    .or_else(|| text_paint.as_ref().map(DirectStaticPaint::css))
                    .unwrap_or(typography.relationship_text_color())
                    .to_owned(),
                marker_fill: marker_fill
                    .as_ref()
                    .map(DirectStaticPaint::css)
                    .unwrap_or(&line)
                    .to_owned(),
                marker_stroke: marker_stroke
                    .as_ref()
                    .map(DirectStaticPaint::css)
                    .unwrap_or(&line)
                    .to_owned(),
                line,
                themed_markers,
                typed_text: label_visible && self.uses_inherited_fill(text_paint.as_ref()),
            });
        }
        let mut boundaries = Vec::with_capacity(layout.boundaries.len());
        let mut boundary_text_owned = Vec::new();
        let source_boundaries: BTreeMap<_, _> = model
            .boundaries
            .iter()
            .map(|boundary| (boundary.alias.as_str(), boundary))
            .collect();
        for boundary in &layout.boundaries {
            if boundary.alias == "global" {
                boundaries.push(None);
                continue;
            }
            let explicit = source_boundaries
                .get(boundary.alias.as_str())
                .and_then(|boundary| boundary.font_color.as_deref())
                .or_else(|| config.get_str("c4.boundary_font_color"));
            let visible = text_is_visible(&boundary.label.text)
                || boundary
                    .ty
                    .as_ref()
                    .is_some_and(|text| text_is_visible(&text.text))
                || boundary
                    .descr
                    .as_ref()
                    .is_some_and(|text| text_is_visible(&text.text));
            let (paint, _) = self.role_paints(
                ThemeTarget::ClusterLabel,
                boundary_text_owned.len() + 1,
                explicit.is_some(),
                work,
            )?;
            if visible {
                boundary_text_owned.push(explicit.is_some());
                record_role_paint(&mut typed_roles, ThemeTarget::ClusterLabel, paint.as_ref());
            }
            boundaries.push(Some(C4LabelTerminalPaint {
                text: explicit
                    .or_else(|| paint.as_ref().map(DirectStaticPaint::css))
                    .unwrap_or("#444444")
                    .to_owned(),
                typed_text: visible && self.uses_inherited_fill(paint.as_ref()),
                visible,
            }));
        }
        let mut role_evidence = FamilyThemeOutcomes::default();
        if let Some(theme) = self.theme.as_ref() {
            reconcile_unsupported_terminal_domains(
                theme,
                &mut role_evidence,
                &[
                    UnsupportedTerminalDomain::direct(
                        ThemeTarget::Node,
                        TerminalVariantDomain::uniform(
                            element_fill_owned.len(),
                            ThemeVariant::Default,
                        ),
                    )
                    .with_source_owned_fill(&element_fill_owned)
                    .with_source_owned_stroke(&element_stroke_owned),
                    UnsupportedTerminalDomain::textual(
                        ThemeTarget::NodeLabel,
                        &[ThemeTarget::NodeLabel],
                        TerminalVariantDomain::uniform(
                            element_text_owned.len(),
                            ThemeVariant::Default,
                        ),
                    )
                    .with_source_owned_fill(&element_text_owned),
                    UnsupportedTerminalDomain::textual(
                        ThemeTarget::ClusterLabel,
                        &[ThemeTarget::ClusterLabel],
                        TerminalVariantDomain::uniform(
                            boundary_text_owned.len(),
                            ThemeVariant::Default,
                        ),
                    )
                    .with_source_owned_fill(&boundary_text_owned),
                    UnsupportedTerminalDomain::textual(
                        ThemeTarget::EdgeLabel,
                        &[ThemeTarget::EdgeLabel],
                        TerminalVariantDomain::uniform(
                            relation_text_owned.len(),
                            ThemeVariant::Default,
                        ),
                    )
                    .with_source_owned_fill(&relation_text_owned),
                    UnsupportedTerminalDomain::direct(
                        ThemeTarget::Edge,
                        TerminalVariantDomain::uniform(
                            relation_line_owned.len(),
                            ThemeVariant::Default,
                        ),
                    )
                    .with_source_owned_stroke(&relation_line_owned),
                    UnsupportedTerminalDomain::direct(
                        ThemeTarget::Marker,
                        TerminalVariantDomain::uniform(
                            relation_line_owned.len(),
                            ThemeVariant::Default,
                        ),
                    )
                    .with_source_owned_fill(&relation_line_owned)
                    .with_source_owned_stroke(&relation_line_owned),
                ],
                work,
            )?;
        }
        self.source_paint
            .set(C4SourceTerminalPaint {
                elements,
                relations,
                boundaries,
                role_evidence,
                typed_roles,
            })
            .expect("C4 source paint prepared once per family artifact");
        Ok(())
    }

    fn role_paints(
        &self,
        target: ThemeTarget,
        ordinal: usize,
        source_owned: bool,
        work: &OperationWorkMeter,
    ) -> Result<(Option<DirectStaticPaint>, Option<DirectStaticPaint>), OperationWorkError> {
        let Some(theme) = self.theme.as_ref().filter(|_| !source_owned) else {
            return Ok((None, None));
        };
        let textual = matches!(
            target,
            ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel | ThemeTarget::ClusterLabel
        );
        let style = if textual {
            theme.text_style_with_work_meter(target, ThemeVariant::Default, Some(ordinal), work)?
        } else {
            theme.style_with_work_meter(target, ThemeVariant::Default, Some(ordinal), work)?
        };
        let targets = [ThemeTarget::Text, target];
        // An owned root textColor suppresses Text fallback, not an explicit C4 label role.
        let inherits_text = textual && self.fill.is_some();
        let targets = if inherits_text {
            &targets[..]
        } else {
            &targets[1..]
        };
        Ok((
            resolve_direct_static_fill(theme, &style, targets, DirectStaticSelectorDomain::Default),
            resolve_direct_static_stroke(
                theme,
                &style,
                targets,
                DirectStaticSelectorDomain::Default,
            ),
        ))
    }

    fn uses_inherited_fill(&self, paint: Option<&DirectStaticPaint>) -> bool {
        self.fill
            .as_ref()
            .zip(paint)
            .is_some_and(|(inherited, paint)| inherited.rule_index() == paint.rule_index())
    }

    pub(crate) fn boundary_text_color(&self, index: usize) -> Option<&str> {
        self.source_paint
            .get()?
            .boundaries
            .get(index)?
            .as_ref()
            .map(|paint| paint.text.as_str())
    }

    pub(crate) fn element_paint(&self, index: usize) -> Option<&C4ElementTerminalPaint> {
        self.source_paint
            .get()
            .expect("C4 source paint prepared before SVG emission")
            .elements
            .get(index)
    }

    pub(crate) fn relation_paint(&self, index: usize) -> Option<&C4RelationTerminalPaint> {
        self.source_paint
            .get()
            .expect("C4 source paint prepared before SVG emission")
            .relations
            .get(index)
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<C4TextPaintReceipt<'_>> {
        (!self.pending.is_empty()
            || self.theme.as_ref().is_some_and(|theme| {
                theme.family_mechanism_routes().iter().any(|route| {
                    matches!(route.mechanism(), FamilyThemeMechanism::RuleFacet { target, .. }
                | FamilyThemeMechanism::OrdinalPalette { target }
                | FamilyThemeMechanism::EffectBinding { target, .. } if is_role_target(target))
                })
            }))
        .then(|| C4TextPaintReceipt {
            expected_fill: self.fill_css(),
            expected_title: self.title.as_deref(),
            css_seen: false,
            title_seen: false,
            text_seen: false,
            source_colors_static: true,
            valid: true,
            typed_fill_seen: false,
            expected_source: self.source_paint.get(),
            elements_seen: BTreeSet::new(),
            boundaries_seen: BTreeSet::new(),
            relations_seen: BTreeSet::new(),
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
                    typed_fill_seen: receipt.typed_fill_seen,
                    roles_complete: receipt.expected_source.is_some_and(|source| {
                        source.elements.len() == receipt.elements_seen.len()
                            && source
                                .boundaries
                                .iter()
                                .flatten()
                                .filter(|paint| paint.visible)
                                .count()
                                == receipt.boundaries_seen.len()
                            && source.relations.len() == receipt.relations_seen.len()
                    }),
                })
                .is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        for pending in &self.pending {
            let key = &pending.key;
            let residual = pending.residual;
            match self.terminal.get() {
                Some(receipt)
                    if !receipt.text_seen
                        || (pending.inherited_fill_only
                            && receipt.source_colors_static
                            && !receipt.title_seen
                            && !receipt.typed_fill_seen) =>
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
        if let Some(source) = self.source_paint.get() {
            evidence.merge_borrowed(&source.role_evidence);
            if self
                .terminal
                .get()
                .is_some_and(|terminal| terminal.roles_complete)
                && let Some(theme) = self.theme.as_ref()
            {
                for route in theme.family_mechanism_routes() {
                    let FamilyThemeMechanism::RuleFacet { target, .. } = route.mechanism() else {
                        continue;
                    };
                    if !is_role_target(target) {
                        continue;
                    }
                    let key = theme.family_mechanism_key(*route);
                    if let Some(capabilities) = source.typed_roles.get(&key) {
                        evidence.mark_applied_with_capabilities(key, capabilities.iter().copied());
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
            }
        }
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

    pub(crate) fn record_boundary(&mut self, index: usize, color: &str) {
        let Some(expected) = self
            .expected_source
            .and_then(|source| source.boundaries.get(index)?.as_ref())
        else {
            self.valid = false;
            return;
        };
        self.valid &= expected.text == color && self.boundaries_seen.insert(index);
        self.typed_fill_seen |= expected.typed_text;
    }

    pub(crate) fn record_element(&mut self, index: usize, paint: &C4ElementTerminalPaint) {
        let Some(expected) = self
            .expected_source
            .and_then(|source| source.elements.get(index))
        else {
            self.valid = false;
            return;
        };
        self.valid &= expected.text == paint.text
            && expected.background == paint.background
            && expected.border == paint.border
            && self.elements_seen.insert(index);
        self.typed_fill_seen |= expected.typed_text;
    }

    pub(crate) fn record_relation(&mut self, index: usize, paint: &C4RelationTerminalPaint) {
        let Some(expected) = self
            .expected_source
            .and_then(|source| source.relations.get(index))
        else {
            self.valid = false;
            return;
        };
        self.valid &= expected.text == paint.text
            && expected.line == paint.line
            && expected.marker_fill == paint.marker_fill
            && expected.marker_stroke == paint.marker_stroke
            && self.relations_seen.insert(index);
        self.typed_fill_seen |= expected.typed_text;
    }
}

fn is_role_target(target: ThemeTarget) -> bool {
    matches!(
        target,
        ThemeTarget::Node
            | ThemeTarget::NodeLabel
            | ThemeTarget::ClusterLabel
            | ThemeTarget::EdgeLabel
            | ThemeTarget::Edge
            | ThemeTarget::Marker
    )
}

fn text_is_visible(text: &str) -> bool {
    text.split("<br/>")
        .flat_map(|line| line.split("<br />"))
        .flat_map(|line| line.split("<br>"))
        .any(|line| !line.trim().is_empty())
}

fn record_role_paint(
    outcomes: &mut BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    target: ThemeTarget,
    paint: Option<&DirectStaticPaint>,
) {
    if let Some(paint) = paint {
        outcomes
            .entry(FamilyThemeMechanismKey::Rule {
                index: paint.rule_index(),
                target,
            })
            .or_default()
            .insert(paint.capability());
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
        let mut external_shape = shape.clone();
        external_shape["type_c4_shape"] = serde_json::json!("external_person");
        let layout = serde_json::from_value(serde_json::json!({
            "bounds": null, "width": 10.0, "height": 10.0,
            "container_width": 100.0, "container_height": 100.0, "c4_type": "C4Context", "title": null,
            "boundaries": [], "shapes": [shape, external_shape], "rels": [relation, relation]
        })).expect("C4 actual layout fixture");
        let config = MermaidConfig::from_value(serde_json::json!({"c4": {
            "person_border_color": "var(--actual-border)", "custom_border_color": "red"
        }}));
        let mut typography =
            super::super::C4TypographyThemePlan::resolve(None, &config, None, &model);
        let plan = C4TextPaintPlan::resolve(
            None,
            &config,
            None,
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .expect("C4 text plan");
        plan.bind_source_terminal_paint(
            &model,
            &layout,
            &mut typography,
            &config,
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .expect("C4 terminal binding");
        let paint = plan.element_paint(0).expect("bound occurrence");
        assert_eq!(paint.background, "");
        assert_eq!(paint.border, "var(--actual-border)");
        assert_eq!(paint.text, "var(--label)");
        let external = plan.element_paint(1).expect("second bound occurrence");
        assert_eq!(external.background, "");
        assert_eq!(external.border, "#8A8A8A");
        assert_eq!(external.text, "var(--label)");
        let mut complete = C4TextPaintReceipt {
            expected_source: plan.source_paint.get(),
            ..title_receipt()
        };
        complete.record_element(0, paint);
        complete.record_element(1, external);
        assert!(
            complete.valid,
            "equal aliases are independent layout occurrences"
        );
        complete.record_element(1, external);
        assert!(!complete.valid, "one occurrence cannot be proved twice");
        let mut wrong = C4TextPaintReceipt {
            expected_source: plan.source_paint.get(),
            ..title_receipt()
        };
        wrong.record_element(0, external);
        assert!(
            !wrong.valid,
            "paint from another occurrence is not valid evidence"
        );
        let paint = plan
            .relation_paint(0)
            .expect("bound relationship occurrence");
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
            typed_fill_seen: false,
            expected_source: None,
            elements_seen: BTreeSet::new(),
            boundaries_seen: BTreeSet::new(),
            relations_seen: BTreeSet::new(),
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
            evidence: FamilyThemeOutcomes::default(),
            pending: Vec::new(),
            terminal: OnceLock::new(),
            source_paint: OnceLock::new(),
            theme: None,
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
