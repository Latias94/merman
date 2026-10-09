//! Text-only filter plans share measured paint bounds with the viewport and label writers.

use rustc_hash::{FxHashMap, FxHashSet};

use super::*;
use crate::diagram_theme::{
    EffectOutsets, MaterializedShadowEffect, SvgShadowEffect, ThemeResourcePolicy, ThemeTarget,
};
use crate::flowchart::{FlowchartShape, FlowchartSvgLabelOwner};

#[derive(Debug)]
pub(super) struct PreparedLabelEffect {
    effect: SvgShadowEffect,
    shadow: MaterializedShadowEffect,
    // Both preparation and emission use the existing label translation, not its ink center.
    translation: (f64, f64),
    absolute_translation: (f64, f64),
}

impl PreparedLabelEffect {
    pub(super) fn open(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        id: &str,
        translation: (f64, f64),
    ) -> bool {
        if !self.matches_translation(translation) {
            return false;
        }
        let reference = crate::svg::parity::shadow::write_theme_shadow_application(
            out,
            id,
            &self.effect,
            self.shadow.region(),
        );
        let _ = write!(out, r#"<g filter="{}">"#, escape_attr(&reference));
        true
    }

    pub(super) fn matches_translation(&self, translation: (f64, f64)) -> bool {
        self.translation == translation
    }

    pub(super) fn html_reference(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        id: &str,
    ) -> String {
        crate::svg::parity::shadow::write_theme_shadow_application(
            out,
            id,
            &self.effect,
            self.shadow.region(),
        )
    }

    pub(super) fn record(&self, ctx: &FlowchartRenderCtx<'_>, id: &str) {
        ctx.effect_evidence
            .record_application(&self.effect, id, self.shadow.region());
    }

    pub(super) fn close(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
        id: &str,
    ) {
        out.push_str("</g>");
        self.record(ctx, id);
    }
}

#[derive(Debug, Default)]
pub(super) struct FlowchartLabelEffects {
    nodes: FxHashMap<String, PreparedLabelEffect>,
    // A verified Clear has no filter application to count, but still needs writer evidence.
    cleared_html_nodes: FxHashSet<String>,
    cleared_html_edges: FxHashSet<crate::flowchart::FlowchartEdgeKey>,
    edges: FxHashMap<crate::flowchart::FlowchartEdgeKey, PreparedLabelEffect>,
}

impl FlowchartLabelEffects {
    pub(super) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        render_edges: &[super::render_input::FlowchartRenderEdge<'_>],
        edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
        resources: &ThemeResourcePolicy,
    ) -> crate::Result<Self> {
        let mut plan = Self::default();
        let (Some(theme), Some(sidecar)) = (ctx.resolved_theme, ctx.svg_label_sidecar) else {
            return Ok(plan);
        };
        if ctx.swimlane_direction.is_some() || flowchart_config_look(ctx.config) != "classic" {
            return Ok(plan);
        }
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                crate::diagram_theme::FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
                    ..
                } | crate::diagram_theme::FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
                    facet: crate::diagram_theme::FamilyThemeRuleFacet::Effect,
                    ..
                }
            )
        }) {
            return Ok(plan);
        }
        let weights = sidecar.label_weights();
        if super::style::label_shadow_structural_styles_are_bounded(
            ctx.class_defs,
            &[
                "root",
                "nodes",
                "node",
                "label",
                "nodeLabel",
                "text-outer-tspan",
                "row",
            ],
        ) {
            for id in hierarchy.rendered_node_ids() {
                let Some(info) = super::render::node::helpers::resolve_node_render_info(ctx, id)
                else {
                    continue;
                };
                // These writers leave the ordinary label translation unchanged. Special shape
                // label placement remains residual until its own preparation shares that position.
                if info.label_type == "markdown"
                    || !matches!(
                        FlowchartShape::resolve(info.shape)?,
                        FlowchartShape::Process
                            | FlowchartShape::RoundedRectangle
                            | FlowchartShape::Diamond
                            | FlowchartShape::Circle
                            | FlowchartShape::DoubleCircle
                    )
                {
                    continue;
                }
                let Some(node) = ctx.layout_nodes_by_id.get(id) else {
                    continue;
                };
                let Some(height) = node.label_height else {
                    continue;
                };
                let Some(prepared) = ctx.prepared_nodes.node(id) else {
                    continue;
                };
                let style = &prepared.style;
                let effect = style.label_effect();
                let cleared = ctx.node_html_labels && style.label_effect_is_cleared();
                if effect.is_none() && !cleared {
                    continue;
                }
                let source = &prepared.source;
                if !source.label_shadow_source_is_bounded() {
                    continue;
                }
                let base_style = if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
                    &ctx.html_label_text_style
                } else {
                    &ctx.text_style
                };
                let text_style = weights.apply_cow(
                    ThemeTarget::NodeLabel,
                    crate::flowchart::flowchart_effective_text_style_for_node_classes(
                        base_style,
                        ctx.class_defs,
                        info.node_classes,
                        info.node_styles,
                    ),
                );
                let raw = if info.label_text_is_node_id {
                    id
                } else {
                    info.label_text
                };
                let (bounds, translation) = if ctx.node_html_labels {
                    if !plain_html_node_classes_are_bounded(ctx, info.node_classes)? {
                        continue;
                    }
                    let Some(width) = node.label_width else {
                        continue;
                    };
                    let Some(bounds) = plain_html_shadow_bounds(
                        ctx,
                        raw,
                        width,
                        height,
                        text_style.as_ref(),
                        false,
                    )?
                    else {
                        continue;
                    };
                    (bounds, (-width / 2.0, -height / 2.0))
                } else {
                    let Some(owner) = sidecar.node_owner(id, false) else {
                        continue;
                    };
                    ctx.work_meter.charge(raw.len())?;
                    let Some(bounds) = sidecar
                        .centered_shadow_bounds(owner, raw, text_style.as_ref())
                        .or_else(|| {
                            node.label_width.and_then(|width| {
                                plain_svg_allocation_bounds(raw, width, height, text_style.as_ref())
                            })
                        })
                    else {
                        continue;
                    };
                    (bounds, (0.0, -height / 2.0))
                };
                if cleared {
                    ctx.work_meter.charge(1)?;
                    plan.cleared_html_nodes.insert(id.to_owned());
                    continue;
                }
                let effect = effect.expect("selected label effect");
                let root_offset = hierarchy
                    .effective_parent(id)
                    .and_then(|root| hierarchy.root_offsets(root));
                let y_shift =
                    root_offset.map_or(0.0, |offset| offset.abs_top_transform - offset.origin_y);
                if let Some(prepared) = prepare_effect(
                    effect,
                    resources,
                    bounds,
                    translation,
                    (
                        node.x + ctx.tx + translation.0,
                        node.y + ctx.ty + y_shift + translation.1,
                    ),
                )? {
                    ctx.work_meter.charge(1)?;
                    plan.nodes.insert(id.to_owned(), prepared);
                }
            }
        }
        let edge_effect = ctx.edge_theme.label_effect();
        let edge_cleared = ctx.edge_html_labels && ctx.edge_theme.label_effect_is_cleared();
        if (edge_effect.is_some() || edge_cleared)
            && super::style::label_shadow_structural_styles_are_bounded(
                ctx.class_defs,
                &[
                    "root",
                    "edgeLabels",
                    "edgeLabel",
                    "label",
                    "text-outer-tspan",
                    "row",
                ],
            )
        {
            for render_edge in render_edges {
                let edge = render_edge.as_ref();
                if edge.edge.label_type.as_deref() == Some("markdown") {
                    continue;
                }
                let Some(layout_edge) = ctx.layout_edges_by_key.get(&edge.key) else {
                    continue;
                };
                let Some(label) = layout_edge.label.as_ref() else {
                    continue;
                };
                let source = ctx.edge_style_plan.edge_for(edge.key)?;
                if !source.label_shadow_source_is_bounded()
                    || (ctx.edge_html_labels && !plain_html_edge_styles_are_bounded(ctx, source)?)
                {
                    continue;
                }
                let raw = ctx
                    .model
                    .edge_label_for_render(edge.key.semantic_index(), edge.edge)
                    .unwrap_or_default();
                let text_style = weights.apply_cow(
                    ThemeTarget::EdgeLabel,
                    source.effective_edge_label_text_style(&ctx.text_style),
                );
                let (bounds, translation) = if ctx.edge_html_labels {
                    let content = ctx
                        .edge_label_padding
                        .content_box(label.width, label.height);
                    let Some(bounds) = plain_html_shadow_bounds(
                        ctx,
                        raw,
                        content.width,
                        content.height,
                        text_style.as_ref(),
                        true,
                    )?
                    else {
                        continue;
                    };
                    (bounds, (content.x, content.y))
                } else {
                    ctx.work_meter.charge(raw.len())?;
                    let Some(bounds) = sidecar
                        .centered_shadow_bounds(
                            FlowchartSvgLabelOwner::Edge(edge.key.semantic_index()),
                            raw,
                            text_style.as_ref(),
                        )
                        .or_else(|| {
                            let content = ctx
                                .edge_label_padding
                                .content_box(label.width, label.height);
                            plain_svg_allocation_bounds(
                                raw,
                                content.width,
                                content.height,
                                text_style.as_ref(),
                            )
                        })
                    else {
                        continue;
                    };
                    let plain = flowchart_label_plain_text(raw, "text", false);
                    let label_box = super::render::edge_label::flowchart_svg_edge_label_box(
                        label.width,
                        label.height,
                        ctx.measurer
                            .measure_svg_create_text_bbox_y_offset_px(&plain, text_style.as_ref()),
                        ctx.edge_label_padding,
                    );
                    (bounds, (label_box.translate_x, label_box.translate_y))
                };
                if edge_cleared {
                    ctx.work_meter.charge(1)?;
                    plan.cleared_html_edges.insert(edge.key);
                    continue;
                }
                let root = hierarchy.edge_root(edge.key)?.unwrap_or("");
                let offsets = hierarchy
                    .root_offsets(root)
                    .unwrap_or(FlowchartRootOffsets {
                        origin_x: 0.0,
                        origin_y: 0.0,
                        abs_top_transform: 0.0,
                    });
                let position = super::render::edge_label::resolve_flowchart_edge_label_position(
                    ctx,
                    edge.key,
                    layout_edge,
                    label,
                    offsets.origin_x,
                    offsets.origin_y,
                    edge_cache,
                    false,
                );
                if let Some(prepared) = prepare_effect(
                    edge_effect.expect("selected edge label effect"),
                    resources,
                    bounds,
                    translation,
                    (
                        position.x + offsets.origin_x + translation.0,
                        position.y + offsets.abs_top_transform + translation.1,
                    ),
                )? {
                    ctx.work_meter.charge(1)?;
                    plan.edges.insert(edge.key, prepared);
                }
            }
        }
        Ok(plan)
    }

    pub(super) fn node(&self, id: &str) -> Option<&PreparedLabelEffect> {
        self.nodes.get(id)
    }
    pub(super) fn html_node_is_cleared(&self, id: &str) -> bool {
        self.cleared_html_nodes.contains(id)
    }
    pub(super) fn edge(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> Option<&PreparedLabelEffect> {
        self.edges.get(&key)
    }
    pub(super) fn html_edge_is_cleared(&self, key: crate::flowchart::FlowchartEdgeKey) -> bool {
        self.cleared_html_edges.contains(&key)
    }
    pub(super) fn expected_applications(&self) -> usize {
        self.nodes.len() + self.edges.len()
    }
    pub(super) fn include_bounds(&self, include: &mut impl FnMut(f64, f64, f64, f64)) {
        for prepared in self.nodes.values().chain(self.edges.values()) {
            let [x, y, width, height] = prepared.shadow.region().as_array().map(f64::from);
            let (tx, ty) = prepared.absolute_translation;
            include(x + tx, y + ty, x + width + tx, y + height + ty);
        }
    }
}

// The actual node wrapper carries its assigned classes plus the generated default/node
// classes. Cluster/title groups are siblings, so their source classes do not inherit here.
fn plain_html_node_classes_are_bounded(
    ctx: &FlowchartRenderCtx<'_>,
    classes: &[String],
) -> crate::Result<bool> {
    // HTML class rules also target descendant spans. Native shape-only declarations
    // such as background/filter are therefore not isolated from the HTML glyph terminal.
    for class in crate::flowchart::flowchart_effective_node_class_names(ctx.class_defs, classes) {
        ctx.work_meter.charge(1)?;
        if matches!(class, "edgeLabel" | "icon-shape" | "image-shape") {
            return Ok(false);
        }
        if let Some(groups) = ctx.class_defs.get(class) {
            for group in groups {
                ctx.work_meter.charge(group.len())?;
                if !crate::flowchart::flowchart_split_mermaid_style_decls(group).all(|raw| {
                    crate::diagram_theme::PreparedSourceStyleDeclaration::parse(raw).is_some_and(
                        |decl| {
                            matches!(
                                decl.property(),
                                "fill"
                                    | "color"
                                    | "opacity"
                                    | "fill-opacity"
                                    | "stroke"
                                    | "stroke-width"
                                    | "stroke-opacity"
                                    | "stroke-dasharray"
                                    | "rx"
                                    | "ry"
                                    | "font-family"
                                    | "font-size"
                                    | "font-weight"
                                    | "font-style"
                            )
                        },
                    )
                }) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

// The new SVG background siblings must not acquire paint from source selectors that
// previously affected only HTML glyphs. Color remains a text-only source declaration.
fn plain_html_edge_styles_are_bounded(
    ctx: &FlowchartRenderCtx<'_>,
    source: &FlowchartCompiledStyles,
) -> crate::Result<bool> {
    if source.label_div_decls.iter().any(|(property, _)| {
        !matches!(
            property.as_str(),
            "color" | "fill" | "font-family" | "font-size" | "font-weight" | "font-style"
        )
    }) {
        return Ok(false);
    }
    for class in ["root", "edgeLabels", "edgeLabel", "label", "labelBkg"] {
        ctx.work_meter.charge(1)?;
        if let Some(groups) = ctx.class_defs.get(class) {
            for group in groups {
                ctx.work_meter.charge(group.len())?;
                if !crate::flowchart::flowchart_split_mermaid_style_decls(group).all(|raw| {
                    crate::diagram_theme::PreparedSourceStyleDeclaration::parse(raw)
                        .is_some_and(|declaration| declaration.property() == "color")
                }) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

// Layout metrics allocate paint without authorizing measurement reuse. Host callbacks
// remain observable; actual native glyph containment is checked by the export observer.
fn plain_svg_allocation_bounds(
    raw: &str,
    width: f64,
    height: f64,
    style: &crate::text::TextStyle,
) -> Option<[f64; 4]> {
    let em = style.font_size;
    if raw.trim().is_empty()
        || raw.contains(['<', '>', '\n', '\r'])
        || raw.contains("$$")
        || ![width, height, em]
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
    {
        return None;
    }
    // SVG rows start at one em. Automatic wrapping is still owned by the writer;
    // this allocation is not a guarantee about line count or final font ink. Fallback font
    // advance differences grow with line length, so keep a line-scaled horizontal reserve.
    let horizontal_reserve = width.max(em);
    Some([
        -width / 2.0 - horizontal_reserve,
        -em,
        width / 2.0 + horizontal_reserve,
        height.max(em) + em,
    ])
}

// Ordinary HTML uses the layout-owned label box in top-left coordinates. Only transparent,
// unstyled text markup can use this allocation; rich HTML needs its own geometry consumer.
fn plain_html_shadow_bounds(
    ctx: &FlowchartRenderCtx<'_>,
    raw: &str,
    width: f64,
    height: f64,
    style: &crate::text::TextStyle,
    single_paragraph: bool,
) -> crate::Result<Option<[f64; 4]>> {
    let em = style.font_size;
    if ![width, height, em]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0)
        || raw.contains("$$")
    {
        return Ok(None);
    }
    ctx.work_meter.charge(raw.len())?;
    let html = flowchart_label_html_with_prepared_math(
        raw,
        "text",
        ctx.config,
        None,
        crate::flowchart::FlowchartPreparedMathResolution::NotPrepared,
    );
    ctx.work_meter.charge(html.len())?;
    let wrapped = format!("<span>{html}</span>");
    let Ok(document) = roxmltree::Document::parse(&wrapped) else {
        return Ok(None);
    };
    if document
        .descendants()
        .filter(|node| node.is_element())
        .any(|node| {
            !matches!(node.tag_name().name(), "span" | "p" | "br")
                || node.attributes().len() != 0
                || node.tag_name().namespace().is_some()
        })
        || !document
            .descendants()
            .any(|node| node.is_text() && node.text().is_some_and(|text| !text.trim().is_empty()))
    {
        return Ok(None);
    }
    if single_paragraph {
        let mut children = document
            .root_element()
            .children()
            .filter(|node| node.is_element());
        if !children.next().is_some_and(|node| node.has_tag_name("p"))
            || children.next().is_some()
            || document
                .descendants()
                .filter(|node| node.has_tag_name("p"))
                .count()
                != 1
            || document.root_element().children().any(|node| {
                node.is_text() && node.text().is_some_and(|text| !text.trim().is_empty())
            })
        {
            return Ok(None);
        }
    }
    // Match the existing host SVG-label allocation policy: reserve one em for glyph
    // overhang around measured content, then let the shared effect add its own outsets.
    Ok(Some([-em, -em, width + em, height + em]))
}

fn prepare_effect(
    effect: &SvgShadowEffect,
    resources: &ThemeResourcePolicy,
    bounds: [f64; 4],
    translation: (f64, f64),
    absolute_translation: (f64, f64),
) -> crate::Result<Option<PreparedLabelEffect>> {
    let [min_x, min_y, max_x, max_y] = bounds;
    Ok(effect
        .materialize_user_space(
            resources,
            min_x,
            min_y,
            max_x,
            max_y,
            EffectOutsets::default(),
        )?
        .map(|shadow| PreparedLabelEffect {
            effect: effect.clone(),
            shadow,
            translation,
            absolute_translation,
        }))
}
