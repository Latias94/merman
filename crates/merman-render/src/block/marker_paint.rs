use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

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

/// Final marker paint and reference ownership. A definition alone is never a terminal witness.
#[derive(Debug)]
pub(crate) struct BlockMarkerPaintPlan {
    final_references: Box<[[Option<FinalMarkerReference>; 2]]>,
    active: bool,
    definitions: Vec<BlockMarkerDefinition>,
    paths: Vec<Option<MarkerPathExpectation>>,
    references: Vec<MarkerReferenceExpectation>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<usize, BTreeMap<ResolvedStyleProperty, ThemeCapability>>,
    terminal: OnceLock<BlockMarkerPaintReceipt>,
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
struct MarkerPathExpectation {
    id: String,
    geometry_digest: [u8; 32],
    valid_geometry: bool,
    references: [Option<usize>; 2],
}

#[derive(Debug)]
struct MarkerReferenceExpectation {
    kind: BaseEdgeMarkerKind,
    definition: usize,
    fill: Option<DirectPaintExpectation>,
    stroke: Option<DirectPaintExpectation>,
}

impl MarkerReferenceExpectation {
    fn signature(&self) -> MarkerPaintSignature {
        MarkerPaintSignature {
            fill: self.fill.as_ref().map(|paint| paint.css().into()),
            stroke: self.stroke.as_ref().map(|paint| paint.css().into()),
        }
    }
}

#[derive(Debug)]
pub(crate) struct BlockMarkerPaintReceipt {
    definitions: Vec<bool>,
    paths: Vec<bool>,
    valid: bool,
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
        config: &merman_core::MermaidConfig,
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
        config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            final_references: Box::new([]),
            active: false,
            definitions: Vec::new(),
            paths: Vec::new(),
            references: Vec::new(),
            evidence: FamilyThemeEvidence::from_theme(theme),
            pending: BTreeMap::new(),
            terminal: OnceLock::new(),
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
        plan.active = true;
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
        let mut layout_edges = BTreeMap::new();
        let mut duplicate_layout = BTreeSet::new();
        for edge in &layout.edges {
            work.charge(1)?;
            if layout_edges.insert(edge.id.as_str(), edge).is_some() {
                duplicate_layout.insert(edge.id.as_str());
            }
        }
        let geometries = layout
            .shape_geometries
            .iter()
            .map(|geometry| (geometry.id.as_str(), geometry))
            .collect::<std::collections::HashMap<_, _>>();
        let mut semantic_edge_counts = BTreeMap::<&str, usize>::new();
        for edge in &model.edges {
            *semantic_edge_counts.entry(edge.id.as_str()).or_default() += 1;
        }
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
            let unique = semantic_edge_counts.get(edge.id.as_str()) == Some(&1)
                && !duplicate_layout.contains(edge.id.as_str());
            let layout_edge = layout_edges.get(edge.id.as_str());
            let path_data = layout_edge
                .map(|layout_edge| crate::svg::block_edge_path_data(edge, layout_edge, &geometries))
                .unwrap_or_default();
            work.charge(1usize.saturating_add(path_data.len().div_ceil(64)))?;
            let valid_geometry = unique
                && layout_edge.is_some_and(|layout_edge| {
                    layout_edge.from == edge.start
                        && layout_edge.to == edge.end
                        && layout_edge
                            .points
                            .iter()
                            .all(|point| point.x.is_finite() && point.y.is_finite())
                        && geometries.get(edge.start.as_str()).is_some_and(|geometry| {
                            geometry.allocated.x.is_finite()
                                && geometry.allocated.y.is_finite()
                                && geometry.allocated.width.is_finite()
                                && geometry.allocated.height.is_finite()
                        })
                        && geometries.get(edge.end.as_str()).is_some_and(|geometry| {
                            geometry.allocated.x.is_finite()
                                && geometry.allocated.y.is_finite()
                                && geometry.allocated.width.is_finite()
                                && geometry.allocated.height.is_finite()
                        })
                })
                && valid_attachment_path(&path_data);
            let mut path = MarkerPathExpectation {
                id: edge.id.clone(),
                geometry_digest: Sha256::digest(path_data.as_bytes()).into(),
                valid_geometry,
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
                plan.references.push(MarkerReferenceExpectation {
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
                FamilyThemeDisposition::LegacyCompatibility => {
                    observation.incomplete = true;
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
                plan.pending.insert(index, observation.capabilities);
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
                .map(MarkerReferenceExpectation::signature)
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

    pub(crate) fn begin_terminal_receipt(&self) -> Option<BlockMarkerPaintReceipt> {
        self.active.then(|| BlockMarkerPaintReceipt {
            definitions: vec![false; self.definitions.len()],
            paths: vec![false; self.paths.len()],
            valid: true,
        })
    }

    pub(crate) fn observe_definition(
        &self,
        receipt: &mut BlockMarkerPaintReceipt,
        definition_index: usize,
        diagram_id: &str,
        fragment: &str,
    ) {
        let Some(seen) = receipt.definitions.get_mut(definition_index) else {
            receipt.valid = false;
            return;
        };
        receipt.valid &= !*seen;
        *seen = true;
        let expected = &self.definitions[definition_index];
        receipt.valid &= definition_matches(expected, diagram_id, fragment);
    }

    pub(crate) fn observe_path(
        &self,
        receipt: &mut BlockMarkerPaintReceipt,
        edge_index: usize,
        diagram_id: &str,
        fragment: &str,
    ) {
        let Some(path) = self.paths.get(edge_index).and_then(Option::as_ref) else {
            return;
        };
        let Some(seen) = receipt.paths.get_mut(edge_index) else {
            receipt.valid = false;
            return;
        };
        receipt.valid &= !*seen && path.valid_geometry;
        *seen = true;
        let Ok(document) = roxmltree::Document::parse(fragment) else {
            receipt.valid = false;
            return;
        };
        let terminal = document.root_element();
        let prefixed = if diagram_id.is_empty() {
            path.id.clone()
        } else {
            format!("{diagram_id}-{}", path.id)
        };
        let id = if diagram_id.is_empty() {
            prefixed.clone()
        } else {
            format!("{diagram_id}-{prefixed}")
        };
        receipt.valid &= terminal.has_tag_name("path")
            && terminal.children().all(|node| !node.is_element())
            && terminal.attribute("id") == Some(id.as_str())
            && terminal.attribute("data-id") == Some(prefixed.as_str())
            && terminal.attribute("data-edge") == Some("true")
            && terminal.attribute("d").is_some_and(|d| {
                <[u8; 32]>::from(Sha256::digest(d.as_bytes())) == path.geometry_digest
            });
        for (side, reference) in path.references.iter().enumerate() {
            let property = if side == 0 {
                "marker-start"
            } else {
                "marker-end"
            };
            if let Some(reference) = reference {
                let reference = &self.references[*reference];
                let definition = &self.definitions[reference.definition];
                let url = format!("url(#{diagram_id}_block-{})", definition.suffix);
                receipt.valid &= receipt.definitions.get(reference.definition) == Some(&true)
                    && terminal.attribute(property) == Some(url.as_str());
            } else {
                receipt.valid &= terminal.attribute(property).is_none();
            }
        }
    }

    pub(crate) fn record_terminal(&self, receipt: Option<BlockMarkerPaintReceipt>) -> bool {
        let Some(receipt) = receipt else {
            return !self.active;
        };
        let complete = receipt.valid
            && receipt.definitions.iter().all(|seen| *seen)
            && self
                .paths
                .iter()
                .enumerate()
                .all(|(index, path)| path.is_none() || receipt.paths[index]);
        complete && self.terminal.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal.get().is_some() {
            for (&index, properties) in &self.pending {
                evidence.mark_applied_with_capabilities(
                    FamilyThemeMechanismKey::Rule {
                        index,
                        target: ThemeTarget::Marker,
                    },
                    properties.values().copied(),
                );
            }
        }
        evidence
    }
}

fn definition_matches(expected: &BlockMarkerDefinition, diagram_id: &str, fragment: &str) -> bool {
    let Ok(document) = roxmltree::Document::parse(fragment) else {
        return false;
    };
    let marker = document.root_element();
    let geometry = expected.kind.geometry();
    let id = format!("{diagram_id}_block-{}", expected.suffix);
    let class = if expected.kind.cross() {
        "marker cross block"
    } else {
        "marker block"
    };
    if !marker.has_tag_name("marker")
        || marker.attribute("id") != Some(id.as_str())
        || marker.attribute("class") != Some(class)
        || marker.attributes().len() != geometry.attributes.len() + 2
        || !geometry
            .attributes
            .iter()
            .all(|(name, value)| marker.attribute(*name) == Some(*value))
    {
        return false;
    }
    let mut children = marker.children().filter(roxmltree::Node::is_element);
    let Some(shape) = children.next() else {
        return false;
    };
    let style = format!("{}{}", geometry.style, expected.paint_style);
    children.next().is_none()
        && shape.has_tag_name(geometry.shape)
        && shape.children().all(|node| !node.is_element())
        && shape.attribute("class") == Some("arrowMarkerPath")
        && shape.attribute("style") == Some(style.as_str())
        && shape.attributes().len() == geometry.shape_attributes.len() + 2
        && geometry
            .shape_attributes
            .iter()
            .all(|(name, value)| shape.attribute(*name) == Some(*value))
}

fn valid_attachment_path(d: &str) -> bool {
    let mut has_point = false;
    for segment in svgtypes::PathParser::from(d) {
        let finite = match segment {
            Ok(svgtypes::PathSegment::MoveTo { abs: true, x, y })
            | Ok(svgtypes::PathSegment::LineTo { abs: true, x, y }) => {
                x.is_finite() && y.is_finite()
            }
            Ok(svgtypes::PathSegment::CurveTo {
                abs: true,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            }) => [x1, y1, x2, y2, x, y].into_iter().all(f64::is_finite),
            _ => false,
        };
        if !finite {
            return false;
        }
        has_point = true;
    }
    has_point
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

    const DEFINITION: &str = r##"<marker id="test_block-pointEnd" class="marker block" viewBox="0 0 10 10" refX="5" refY="5" markerUnits="userSpaceOnUse" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z" class="arrowMarkerPath" style="stroke-width: 1; stroke-dasharray: 1, 0;fill:#123456;stroke:#abcdef;"/></marker>"##;
    const PATH: &str = r##"<path id="test-test-edge" data-id="test-edge" data-edge="true" d="M0,0L20,0" marker-end="url(#test_block-pointEnd)"/>"##;

    fn paint(rule_index: usize, css: &str) -> DirectPaintExpectation {
        DirectPaintExpectation::new(rule_index, css, ThemeCapability::SolidPaint)
    }

    fn fixture_plan() -> BlockMarkerPaintPlan {
        BlockMarkerPaintPlan {
            final_references: Box::new([]),
            active: true,
            definitions: vec![BlockMarkerDefinition {
                kind: BaseEdgeMarkerKind::PointEnd,
                suffix: "pointEnd".to_string(),
                paint_style: "fill:#123456;stroke:#abcdef;".to_string(),
            }],
            paths: vec![Some(MarkerPathExpectation {
                id: "edge".to_string(),
                geometry_digest: Sha256::digest(b"M0,0L20,0").into(),
                valid_geometry: true,
                references: [None, Some(0)],
            })],
            references: vec![MarkerReferenceExpectation {
                kind: BaseEdgeMarkerKind::PointEnd,
                definition: 0,
                fill: Some(paint(0, "#123456")),
                stroke: Some(paint(1, "#abcdef")),
            }],
            evidence: FamilyThemeEvidence::from_theme(None),
            pending: BTreeMap::new(),
            terminal: OnceLock::new(),
        }
    }

    #[test]
    fn marker_receipt_requires_exact_definition_and_actual_reference_once() {
        let plan = fixture_plan();
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_definition(&mut receipt, 0, "test", DEFINITION);
        assert!(
            !plan.record_terminal(Some(receipt)),
            "definitions alone have no terminal witness"
        );
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_path(&mut receipt, 0, "test", PATH);
        assert!(
            !plan.record_terminal(Some(receipt)),
            "references cannot certify missing definitions"
        );
        for duplicate_definition in [false, true] {
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            plan.observe_definition(&mut receipt, 0, "test", DEFINITION);
            plan.observe_path(&mut receipt, 0, "test", PATH);
            if duplicate_definition {
                plan.observe_definition(&mut receipt, 0, "test", DEFINITION);
            } else {
                plan.observe_path(&mut receipt, 0, "test", PATH);
            }
            assert!(!plan.record_terminal(Some(receipt)));
        }
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_definition(&mut receipt, 0, "test", DEFINITION);
        plan.observe_path(&mut receipt, 0, "test", PATH);
        assert!(plan.record_terminal(Some(receipt)));
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_definition(&mut receipt, 0, "test", DEFINITION);
        plan.observe_path(&mut receipt, 0, "test", PATH);
        assert!(!plan.record_terminal(Some(receipt)), "seal only once");
    }

    #[test]
    fn marker_receipt_rejects_wrong_identity_geometry_and_independent_paints() {
        let plan = fixture_plan();
        for mutation in [
            DEFINITION.replace("test_block-pointEnd", "test_block-pointStart"),
            DEFINITION.replace("M 0 0 L 10 5 L 0 10 z", "M 0 0 L 2 5 L 0 10 z"),
            DEFINITION.replace("viewBox=\"0 0 10 10\"", "viewBox=\"0 0 20 20\""),
            DEFINITION.replace("refX=\"5\"", "refX=\"4\""),
            DEFINITION.replace("fill:#123456", "fill:#abcdef"),
            DEFINITION.replace("stroke:#abcdef", "stroke:#123456"),
            DEFINITION.replace("<path", "<polygon"),
            DEFINITION.replace("</marker>", "<circle/></marker>"),
        ] {
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            plan.observe_definition(&mut receipt, 0, "test", &mutation);
            plan.observe_path(&mut receipt, 0, "test", PATH);
            assert!(!plan.record_terminal(Some(receipt)), "{mutation}");
        }
        for mutation in [
            PATH.replace("test-test-edge", "test-test-other"),
            PATH.replace("M0,0L20,0", "M0,0L30,0"),
            PATH.replace("pointEnd", "pointStart"),
            PATH.replace("marker-end", "marker-start"),
            PATH.replace(r#" marker-end="url(#test_block-pointEnd)""#, ""),
        ] {
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            plan.observe_definition(&mut receipt, 0, "test", DEFINITION);
            plan.observe_path(&mut receipt, 0, "test", &mutation);
            assert!(!plan.record_terminal(Some(receipt)), "{mutation}");
        }
        let mut invalid = fixture_plan();
        invalid.paths[0].as_mut().unwrap().valid_geometry = false;
        let mut receipt = invalid.begin_terminal_receipt().unwrap();
        invalid.observe_definition(&mut receipt, 0, "test", DEFINITION);
        invalid.observe_path(&mut receipt, 0, "test", PATH);
        assert!(!invalid.record_terminal(Some(receipt)));
    }

    #[test]
    fn marker_definitions_split_only_for_different_final_paints() {
        for divergent in [false, true] {
            let mut plan = fixture_plan();
            plan.references.push(MarkerReferenceExpectation {
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
