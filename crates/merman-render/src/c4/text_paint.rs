use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence,
    FamilyThemeOutcomes, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::C4TextBlockLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Owns inherited text paint and the independently selected source colors of C4 terminals.
#[derive(Debug)]
pub(crate) struct C4TextPaintPlan {
    fill: Option<DirectStaticPaint>,
    title_fill: Option<DirectPaintExpectation>,
    title: Option<Box<str>>,
    evidence: FamilyThemeOutcomes,
    title_evidence: FamilyThemeOutcomes,
    pending: Vec<C4PendingTextMechanism>,
    terminal: OnceLock<C4TextPaintTerminalSeal>,
    source_paint: OnceLock<C4SourceTerminalPaint>,
    theme: Option<ResolvedDiagramTheme>,
}

#[derive(Debug)]
struct C4SourceTerminalPaint {
    title_visible: bool,
    title_visibility_verified: bool,
    elements: Vec<C4ElementTerminalPaint>,
    relations: Vec<C4RelationTerminalPaint>,
    boundaries: Vec<Option<C4LabelTerminalPaint>>,
    role_evidence: FamilyThemeOutcomes,
    typed_roles: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    unverified_text_roles: BTreeSet<FamilyThemeMechanismKey>,
}

#[derive(Debug)]
pub(crate) struct C4ElementTerminalPaint {
    pub(crate) background: String,
    pub(crate) border: String,
    pub(crate) text: String,
    typed_text: bool,
    visible: bool,
    visibility_verified: bool,
}

#[derive(Debug)]
pub(crate) struct C4RelationTerminalPaint {
    pub(crate) text: String,
    pub(crate) line: String,
    pub(crate) themed_markers: bool,
    pub(crate) marker_fill: String,
    pub(crate) marker_stroke: String,
    typed_text: bool,
    visible: bool,
    visibility_verified: bool,
}

#[derive(Debug)]
struct C4LabelTerminalPaint {
    text: String,
    typed_text: bool,
    visible: bool,
    visibility_verified: bool,
}

#[derive(Debug)]
struct C4PendingTextMechanism {
    key: FamilyThemeMechanismKey,
    residual: Option<FamilyThemeResidualReason>,
    inherited_fill_only: bool,
}

#[derive(Debug)]
struct C4TextPaintTerminalSeal {
    inherited_title_seen: bool,
    text_seen: bool,
    source_colors_static: bool,
    typed_fill_seen: bool,
    roles_complete: bool,
}

#[derive(Debug)]
pub(crate) struct C4TextPaintReceipt<'a> {
    expected_fill: Option<&'a str>,
    expected_title_fill: Option<&'a str>,
    title_inherits_fill: bool,
    expected_title: Option<&'a str>,
    expected_title_visible: bool,
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
        config: &PresentationConfig,
        title: Option<&str>,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let title = title
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .map(Into::into);
        let mut plan = Self {
            fill: None,
            title_fill: None,
            title,
            evidence: FamilyThemeOutcomes::default(),
            title_evidence: FamilyThemeOutcomes::default(),
            pending: Vec::new(),
            terminal: OnceLock::new(),
            source_paint: OnceLock::new(),
            theme: theme.cloned(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        let config_owned = config.config_paint_overrides_typed_default("themeVariables.textColor");
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.title_evidence,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Title,
                TerminalVariantDomain::uniform(
                    usize::from(plan.title.is_some()),
                    ThemeVariant::Default,
                ),
            )
            .with_source_owned_fill(&[config_owned])],
            work,
        )?;
        let mut title_fill_winner = None;
        if plan.title.is_some() && !config_owned {
            let style = theme.style_with_work_meter(
                ThemeTarget::Title,
                ThemeVariant::Default,
                Some(1),
                work,
            )?;
            title_fill_winner = style
                .fill_resolution()
                .winner()
                .map(|origin| origin.rule_index());
            plan.title_fill = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Title],
                DirectStaticSelectorDomain::Default,
            )
            .map(DirectPaintExpectation::from_paint);
            if matches!(style.fill_resolution().specified(), Specified::Clear)
                && let Some(origin) = style.fill_resolution().winner()
                && origin.ordinal().is_none()
                && matches!(origin.variant(), None | Some(ThemeVariant::Default))
                && theme.rule_facet_disposition(
                    origin.rule_index(),
                    FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Clear),
                ) == Some(FamilyThemeDisposition::TypedAdapter)
            {
                // Clear blocks Text fallback and restores the configured root text paint.
                let baseline = crate::config::config_string(
                    config.as_value(),
                    &["themeVariables", "textColor"],
                )
                .unwrap_or_else(|| "#333".to_owned());
                plan.title_fill = Some(DirectPaintExpectation::new(
                    origin.rule_index(),
                    baseline,
                    ThemeCapability::SemanticRules,
                ));
            }
        }
        for route in theme.family_mechanism_routes() {
            if let FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Title,
                rule_index,
                facet: FamilyThemeRuleFacet::Fill(_),
                ..
            } = route.mechanism()
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
            {
                if title_fill_winner != Some(rule_index) {
                    plan.title_evidence
                        .mark_not_applicable(theme.family_mechanism_key(*route));
                } else if plan
                    .title_fill
                    .as_ref()
                    .is_none_or(|fill| fill.rule_index() != rule_index)
                {
                    plan.title_evidence.mark_residual(
                        theme.family_mechanism_key(*route),
                        FamilyThemeResidualReason::UnsupportedPaint,
                    );
                }
            }
        }
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

    pub(crate) fn title_fill_css(&self) -> Option<&str> {
        self.title_fill
            .as_ref()
            .map(DirectPaintExpectation::css)
            .or_else(|| self.fill_css())
    }

    pub(crate) fn bind_source_terminal_paint(
        &self,
        model: &merman_model::families::c4::C4DiagramModel,
        layout: &crate::model::C4DiagramLayout,
        typography: &mut super::C4TypographyThemePlan,
        config: &PresentationConfig,
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
        let mut unverified_text_roles = BTreeSet::new();
        let mut element_text_owned = Vec::new();
        let mut element_fill_owned = Vec::new();
        let mut element_stroke_owned = Vec::new();
        for shape in &layout.shapes {
            let source = source_shapes.get(shape.alias.as_str()).ok_or_else(|| {
                crate::Error::InvalidModel {
                    message: format!("c4: missing model shape {}", shape.alias),
                }
            })?;
            let font = &typography
                .element_style(&shape.type_c4_shape)
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: format!("c4: missing prepared element style {}", shape.type_c4_shape),
                })?
                .font;
            let visibility_verified = font.font_size_is_known();
            let visible = font.potentially_visible()
                && (text_has_visible_content(&shape.label)
                    || text_has_visible_content(&shape.type_block)
                    || shape.descr.as_ref().is_some_and(text_has_visible_content));
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
                || config.config_paint_overrides_typed_default(&format!(
                    "c4.{}_bg_color",
                    shape.type_c4_shape
                ));
            let stroke_owned = source.border_color.is_some()
                || config.config_paint_overrides_typed_default(&format!(
                    "c4.{}_border_color",
                    shape.type_c4_shape
                ));
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
                if !visibility_verified && let Some(paint) = &text_paint {
                    unverified_text_roles.insert(FamilyThemeMechanismKey::Rule {
                        index: paint.rule_index(),
                        target: ThemeTarget::NodeLabel,
                    });
                }
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
                visible,
                visibility_verified,
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
            let font_size = typography.message_font().measurement_style.font_size;
            let label_visible = text_is_visible(&relation.label, font_size)
                || (font_size > 0.0
                    && relation
                        .techn
                        .as_ref()
                        // The writer adds visible brackets around every nonempty technology,
                        // including one whose prepared contents consist only of break tags.
                        .is_some_and(|text| !text.text.trim().is_empty()));
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
                if !typography.message_font().font_size_is_known()
                    && let Some(paint) = &text_paint
                {
                    unverified_text_roles.insert(FamilyThemeMechanismKey::Rule {
                        index: paint.rule_index(),
                        target: ThemeTarget::EdgeLabel,
                    });
                }
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
                visible: label_visible,
                visibility_verified: typography.message_font().font_size_is_known(),
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
            let fonts = typography.boundary_fonts();
            let visible = text_is_visible(&boundary.label, fonts.name.measurement_style.font_size)
                || boundary.ty.as_ref().is_some_and(|block| {
                    text_is_visible(block, fonts.ty.measurement_style.font_size)
                })
                || boundary.descr.as_ref().is_some_and(|block| {
                    text_is_visible(block, fonts.descr.measurement_style.font_size)
                });
            let visibility_verified = (fonts.name.font_size_is_known()
                || !text_has_visible_content(&boundary.label))
                && boundary.ty.as_ref().is_none_or(|block| {
                    fonts.ty.font_size_is_known() || !text_has_visible_content(block)
                })
                && boundary.descr.as_ref().is_none_or(|block| {
                    fonts.descr.font_size_is_known() || !text_has_visible_content(block)
                });
            let (paint, _) = self.role_paints(
                ThemeTarget::ClusterLabel,
                boundary_text_owned.len() + 1,
                explicit.is_some(),
                work,
            )?;
            if visible {
                boundary_text_owned.push(explicit.is_some());
                record_role_paint(&mut typed_roles, ThemeTarget::ClusterLabel, paint.as_ref());
                if !visibility_verified && let Some(paint) = &paint {
                    unverified_text_roles.insert(FamilyThemeMechanismKey::Rule {
                        index: paint.rule_index(),
                        target: ThemeTarget::ClusterLabel,
                    });
                }
            }
            boundaries.push(Some(C4LabelTerminalPaint {
                text: explicit
                    .or_else(|| paint.as_ref().map(DirectStaticPaint::css))
                    .unwrap_or("#444444")
                    .to_owned(),
                typed_text: visible && self.uses_inherited_fill(paint.as_ref()),
                visible,
                visibility_verified,
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
                title_visible: self.title.is_some() && typography.font_size_px() > 0.0,
                title_visibility_verified: typography.font_size_is_known(),
                elements,
                relations,
                boundaries,
                role_evidence,
                typed_roles,
                unverified_text_roles,
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
                | FamilyThemeMechanism::EffectBinding { target, .. } if is_role_target(target) || target == ThemeTarget::Title)
                })
            }))
        .then(|| C4TextPaintReceipt {
            expected_fill: self.fill_css(),
            expected_title_fill: self.title_fill_css(),
            title_inherits_fill: self.title_fill.is_none(),
            expected_title: self.title.as_deref(),
            expected_title_visible: self.source_paint.get().is_some_and(|source| source.title_visible),
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
                    inherited_title_seen: receipt.title_seen
                        && receipt.expected_title_visible
                        && receipt.title_inherits_fill,
                    text_seen: receipt.text_seen,
                    source_colors_static: receipt.source_colors_static,
                    typed_fill_seen: receipt.typed_fill_seen,
                    roles_complete: receipt.expected_source.is_some_and(|source| {
                        source.elements.len() == receipt.elements_seen.len()
                            && source.boundaries.iter().flatten().count()
                                == receipt.boundaries_seen.len()
                            && source.relations.len() == receipt.relations_seen.len()
                    }),
                })
                .is_ok()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        // Title selection retains its DOM paint even at zero size. Only the prepared root size
        // establishes a visible title domain for applied/unsupported paint evidence.
        let title_visible = self
            .source_paint
            .get()
            .is_some_and(|source| source.title_visible);
        if title_visible {
            evidence.merge_borrowed(&self.title_evidence);
        } else if let Some(theme) = self.theme.as_ref() {
            for route in theme.family_mechanism_routes() {
                if matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Title,
                        ..
                    } | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Title
                    } | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Title,
                        ..
                    }
                ) {
                    evidence.mark_not_applicable(theme.family_mechanism_key(*route));
                }
            }
        }
        if title_visible
            && self.terminal.get().is_some()
            && let Some(fill) = &self.title_fill
        {
            let key = FamilyThemeMechanismKey::Rule {
                index: fill.rule_index(),
                target: ThemeTarget::Title,
            };
            if self
                .source_paint
                .get()
                .is_some_and(|source| source.title_visibility_verified)
            {
                evidence.mark_applied_with_capabilities(key, [fill.capability()]);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
        }
        for pending in &self.pending {
            let key = &pending.key;
            let residual = pending.residual;
            match self.terminal.get() {
                Some(receipt)
                    if !receipt.text_seen
                        || (pending.inherited_fill_only
                            && receipt.source_colors_static
                            && !receipt.inherited_title_seen
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
                    if source.unverified_text_roles.contains(&key) {
                        evidence
                            .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                    } else if let Some(capabilities) = source.typed_roles.get(&key) {
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

    pub(crate) fn record_title(&mut self, title: &str, fill: Option<&str>, visible: bool) {
        self.valid &= !self.title_seen
            && Some(title) == self.expected_title
            && fill == self.expected_title_fill
            && visible == self.expected_title_visible;
        self.title_seen = true;
        self.text_seen |= visible;
        self.typed_fill_seen |= visible && self.title_inherits_fill && self.expected_fill.is_some();
        self.source_colors_static &= !visible
            || self
                .expected_source
                .is_none_or(|source| source.title_visibility_verified);
    }

    /// Source-directed inheritance and browser-dependent colors need their own terminal proof.
    pub(crate) fn record_owned_color(&mut self, color: &str) {
        self.text_seen = true;
        self.source_colors_static =
            self.source_colors_static && crate::mermaid_style::is_supported_css_color_value(color);
    }

    pub(crate) fn record_boundary(&mut self, index: usize, color: &str, visible: bool) {
        let Some(expected) = self
            .expected_source
            .and_then(|source| source.boundaries.get(index)?.as_ref())
        else {
            self.valid = false;
            return;
        };
        self.valid &= expected.text == color
            && expected.visible == visible
            && self.boundaries_seen.insert(index);
        self.typed_fill_seen |= expected.typed_text;
        self.source_colors_static &= !visible || expected.visibility_verified;
    }

    pub(crate) fn record_element(
        &mut self,
        index: usize,
        paint: &C4ElementTerminalPaint,
        visible: bool,
    ) {
        let Some(expected) = self
            .expected_source
            .and_then(|source| source.elements.get(index))
        else {
            self.valid = false;
            return;
        };
        self.valid &= expected.text == paint.text
            && expected.visible == visible
            && expected.background == paint.background
            && expected.border == paint.border
            && self.elements_seen.insert(index);
        self.typed_fill_seen |= expected.typed_text;
        self.source_colors_static &= !visible || expected.visibility_verified;
    }

    pub(crate) fn record_relation(
        &mut self,
        index: usize,
        paint: &C4RelationTerminalPaint,
        visible: bool,
    ) {
        let Some(expected) = self
            .expected_source
            .and_then(|source| source.relations.get(index))
        else {
            self.valid = false;
            return;
        };
        self.valid &= expected.text == paint.text
            && expected.visible == visible
            && expected.line == paint.line
            && expected.marker_fill == paint.marker_fill
            && expected.marker_stroke == paint.marker_stroke
            && self.relations_seen.insert(index);
        self.typed_fill_seen |= expected.typed_text;
        self.source_colors_static &= !visible || expected.visibility_verified;
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

fn text_is_visible(block: &C4TextBlockLayout, used_font_size: f64) -> bool {
    used_font_size > 0.0 && text_has_visible_content(block)
}

fn text_has_visible_content(block: &C4TextBlockLayout) -> bool {
    if let Some(plan) = &block.render_plan {
        // Match the unified writer's prepared words and one-layer textContent decoding.
        // A closing break tag or a decoded entity remains a literal word on this path.
        !block.text.trim().is_empty()
            && plan.rows.iter().flat_map(|row| &row.words).any(|word| {
                !crate::entities::decode_svg_text_content_entities(word)
                    .trim()
                    .is_empty()
            })
    } else {
        crate::text::split_mermaid_common_br_lines(&block.text)
            .into_iter()
            .any(|line| !line.trim().is_empty())
    }
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
    fn text_visibility_uses_prepared_unified_words_or_legacy_common_rows() {
        let mut block = C4TextBlockLayout {
            text: String::new(),
            y: 0.0,
            width: 0.0,
            height: 0.0,
            line_count: 1,
            render_plan: None,
        };
        for text in [
            "",
            "<br/>",
            "</br>",
            "<BR />",
            "<br\u{00a0}/>",
            "</BR\u{feff}/>",
        ] {
            block.text = text.to_owned();
            assert!(!text_is_visible(&block, 16.0), "legacy: {text:?}");
        }
        for text in ["[<br/>]", "&lt;br/&gt;", "literal"] {
            block.text = text.to_owned();
            assert!(text_is_visible(&block, 16.0), "legacy: {text:?}");
            assert!(!text_is_visible(&block, 0.0), "zero-sized legacy: {text:?}");
        }
        for (text, words, visible) in [
            ("</br>", vec!["</br>"], true),
            ("&lt;br/&gt;", vec!["&lt;br/&gt;"], true),
            ("<br/>", vec![], false),
            ("<br/>", vec!["", "\u{00a0}"], false),
        ] {
            block.text = text.to_owned();
            block.render_plan = Some(crate::model::C4TextRenderPlan {
                rows: vec![crate::model::C4TextRowLayout {
                    words: words.into_iter().map(str::to_owned).collect(),
                }],
                bbox_x: 0.0,
                bbox_y: 0.0,
            });
            assert_eq!(text_is_visible(&block, 16.0), visible, "unified: {text:?}");
            assert!(
                !text_is_visible(&block, 0.0),
                "zero-sized unified: {text:?}"
            );
        }
    }

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
        let config = crate::test_support::presentation_config(serde_json::json!({"c4": {
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
        complete.record_element(0, paint, false);
        complete.record_element(1, external, false);
        assert!(
            complete.valid,
            "equal aliases are independent layout occurrences"
        );
        complete.record_element(1, external, false);
        assert!(!complete.valid, "one occurrence cannot be proved twice");
        let mut wrong = C4TextPaintReceipt {
            expected_source: plan.source_paint.get(),
            ..title_receipt()
        };
        wrong.record_element(0, external, false);
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
            expected_title_fill: Some("#123456"),
            title_inherits_fill: true,
            expected_title: Some("Title"),
            expected_title_visible: true,
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
        let mut separate_title = C4TextPaintReceipt {
            expected_title_fill: Some("#987654"),
            title_inherits_fill: false,
            ..title_receipt()
        };
        separate_title.record_title("Title", Some("#987654"), true);
        assert!(separate_title.valid);
        assert!(!separate_title.typed_fill_seen);

        let mut wrong_css = title_receipt();
        wrong_css.record_css(Some("#654321"));
        assert!(!wrong_css.valid);

        let mut wrong_title = title_receipt();
        wrong_title.record_title("Other", Some("#123456"), true);
        assert!(!wrong_title.valid);

        let mut wrong_fill = title_receipt();
        wrong_fill.record_title("Title", None, true);
        assert!(!wrong_fill.valid);

        let mut duplicate = title_receipt();
        duplicate.record_css(Some("#123456"));
        duplicate.record_css(Some("#123456"));
        assert!(!duplicate.valid);

        let mut duplicate = title_receipt();
        duplicate.record_title("Title", Some("#123456"), true);
        duplicate.record_title("Title", Some("#123456"), true);
        assert!(!duplicate.valid);
    }

    #[test]
    fn sealing_requires_css_and_the_expected_title_exactly_once() {
        let plan = C4TextPaintPlan {
            fill: None,
            title_fill: None,
            title: Some("Title".into()),
            evidence: FamilyThemeOutcomes::default(),
            title_evidence: FamilyThemeOutcomes::default(),
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
            complete.record_title("Title", Some("#123456"), true);
            assert_eq!(plan.record_terminal(complete), expected_success);
        }
    }
}
