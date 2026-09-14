use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget,
    ThemeVariant,
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

/// Block owns edge paint at the visible path, independently of shared marker definitions.
#[derive(Debug)]
pub(crate) struct BlockEdgePaintPlan {
    expectations: Vec<Option<BlockEdgeExpectation>>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    terminal: OnceLock<BlockEdgePaintReceipt>,
}

#[derive(Debug)]
struct BlockEdgeExpectation {
    id: String,
    paint: DirectPaintExpectation,
    valid_geometry: bool,
    path_data: String,
}

#[derive(Debug)]
pub(crate) struct BlockEdgePaintReceipt {
    seen: Vec<bool>,
    valid: bool,
}

#[derive(Default)]
struct RuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

impl BlockEdgePaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            expectations: Vec::new(),
            evidence: FamilyThemeEvidence::from_theme(theme),
            pending: BTreeMap::new(),
            terminal: OnceLock::new(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Edge,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Edge
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Edge,
                    ..
                }
            )
        }) {
            return Ok(plan);
        }
        let config_owned = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.lineColor",
        );
        // The source class rule `.block > *` matches paths through their actual parent group.
        let source_owned = config_owned
            || model.class_defs.values().any(|class| {
                class.id == "block"
                    && class.styles.iter().any(|raw| {
                        crate::mermaid_style::parse_style_declaration(raw)
                            .is_some_and(|declaration| declaration.property() == "stroke")
                    })
            });
        let has_ordinal = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Edge && rule.ordinal().is_some());
        let static_style =
            theme.style_with_work_meter(ThemeTarget::Edge, ThemeVariant::Default, None, work)?;
        let mut layout_by_id = BTreeMap::new();
        let mut duplicate_layout_ids = BTreeSet::new();
        for edge in &layout.edges {
            work.charge(1)?;
            if layout_by_id.insert(edge.id.as_str(), edge).is_some() {
                duplicate_layout_ids.insert(edge.id.as_str());
            }
        }
        let geometries = layout
            .shape_geometries
            .iter()
            .map(|geometry| (geometry.id.as_str(), geometry))
            .collect::<std::collections::HashMap<_, _>>();
        let mut semantic_ids = BTreeSet::new();
        let mut winners = BTreeSet::new();
        let mut capabilities = BTreeMap::new();
        let mut source_owned_fill = Vec::with_capacity(model.edges.len());
        let mut source_owned_stroke = Vec::with_capacity(model.edges.len());
        for (index, edge) in model.edges.iter().enumerate() {
            work.charge(1)?;
            let dynamic_style;
            let style = if has_ordinal {
                dynamic_style = theme.style_with_work_meter(
                    ThemeTarget::Edge,
                    ThemeVariant::Default,
                    Some(index + 1),
                    work,
                )?;
                &dynamic_style
            } else {
                &static_style
            };
            // Fill is the historical line-paint fallback. Clear and unsupported stroke
            // values still own the terminal, so neither can reactivate the fill winner.
            let fill_fallback = matches!(
                style.stroke_resolution().specified(),
                Specified::Unspecified
            );
            let unique = semantic_ids.insert(edge.id.as_str())
                && !duplicate_layout_ids.contains(edge.id.as_str());
            let valid_geometry = unique
                && layout_by_id
                    .get(edge.id.as_str())
                    .is_some_and(|layout_edge| {
                        layout_edge.from == edge.start
                            && layout_edge.to == edge.end
                            && (layout_edge.points.len() >= 2
                                || (geometries.contains_key(edge.start.as_str())
                                    && geometries.contains_key(edge.end.as_str())))
                            && layout_edge
                                .points
                                .iter()
                                .all(|point| point.x.is_finite() && point.y.is_finite())
                    });
            let path_data = layout_by_id
                .get(edge.id.as_str())
                .map(|layout_edge| crate::svg::block_edge_path_data(edge, layout_edge, &geometries))
                .unwrap_or_default();
            work.charge(1usize.saturating_add(path_data.len().div_ceil(64)))?;
            let visibility = path_visibility(&path_data);
            // A valid degenerate path (for example A --> A) has no edge-paint
            // terminal. Keep its receipt to distinguish it from missing or corrupt geometry.
            let invisible = valid_geometry && visibility == Some(false);
            source_owned_stroke.push(source_owned || invisible);
            source_owned_fill.push(source_owned || !fill_fallback || invisible);
            winners.extend(
                style
                    .winner_rule_properties()
                    .filter_map(|(property, origin)| {
                        (!invisible
                            && !(property == ResolvedStyleProperty::Fill
                                && (!fill_fallback || source_owned))
                            && !(property == ResolvedStyleProperty::Stroke && source_owned))
                            .then_some((origin.rule_index(), property))
                    }),
            );
            let paint = if source_owned {
                None
            } else if fill_fallback {
                resolve_direct_static_fill(
                    theme,
                    style,
                    &[ThemeTarget::Edge],
                    DirectStaticSelectorDomain::Default,
                )
            } else {
                resolve_direct_static_stroke(
                    theme,
                    style,
                    &[ThemeTarget::Edge],
                    DirectStaticSelectorDomain::Default,
                )
            }
            .map(DirectPaintExpectation::from_paint);
            if let Some(paint) = &paint
                && !invisible
            {
                capabilities.insert(
                    (
                        paint.rule_index(),
                        if fill_fallback {
                            ResolvedStyleProperty::Fill
                        } else {
                            ResolvedStyleProperty::Stroke
                        },
                    ),
                    paint.capability(),
                );
            }
            if let Some(paint) = paint {
                plan.expectations.push(Some(BlockEdgeExpectation {
                    id: edge.id.clone(),
                    paint,
                    valid_geometry: valid_geometry && visibility.is_some(),
                    path_data,
                }));
            } else {
                plan.expectations.push(None);
            }
        }
        let mut observations = BTreeMap::<usize, RuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Edge,
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
                    if matches!(
                        facet,
                        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
                    ) && let Some(capability) = capabilities.get(&(rule_index, property))
                    {
                        observation.capabilities.insert(*capability);
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
                target: ThemeTarget::Edge,
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
                ThemeTarget::Edge,
                TerminalVariantDomain::uniform(model.edges.len(), ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)
            .with_source_owned_stroke(&source_owned_stroke)],
            work,
        )?;
        Ok(plan)
    }

    pub(crate) fn color(&self, edge_index: usize) -> Option<&str> {
        self.expectations
            .get(edge_index)?
            .as_ref()
            .map(|expected| expected.paint.css())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<BlockEdgePaintReceipt> {
        self.expectations
            .iter()
            .any(Option::is_some)
            .then(|| BlockEdgePaintReceipt {
                seen: vec![false; self.expectations.len()],
                valid: true,
            })
    }

    /// Inspect only the checkpointed SVG fragment produced for this semantic edge.
    pub(crate) fn observe_path(
        &self,
        receipt: &mut BlockEdgePaintReceipt,
        edge_index: usize,
        diagram_id: &str,
        fragment: &str,
    ) {
        let Some(seen) = receipt.seen.get_mut(edge_index) else {
            receipt.valid = false;
            return;
        };
        receipt.valid &= !*seen;
        *seen = true;
        let Some(expected) = self.expectations[edge_index].as_ref() else {
            return;
        };
        receipt.valid &= expected.valid_geometry && path_matches(expected, diagram_id, fragment);
    }

    pub(crate) fn record_terminal(&self, receipt: Option<BlockEdgePaintReceipt>) -> bool {
        let Some(receipt) = receipt else {
            return self.expectations.iter().all(Option::is_none);
        };
        let complete = receipt.valid
            && self
                .expectations
                .iter()
                .enumerate()
                .all(|(index, expected)| {
                    expected.is_none() || receipt.seen.get(index) == Some(&true)
                });
        complete && self.terminal.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal.get().is_some() {
            for (&index, capabilities) in &self.pending {
                evidence.mark_applied_with_capabilities(
                    FamilyThemeMechanismKey::Rule {
                        index,
                        target: ThemeTarget::Edge,
                    },
                    capabilities.iter().copied(),
                );
            }
        }
        evidence
    }
}

fn path_matches(expected: &BlockEdgeExpectation, diagram_id: &str, fragment: &str) -> bool {
    let Ok(document) = roxmltree::Document::parse(fragment) else {
        return false;
    };
    let path = document.root_element();
    let prefixed_id = if diagram_id.is_empty() {
        expected.id.clone()
    } else {
        format!("{diagram_id}-{}", expected.id)
    };
    let path_id = if diagram_id.is_empty() {
        prefixed_id.clone()
    } else {
        format!("{diagram_id}-{prefixed_id}")
    };
    if !path.has_tag_name("path")
        || path.children().any(|node| node.is_element())
        || path.attribute("id") != Some(path_id.as_str())
        || path.attribute("data-id") != Some(prefixed_id.as_str())
        || path.attribute("data-edge") != Some("true")
    {
        return false;
    }
    let style = path.attribute("style").unwrap_or("");
    let property = |name| {
        style
            .split(';')
            .filter_map(|raw| raw.split_once(':'))
            .filter(|(key, _)| key.trim() == name)
            .map(|(_, value)| value.trim())
            .next_back()
    };
    property("stroke") == Some(expected.paint.css())
        && property("fill") == Some("none")
        && path.attribute("d") == Some(expected.path_data.as_str())
}

fn path_visibility(d: &str) -> Option<bool> {
    let mut current = None;
    let mut visible = false;
    // Block's basis writer emits absolute move, line, and cubic commands.
    for segment in svgtypes::PathParser::from(d) {
        let (point, drawing) = match segment {
            Ok(svgtypes::PathSegment::MoveTo { abs: true, x, y }) => ((x, y), false),
            Ok(svgtypes::PathSegment::LineTo { abs: true, x, y }) => ((x, y), true),
            Ok(svgtypes::PathSegment::CurveTo {
                abs: true,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            }) if [x1, y1, x2, y2].into_iter().all(f64::is_finite) => {
                visible |=
                    current.is_some_and(|previous| previous != (x1, y1) || previous != (x2, y2));
                ((x, y), true)
            }
            _ => return None,
        };
        if !point.0.is_finite() || !point.1.is_finite() {
            return None;
        }
        visible |= drawing && current.is_some_and(|previous| previous != point);
        current = Some(point);
    }
    current.map(|_| visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt_plan() -> BlockEdgePaintPlan {
        BlockEdgePaintPlan {
            expectations: vec![Some(BlockEdgeExpectation {
                id: "edge-1".to_string(),
                paint: DirectPaintExpectation::new(0, "#123456", ThemeCapability::SolidPaint),
                valid_geometry: true,
                path_data: "M0,0L10,0".to_string(),
            })],
            evidence: FamilyThemeEvidence::from_theme(None),
            pending: BTreeMap::new(),
            terminal: OnceLock::new(),
        }
    }

    const PATH: &str = r##"<path id="test-test-edge-1" data-id="test-edge-1" data-edge="true" d="M0,0L10,0" style="stroke:#123456;fill:none;"/>"##;

    #[test]
    fn block_edge_receipt_rejects_missing_duplicate_and_wrong_terminal() {
        let plan = receipt_plan();
        assert!(!plan.record_terminal(plan.begin_terminal_receipt()));
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_path(&mut receipt, 0, "test", PATH);
        plan.observe_path(&mut receipt, 0, "test", PATH);
        assert!(!plan.record_terminal(Some(receipt)));
        for fragment in [
            PATH.replace("edge-1", "edge-2"),
            PATH.replace("<path", "<rect"),
            PATH.replace("#123456", "#abcdef"),
            PATH.replace("M0,0L10,0", "M0,0L0,0"),
            PATH.replace("M0,0L10,0", "M0,0L20,0"),
            PATH.replace("M0,0L10,0", "MNaN,0L10,0"),
            PATH.replace("fill:none", "fill:red"),
        ] {
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            plan.observe_path(&mut receipt, 0, "test", &fragment);
            assert!(!plan.record_terminal(Some(receipt)), "{fragment}");
        }
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_path(&mut receipt, 0, "test", PATH);
        assert!(plan.record_terminal(Some(receipt)));
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_path(&mut receipt, 0, "test", PATH);
        assert!(
            !plan.record_terminal(Some(receipt)),
            "a receipt seals exactly once"
        );
    }

    #[test]
    fn block_edge_degenerate_path_keeps_exact_terminal_validation() {
        assert_eq!(path_visibility("M0,0L0,0"), Some(false));
        assert_eq!(path_visibility("M0,0L10,0"), Some(true));
        assert_eq!(path_visibility("M0,0C10,0,10,10,0,0"), Some(true));
        assert_eq!(path_visibility(""), None);
        assert_eq!(path_visibility("MNaN,0L0,0"), None);
        let mut plan = receipt_plan();
        plan.expectations[0].as_mut().unwrap().path_data = "M0,0L0,0".to_string();
        assert!(!plan.record_terminal(plan.begin_terminal_receipt()));
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_path(&mut receipt, 0, "test", PATH);
        assert!(!plan.record_terminal(Some(receipt)));
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_path(
            &mut receipt,
            0,
            "test",
            &PATH.replace("M0,0L10,0", "M0,0L0,0"),
        );
        assert!(plan.record_terminal(Some(receipt)));
    }

    #[test]
    fn block_edge_receipt_rejects_missing_layout_geometry() {
        let mut plan = receipt_plan();
        plan.expectations[0].as_mut().unwrap().valid_geometry = false;
        let mut receipt = plan.begin_terminal_receipt().unwrap();
        plan.observe_path(&mut receipt, 0, "test", PATH);
        assert!(!plan.record_terminal(Some(receipt)));
    }
}
