//! Mermaid 12 Usecase paint over its prepared graph and measured labels.

use super::*;
use crate::usecase::{
    UsecaseLabelPlan, UsecaseNodePlan, UsecasePreparedArtifact, UsecaseThemeAdapter,
};
use kurbo::{Ellipse, Rect, Shape};
use merman_model::families::usecase::{
    UsecaseActorType, UsecaseAnimation, UsecaseDiagramModel, UsecaseLabelType, UsecaseNodeKind,
    UsecaseRelationshipType,
};
use std::borrow::Cow;
use std::collections::HashMap;

#[cfg(feature = "layout-shoal")]
mod canonical;
mod shapes;
mod theme;

fn dom_part(value: &str) -> String {
    let mut result = String::new();
    let mut replacement = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
            result.push(ch);
            replacement = false;
        } else if !replacement {
            result.push('_');
            replacement = true;
        }
    }
    let result = result.trim_matches('_');
    if result.is_empty() {
        "element".into()
    } else {
        result.into()
    }
}

fn element_role_classes(plan: &UsecaseNodePlan, business: bool) -> String {
    format!(
        "usecase-element usecase-{}{}",
        if plan.ellipse { "ellipse" } else { "rect" },
        if business && plan.ellipse {
            " usecase-business usecase-business-shape"
        } else if business {
            " usecase-business"
        } else {
            ""
        }
    )
}

fn accessible_label(label: &UsecaseLabelPlan) -> String {
    accessible_text(&label.text, label.label_type)
}

fn accessible_text(text: &str, kind: UsecaseLabelType) -> String {
    match kind {
        UsecaseLabelType::Text => text.to_owned(),
        UsecaseLabelType::Markdown => crate::text::mermaid_markdown_to_lines(text, true)
            .iter()
            .map(|line| {
                line.iter()
                    .map(|word| word.0.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[derive(Clone, Copy)]
struct UsecaseLabelRenderContext<'a> {
    config: &'a merman_model::presentation::PresentationConfig,
    measurer: &'a dyn TextMeasurer,
}

enum UsecaseLabelKind<'a> {
    Node(&'a str),
    PositionedNode(&'a str),
    Boundary { canonical: bool },
    Edge(&'a str),
}

fn write_label(
    out: &mut String,
    plan: &UsecaseLabelPlan,
    kind: UsecaseLabelKind<'_>,
    center_x: f64,
    center_y: f64,
    context: UsecaseLabelRenderContext<'_>,
) {
    let UsecaseLabelRenderContext { config, measurer } = context;
    let html = config_bool(config.as_value(), &["htmlLabels"]).unwrap_or(true);
    let font_family = plan.style.font_family.as_deref().unwrap_or("sans-serif");
    let font_weight = plan.style.font_weight.as_deref().unwrap_or("normal");
    let css: String = plan
        .styles
        .iter()
        .map(|(key, value)| {
            let key = if !html && key == "color" {
                "fill"
            } else {
                key.as_str()
            };
            format!(
                "{key}:{} !important;",
                value.trim_end_matches("!important").trim()
            )
        })
        .collect();
    let is_edge_label = matches!(kind, UsecaseLabelKind::Edge(_));
    let is_boundary = matches!(kind, UsecaseLabelKind::Boundary { .. });
    let canonical_boundary = matches!(kind, UsecaseLabelKind::Boundary { canonical: true });
    let positioned_node = matches!(kind, UsecaseLabelKind::PositionedNode(_));
    let svg_bbox_y = if !html && (is_edge_label || positioned_node || canonical_boundary) {
        measurer.measure_svg_create_text_bbox_y_offset_px(&plan.text, &plan.style)
    } else {
        0.0
    };
    let class_attr = match kind {
        UsecaseLabelKind::Node(class) | UsecaseLabelKind::PositionedNode(class) => class,
        UsecaseLabelKind::Boundary { .. } => "cluster-label system-boundary-title",
        UsecaseLabelKind::Edge(_) => "label",
    };
    if let UsecaseLabelKind::Edge(data_id) = kind {
        let _ = write!(
            out,
            r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({}, {})">"#,
            fmt(center_x),
            fmt(center_y),
            escape_attr(data_id),
            fmt(if html { -plan.metrics.width / 2.0 } else { 0.0 }),
            // insertEdgeLabel centers the SVG text bbox, including its local Y offset.
            fmt(-svg_bbox_y - plan.text_metrics.height / 2.0),
        );
    } else if html {
        let _ = write!(
            out,
            r#"<g class="{}" transform="translate({},{})" style="font-family:{};font-size:{}px;font-weight:{}">"#,
            escape_attr(class_attr),
            fmt(center_x - plan.metrics.width / 2.0),
            fmt(center_y - plan.metrics.height / 2.0),
            escape_attr(font_family),
            fmt(plan.style.font_size),
            escape_attr(font_weight)
        );
    } else {
        // Plain labelHelper centers only Y; custom shapes reposition the whole local bbox.
        let x = if is_boundary || positioned_node {
            center_x - plan.metrics.width / 2.0
        } else {
            center_x
        };
        let _ = write!(
            out,
            r#"<g class="{}" transform="translate({},{})" style="{}">"#,
            escape_attr(class_attr),
            fmt(x),
            fmt(center_y - plan.metrics.height / 2.0 - svg_bbox_y),
            escape_attr(&css),
        );
    }
    if html {
        let content = plan.html.as_ref().map_or("", |label| label.xhtml.as_str());
        if !is_edge_label && !is_boundary {
            out.push_str("<rect/>");
        }
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml"{} style="{}display:inline-block;white-space:normal;line-height:1.5;text-align:center;width:{}px"><span class="{}" style="{}">{}</span></div></foreignObject>"#,
            fmt(plan.metrics.width),
            fmt(plan.metrics.height),
            if is_edge_label {
                r#" class="labelBkg""#
            } else {
                ""
            },
            escape_attr(&css),
            fmt(plan.metrics.width),
            if is_edge_label {
                "edgeLabel"
            } else {
                "nodeLabel"
            },
            escape_attr(&css),
            content
        );
    } else {
        if !is_edge_label && !is_boundary {
            out.push_str("<rect/>");
        }
        out.push_str("<g>");
        if is_edge_label && !plan.text.is_empty() {
            let _ = write!(
                out,
                r#"<rect class="background" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                fmt(-plan.text_metrics.width / 2.0 - 2.0),
                fmt(svg_bbox_y - 2.0),
                fmt(plan.metrics.width),
                fmt(plan.metrics.height),
            );
        } else {
            out.push_str(r#"<rect class="background" style="stroke: none"/>"#);
        }
        // Usecase plain labels are literal text, including strings that resemble HTML.
        match plan.label_type {
            UsecaseLabelType::Text => {
                // prepareUsecaseLayoutData escapes plain labels before createText splits
                // lines and words. Decode those entities only when writing text content,
                // so a literal <br/> remains visible instead of becoming a line break.
                let source = crate::usecase::normalize_plain_label_line_breaks(&plan.text)
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                if canonical_boundary {
                    let words =
                        crate::text::DeterministicTextMeasurer::normalized_text_lines(&source)
                            .iter()
                            .map(|line| {
                                crate::text::non_markdown_svg_words(line)
                                    .map(str::to_owned)
                                    .collect()
                            })
                            .collect::<Vec<_>>();
                    label::write_measured_svg_title(out, &words, &css, None, plan.metrics.width);
                } else {
                    label::write_svg_text_from_create_text_source_with_style(
                        out,
                        &source,
                        &css,
                        is_edge_label,
                    );
                }
            }
            UsecaseLabelType::Markdown => {
                if is_edge_label {
                    label::write_svg_text_markdown_wrapped_centered_with_style(
                        out,
                        &plan.text,
                        &css,
                        measurer,
                        &plan.style,
                        plan.max_width,
                    )
                } else {
                    label::write_svg_text_markdown_wrapped_from_create_text_source_with_style(
                        out,
                        &plan.text,
                        &css,
                        measurer,
                        &plan.style,
                        plan.max_width,
                        false,
                    )
                }
            }
        }
        out.push_str("</g>");
    }
    out.push_str("</g>");
    if is_edge_label {
        out.push_str("</g>");
    }
}

pub(crate) fn render_usecase_diagram_svg_model(
    prepared: &UsecasePreparedArtifact,
    model: &UsecaseDiagramModel,
    config: &merman_model::presentation::PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("usecase");
    let cfg = config.as_value();
    let css_binding = &prepared.css_binding;
    let native = options.native_family_theme::<UsecaseThemeAdapter>();
    let labels = UsecaseLabelRenderContext { config, measurer };
    let layout = prepared.layout();
    let geometry: HashMap<_, _> = layout
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let plans: HashMap<_, _> = prepared
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let source_nodes: HashMap<_, _> = model
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let boundaries: HashMap<_, _> = model
        .boundaries
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let notes: HashMap<_, _> = model
        .notes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let json_nodes: HashMap<_, _> = model
        .json_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let relationships: HashMap<_, _> = model
        .relationships
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect();
    let edge_geometry: HashMap<_, _> = layout
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect();
    let source_labels: HashMap<_, _> = plans
        .iter()
        .map(|(&id, plan)| {
            let kind = source_nodes
                .get(id)
                .map_or(plan.label.label_type, |node| node.label_type);
            (id, accessible_text(&plan.source_label, kind))
        })
        .collect();
    let padding = config_f64(cfg, &["usecase", "diagramPadding"]).unwrap_or(20.0);
    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    });
    let title = model
        .title
        .as_deref()
        .or(diagram_title)
        .filter(|text| !text.is_empty());
    let title_x = (bounds.min_x + bounds.max_x) / 2.0;
    let mut viewport_bounds = bounds.clone();
    let elevation = native
        .map(|theme| &theme.elevation)
        .filter(|elevation| **elevation != merman_theme_contract::v2::Elevation::Flat);
    let mut depth = Vec::new();
    if let Some(elevation) = elevation {
        for (index, plan) in prepared.nodes.iter().enumerate() {
            let Some(source) = source_nodes.get(plan.id.as_str()) else {
                continue;
            };
            if source.kind == UsecaseNodeKind::Actor {
                continue;
            }
            let node = geometry[plan.id.as_str()];
            let [width, height] = [node.width, node.height];
            let rect = Rect::new(-width / 2.0, -height / 2.0, width / 2.0, height / 2.0);
            let silhouette = if plan.ellipse {
                Ellipse::from_rect(rect).to_path(0.1)
            } else {
                rect.to_path(0.1)
            };
            let bounds = crate::theme::elevation_svg::bounds(elevation, rect)
                + kurbo::Vec2::new(node.x, node.y);
            viewport_bounds.min_x = viewport_bounds.min_x.min(bounds.x0);
            viewport_bounds.min_y = viewport_bounds.min_y.min(bounds.y0);
            viewport_bounds.max_x = viewport_bounds.max_x.max(bounds.x1);
            viewport_bounds.max_y = viewport_bounds.max_y.max(bounds.y1);
            depth.push((index, silhouette, (node.x, node.y)));
        }
    }
    #[cfg(feature = "layout-shoal")]
    let canonical_edges = canonical::prepare_edges(prepared, cfg, &mut viewport_bounds, options)?;
    #[cfg(feature = "layout-shoal")]
    let uses_canonical = canonical_edges.is_some();
    #[cfg(not(feature = "layout-shoal"))]
    let uses_canonical = false;
    if let Some(title) = title {
        let style = css_binding
            .native_title_font
            .as_ref()
            .unwrap_or(&css_binding.root_font);
        let (left, right) = measurer.measure_svg_title_bbox_x(title, style);
        let (ascent, descent) = crate::text::svg_text_vertical_extents_px(
            measurer,
            title,
            style,
            crate::text::svg_title_bbox_vertical_extents_px(style),
        );
        viewport_bounds.min_x = viewport_bounds.min_x.min(title_x - left);
        viewport_bounds.max_x = viewport_bounds.max_x.max(title_x + right);
        viewport_bounds.min_y = viewport_bounds.min_y.min(-ascent);
        viewport_bounds.max_y = viewport_bounds.max_y.max(descent);
    }
    let spec = root_svg::RootViewportSpec::mermaid(
        root_svg::DiagramBounds::from_extents(
            viewport_bounds.min_x,
            viewport_bounds.min_y,
            viewport_bounds.max_x,
            viewport_bounds.max_y,
            padding,
        ),
        config_bool(cfg, &["usecase", "useMaxWidth"]).unwrap_or(true),
    );
    let acc_title_id = model
        .acc_title
        .as_ref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let acc_descr_id = model
        .acc_description
        .as_ref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let mut chrome = root_svg::RootChrome::new(diagram_id, "usecase");
    chrome.class = Some("usecaseDiagram");
    chrome.aria_labelledby = acc_title_id.as_deref();
    chrome.aria_describedby = acc_descr_id.as_deref();
    let actor_font = &css_binding.actor_font;
    let usecase_font = &css_binding.usecase_font;
    let actor_font_size = format!("{}px", fmt(actor_font.font_size));
    let usecase_font_size = format!("{}px", fmt(usecase_font.font_size));
    let font_properties = [
        (
            "--mermaid-usecase-actor-font-size",
            actor_font_size.as_str(),
        ),
        (
            "--mermaid-usecase-actor-font-family",
            actor_font.font_family.as_deref().unwrap_or("sans-serif"),
        ),
        (
            "--mermaid-usecase-actor-font-weight",
            actor_font.font_weight.as_deref().unwrap_or("normal"),
        ),
        ("--mermaid-usecase-font-size", usecase_font_size.as_str()),
        (
            "--mermaid-usecase-font-family",
            usecase_font.font_family.as_deref().unwrap_or("sans-serif"),
        ),
        (
            "--mermaid-usecase-font-weight",
            usecase_font.font_weight.as_deref().unwrap_or("normal"),
        ),
    ];
    chrome.custom_properties = &font_properties;
    let mut out = String::new();
    let document = options
        .root_viewport(crate::DiagramFamilyId::USECASE, diagram_id)
        .write_open(&mut out, spec, chrome)?;
    if let Some(title) = &model.acc_title {
        let _ = write!(
            out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
    }
    if let Some(description) = &model.acc_description {
        let _ = write!(
            out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(description)
        );
    }
    theme::write_css(&mut out, diagram_id, css_binding)?;
    // Mermaid emits marker wrappers that are observable in strict SVG DOM parity.
    out.push_str("<g>");
    if !uses_canonical {
        markers::push_base_edge_markers(&mut out, diagram_id, "usecase");
        out.push_str("<defs>");
        let _ = write!(
            out,
            r#"<marker id="{diagram_id}_usecase-extensionStart" class="marker extension usecase" refX="18" refY="7" markerWidth="20" markerHeight="28" orient="auto" markerUnits="userSpaceOnUse"><path d="M 1,7 L18,13 V 1 Z"/></marker>"#
        );
        out.push_str("</defs><defs>");
        let _ = write!(
            out,
            r#"<marker id="{diagram_id}_usecase-extensionEnd" class="marker extension usecase" refX="1" refY="7" markerWidth="20" markerHeight="28" markerUnits="userSpaceOnUse" orient="auto"><path d="M 1,1 V 13 L18,7 Z"/></marker>"#
        );
        out.push_str("</defs>");
        let _ = write!(
            out,
            r#"<marker id="{diagram_id}_usecase-extensionStart-margin" class="marker extension usecase" refX="18" refY="7" markerWidth="20" markerHeight="28" orient="auto" markerUnits="userSpaceOnUse" viewBox="0 0 20 14"><polygon points="10,7 18,13 18,1" style="stroke-width: 2; stroke-dasharray: 0;"/></marker>"#
        );
        out.push_str("<defs>");
        let _ = write!(
            out,
            r#"<marker id="{diagram_id}_usecase-extensionEnd-margin" class="marker extension usecase" refX="9" refY="7" markerWidth="20" markerHeight="28" orient="auto" markerUnits="userSpaceOnUse" viewBox="0 0 20 14"><polygon points="10,1 10,13 18,7" style="stroke-width: 2; stroke-dasharray: 0;"/></marker>"#
        );
        out.push_str("</defs>");
    }
    out.push_str(r#"<g class="root"><g class="clusters">"#);
    let mut legacy_boundaries = prepared
        .nodes
        .iter()
        .filter(|plan| plan.is_boundary)
        .enumerate();
    #[cfg(feature = "layout-shoal")]
    let mut canonical_boundaries = prepared
        .canonical_boundary_order()
        .map(|order| order.iter());
    let boundary_plans = std::iter::from_fn(|| {
        #[cfg(feature = "layout-shoal")]
        if let Some(order) = &mut canonical_boundaries {
            return order
                .next()
                .map(|&(color, plan)| (color, &prepared.nodes[plan]));
        }
        legacy_boundaries.next()
    });
    for (color_index, plan) in boundary_plans {
        options.checkpoint_emit()?;
        let node = geometry[plan.id.as_str()];
        let boundary = boundaries[plan.id.as_str()];
        let kind = if plan.package { "package" } else { "rect" };
        let accessible = format!("{kind} system boundary {}", accessible_label(&plan.label));
        let appearance = theme::appearance_attributes(css_binding, Some(color_index));
        let canonical_attributes = if uses_canonical {
            format!(r#" data-et="cluster" data-id="{}""#, escape_attr(&plan.id))
        } else {
            String::new()
        };
        let _ = write!(
            out,
            r#"<g id="{}-usecase-{}" class="cluster usecase-system-boundary usecase-system-boundary-{kind} default system-boundary system-boundary-{kind} {}" data-boundary-type="{kind}"{appearance} data-usecase-id="{}" data-usecase-kind="boundary" role="img" aria-label="{}"{canonical_attributes}>"#,
            dom_part(diagram_id.semantic_str()),
            dom_part(&plan.id),
            escape_attr(&boundary.classes.join(" ")),
            escape_attr(&plan.id),
            escape_attr(&accessible)
        );
        #[cfg(feature = "layout-shoal")]
        let placed_title = prepared.canonical_boundary_title(&plan.id);
        #[cfg(not(feature = "layout-shoal"))]
        let placed_title: Option<&crate::model::LayoutLabel> = None;
        let left = node.x - node.width / 2.0;
        let top = node.y - node.height / 2.0;
        let tab_height = if plan.package {
            placed_title.map_or(plan.label.metrics.height + 10.0, |title| {
                title.y - top + title.height / 2.0 + 5.0
            })
        } else {
            0.0
        };
        let tab_width = node
            .width
            .min(80.0_f64.max(plan.label.metrics.width + 20.0));
        let tab_left = placed_title.map_or(left, |title| {
            (title.x - tab_width / 2.0).clamp(left, (left + node.width - tab_width).max(left))
        });
        let write_tab = |out: &mut String| {
            if plan.package {
                shapes::rect(
                    out,
                    "boundary-tab system-boundary-package-tab label-container",
                    tab_left,
                    top,
                    tab_width,
                    tab_height,
                    &plan.source_style,
                );
            }
        };
        if !uses_canonical {
            write_tab(&mut out);
        }
        let body_inset = if uses_canonical { 0.0 } else { tab_height };
        shapes::rect(
            &mut out,
            "boundary-body label-container",
            left,
            top + body_inset,
            node.width,
            node.height - body_inset,
            &plan.source_style,
        );
        if uses_canonical {
            write_tab(&mut out);
        }
        let title_x = placed_title.map_or_else(
            || {
                if plan.package {
                    left + tab_width / 2.0
                } else {
                    node.x
                }
            },
            |title| title.x,
        );
        let title_y = placed_title.map_or_else(
            || {
                top + if plan.package {
                    tab_height / 2.0
                } else {
                    plan.label.metrics.height / 2.0
                        + config_f64(cfg, &["flowchart", "subGraphTitleMargin", "top"])
                            .unwrap_or(0.0)
                }
            },
            |title| title.y,
        );
        write_label(
            &mut out,
            &plan.label,
            UsecaseLabelKind::Boundary {
                canonical: uses_canonical,
            },
            title_x,
            title_y,
            labels,
        );
        out.push_str("</g>");
    }
    let elk = crate::layout_backend::resolve_graph_layout(cfg).backend
        == crate::layout_backend::GraphLayoutBackend::Elk;
    let hop_points = if elk && config_bool(cfg, &["elk", "lineHops"]) != Some(false) {
        let clone_work = prepared.edges.iter().fold(0usize, |total, plan| {
            total.saturating_add(
                edge_geometry[plan.id.as_str()]
                    .points
                    .len()
                    .saturating_add(3),
            )
        });
        options.work_meter().charge(clone_work)?;
        prepared
            .edges
            .iter()
            .map(|plan| prepared.edge_points(edge_geometry[plan.id.as_str()]))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let hop_edges = prepared
        .edges
        .iter()
        .zip(&hop_points)
        .map(|(plan, points)| line_hops::LineHopEdge {
            id: plan.id.as_str(),
            points,
            curve: Some(if edge_geometry[plan.id.as_str()].points.is_empty() {
                "linear"
            } else {
                "rounded"
            }),
            arrow_type_start: plan.start_marker.as_deref(),
            arrow_type_end: plan.end_marker.as_deref(),
        })
        .collect::<Vec<_>>();
    let hopped_paths = line_hops::elk_line_hop_paths(cfg, &hop_edges, options.work_meter())?;
    out.push_str("</g>");
    if let Some(elevation) = elevation {
        for (index, silhouette, center) in depth {
            let id = format!("{diagram_id}-usecase-{index}");
            let _ = crate::theme::elevation_svg::write_depth(
                &mut out,
                elevation,
                &id,
                &silhouette,
                center,
            );
            options.checkpoint_emit()?;
        }
    }
    out.push_str("<g class=\"edgePaths edges\">");
    for (edge_index, plan) in prepared.edges.iter().enumerate() {
        #[cfg(not(feature = "layout-shoal"))]
        let _ = edge_index;
        options.checkpoint_emit()?;
        let edge = edge_geometry[plan.id.as_str()];
        let data_look = &css_binding.edge_look;
        #[cfg(feature = "layout-shoal")]
        let canonical_edge = canonical_edges.as_ref().map(|edges| &edges[edge_index]);
        let legacy = || {
            let mut points = prepared.edge_points(edge);
            let curve = if elk {
                if edge.points.is_empty() {
                    "linear"
                } else {
                    "rounded"
                }
            } else {
                cfg.pointer("/flowchart/curve")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("basis")
            };
            let data_points = crate::svg::parity::util::json_stringify_points(&points);
            let path = edge_path::render_path(
                &mut points,
                curve,
                plan.start_marker.as_deref(),
                plan.end_marker.as_deref(),
            );
            let path = match hopped_paths.get(plan.id.as_str()) {
                Some(hop) => Cow::Borrowed(hop.as_str()),
                None => Cow::Owned(path),
            };
            (data_points, path)
        };
        #[cfg(feature = "layout-shoal")]
        let (data_points, path) = canonical_edge.map_or_else(legacy, |painted| {
            (
                crate::svg::parity::util::json_stringify_points(&painted.points),
                Cow::Borrowed(painted.path.as_str()),
            )
        });
        #[cfg(not(feature = "layout-shoal"))]
        let (data_points, path) = legacy();
        let data_points = base64::engine::general_purpose::STANDARD.encode(data_points);
        let relation = relationships
            .get(plan.original_id.as_deref().unwrap_or(&plan.id))
            .copied();
        let label = relation.map(|edge| match edge.relationship_type {
            UsecaseRelationshipType::Association => plan
                .label
                .as_ref()
                .map(|label| format!("association {}", accessible_label(label)))
                .unwrap_or_else(|| "association".into()),
            UsecaseRelationshipType::Include => "include".into(),
            UsecaseRelationshipType::Extend => "extend".into(),
            UsecaseRelationshipType::Generalization => "generalization".into(),
        });
        let relation_kind = relation.map_or("note", |edge| match edge.relationship_type {
            UsecaseRelationshipType::Association => "association",
            UsecaseRelationshipType::Include => "include",
            UsecaseRelationshipType::Extend => "extend",
            UsecaseRelationshipType::Generalization => "generalization",
        });
        let margin = if config_string(cfg, &["look"]).as_deref() == Some("neo") && !plan.animated {
            "-margin"
        } else {
            ""
        };
        let mut marker_references = Vec::new();
        for (start, marker) in [(true, &plan.start_marker), (false, &plan.end_marker)] {
            if let Some(marker) = marker {
                let end = if start { "start" } else { "end" };
                #[cfg(feature = "layout-shoal")]
                if let Some(painted) = canonical_edge {
                    if let Some(marker) = &painted.markers[usize::from(!start)] {
                        let reference = canonical::write_marker(
                            &mut out, diagram_id, marker, start, edge_index,
                        );
                        marker_references.push((end, reference));
                    }
                    continue;
                }
                let name = marker.strip_prefix("arrow_").unwrap_or(marker);
                let side = if start { "Start" } else { "End" };
                let reference = format!("url(#{diagram_id}_usecase-{name}{side}{margin})");
                marker_references.push((end, reference));
            }
        }
        let mut classes = format!(
            "edge-thickness-normal edge-pattern-{} default relationship relationship-{relation_kind}",
            if plan.dotted { "dotted" } else { "solid" }
        );
        if let Some(edge) = relation {
            for class in &edge.classes {
                classes.push(' ');
                classes.push_str(class);
            }
            if edge.animate || edge.animation.is_some() {
                classes.push_str(match edge.animation {
                    Some(UsecaseAnimation::Slow) => " edge-animation-slow",
                    _ => " edge-animation-fast",
                });
            }
        }
        let _ = write!(
            out,
            r#"<path id="{}{}-{}" data-id="{}" data-et="edge" data-edge="true" data-look="{}" data-points="{}" class="{}" d="{}""#,
            if plan.dagre_recursive { "" } else { "usecase-" },
            dom_part(diagram_id.semantic_str()),
            dom_part(&plan.id),
            escape_attr(&plan.id),
            escape_attr(data_look),
            escape_attr(&data_points),
            escape_attr(&classes),
            path
        );
        if !plan.dagre_recursive {
            let _ = write!(
                out,
                r#" data-usecase-id="{}" data-usecase-kind="{}""#,
                escape_attr(&plan.id),
                if plan.internal {
                    "note-connector"
                } else {
                    "relationship"
                }
            );
        }
        if plan.dagre_recursive {
            // Extracted edges are absent from the top-level LayoutData annotation pass.
        } else if plan.internal {
            out.push_str(" aria-hidden=\"true\"");
        } else {
            let name = if plan.original_id.is_some() {
                plan.id.clone()
            } else {
                format!(
                    "{} from {} to {}",
                    label.unwrap_or_default(),
                    source_labels[plan.source.as_str()],
                    source_labels[plan.target.as_str()]
                )
            };
            let _ = write!(out, r#" role="img" aria-label="{}""#, escape_attr(&name));
        }
        let mut source_style = plan.source_style.clone();
        if plan.dotted {
            source_style.push_str("stroke-dasharray:3;");
        }
        // The authored mask encodes offsets along the path before line hops.
        let source_style = if let Some(hopped_d) = hopped_paths.get(plan.id.as_str()) {
            line_hops::rewrite_style_after_line_hop(&source_style, hopped_d, options.work_meter())?
        } else {
            std::borrow::Cow::Borrowed(source_style.as_str())
        };
        let _ = write!(out, r#" style="{}""#, escape_attr(&source_style));
        for (end, reference) in marker_references {
            let _ = write!(out, r#" marker-{end}="{}""#, escape_attr(&reference));
        }
        out.push_str("/>");
    }
    out.push_str("</g><g class=\"edgeLabels\">");
    for plan in &prepared.edges {
        if let Some(label) = &plan.label {
            let edge = edge_geometry[plan.id.as_str()];
            if let Some(position) = &edge.label {
                write_label(
                    &mut out,
                    label,
                    UsecaseLabelKind::Edge(&plan.id),
                    position.x,
                    position.y,
                    labels,
                );
            }
        } else if plan.original_id.is_some() {
            shapes::write_empty_edge_label(&mut out, &plan.id, config);
        }
    }
    out.push_str("</g><g class=\"nodes\">");
    for (color_index, plan) in prepared
        .nodes
        .iter()
        .filter(|plan| !plan.is_boundary)
        .enumerate()
    {
        options.checkpoint_emit()?;
        let node = geometry[plan.id.as_str()];
        if plan.dagre_helper {
            shapes::write_dagre_helper(&mut out, node, config);
            continue;
        }
        let source = source_nodes.get(plan.id.as_str()).copied();
        let note = notes.get(plan.id.as_str()).copied();
        let json = json_nodes.get(plan.id.as_str()).copied();
        let (kind, name, classes) = if let Some(source) = source {
            let label = &source_labels[plan.id.as_str()];
            let stereotype = source
                .stereotype
                .as_ref()
                .map(|value| {
                    format!(
                        ", stereotype {}",
                        merman_markup::sanitize::sanitize_text(value, config.as_config())
                    )
                })
                .unwrap_or_default();
            let business = if source.business { "business " } else { "" };
            let actor = source.kind == UsecaseNodeKind::Actor;
            let variant = match source.actor_type {
                UsecaseActorType::Normal => "",
                UsecaseActorType::Hollow => "hollow ",
                UsecaseActorType::Awesome => "awesome ",
                UsecaseActorType::Icon => "icon ",
            };
            (
                if actor { "actor" } else { "usecase" },
                if actor {
                    format!("{business}{variant}actor {label}{stereotype}")
                } else {
                    format!("{business}use case {label}{stereotype}")
                },
                source.classes.as_slice(),
            )
        } else if let Some(note) = note {
            (
                "note",
                format!(
                    "Note for {}: {}",
                    source_labels[note.target.as_str()],
                    accessible_label(&plan.label)
                ),
                [].as_slice(),
            )
        } else if let Some(json) = json {
            let rows = plan
                .table
                .as_ref()
                .map(|table| {
                    table
                        .rows
                        .iter()
                        .map(|row| format!("{}: {}", row.accessible_key, row.value.text))
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();
            (
                "json",
                if rows.is_empty() {
                    json.id.clone()
                } else {
                    format!("{}: {rows}", json.id)
                },
                json.classes.as_slice(),
            )
        } else {
            return Err(Error::InvalidModel {
                message: format!("missing Usecase node {}", plan.id),
            });
        };
        let appearance = theme::appearance_attributes(css_binding, source.map(|_| color_index));
        let role_classes = match source {
            Some(node) if node.kind == UsecaseNodeKind::Actor => {
                let variant = match node.actor_type {
                    UsecaseActorType::Normal => "normal",
                    UsecaseActorType::Hollow => "hollow",
                    UsecaseActorType::Awesome => "awesome",
                    UsecaseActorType::Icon => "icon",
                };
                format!(
                    "usecase-actor usecase-actor-{variant}{} usecase-actor-variant usecase-actor-{variant}",
                    if node.business {
                        " usecase-business"
                    } else {
                        ""
                    }
                )
            }
            Some(node) => element_role_classes(plan, node.business),
            None if note.is_some() => "usecase-note".into(),
            None => "usecase-json-table usecase-json-table".into(),
        };
        let canonical_attributes = if uses_canonical {
            format!(r#" data-et="node" data-id="{}""#, escape_attr(&plan.id))
        } else {
            String::new()
        };
        let _ = write!(
            out,
            r#"<g id="{}-usecase-{}"{appearance} class="node default {role_classes} {}" data-usecase-id="{}" data-usecase-kind="{kind}" role="img" aria-label="{}" transform="translate({},{})"{canonical_attributes}>"#,
            dom_part(diagram_id.semantic_str()),
            dom_part(&plan.id),
            escape_attr(&classes.join(" ")),
            escape_attr(&plan.id),
            escape_attr(&name),
            fmt(node.x),
            fmt(node.y)
        );
        shapes::write_node(
            &mut out,
            node,
            plan,
            &shapes::UsecaseNodeRenderContext {
                source,
                note: note.is_some(),
                css_binding,
                labels,
                diagram_id,
                options,
                #[cfg(feature = "layout-shoal")]
                canonical: uses_canonical,
            },
        )?;
        out.push_str("</g>");
    }
    out.push_str("</g></g></g>");
    if native.is_none() {
        css_binding
            .look_defs
            .write_shadow_defs(&mut out, diagram_id)?;
        css_binding.look_defs.write_gradient(&mut out, diagram_id)?;
    }
    if let Some(title) = title {
        let _ = write!(
            out,
            r#"<text class="usecaseDiagramTitleText" x="{}" y="{}" text-anchor="middle">{}</text>"#,
            fmt(title_x),
            fmt(0.0),
            escape_xml(title)
        );
    }
    out.push_str("</svg>");
    options.checkpoint_emit()?;
    document.complete(out)
}
