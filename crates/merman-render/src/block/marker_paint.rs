use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ResolvedStyleProperty, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticSelectorDomain, FamilyThemeEvidence,
    FamilyThemeResidualReason, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::BlockDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::svg::BaseEdgeMarkerKind;

/// Final marker paint and references consumed together by the Block SVG writer.
#[derive(Debug)]
pub(crate) struct BlockMarkerPaintPlan {
    final_references: Box<[[Option<FinalMarkerReference>; 2]]>,
    definitions: Vec<BlockMarkerDefinition>,
    paths: Vec<Option<MarkerPathBinding>>,
    references: Vec<MarkerReferenceBinding>,
    evidence: FamilyThemeEvidence,
}

#[derive(Debug, Clone, Copy)]
enum FinalMarkerReference {
    Base(BaseEdgeMarkerKind),
    Definition(usize),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
struct MarkerPaintSignature {
    fill: Option<Box<str>>,
    stroke: Option<Box<str>>,
}

impl MarkerPaintSignature {
    fn style(&self) -> String {
        let mut style = String::new();
        if let Some(fill) = &self.fill {
            style.push_str("fill:");
            style.push_str(fill);
            style.push(';');
        }
        if let Some(stroke) = &self.stroke {
            style.push_str("stroke:");
            style.push_str(stroke);
            style.push(';');
        }
        style
    }
}

#[derive(Debug)]
pub(crate) struct BlockMarkerDefinition {
    kind: BaseEdgeMarkerKind,
    suffix: String,
    paint_style: String,
}

impl BlockMarkerDefinition {
    pub(crate) const fn kind(&self) -> BaseEdgeMarkerKind {
        self.kind
    }
    pub(crate) fn suffix(&self) -> &str {
        &self.suffix
    }
    pub(crate) fn paint_style(&self) -> &str {
        &self.paint_style
    }
}

#[derive(Debug)]
struct MarkerPathBinding {
    references: [Option<usize>; 2],
}

#[derive(Debug)]
struct MarkerReferenceBinding {
    kind: BaseEdgeMarkerKind,
    definition: usize,
    fill: Option<DirectPaintExpectation>,
    stroke: Option<DirectPaintExpectation>,
}

impl MarkerReferenceBinding {
    fn signature(&self) -> MarkerPaintSignature {
        MarkerPaintSignature {
            fill: self.fill.as_ref().map(|paint| paint.css().into()),
            stroke: self.stroke.as_ref().map(|paint| paint.css().into()),
        }
    }
}

#[derive(Default)]
struct RuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeMap<ResolvedStyleProperty, ThemeCapability>,
}

impl BlockMarkerPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::presentation::PresentationConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::resolve_paint(theme, config, model, layout, work)?;
        work.charge(model.edges.len())?;
        plan.final_references = model
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                [
                    (true, edge.arrow_type_start.as_deref()),
                    (false, edge.arrow_type_end.as_deref()),
                ]
                .map(|(start, arrow)| {
                    BaseEdgeMarkerKind::from_arrow(arrow, start).map(|kind| {
                        plan.reference_definition(index, start).map_or(
                            FinalMarkerReference::Base(kind),
                            FinalMarkerReference::Definition,
                        )
                    })
                })
            })
            .collect();
        Ok(plan)
    }

    fn resolve_paint(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::presentation::PresentationConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        _layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            final_references: Box::new([]),
            definitions: Vec::new(),
            paths: Vec::new(),
            references: Vec::new(),
            evidence: FamilyThemeEvidence::from_theme(theme),
        };
        let Some(theme) = theme else { return Ok(plan) };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Marker,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Marker
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Marker,
                    ..
                }
            )
        }) {
            return Ok(plan);
        }
        let config_fill = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.arrowheadColor",
        );
        let config_stroke = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.lineColor",
        );
        let mut marker_properties = BTreeSet::new();
        let mut cross_properties = BTreeSet::new();
        let mut block_properties = BTreeSet::new();
        for class in model.class_defs.values() {
            work.charge(1)?;
            let properties = match class.id.as_str() {
                "marker" => &mut marker_properties,
                "cross" => &mut cross_properties,
                "block" => &mut block_properties,
                _ => continue,
            };
            for raw in &class.styles {
                work.charge(1usize.saturating_add(raw.len().div_ceil(64)))?;
                if let Some(declaration) = crate::mermaid_style::parse_style_declaration(raw) {
                    properties.insert(declaration.property().to_string());
                }
            }
        }
        let has_ordinal = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Marker && rule.ordinal().is_some());
        let static_style =
            theme.style_with_work_meter(ThemeTarget::Marker, ThemeVariant::Default, None, work)?;
        let mut winners = BTreeSet::new();
        let mut capabilities = BTreeMap::new();
        let mut owned_fills = Vec::new();
        let mut owned_strokes = Vec::new();
        for edge in &model.edges {
            work.charge(1)?;
            let kinds = [
                BaseEdgeMarkerKind::from_arrow(edge.arrow_type_start.as_deref(), true),
                BaseEdgeMarkerKind::from_arrow(edge.arrow_type_end.as_deref(), false),
            ];
            if kinds.iter().all(Option::is_none) {
                plan.paths.push(None);
                continue;
            }
            let mut path = MarkerPathBinding {
                references: [None; 2],
            };
            for (side, kind) in kinds.into_iter().enumerate() {
                let Some(kind) = kind else { continue };
                let reference_index = plan.references.len();
                path.references[side] = Some(reference_index);
                let reference_owned = block_properties.contains("marker")
                    || block_properties.contains(if side == 0 {
                        "marker-start"
                    } else {
                        "marker-end"
                    })
                    || block_properties.contains("all");
                let source_owns = |property| {
                    reference_owned
                        || marker_properties.contains(property)
                        || marker_properties.contains("all")
                        || block_properties.contains(property)
                        || (kind.cross()
                            && (cross_properties.contains(property)
                                || cross_properties.contains("all")))
                };
                let fill_owned = config_fill || source_owns("fill") || kind.cross();
                let stroke_owned = config_stroke || source_owns("stroke");
                owned_fills.push(fill_owned);
                owned_strokes.push(stroke_owned);
                let ordinal_style;
                let style = if has_ordinal {
                    ordinal_style = theme.style_with_work_meter(
                        ThemeTarget::Marker,
                        ThemeVariant::Default,
                        Some(reference_index + 1),
                        work,
                    )?;
                    &ordinal_style
                } else {
                    &static_style
                };
                winners.extend(
                    style
                        .winner_rule_properties()
                        .filter_map(|(property, origin)| {
                            (!(reference_owned
                                || fill_owned && property == ResolvedStyleProperty::Fill
                                || stroke_owned && property == ResolvedStyleProperty::Stroke))
                                .then_some((origin.rule_index(), property))
                        }),
                );
                let fill = (!fill_owned)
                    .then(|| {
                        resolve_direct_static_fill(
                            theme,
                            style,
                            &[ThemeTarget::Marker],
                            DirectStaticSelectorDomain::Default,
                        )
                    })
                    .flatten()
                    .map(DirectPaintExpectation::from_paint);
                let stroke = (!stroke_owned)
                    .then(|| {
                        resolve_direct_static_stroke(
                            theme,
                            style,
                            &[ThemeTarget::Marker],
                            DirectStaticSelectorDomain::Default,
                        )
                    })
                    .flatten()
                    .map(DirectPaintExpectation::from_paint);
                for (property, paint) in [
                    (ResolvedStyleProperty::Fill, fill.as_ref()),
                    (ResolvedStyleProperty::Stroke, stroke.as_ref()),
                ] {
                    if let Some(paint) = paint {
                        capabilities.insert((paint.rule_index(), property), paint.capability());
                    }
                }
                plan.references.push(MarkerReferenceBinding {
                    kind,
                    definition: 0,
                    fill,
                    stroke,
                });
            }
            plan.paths.push(Some(path));
        }
        plan.assign_definitions();
        let mut observations = BTreeMap::<usize, RuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Marker,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            let observation = observations.entry(rule_index).or_default();
            let property = resolved_style_property_for_facet(facet);
            if !winners.contains(&(rule_index, property)) {
                continue;
            }
            observation.applicable = true;
            match route.disposition() {
                FamilyThemeDisposition::TypedAdapter => {
                    if let Some(capability) = capabilities.get(&(rule_index, property)) {
                        observation.capabilities.insert(property, *capability);
                    } else {
                        observation.incomplete = true;
                    }
                }
                FamilyThemeDisposition::Unsupported => {
                    observation
                        .residual
                        .get_or_insert(unsupported_residual_for_facet(facet));
                }
            }
        }
        for (index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Marker,
            };
            if !observation.applicable {
                plan.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if !observation.incomplete && !observation.capabilities.is_empty() {
                plan.evidence
                    .mark_applied_with_capabilities(key, observation.capabilities.into_values());
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Marker,
                TerminalVariantDomain::uniform(plan.references.len(), ThemeVariant::Default),
            )
            .with_source_owned_fill(&owned_fills)
            .with_source_owned_stroke(&owned_strokes)],
            work,
        )?;
        Ok(plan)
    }

    fn assign_definitions(&mut self) {
        self.definitions = BaseEdgeMarkerKind::ALL
            .into_iter()
            .map(|kind| BlockMarkerDefinition {
                kind,
                suffix: kind.suffix().to_string(),
                paint_style: String::new(),
            })
            .collect();
        // Partition references by final paint, not by rule id: equal paint can safely share
        // geometry while unsupported ordinal winners keep their unmodified definition.
        for (base_index, kind) in BaseEdgeMarkerKind::ALL.into_iter().enumerate() {
            let mut signatures = self
                .references
                .iter()
                .filter(|reference| reference.kind == kind)
                .map(MarkerReferenceBinding::signature)
                .collect::<BTreeSet<_>>();
            let Some(base_signature) = signatures.pop_first() else {
                continue;
            };
            self.definitions[base_index].paint_style = base_signature.style();
            let mut assignments = BTreeMap::from([(base_signature, base_index)]);
            for signature in signatures {
                let index = self.definitions.len();
                self.definitions.push(BlockMarkerDefinition {
                    kind,
                    suffix: format!("{}-theme-{index}", kind.suffix()),
                    paint_style: signature.style(),
                });
                assignments.insert(signature, index);
            }
            for reference in self
                .references
                .iter_mut()
                .filter(|reference| reference.kind == kind)
            {
                reference.definition = assignments[&reference.signature()];
            }
        }
    }

    pub(crate) fn definitions(&self) -> &[BlockMarkerDefinition] {
        &self.definitions
    }

    fn reference_definition(&self, edge_index: usize, start: bool) -> Option<usize> {
        let path = self.paths.get(edge_index)?.as_ref()?;
        let reference = &self.references[path.references[usize::from(!start)]?];
        Some(reference.definition)
    }

    pub(crate) fn final_reference(
        &self,
        edge_index: usize,
        start: bool,
    ) -> Option<(BaseEdgeMarkerKind, &str)> {
        match self.final_references.get(edge_index)?[usize::from(!start)]? {
            FinalMarkerReference::Base(kind) => Some((kind, kind.suffix())),
            FinalMarkerReference::Definition(index) => {
                let definition = &self.definitions[index];
                Some((definition.kind(), definition.suffix()))
            }
        }
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        self.evidence.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unthemed_reference_binding_preserves_unknown_arrow_asymmetry() {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "block-beta\nA --> B\n",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let merman_core::RenderSemanticModel::Block(model) = parsed.model() else {
            panic!("Block model")
        };
        let layout = crate::block::layout_block_diagram_typed(
            model,
            parsed.metadata().effective_config.as_value(),
            &crate::text::DeterministicTextMeasurer::default(),
        )
        .unwrap();
        let mut model = model.clone();
        model.edges[0].arrow_type_start = Some("unknown".into());
        model.edges[0].arrow_type_end = Some("unknown".into());
        let work = OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive());
        let plan = BlockMarkerPaintPlan::resolve(
            None,
            &parsed.metadata().effective_config,
            &model,
            &layout,
            &work,
        )
        .unwrap();
        assert_eq!(plan.final_reference(0, true), None);
        assert_eq!(
            plan.final_reference(0, false),
            Some((BaseEdgeMarkerKind::PointEnd, "pointEnd"))
        );
        assert_eq!(plan.final_references.len(), model.edges.len());
    }

    fn paint(rule_index: usize, css: &str) -> DirectPaintExpectation {
        DirectPaintExpectation::new(rule_index, css, ThemeCapability::SolidPaint)
    }

    fn fixture_plan() -> BlockMarkerPaintPlan {
        BlockMarkerPaintPlan {
            final_references: Box::new([]),
            definitions: Vec::new(),
            paths: Vec::new(),
            references: vec![MarkerReferenceBinding {
                kind: BaseEdgeMarkerKind::PointEnd,
                definition: 0,
                fill: Some(paint(0, "#123456")),
                stroke: Some(paint(1, "#abcdef")),
            }],
            evidence: FamilyThemeEvidence::from_theme(None),
        }
    }

    #[test]
    fn marker_definitions_split_only_for_different_final_paints() {
        for divergent in [false, true] {
            let mut plan = fixture_plan();
            plan.references.push(MarkerReferenceBinding {
                kind: BaseEdgeMarkerKind::PointEnd,
                definition: 0,
                fill: if divergent {
                    None
                } else {
                    Some(paint(2, "#123456"))
                },
                stroke: Some(paint(1, "#abcdef")),
            });
            plan.assign_definitions();
            assert_eq!(plan.definitions.len(), if divergent { 13 } else { 12 });
            assert_eq!(
                plan.references[0].definition != plan.references[1].definition,
                divergent
            );
            assert!(
                plan.definitions
                    .iter()
                    .take(12)
                    .zip(BaseEdgeMarkerKind::ALL)
                    .all(|(definition, kind)| definition.suffix == kind.suffix())
            );
            if divergent {
                let baseline = &plan.definitions[plan.references[1].definition];
                assert!(!baseline.paint_style.contains("fill:"));
                assert!(baseline.paint_style.contains("stroke:#abcdef;"));
            }
        }
    }
}
