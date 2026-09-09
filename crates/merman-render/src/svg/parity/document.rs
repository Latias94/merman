//! SVG encoder for the renderer-neutral [`RenderDocument`].
//!
//! This module deliberately knows nothing about a Mermaid family model.  Family adapters own
//! geometry and resolved style; this encoder only turns the validated public command stream into
//! SVG. The private sidecar selects DOM structure and equivalent SVG coordinate representations;
//! it must never change the public geometry, paint, or effective transform.

use super::output::{self, SvgOutput};
use super::root_svg;
use super::util::{escape_attr_into, fmt};
use super::{SvgDebugOptions, SvgRenderOptions, sanitize_svg_id};
use crate::drawing_list::{BlockInlinePathProperty, RenderDocument, SvgStructureBody};
use crate::environment::RenderSession;
use crate::family::RenderFamilyKind;
use crate::wardley::WardleyTheme;
use crate::{Error, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use merman_core::OperationPhase;
use merman_core::svg_security::{MermaidNavigationSecurity, prepare_mermaid_navigation_uri};
use merman_display_list::{
    BlendMode, Color, DrawingCommand, DrawingListDocument, DrawingResource, FillRule, FontStyle,
    GradientSpread, LineCap, LineJoin, Paint, PathResource, PathSegment, PathStyle, Point, Rect,
    ResourceId, SemanticAnnotation, SemanticRole, TextAnchor, TextBaseline, TextDirection,
    TextObligation, TextRun, Transform,
};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

mod cynefin;
mod gantt;
mod info;
mod pie;
mod sankey;
mod venn;

/// Serializes one validated canonical document to SVG.
pub(crate) fn render_document_svg(
    document: &RenderDocument,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
    effective_config: &Value,
    session: &RenderSession,
) -> Result<String> {
    let encoder = DocumentSvgEncoder::new(document, options, debug, effective_config, session)?;
    document.admit_serialization(crate::drawing_list::DocumentBudget::SvgOperation, session)?;
    let svg = encoder.render()?;
    if matches!(
        document.svg.body,
        SvgStructureBody::Info(_)
            | SvgStructureBody::Packet(_)
            | SvgStructureBody::Pie(_)
            | SvgStructureBody::Cynefin(_)
            | SvgStructureBody::Sankey(_)
            | SvgStructureBody::Journey(_)
            | SvgStructureBody::Gantt(_)
            | SvgStructureBody::Venn(_)
    ) {
        // These families resolve styles in the command stream. Theme CSS is rejected by the
        // builder; an encoder must not reintroduce a second visual source from external config.
        Ok(svg)
    } else {
        super::apply_theme_css(svg, effective_config, session)
    }
}

struct DocumentSvgEncoder<'a> {
    document: &'a DrawingListDocument,
    svg_body: &'a SvgStructureBody,
    family: RenderFamilyKind,
    diagram_type: &'static str,
    options: &'a SvgRenderOptions,
    debug: &'a SvgDebugOptions,
    effective_config: &'a Value,
    mermaid_config: Option<merman_core::MermaidConfig>,
    session: &'a RenderSession,
    diagram_id: String,
    resources: BTreeMap<String, &'a DrawingResource>,
    resource_svg_ids: BTreeMap<String, String>,
    semantics: BTreeMap<String, &'a SemanticAnnotation>,
    semantic_svg_ids: BTreeMap<String, String>,
    fallbacks: BTreeMap<String, &'a merman_display_list::RasterFallback>,
    error_projection: Option<super::error::ErrorProjection<'a>>,
    packet_styles: Option<super::packet::PacketSvgStyles<'a>>,
    pie_styles: Option<super::pie::PieSvgStyles<'a>>,
    cynefin_marker_definitions: BTreeMap<String, String>,
    cynefin_text_styles: BTreeMap<String, Option<&'a merman_display_list::TextStyle>>,
    cynefin_path_styles: BTreeMap<String, Option<cynefin::PathCssStyle<'a>>>,
    sankey_inline_gradients: BTreeSet<String>,
    emitted_sankey_gradients: BTreeSet<String>,
    sankey_label_style: Option<&'a merman_display_list::TextStyle>,
    info_text_style: Option<&'a merman_display_list::TextStyle>,
    venn_text_styles: Option<venn::TextStyles<'a>>,
    sankey_links: Option<sankey::LinkProjection>,
    gantt_text: Option<gantt::TextProjection<'a>>,
    command_index: usize,
    state: GraphicsState,
    saves: Vec<SavePoint>,
    groups: Vec<GroupKind>,
    semantic_text_counts: BTreeMap<String, usize>,
    output: SvgOutput<'a>,
}

#[derive(Debug, Clone, Copy)]
struct GraphicsState {
    transform: Transform,
    opacity: f64,
    blend_mode: BlendMode,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            transform: Transform::IDENTITY,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SavePoint {
    state: GraphicsState,
    group_len: usize,
}

#[derive(Debug, Clone)]
enum GroupKind {
    Semantic {
        linked: bool,
        emitted: bool,
        semantic_id: String,
        projected_transform: Transform,
    },
    Layer {
        projected_transform: Transform,
    },
    Clip,
}

impl<'a> DocumentSvgEncoder<'a> {
    fn new(
        document: &'a RenderDocument,
        options: &'a SvgRenderOptions,
        debug: &'a SvgDebugOptions,
        effective_config: &'a Value,
        session: &'a RenderSession,
    ) -> Result<Self> {
        crate::drawing_list::DocumentBudget::SvgOperation.validate(&document.public, session)?;
        let error_projection = match &document.svg.body {
            SvgStructureBody::Error(body) => Some(
                super::error::validate_error_projection_contract(&document.public, body)?,
            ),
            _ => None,
        };

        let family = document.svg.family;
        let diagram_id = options
            .diagram_id
            .clone()
            .unwrap_or_else(|| default_diagram_id(family).to_string());
        let mut resources = BTreeMap::new();
        let mut resource_svg_ids = BTreeMap::new();
        for (index, resource) in document.public.resources.iter().enumerate() {
            let raw_id = resource.id().as_str().to_string();
            resources.insert(raw_id.clone(), resource);
            resource_svg_ids.insert(
                raw_id.clone(),
                scoped_id(diagram_id.as_str(), "resource", index, raw_id.as_str()),
            );
        }

        let mut semantics = BTreeMap::new();
        let mut semantic_svg_ids = BTreeMap::new();
        for (index, semantic) in document.public.semantics.iter().enumerate() {
            let raw_id = semantic.id.clone();
            semantics.insert(raw_id.clone(), semantic);
            semantic_svg_ids.insert(
                raw_id.clone(),
                scoped_id(diagram_id.as_str(), "semantic", index, &raw_id),
            );
        }

        let fallbacks = document
            .public
            .fallbacks
            .iter()
            .map(|fallback| (fallback.id.clone(), fallback))
            .collect();

        let mut sankey_inline_gradients = BTreeSet::new();
        if matches!(document.svg.body, SvgStructureBody::Sankey(_)) {
            let mut counter = document
                .public
                .semantics
                .iter()
                .filter(|semantic| semantic.id.starts_with("sankey.node."))
                .count();
            for command in &document.public.commands {
                session.checkpoint(OperationPhase::Emit)?;
                if let DrawingCommand::DrawPath { path, style } = command
                    && path.as_str().starts_with("sankey.link.")
                    && let Some(Paint::Resource { id }) =
                        style.stroke.as_ref().map(|stroke| &stroke.paint)
                    && matches!(
                        resources.get(id.as_str()),
                        Some(DrawingResource::LinearGradient(_))
                    )
                    && sankey_inline_gradients.insert(id.as_str().to_owned())
                {
                    counter += 1;
                    let prefix = if options.diagram_id.is_some() {
                        format!("{diagram_id}-")
                    } else {
                        String::new()
                    };
                    resource_svg_ids.insert(
                        id.as_str().to_owned(),
                        format!("{prefix}linearGradient-{counter}"),
                    );
                }
            }
        }
        Ok(Self {
            document: &document.public,
            svg_body: &document.svg.body,
            family,
            diagram_type: document.svg.diagram_type(),
            options,
            debug,
            effective_config,
            mermaid_config: matches!(document.svg.body, SvgStructureBody::Mindmap(_))
                .then(|| merman_core::MermaidConfig::from_value(effective_config.clone())),
            session,
            diagram_id,
            resources,
            resource_svg_ids,
            semantics,
            semantic_svg_ids,
            fallbacks,
            error_projection,
            packet_styles: if matches!(document.svg.body, SvgStructureBody::Packet(_)) {
                Some(super::packet::PacketSvgStyles::new(
                    &document.public,
                    session,
                )?)
            } else {
                None
            },
            pie_styles: if let SvgStructureBody::Pie(body) = &document.svg.body {
                Some(super::pie::PieSvgStyles::new(
                    &document.public,
                    body,
                    session,
                )?)
            } else {
                None
            },
            state: GraphicsState::default(),
            cynefin_marker_definitions: BTreeMap::new(),
            cynefin_text_styles: if let SvgStructureBody::Cynefin(body) = &document.svg.body {
                cynefin::shared_text_styles(&document.public, body, session)?
            } else {
                BTreeMap::new()
            },
            cynefin_path_styles: if let SvgStructureBody::Cynefin(body) = &document.svg.body {
                cynefin::shared_path_styles(&document.public, body, session)?
            } else {
                BTreeMap::new()
            },
            sankey_inline_gradients,
            emitted_sankey_gradients: BTreeSet::new(),
            sankey_links: if matches!(document.svg.body, SvgStructureBody::Sankey(_)) {
                sankey::LinkProjection::new(&document.public, session)?
            } else {
                None
            },
            gantt_text: if matches!(document.svg.body, SvgStructureBody::Gantt(_)) {
                Some(gantt::TextProjection::new(&document.public, session)?)
            } else {
                None
            },
            command_index: 0,
            info_text_style: if matches!(document.svg.body, SvgStructureBody::Info(_)) {
                info::shared_text_style(&document.public, session)?
            } else {
                None
            },
            venn_text_styles: if let SvgStructureBody::Venn(body) = &document.svg.body {
                Some(venn::TextStyles::new(&document.public, body, session)?)
            } else {
                None
            },
            sankey_label_style: if matches!(document.svg.body, SvgStructureBody::Sankey(_)) {
                sankey::shared_label_style(&document.public, session)?
            } else {
                None
            },
            saves: Vec::new(),
            groups: Vec::new(),
            semantic_text_counts: BTreeMap::new(),
            output: SvgOutput::new(session),
        })
    }

    fn render(mut self) -> Result<String> {
        let padding = self.options.viewbox_padding;
        if !padding.is_finite() || padding < 0.0 {
            return Err(invalid(
                "SVG viewbox padding must be finite and non-negative",
            ));
        }

        let viewport = self.document.viewport.bounds;
        let viewport_bounds = root_svg::DiagramBounds::from_view_box(
            viewport.x,
            viewport.y,
            viewport.width,
            viewport.height,
        );
        let padded_bounds = root_svg::DiagramBounds::from_view_box(
            viewport.x - padding,
            viewport.y - padding,
            viewport.width + 2.0 * padding,
            viewport.height + 2.0 * padding,
        );
        if ![
            viewport_bounds.min_x,
            viewport_bounds.min_y,
            viewport_bounds.width,
            viewport_bounds.height,
            padded_bounds.min_x,
            padded_bounds.min_y,
            padded_bounds.width,
            padded_bounds.height,
        ]
        .into_iter()
        .all(f64::is_finite)
            || viewport_bounds.width < 0.0
            || viewport_bounds.height < 0.0
            || padded_bounds.width < 0.0
            || padded_bounds.height < 0.0
        {
            return Err(invalid(
                "canonical document viewport cannot be serialized to SVG",
            ));
        }

        let (title, description) = match self.svg_body {
            // Info's version label is the authored payload, not an SVG document title.  The
            // pinned Mermaid renderer keeps the root free of title/ARIA references and emits the
            // label inside the document body instead.
            SvgStructureBody::Error(_)
            | SvgStructureBody::Info(_)
            | SvgStructureBody::Mindmap(_) => (None, None),
            SvgStructureBody::C4(body) => (body.acc_title.clone(), body.acc_description.clone()),
            SvgStructureBody::Wardley(body) => {
                (body.acc_title.clone(), body.acc_description.clone())
            }
            SvgStructureBody::Packet(body) => (
                self.document_semantic().and_then(|semantic| {
                    body.expose_accessibility_title
                        .then(|| semantic.title.clone())
                        .flatten()
                }),
                self.document_semantic()
                    .and_then(|semantic| semantic.description.clone()),
            ),
            SvgStructureBody::Cynefin(body) => (
                self.document_semantic().and_then(|semantic| {
                    body.expose_accessibility_title
                        .then(|| semantic.title.clone())
                        .flatten()
                }),
                self.document_semantic()
                    .and_then(|semantic| semantic.description.clone()),
            ),
            SvgStructureBody::Gantt(body) => (
                self.document_semantic().and_then(|semantic| {
                    let visible_title = self
                        .semantics
                        .get("gantt.title")
                        .and_then(|title| title.title.as_deref())
                        .filter(|title| !title.is_empty())
                        .unwrap_or(body.diagram_type.as_str());
                    // The source omits a redundant root title, but an independent public name
                    // cannot be suppressed merely because the original input lacked accTitle.
                    (body.expose_accessibility_title
                        || semantic
                            .title
                            .as_deref()
                            .is_some_and(|title| title != visible_title))
                    .then(|| semantic.title.clone())
                    .flatten()
                }),
                self.document_semantic()
                    .and_then(|semantic| semantic.description.clone()),
            ),
            #[cfg(feature = "layout-cytoscape")]
            SvgStructureBody::Architecture(body) => {
                (body.acc_title.clone(), body.acc_description.clone())
            }
            _ => (
                self.document_semantic()
                    .and_then(|semantic| semantic.title.clone()),
                self.document_semantic()
                    .and_then(|semantic| semantic.description.clone()),
            ),
        };
        let title_id = title.as_ref().map(|_| self.accessibility_id("title"));
        let description_id = description
            .as_ref()
            .map(|_| self.accessibility_id("description"));

        let root_spec = self.root_spec(viewport_bounds, padded_bounds)?;
        let root_context =
            root_svg::RootViewportContext::new(self.family, self.diagram_id.as_str());
        let mut chrome = root_svg::RootChrome::new(self.diagram_id.as_str(), self.diagram_type);
        chrome.class = self.root_class();
        chrome.aria_labelledby = title_id.as_deref();
        chrome.aria_describedby = description_id.as_deref();
        chrome.dom.trailing_newline = false;
        let radar_extra_attrs = [("overflow", "visible")];
        if matches!(self.svg_body, SvgStructureBody::Radar(_)) {
            chrome.extra_attrs = &radar_extra_attrs;
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::QuadrantChart(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::Pie(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::Timeline(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
        }
        let journey_extra_attrs = [("preserveAspectRatio", "xMinYMin meet")];
        let treemap_extra_attrs = [("class", "flowchart")];
        if matches!(self.svg_body, SvgStructureBody::Journey(_)) {
            chrome.extra_attrs = &journey_extra_attrs;
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
            chrome.dom.responsive_height_placement =
                root_svg::RootResponsiveHeightPlacement::AfterExtraAttrs;
        }
        if matches!(self.svg_body, SvgStructureBody::Treemap(_)) {
            chrome.extra_attrs = &treemap_extra_attrs;
            chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
        }
        if matches!(self.svg_body, SvgStructureBody::Kanban(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::Sankey(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::Requirement(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::State(_)) {
            chrome.dom.aria_attr_order = root_svg::SvgRootAriaAttrOrder::LabelledbyThenDescribedby;
        }
        if matches!(self.svg_body, SvgStructureBody::Er(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::Venn(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::Railroad(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(
            self.svg_body,
            SvgStructureBody::Error(_) | SvgStructureBody::Packet(_) | SvgStructureBody::Pie(_)
        ) {
            chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
        }
        if matches!(self.svg_body, SvgStructureBody::XyChart(_)) {
            chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
        }
        let root_document = root_context.write_open(&mut self.output, root_spec, chrome)?;

        if matches!(
            self.svg_body,
            SvgStructureBody::Packet(_)
                | SvgStructureBody::Pie(_)
                | SvgStructureBody::Cynefin(_)
                | SvgStructureBody::Venn(_)
        ) {
            self.write_accessibility_metadata(
                title.as_deref(),
                description.as_deref(),
                title_id.as_deref(),
                description_id.as_deref(),
            )?;
        }
        self.write_family_style()?;
        if matches!(
            self.svg_body,
            SvgStructureBody::Error(_) | SvgStructureBody::Info(_)
        ) {
            // Preserve the empty structural group emitted before the family body by Mermaid's
            // Error and Info renderers. It is part of their established DOM shape and carries no
            // visual content.
            self.output.push_str("<g/>")?;
        }
        self.write_defs()?;
        if !matches!(
            self.svg_body,
            SvgStructureBody::Packet(_)
                | SvgStructureBody::Pie(_)
                | SvgStructureBody::Cynefin(_)
                | SvgStructureBody::Venn(_)
        ) {
            self.write_accessibility_metadata(
                title.as_deref(),
                description.as_deref(),
                title_id.as_deref(),
                description_id.as_deref(),
            )?;
        }
        if matches!(self.svg_body, SvgStructureBody::Cynefin(_)) {
            self.write_cynefin_accessibility_copies(title.as_deref(), description.as_deref())?;
        }
        let root_background = self.document_background_is_root_paint();
        let mut consumed_until = 0;
        for (index, command) in self.document.commands.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.command_index = index;
            if index < consumed_until {
                continue;
            }
            if root_background && index == 2 {
                continue;
            }
            if let Some(count) = self.emit_gantt_section_text(index)? {
                consumed_until = index + count;
                continue;
            }
            if let Some(count) = self.emit_venn_text_node(index)? {
                consumed_until = index + count;
                continue;
            }
            if let Some(count) = self.emit_venn_rough_group(index)? {
                consumed_until = index + count;
                continue;
            }
            if let Some(count) = self.emit_venn_fill_and_stroke(index)? {
                consumed_until = index + count;
                continue;
            }
            if matches!(self.svg_body, SvgStructureBody::Cynefin(_))
                && self.emit_cynefin_fill_and_stroke(index)?
            {
                consumed_until = index + 5;
                continue;
            }
            if matches!(self.svg_body, SvgStructureBody::Cynefin(_))
                && self.emit_cynefin_marked_edge(index)?
            {
                consumed_until = index + 5;
                continue;
            }
            self.emit_command(command)?;
        }
        if matches!(self.svg_body, SvgStructureBody::Cynefin(_)) {
            self.write_cynefin_marker_defs()?;
        }

        // Mermaid places Wardley's marker definitions after the map layers.  They are referenced
        // by the canonical line elements above, and SVG permits forward references, so retain
        // that source-backed root order without changing the renderer-neutral command stream.
        if matches!(self.svg_body, SvgStructureBody::Wardley(_)) {
            self.write_wardley_marker_defs()?;
        }
        if matches!(self.svg_body, SvgStructureBody::Er(_)) {
            self.write_er_marker_defs()?;
        }

        if !self.saves.is_empty() || !self.groups.is_empty() {
            return Err(invalid(
                "canonical SVG command stream ended with unbalanced state",
            ));
        }
        self.output.push_str("</svg>")?;
        root_document
            .complete(self.output.finish()?)?
            .into_string_for(self.family)
    }

    fn root_spec(
        &self,
        viewport_bounds: root_svg::DiagramBounds,
        padded_bounds: root_svg::DiagramBounds,
    ) -> Result<root_svg::RootViewportSpec> {
        match self.svg_body {
            SvgStructureBody::Info(_) => {
                let spec =
                    root_svg::RootViewportSpec::responsive_without_view_box(viewport_bounds.width);
                Ok(if self.document_background_is_root_paint() {
                    spec
                } else {
                    spec.without_background()
                })
            }
            SvgStructureBody::Error(body) => {
                if !body.max_width_px.is_finite() || body.max_width_px <= 0.0 {
                    return Err(invalid("canonical error SVG max-width is invalid"));
                }
                Ok(root_svg::RootViewportSpec::responsive(viewport_bounds)
                    .with_max_width(root_svg::RootMaxWidth::SvgNumber(body.max_width_px))
                    .without_background())
            }
            SvgStructureBody::Packet(_) => {
                let spec = root_svg::RootViewportSpec::responsive(viewport_bounds).with_max_width(
                    root_svg::RootMaxWidth::CssSixSignificant(viewport_bounds.width),
                );
                Ok(if self.document_background_is_root_paint() {
                    spec
                } else {
                    spec.without_background()
                })
            }
            SvgStructureBody::Mindmap(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::QuadrantChart(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            SvgStructureBody::Pie(body) => {
                let spec = root_svg::RootViewportSpec::mermaid(viewport_bounds, body.use_max_width)
                    .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                        viewport_bounds.width,
                    ));
                Ok(if self.document_background_is_root_paint() {
                    spec
                } else {
                    spec.without_background()
                })
            }
            SvgStructureBody::Timeline(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::Journey(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_mermaid_responsive_height(body.use_max_width, body.svg_height)
            .with_fixed_size(body.width, body.svg_height)
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(body.width))),
            SvgStructureBody::Treemap(_) => Ok(root_svg::RootViewportSpec::responsive(
                viewport_bounds,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::Kanban(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::Sankey(body) => {
                let spec = root_svg::RootViewportSpec::mermaid(viewport_bounds, body.use_max_width)
                    .with_max_width(root_svg::RootMaxWidth::SvgNumber(viewport_bounds.width));
                Ok(if self.document_background_is_root_paint() {
                    spec
                } else {
                    spec.without_background()
                })
            }
            SvgStructureBody::Requirement(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::State(_) => Ok(root_svg::RootViewportSpec::responsive(
                viewport_bounds,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::Er(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::Venn(body) => {
                let spec = root_svg::RootViewportSpec::mermaid(viewport_bounds, body.use_max_width);
                Ok(if self.document_background_is_root_paint() {
                    spec
                } else {
                    spec.without_background()
                })
            }
            SvgStructureBody::Railroad(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            SvgStructureBody::EventModeling(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::SvgNumber(viewport_bounds.width))),
            SvgStructureBody::Ishikawa(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            SvgStructureBody::Cynefin(body) => {
                let spec = root_svg::RootViewportSpec::mermaid(viewport_bounds, body.use_max_width);
                Ok(if self.document_background_is_root_paint() {
                    spec
                } else {
                    spec.without_background()
                })
            }
            SvgStructureBody::TreeView(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            SvgStructureBody::Gantt(_) => {
                let spec = root_svg::RootViewportSpec::responsive(viewport_bounds)
                    .with_max_width(root_svg::RootMaxWidth::SvgNumber(viewport_bounds.width));
                Ok(if self.document_background_is_root_paint() {
                    spec
                } else {
                    spec.without_background()
                })
            }
            SvgStructureBody::Radar(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::XyChart(_) => Ok(root_svg::RootViewportSpec::responsive(
                viewport_bounds,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::GitGraph(_) => {
                Ok(root_svg::RootViewportSpec::responsive(viewport_bounds)
                    .with_max_width(root_svg::RootMaxWidth::SvgNumber(viewport_bounds.width)))
            }
            SvgStructureBody::Block(_) => Ok(root_svg::RootViewportSpec::responsive(
                viewport_bounds,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::Wardley(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            SvgStructureBody::C4(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::SvgNumber(viewport_bounds.width))),
            SvgStructureBody::Zenuml(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            #[cfg(feature = "layout-cytoscape")]
            SvgStructureBody::Architecture(body) => {
                Ok(root_svg::RootViewportSpec::mermaid_or_intrinsic(
                    viewport_bounds,
                    body.use_max_width,
                ))
            }
            _ => Ok(
                root_svg::RootViewportSpec::responsive(padded_bounds).with_max_width(
                    root_svg::RootMaxWidth::CssSixSignificant(padded_bounds.width),
                ),
            ),
        }
    }

    /// Project the first full-viewport white path into Mermaid's root background CSS. Only an
    /// exact, untransformed paint can take this DOM-preserving form; edited documents retain the
    /// ordinary path and a transparent root instead.
    fn document_background_is_root_paint(&self) -> bool {
        let (document_id, background_id) = match self.svg_body {
            SvgStructureBody::Info(_) => ("info.document", "info.background"),
            SvgStructureBody::Packet(_) => ("packet.document", "packet.background"),
            SvgStructureBody::Pie(_) => ("pie.document", "pie.background"),
            SvgStructureBody::Cynefin(_) => ("cynefin.document", "cynefin.background"),
            SvgStructureBody::Venn(_) => ("venn.document", "venn.background"),
            SvgStructureBody::Sankey(_) => ("sankey.document", "sankey.background"),
            SvgStructureBody::Gantt(_) => ("gantt.document", "gantt.background"),
            _ => return false,
        };
        let [
            DrawingCommand::Save,
            DrawingCommand::BeginSemanticGroup { semantic_id },
            DrawingCommand::DrawPath { path, style },
            ..,
        ] = self.document.commands.as_slice()
        else {
            return false;
        };
        if semantic_id != document_id
            || path.as_str() != background_id
            || style.stroke.is_some()
            || style.fill != Some(Paint::solid(Color::rgba(255, 255, 255, 255)))
        {
            return false;
        }
        self.path_resource(path).ok().and_then(rectangle_from_path)
            == Some(self.document.viewport.bounds)
    }

    fn write_family_style(&mut self) -> Result<()> {
        if matches!(self.svg_body, SvgStructureBody::Info(_)) {
            return self.write_info_style();
        }
        if matches!(self.svg_body, SvgStructureBody::Cynefin(_)) {
            return self.write_cynefin_styles();
        }
        if matches!(self.svg_body, SvgStructureBody::Venn(_)) {
            return self.write_venn_style();
        }
        if matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            return self.write_gantt_style();
        }
        let css = match self.svg_body {
            SvgStructureBody::Error(_) => Some((
                false,
                super::error::canonical_error_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                    self.error_projection.as_ref().ok_or_else(|| {
                        invalid("Error SVG is missing its validated document projection")
                    })?,
                )?,
            )),
            SvgStructureBody::Packet(_) => Some((
                false,
                self.packet_styles
                    .as_ref()
                    .ok_or_else(|| invalid("Packet styles are missing"))?
                    .css(self.diagram_id.as_str())?,
            )),
            SvgStructureBody::Mindmap(_) => Some((
                false,
                super::mindmap::canonical_mindmap_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            SvgStructureBody::QuadrantChart(_) => Some((
                false,
                super::info_css_with_config(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::Pie(_) => Some((
                false,
                self.pie_styles
                    .as_ref()
                    .ok_or_else(|| invalid("missing Pie styles"))?
                    .css(&self.diagram_id)?,
            )),
            SvgStructureBody::Timeline(_) => Some((
                false,
                super::timeline::canonical_timeline_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            // Journey commands already resolve the theme's paints and fonts. Replaying the
            // legacy CSS would apply section background fills to the visible text as well.
            SvgStructureBody::Journey(_) => None,
            SvgStructureBody::Treemap(_) => Some((
                false,
                super::treemap::canonical_treemap_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                )?,
            )),
            SvgStructureBody::Kanban(_) => Some((
                false,
                super::kanban::canonical_kanban_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                )?,
            )),
            SvgStructureBody::Sankey(_) => Some((false, self.sankey_css()?)),
            SvgStructureBody::Requirement(_) => Some((
                false,
                super::requirement_css(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::State(_) => Some((
                false,
                super::state::canonical_state_css(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::Er(_) => Some((
                false,
                super::er_css(self.diagram_id.as_str(), self.effective_config)?,
            )),
            SvgStructureBody::Railroad(_) => Some((
                false,
                super::railroad::canonical_railroad_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            SvgStructureBody::EventModeling(_) => Some((
                false,
                super::eventmodeling::canonical_eventmodeling_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            SvgStructureBody::Ishikawa(body) => Some((
                false,
                super::ishikawa::canonical_ishikawa_css(
                    self.diagram_id.as_str(),
                    body.font_size,
                    self.effective_config,
                ),
            )),
            SvgStructureBody::TreeView(_) => Some((
                false,
                super::tree_view::canonical_tree_view_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            SvgStructureBody::XyChart(_) => {
                let mut css = String::new();
                super::push_xychart_css(&mut css, self.diagram_id.as_str());
                Some((false, css))
            }
            SvgStructureBody::GitGraph(_) => Some((
                false,
                super::gitgraph::canonical_gitgraph_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            SvgStructureBody::Radar(_) => Some((
                false,
                super::radar::canonical_radar_css(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::Block(body) => Some((
                false,
                super::block::block_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                    &body.class_defs,
                    || self.session.checkpoint(OperationPhase::Emit),
                )?,
            )),
            SvgStructureBody::C4(_) => Some((
                false,
                super::c4::c4_css(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::Zenuml(_) => Some((false, super::zenuml::zenuml_css().to_string())),
            #[cfg(feature = "layout-cytoscape")]
            SvgStructureBody::Architecture(_) => Some((
                false,
                super::css::architecture_css_with_config(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            _ => None,
        };
        let Some((xhtml_namespace, css)) = css else {
            return Ok(());
        };
        if xhtml_namespace {
            self.output
                .push_str(r#"<style xmlns="http://www.w3.org/1999/xhtml">"#)?;
        } else {
            self.output.push_str("<style>")?;
        }
        self.session
            .work_meter()
            .preflight_svg_byte_count(
                css.len(),
                crate::resources::ResourceLimitPhase::SvgOutput,
                OperationPhase::Emit,
            )
            .map_err(Error::from)?;
        self.output.push_str(&css)?;
        self.output.push_str("</style>")?;
        #[cfg(feature = "layout-cytoscape")]
        let architecture = matches!(self.svg_body, SvgStructureBody::Architecture(_));
        #[cfg(not(feature = "layout-cytoscape"))]
        let architecture = false;
        if architecture
            || matches!(
                self.svg_body,
                SvgStructureBody::Packet(_)
                    | SvgStructureBody::QuadrantChart(_)
                    | SvgStructureBody::Pie(_)
                    | SvgStructureBody::Timeline(_)
                    | SvgStructureBody::Journey(_)
                    | SvgStructureBody::Treemap(_)
                    | SvgStructureBody::Kanban(_)
                    | SvgStructureBody::Sankey(_)
                    | SvgStructureBody::Requirement(_)
                    | SvgStructureBody::State(_)
                    | SvgStructureBody::Er(_)
                    | SvgStructureBody::Railroad(_)
                    | SvgStructureBody::EventModeling(_)
                    | SvgStructureBody::Ishikawa(_)
                    | SvgStructureBody::Cynefin(_)
                    | SvgStructureBody::TreeView(_)
                    | SvgStructureBody::Gantt(_)
                    | SvgStructureBody::GitGraph(_)
                    | SvgStructureBody::Radar(_)
                    | SvgStructureBody::XyChart(_)
                    | SvgStructureBody::Block(_)
                    | SvgStructureBody::C4(_)
            )
        {
            self.output.push_str("<g/>")?;
        }
        Ok(())
    }

    fn root_class(&self) -> Option<&'static str> {
        match self.svg_body {
            SvgStructureBody::Railroad(_) => Some("railroad-diagram"),
            SvgStructureBody::GitGraph(_) => Some("gitGraph"),
            SvgStructureBody::Requirement(_) => Some("requirementDiagram"),
            SvgStructureBody::State(_) => Some("statediagram"),
            SvgStructureBody::Er(_) => Some("erDiagram"),
            SvgStructureBody::Mindmap(_) => Some("mindmapDiagram"),
            _ => None,
        }
    }

    fn accessibility_id(&self, suffix: &str) -> String {
        match self.svg_body {
            #[cfg(feature = "layout-cytoscape")]
            SvgStructureBody::Architecture(_) => {
                let suffix = match suffix {
                    "description" => "desc",
                    other => other,
                };
                format!("chart-{suffix}-{}", self.diagram_id)
            }
            SvgStructureBody::Packet(_)
            | SvgStructureBody::QuadrantChart(_)
            | SvgStructureBody::Pie(_)
            | SvgStructureBody::Timeline(_)
            | SvgStructureBody::Journey(_)
            | SvgStructureBody::Kanban(_)
            | SvgStructureBody::Venn(_)
            | SvgStructureBody::Railroad(_)
            | SvgStructureBody::EventModeling(_)
            | SvgStructureBody::Cynefin(_)
            | SvgStructureBody::TreeView(_)
            | SvgStructureBody::Gantt(_)
            | SvgStructureBody::GitGraph(_)
            | SvgStructureBody::Radar(_)
            | SvgStructureBody::Treemap(_)
            | SvgStructureBody::Requirement(_)
            | SvgStructureBody::State(_)
            | SvgStructureBody::Er(_)
            | SvgStructureBody::XyChart(_)
            | SvgStructureBody::Wardley(_)
            | SvgStructureBody::C4(_) => {
                let suffix = match suffix {
                    "description" => "desc",
                    other => other,
                };
                format!("chart-{suffix}-{}", self.diagram_id)
            }
            _ => format!("{}-{suffix}", self.diagram_id),
        }
    }

    fn document_semantic(&self) -> Option<&SemanticAnnotation> {
        self.document
            .semantics
            .iter()
            .find(|semantic| semantic.role == SemanticRole::Document)
    }

    fn write_accessibility_metadata(
        &mut self,
        title: Option<&str>,
        description: Option<&str>,
        title_id: Option<&str>,
        description_id: Option<&str>,
    ) -> Result<()> {
        if let (Some(title), Some(id)) = (title, title_id) {
            self.output.push_str("<title id=\"")?;
            output::escape_attr(&mut self.output, id)?;
            self.output.push_str("\">")?;
            output::escape_xml(&mut self.output, title)?;
            self.output.push_str("</title>")?;
        }
        if let (Some(description), Some(id)) = (description, description_id) {
            self.output.push_str("<desc id=\"")?;
            output::escape_attr(&mut self.output, id)?;
            self.output.push_str("\">")?;
            output::escape_xml(&mut self.output, description)?;
            self.output.push_str("</desc>")?;
        }
        Ok(())
    }

    fn write_defs(&mut self) -> Result<()> {
        if matches!(self.svg_body, SvgStructureBody::Mindmap(_)) {
            self.output
                .push_str(&super::mindmap::mindmap_gradient_defs(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ))?;
            return Ok(());
        }
        let clip_paths = self
            .document
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawingCommand::ClipPath { path, .. } => Some(path.as_str().to_owned()),
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let has_defs = self.resources.iter().any(|(raw_id, resource)| {
            if self.sankey_inline_gradients.contains(raw_id) {
                return false;
            }
            matches!(
                resource,
                DrawingResource::LinearGradient(_)
                    | DrawingResource::RadialGradient(_)
                    | DrawingResource::Pattern(_)
                    | DrawingResource::Font(_)
            ) || matches!(resource, DrawingResource::Path(_)) && clip_paths.contains(raw_id)
        });
        if !has_defs {
            return Ok(());
        }
        self.output.push_str("<defs>")?;
        let resources = self
            .resources
            .iter()
            .map(|(raw_id, resource)| (raw_id.clone(), *resource))
            .collect::<Vec<_>>();
        for (raw_id, resource) in resources {
            if self.sankey_inline_gradients.contains(&raw_id) {
                continue;
            }
            if matches!(resource, DrawingResource::Path(_)) && !clip_paths.contains(&raw_id) {
                continue;
            }
            let svg_id = self.svg_resource_id(raw_id.as_str())?;
            match resource {
                DrawingResource::LinearGradient(gradient) => {
                    self.write_linear_gradient(svg_id.as_str(), gradient)?;
                }
                DrawingResource::RadialGradient(gradient) => {
                    self.write_radial_gradient(svg_id.as_str(), gradient)?;
                }
                DrawingResource::Pattern(pattern) => {
                    self.write_pattern(svg_id.as_str(), pattern)?;
                }
                DrawingResource::Font(font) => {
                    self.write_font(svg_id.as_str(), font)?;
                }
                DrawingResource::Path(_) | DrawingResource::Image(_) => {}
            }
        }

        // ClipPath is a command/resource relationship.  Defining the path once keeps command
        // emission renderer-neutral and avoids copying geometry into every clipped element.
        let paths = self
            .resources
            .iter()
            .filter_map(|(raw_id, resource)| match *resource {
                DrawingResource::Path(path) if clip_paths.contains(raw_id) => {
                    Some((raw_id.clone(), path.segments.as_slice()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (raw_id, segments) in paths {
            let clip_id = self.clip_svg_id(raw_id.as_str())?;
            self.output.push_str("<clipPath id=\"")?;
            output::escape_attr(&mut self.output, clip_id.as_str())?;
            self.output
                .push_str("\" clipPathUnits=\"userSpaceOnUse\">")?;
            self.output.push_str("<path d=\"")?;
            let path_data = path_d(segments);
            output::escape_attr(&mut self.output, &path_data)?;
            self.output.push_str("\"/>")?;
            self.output.push_str("</clipPath>")?;
        }
        self.output.push_str("</defs>")?;
        Ok(())
    }

    fn write_wardley_marker_defs(&mut self) -> Result<()> {
        let theme = WardleyTheme::from_config(self.effective_config);
        self.output.push_str("<defs>")?;
        write!(
            self.output,
            "<marker id=\"arrow-{}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path d=\"M 0 0 L 10 5 L 0 10 z\" fill=\"{}\" stroke=\"none\"/></marker><marker id=\"link-arrow-end-{}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"5\" markerHeight=\"5\" orient=\"auto\"><path d=\"M 0 0 L 10 5 L 0 10 z\" fill=\"{}\" stroke=\"none\"/></marker><marker id=\"link-arrow-start-{}\" viewBox=\"0 0 10 10\" refX=\"1\" refY=\"5\" markerWidth=\"5\" markerHeight=\"5\" orient=\"auto\"><path d=\"M 10 0 L 0 5 L 10 10 z\" fill=\"{}\" stroke=\"none\"/></marker>",
            escaped_attr(self.diagram_id.as_str()),
            escaped_attr(theme.evolution_stroke.as_str()),
            escaped_attr(self.diagram_id.as_str()),
            escaped_attr(theme.link_stroke.as_str()),
            escaped_attr(self.diagram_id.as_str()),
            escaped_attr(theme.link_stroke.as_str()),
        )?;
        self.output.push_str("</defs>")?;
        Ok(())
    }

    fn write_er_marker_defs(&mut self) -> Result<()> {
        let SvgStructureBody::Er(body) = self.svg_body else {
            return Ok(());
        };
        if body.marker_types.is_empty() {
            return Ok(());
        }
        let diagram_id = escaped_attr(self.diagram_id.as_str());
        let diagram_type = escaped_attr(body.diagram_type.as_str());
        self.output.push_str("<defs>")?;
        for marker_type in &body.marker_types {
            match marker_type.as_str() {
                "mdParent" => {
                    write!(
                        self.output,
                        "<marker id=\"{diagram_id}_{diagram_type}-mdParentStart\" class=\"marker mdParent er\" refX=\"0\" refY=\"7\" markerWidth=\"190\" markerHeight=\"240\" orient=\"auto\"><path d=\"M 18,7 L9,13 L1,7 L9,1 Z\"/></marker><marker id=\"{diagram_id}_{diagram_type}-mdParentEnd\" class=\"marker mdParent er\" refX=\"19\" refY=\"7\" markerWidth=\"20\" markerHeight=\"28\" orient=\"auto\"><path d=\"M 18,7 L9,13 L1,7 L9,1 Z\"/></marker>"
                    )?;
                }
                "onlyOne" => {
                    write!(
                        self.output,
                        "<marker id=\"{diagram_id}_{diagram_type}-onlyOneStart\" class=\"marker onlyOne er\" refX=\"0\" refY=\"9\" markerWidth=\"18\" markerHeight=\"18\" orient=\"auto\"><path d=\"M9,0 L9,18 M15,0 L15,18\"/></marker><marker id=\"{diagram_id}_{diagram_type}-onlyOneEnd\" class=\"marker onlyOne er\" refX=\"18\" refY=\"9\" markerWidth=\"18\" markerHeight=\"18\" orient=\"auto\"><path d=\"M3,0 L3,18 M9,0 L9,18\"/></marker>"
                    )?;
                }
                "zeroOrOne" => {
                    write!(
                        self.output,
                        "<marker id=\"{diagram_id}_{diagram_type}-zeroOrOneStart\" class=\"marker zeroOrOne er\" refX=\"0\" refY=\"9\" markerWidth=\"30\" markerHeight=\"18\" orient=\"auto\"><circle fill=\"white\" cx=\"21\" cy=\"9\" r=\"6\"/><path d=\"M9,0 L9,18\"/></marker><marker id=\"{diagram_id}_{diagram_type}-zeroOrOneEnd\" class=\"marker zeroOrOne er\" refX=\"30\" refY=\"9\" markerWidth=\"30\" markerHeight=\"18\" orient=\"auto\"><circle fill=\"white\" cx=\"9\" cy=\"9\" r=\"6\"/><path d=\"M21,0 L21,18\"/></marker>"
                    )?;
                }
                "oneOrMore" => {
                    write!(
                        self.output,
                        "<marker id=\"{diagram_id}_{diagram_type}-oneOrMoreStart\" class=\"marker oneOrMore er\" refX=\"18\" refY=\"18\" markerWidth=\"45\" markerHeight=\"36\" orient=\"auto\"><path d=\"M0,18 Q 18,0 36,18 Q 18,36 0,18 M42,9 L42,27\"/></marker><marker id=\"{diagram_id}_{diagram_type}-oneOrMoreEnd\" class=\"marker oneOrMore er\" refX=\"27\" refY=\"18\" markerWidth=\"45\" markerHeight=\"36\" orient=\"auto\"><path d=\"M3,9 L3,27 M9,18 Q27,0 45,18 Q27,36 9,18\"/></marker>"
                    )?;
                }
                "zeroOrMore" => {
                    write!(
                        self.output,
                        "<marker id=\"{diagram_id}_{diagram_type}-zeroOrMoreStart\" class=\"marker zeroOrMore er\" refX=\"18\" refY=\"18\" markerWidth=\"57\" markerHeight=\"36\" orient=\"auto\"><circle fill=\"white\" cx=\"48\" cy=\"18\" r=\"6\"/><path d=\"M0,18 Q18,0 36,18 Q18,36 0,18\"/></marker><marker id=\"{diagram_id}_{diagram_type}-zeroOrMoreEnd\" class=\"marker zeroOrMore er\" refX=\"39\" refY=\"18\" markerWidth=\"57\" markerHeight=\"36\" orient=\"auto\"><circle fill=\"white\" cx=\"9\" cy=\"18\" r=\"6\"/><path d=\"M21,18 Q39,0 57,18 Q39,36 21,18\"/></marker>"
                    )?;
                }
                _ => {}
            }
        }
        self.output.push_str("</defs>")?;
        Ok(())
    }

    fn write_linear_gradient(
        &mut self,
        id: &str,
        gradient: &merman_display_list::LinearGradientResource,
    ) -> Result<()> {
        let compact = matches!(self.svg_body, SvgStructureBody::Sankey(_));
        write!(
            self.output,
            "<linearGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" x2=\"{}\"",
            escaped_attr(id),
            fmt(gradient.start.x),
            fmt(gradient.end.x),
        )?;
        for (name, value) in [("y1", gradient.start.y), ("y2", gradient.end.y)] {
            if !compact || value != 0.0 {
                write!(self.output, " {name}=\"{}\"", fmt(value))?;
            }
        }
        if !compact || gradient.transform != Transform::IDENTITY {
            write!(
                self.output,
                " gradientTransform=\"matrix({})\"",
                matrix_attr(gradient.transform)
            )?;
        }
        if !compact || gradient.spread != GradientSpread::Pad {
            write!(
                self.output,
                " spreadMethod=\"{}\"",
                spread_method(gradient.spread)
            )?;
        }
        self.output.push('>')?;
        self.write_stops(&gradient.stops, compact)?;
        self.output.push_str("</linearGradient>")?;
        Ok(())
    }

    fn write_radial_gradient(
        &mut self,
        id: &str,
        gradient: &merman_display_list::RadialGradientResource,
    ) -> Result<()> {
        write!(
            self.output,
            "<radialGradient id=\"{}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" cy=\"{}\" fx=\"{}\" fy=\"{}\" r=\"{}\" gradientTransform=\"matrix({})\" spreadMethod=\"{}\">",
            escaped_attr(id),
            fmt(gradient.center.x),
            fmt(gradient.center.y),
            fmt(gradient.focal.x),
            fmt(gradient.focal.y),
            fmt(gradient.radius),
            matrix_attr(gradient.transform),
            spread_method(gradient.spread),
        )?;
        self.write_stops(&gradient.stops, false)?;
        self.output.push_str("</radialGradient>")?;
        Ok(())
    }

    fn write_stops(
        &mut self,
        stops: &[merman_display_list::GradientStop],
        percentage_offsets: bool,
    ) -> Result<()> {
        for stop in stops {
            let color = color_css(stop.color);
            write!(
                self.output,
                "<stop offset=\"{}{}\" stop-color=\"{}\"{} />",
                fmt(if percentage_offsets {
                    stop.offset * 100.0
                } else {
                    stop.offset
                }),
                if percentage_offsets { "%" } else { "" },
                color,
                opacity_attr(stop.color.alpha),
            )?;
        }
        Ok(())
    }

    fn write_pattern(
        &mut self,
        id: &str,
        pattern: &merman_display_list::PatternResource,
    ) -> Result<()> {
        let (image_uri, pixel_width, pixel_height) = {
            let image = self.image_resource(&pattern.image)?;
            (
                data_uri(&image.image.media_type, &image.image.data),
                image.pixel_width,
                image.pixel_height,
            )
        };
        let (width, height) = match pattern.repetition {
            merman_display_list::PatternRepeat::Repeat => (pattern.tile.width, pattern.tile.height),
            merman_display_list::PatternRepeat::RepeatX => (
                pattern.tile.width,
                self.document
                    .viewport
                    .bounds
                    .height
                    .max(pattern.tile.height),
            ),
            merman_display_list::PatternRepeat::RepeatY => (
                self.document.viewport.bounds.width.max(pattern.tile.width),
                pattern.tile.height,
            ),
            merman_display_list::PatternRepeat::NoRepeat => (
                self.document.viewport.bounds.width.max(pattern.tile.width),
                self.document
                    .viewport
                    .bounds
                    .height
                    .max(pattern.tile.height),
            ),
        };
        write!(
            self.output,
            "<pattern id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" patternUnits=\"userSpaceOnUse\" patternContentUnits=\"userSpaceOnUse\" patternTransform=\"matrix({})\">",
            escaped_attr(id),
            fmt(pattern.tile.x),
            fmt(pattern.tile.y),
            fmt(width),
            fmt(height),
            matrix_attr(pattern.transform),
        )?;
        write!(
            self.output,
            "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"/>",
            escaped_attr(image_uri),
            fmt(pattern.tile.x),
            fmt(pattern.tile.y),
            fmt(f64::from(pixel_width)),
            fmt(f64::from(pixel_height)),
        )?;
        self.output.push_str("</pattern>")?;
        Ok(())
    }

    fn write_font(&mut self, id: &str, font: &merman_display_list::FontResource) -> Result<()> {
        let media_type = safe_media_type(&font.font.media_type);
        let encoded = base64::display::Base64Display::new(&font.font.data, &STANDARD);
        write!(
            self.output,
            "<style>@font-face{{font-family:\"{}\";src:url(\"data:{};base64,{}\");}}</style>",
            escaped_css_string(id),
            escaped_css_string(media_type.as_str()),
            encoded,
        )?;
        Ok(())
    }

    fn emit_command(&mut self, command: &DrawingCommand) -> Result<()> {
        match command {
            DrawingCommand::Save => {
                self.saves.push(SavePoint {
                    state: self.state,
                    group_len: self.groups.len(),
                });
            }
            DrawingCommand::Restore => self.restore_state()?,
            DrawingCommand::SetOpacity { opacity } => self.state.opacity = *opacity,
            DrawingCommand::SetBlendMode { blend_mode } => self.state.blend_mode = *blend_mode,
            DrawingCommand::ConcatTransform { transform } => {
                self.state.transform = multiply_transform(self.state.transform, *transform);
            }
            DrawingCommand::BeginLayer {
                bounds,
                opacity,
                blend_mode,
            } => self.begin_layer(*bounds, *opacity, *blend_mode)?,
            DrawingCommand::EndLayer => self.end_layer()?,
            DrawingCommand::DrawPath { path, style } => self.emit_path(path, style)?,
            DrawingCommand::ClipPath { path, fill_rule } => self.begin_clip(path, *fill_rule)?,
            DrawingCommand::DrawImage {
                image,
                bounds,
                opacity,
            } => self.emit_image(image, *bounds, *opacity, None)?,
            DrawingCommand::BeginSemanticGroup { semantic_id } => {
                self.begin_semantic_group(semantic_id)?
            }
            DrawingCommand::EndSemanticGroup => self.end_semantic_group()?,
            DrawingCommand::DrawText { run } => self.emit_text(run)?,
            DrawingCommand::DrawRasterSubtree { fallback_id } => {
                self.emit_fallback(fallback_id, None)?
            }
        }
        Ok(())
    }

    fn restore_state(&mut self) -> Result<()> {
        let save = self
            .saves
            .pop()
            .ok_or_else(|| invalid("SVG restore has no matching save"))?;
        while self.groups.len() > save.group_len {
            match self.groups.pop() {
                Some(GroupKind::Clip) => self.output.push_str("</g>")?,
                Some(GroupKind::Semantic { .. }) | Some(GroupKind::Layer { .. }) => {
                    return Err(invalid(
                        "SVG restore crossed an open semantic or layer group",
                    ))?;
                }
                None => break,
            }
        }
        if self.groups.len() != save.group_len {
            return Err(invalid("SVG restore group scope is unbalanced"));
        }
        self.state = save.state;
        Ok(())
    }

    fn begin_layer(&mut self, bounds: Rect, opacity: f64, blend_mode: BlendMode) -> Result<()> {
        if self.begin_gantt_tick_layer(opacity, blend_mode)? {
            return Ok(());
        }
        write!(
            self.output,
            "<g class=\"merman-layer\" data-bounds=\"{},{},{},{}\" opacity=\"{}\"",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(opacity),
        )?;
        write_blend_style(&mut self.output, blend_mode)?;
        self.output.push('>')?;
        self.groups.push(GroupKind::Layer {
            projected_transform: Transform::IDENTITY,
        });
        Ok(())
    }

    fn end_layer(&mut self) -> Result<()> {
        match self.groups.pop() {
            Some(GroupKind::Layer {
                projected_transform,
            }) => {
                if projected_transform != Transform::IDENTITY {
                    self.state.transform =
                        multiply_transform(projected_transform, self.state.transform);
                }
                self.output.push_str("</g>")?;
                Ok(())
            }
            Some(other) => {
                self.groups.push(other);
                Err(invalid("SVG layer end does not match the open group"))
            }
            None => Err(invalid("SVG layer end has no matching layer")),
        }
    }

    fn begin_clip(&mut self, path: &ResourceId, fill_rule: FillRule) -> Result<()> {
        if self.saves.is_empty() {
            return Err(invalid("SVG clip path must be scoped by save/restore"));
        }
        let clip_id = self.clip_svg_id(path.as_str())?;
        write!(
            self.output,
            "<g clip-path=\"url(#{})\" data-fill-rule=\"{}\"",
            escaped_attr(clip_id.as_str()),
            fill_rule_name(fill_rule),
        )?;
        // Clip resources use user-space coordinates. Preserve the transform that was active at
        // the ClipPath command on the wrapper group, then render clipped children relative to the
        // new group coordinate system. This keeps local family clips (for example Treemap label
        // cells) aligned with their translated content.
        if self.state.transform != Transform::IDENTITY {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(self.state.transform),
            )?;
            self.state.transform = Transform::IDENTITY;
        }
        self.output.push('>')?;
        self.groups.push(GroupKind::Clip);
        Ok(())
    }

    fn begin_semantic_group(&mut self, semantic_id: &str) -> Result<()> {
        let semantic = self.semantics.get(semantic_id).cloned().ok_or_else(|| {
            invalid(format!(
                "SVG semantic group references unknown id {semantic_id}"
            ))
        })?;
        if matches!(self.svg_body, SvgStructureBody::Mindmap(_)) {
            return self.begin_mindmap_semantic_group(semantic_id);
        }
        if matches!(self.svg_body, SvgStructureBody::Cynefin(_)) {
            return self.begin_cynefin_semantic_group(semantic_id, semantic.role);
        }
        if matches!(self.svg_body, SvgStructureBody::Sankey(_)) {
            return self.begin_sankey_semantic_group(semantic_id);
        }
        if self.begin_venn_semantic_group(semantic_id, semantic)? {
            return Ok(());
        }
        if self.begin_gantt_task(semantic_id)? {
            return Ok(());
        }
        if self.begin_gantt_collection(semantic_id)? {
            return Ok(());
        }
        if matches!(self.svg_body, SvgStructureBody::Pie(_)) {
            let emitted = semantic_id == "pie.content"
                || semantic_id == "pie.plot"
                || semantic_id.starts_with("pie.legend.");
            let projected_transform = if emitted {
                self.state.transform
            } else {
                Transform::IDENTITY
            };
            if emitted {
                self.output.push_str("<g")?;
                if let Some(class) = self.semantic_extra_class(semantic_id).map(str::to_owned) {
                    write!(self.output, " class=\"{}\"", escaped_attr(&class))?;
                }
                if self.state.transform != Transform::IDENTITY {
                    let transform = self.state.transform;
                    if transform.a == 1.0
                        && transform.d == 1.0
                        && transform.b == 0.0
                        && transform.c == 0.0
                    {
                        write!(
                            self.output,
                            " transform=\"translate({},{})\"",
                            fmt(transform.e),
                            fmt(transform.f)
                        )?;
                    } else {
                        write!(
                            self.output,
                            " transform=\"matrix({})\"",
                            matrix_attr(transform)
                        )?;
                    }
                    self.state.transform = Transform::IDENTITY;
                }
                self.output.push('>')?;
            }
            self.groups.push(GroupKind::Semantic {
                linked: false,
                emitted,
                semantic_id: semantic_id.to_owned(),
                projected_transform,
            });
            return Ok(());
        }
        if matches!(self.svg_body, SvgStructureBody::Packet(_)) {
            let emitted = semantic_id.starts_with("packet.word.");
            if emitted {
                self.output.push_str("<g>")?;
            }
            self.groups.push(GroupKind::Semantic {
                linked: false,
                emitted,
                semantic_id: semantic_id.to_owned(),
                projected_transform: Transform::IDENTITY,
            });
            return Ok(());
        }
        if matches!(self.svg_body, SvgStructureBody::Info(_)) {
            // Info has two semantic scopes in the renderer-neutral document, but Mermaid's SVG
            // contract exposes only one ordinary group around the version text.  Keep both scopes
            // on the command stack so semantic ownership remains validated without adding DOM
            // wrappers that the source renderer never emits.
            let emitted = semantic_id == "info.document";
            if emitted {
                self.output.push_str("<g>")?;
            }
            self.groups.push(GroupKind::Semantic {
                linked: false,
                emitted,
                semantic_id: semantic_id.to_owned(),
                projected_transform: Transform::IDENTITY,
            });
            return Ok(());
        }
        if matches!(self.svg_body, SvgStructureBody::Error(_)) {
            if semantic_id != "error.document" {
                return Err(invalid(format!(
                    "Error SVG has an unexpected semantic group {semantic_id}"
                )));
            }
            // Mermaid exposes the Error body as one ordinary group. Keep the semantic scope in
            // the canonical command stream, but do not invent DOM metadata or accessibility
            // children that the source renderer does not emit.
            self.output.push_str("<g>")?;
            self.groups.push(GroupKind::Semantic {
                linked: false,
                emitted: true,
                semantic_id: semantic_id.to_owned(),
                projected_transform: Transform::IDENTITY,
            });
            return Ok(());
        }
        if matches!(self.svg_body, SvgStructureBody::Zenuml(_))
            && let Some(class) = self.semantic_extra_class(semantic_id).map(str::to_owned)
        {
            let visible = self.debug_visibility(semantic.role);
            let security = MermaidNavigationSecurity::from_security_level_loose(
                self.effective_config
                    .get("securityLevel")
                    .and_then(Value::as_str)
                    == Some("loose"),
            );
            let link = semantic
                .link
                .as_deref()
                .and_then(|value| prepare_mermaid_navigation_uri(value, security));
            let linked = link.is_some();
            if let Some(link) = link {
                self.output.push_str("<a href=\"")?;
                output::escape_attr(&mut self.output, link.as_str())?;
                self.output.push_str("\">")?;
            }
            self.output.push_str("<g class=\"")?;
            output::escape_attr(&mut self.output, class.as_str())?;
            self.output.push('"')?;
            self.output.push_str(" id=\"")?;
            let svg_id = self.semantic_svg_id(semantic_id)?;
            output::escape_attr(&mut self.output, svg_id.as_str())?;
            self.output.push('"')?;
            if let SvgStructureBody::Zenuml(body) = self.svg_body
                && let Some(statement_id) = body.semantic_data_statements.get(semantic_id)
            {
                self.output.push_str(" data-statement=\"")?;
                output::escape_attr(&mut self.output, statement_id)?;
                self.output.push('"')?;
            }
            self.output.push_str(" role=\"group\"")?;
            write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
            if !visible {
                self.output.push_str(" display=\"none\"")?;
            }
            self.output.push('>')?;
            if let Some(title) = semantic.title.as_deref() {
                self.output.push_str("<title>")?;
                output::escape_xml(&mut self.output, title)?;
                self.output.push_str("</title>")?;
            }
            if let Some(description) = semantic.description.as_deref() {
                self.output.push_str("<desc>")?;
                output::escape_xml(&mut self.output, description)?;
                self.output.push_str("</desc>")?;
            }
            self.groups.push(GroupKind::Semantic {
                linked,
                emitted: true,
                semantic_id: semantic_id.to_owned(),
                projected_transform: Transform::IDENTITY,
            });
            return Ok(());
        }
        let svg_id = self.semantic_svg_id(semantic_id)?;
        let role_class = semantic_role_class(semantic.role);
        let visible = if matches!(self.svg_body, SvgStructureBody::Er(_))
            && is_er_container_semantic(semantic_id)
            || matches!(self.svg_body, SvgStructureBody::Wardley(_))
                && semantic.role == SemanticRole::Group
        {
            true
        } else {
            self.debug_visibility(semantic.role)
        };
        let security = MermaidNavigationSecurity::from_security_level_loose(
            self.effective_config
                .get("securityLevel")
                .and_then(Value::as_str)
                == Some("loose"),
        );
        let link = semantic
            .link
            .as_deref()
            .and_then(|value| prepare_mermaid_navigation_uri(value, security));
        let kanban_ticket_href = match self.svg_body {
            SvgStructureBody::Kanban(body) if semantic_id.starts_with("kanban.item.") => {
                body.ticket_links.get(semantic_id).cloned()
            }
            _ => None,
        };
        let kanban_ticket_anchor = kanban_ticket_href.is_some();
        let linked = link.is_some() || kanban_ticket_anchor;
        if kanban_ticket_anchor {
            self.output.push_str("<a class=\"kanban-ticket-link\"")?;
            if let Some(href) = kanban_ticket_href.flatten() {
                self.output.push_str(" xlink:href=\"")?;
                // DrawingList stores the logical URI; serialize it exactly once at the SVG
                // attribute boundary.
                output::escape_attr(&mut self.output, href.as_str())?;
                self.output.push('"')?;
            }
            if security == MermaidNavigationSecurity::Loose {
                self.output.push_str(" target=\"_blank\"")?;
            }
            self.output.push('>')?;
        } else if let Some(link) = link {
            self.output.push_str("<a href=\"")?;
            output::escape_attr(&mut self.output, link.as_str())?;
            self.output.push_str("\">")?;
        }
        write!(self.output, "<g id=\"{}\"", escaped_attr(svg_id.as_str()))?;
        if let SvgStructureBody::Kanban(body) = self.svg_body
            && semantic_id.starts_with("kanban.section.")
        {
            // Preserve Mermaid's stable section attribute ordering for DOM consumers.
            self.output.push_str(" data-look=\"")?;
            output::escape_attr(&mut self.output, body.look.as_str())?;
            self.output.push('"')?;
        }
        let semantic_extra_class = self.semantic_extra_class(semantic_id).map(str::to_owned);
        #[cfg(feature = "layout-cytoscape")]
        let exact_family_class = matches!(self.svg_body, SvgStructureBody::Architecture(_));
        #[cfg(not(feature = "layout-cytoscape"))]
        let exact_family_class = false;
        if matches!(self.svg_body, SvgStructureBody::Er(_))
            && is_er_container_semantic(semantic_id)
            && let Some(class) = semantic_extra_class.as_deref()
        {
            write!(
                self.output,
                " role=\"group\" class=\"{}\"",
                escaped_attr(class),
            )?;
        } else if matches!(self.svg_body, SvgStructureBody::Timeline(_))
            && let Some(class) = semantic_extra_class.as_deref()
        {
            // Timeline's current Mermaid DOM uses the family class as the complete group class;
            // retain that stable selector, ARIA role, and optional diagnostic identity.
            write!(self.output, " role=\"group\" class=\"{}\"", class,)?;
        } else if (exact_family_class
            || matches!(
                self.svg_body,
                SvgStructureBody::Block(_) | SvgStructureBody::C4(_)
            ))
            && let Some(class) = semantic_extra_class.as_deref()
        {
            write!(
                self.output,
                " role=\"group\" class=\"{}\"",
                escaped_attr(class),
            )?;
        } else {
            write!(
                self.output,
                " class=\"merman-semantic {}{}\" role=\"group\"",
                role_class,
                semantic_extra_class
                    .as_deref()
                    .map_or_else(String::new, |class| format!(" {class}")),
            )?;
        }
        write_semantic_metadata(&mut self.output, self.debug, semantic.id.as_str())?;
        let requirement_semantic_attrs = self.requirement_semantic_attrs(semantic_id);
        self.output.push_str(&requirement_semantic_attrs)?;
        if let SvgStructureBody::Venn(body) = self.svg_body
            && let Some(sets) = body.semantic_data_sets.get(semantic_id)
        {
            self.output.push_str(" data-venn-sets=\"")?;
            output::escape_attr(&mut self.output, sets)?;
            self.output.push('"')?;
        }
        if !visible {
            self.output.push_str(" display=\"none\"")?;
        }
        self.output.push('>')?;
        if let Some(title) = semantic.title.as_deref() {
            self.output.push_str("<title>")?;
            output::escape_xml(&mut self.output, title)?;
            self.output.push_str("</title>")?;
        }
        if let Some(description) = semantic.description.as_deref() {
            self.output.push_str("<desc>")?;
            output::escape_xml(&mut self.output, description)?;
            self.output.push_str("</desc>")?;
        }
        self.groups.push(GroupKind::Semantic {
            linked,
            emitted: true,
            semantic_id: semantic_id.to_owned(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(())
    }

    fn begin_mindmap_semantic_group(&mut self, semantic_id: &str) -> Result<()> {
        let semantic = self
            .semantics
            .get(semantic_id)
            .ok_or_else(|| invalid(format!("unknown Mindmap semantic id {semantic_id}")))?;
        let visible = self.debug_visibility(semantic.role);
        let body = match self.svg_body {
            SvgStructureBody::Mindmap(body) => body,
            _ => return Err(invalid("Mindmap semantic group requires a Mindmap sidecar")),
        };
        let mut fragment = String::new();
        let emitted = match semantic_id {
            "mindmap.document" => {
                fragment.push_str("<g>");
                super::mindmap::push_mindmap_marker_defs(&mut fragment, self.diagram_id.as_str());
                true
            }
            "mindmap.subgraphs" => {
                fragment.push_str(r#"<g class="subgraphs">"#);
                true
            }
            "mindmap.edges" => {
                fragment.push_str(r#"<g class="edgePaths">"#);
                true
            }
            "mindmap.edge_labels" => {
                fragment.push_str(r#"<g class="edgeLabels">"#);
                for semantic in &self.document.semantics {
                    if !semantic.id.starts_with("mindmap.edge.") {
                        continue;
                    }
                    let edge = body.edges.get(semantic.id.as_str()).ok_or_else(|| {
                        invalid(format!(
                            "Mindmap SVG sidecar is missing edge {}",
                            semantic.id
                        ))
                    })?;
                    write!(fragment, r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#,
                        escaped_attr(edge.data_id.as_str()),
                    ).map_err(|_| invalid("SVG component formatting failed"))?;
                }
                true
            }
            "mindmap.nodes" => {
                fragment.push_str(r#"<g class="nodes">"#);
                true
            }
            _ if semantic_id.starts_with("mindmap.node.") => {
                let node = body.nodes.get(semantic_id).ok_or_else(|| {
                    invalid(format!("Mindmap SVG sidecar is missing node {semantic_id}"))
                })?;
                write!(fragment, "<g class=\"{}\" id=\"{}-{}\" data-look=\"{}\" transform=\"translate({}, {})\" role=\"group\"",
                    escaped_attr(node.class.as_str()),
                    escaped_attr(self.diagram_id.as_str()),
                    escaped_attr(node.dom_id.as_str()),
                    escaped_attr(node.look.as_str()),
                    fmt(node.origin.x),
                    fmt(node.origin.y),
                ).map_err(|_| invalid("SVG component formatting failed"))?;
                if self.debug.include_drawing_list_metadata {
                    write!(
                        fragment,
                        " data-merman-semantic-id=\"{}\"",
                        escaped_attr(semantic_id)
                    )
                    .map_err(|_| invalid("SVG component formatting failed"))?;
                }
                if !visible {
                    fragment.push_str(" display=\"none\"");
                }
                fragment.push('>');
                true
            }
            // Mermaid keeps edge paths as direct children of `.edgePaths`; retain the semantic
            // scope for DrawingList command interpretation without inserting an SVG wrapper.
            _ if semantic_id.starts_with("mindmap.edge.") => false,
            _ => {
                return Err(invalid(format!(
                    "Mindmap SVG sidecar has no structure for semantic id {semantic_id}"
                )));
            }
        };
        self.output.push_str(&fragment)?;
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_string(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(())
    }

    fn semantic_extra_class(&self, semantic_id: &str) -> Option<&str> {
        match self.svg_body {
            SvgStructureBody::QuadrantChart(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Pie(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Timeline(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Journey(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Kanban(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Sankey(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Venn(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Railroad(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::EventModeling(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Ishikawa(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Cynefin(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::TreeView(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Gantt(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::XyChart(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::GitGraph(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Treemap(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Requirement(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::State(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Er(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Block(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::C4(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            #[cfg(feature = "layout-cytoscape")]
            SvgStructureBody::Architecture(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Wardley(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            SvgStructureBody::Zenuml(body) => {
                body.semantic_classes.get(semantic_id).map(String::as_str)
            }
            _ => None,
        }
    }

    fn requirement_semantic_attrs(&self, semantic_id: &str) -> String {
        let (looks, color_ids) = match self.svg_body {
            SvgStructureBody::Requirement(body) => {
                (Some(&body.semantic_looks), Some(&body.semantic_color_ids))
            }
            SvgStructureBody::State(body) => (Some(&body.semantic_looks), None),
            _ => (None, None),
        };
        let mut attrs = String::new();
        if let Some(looks) = looks
            && let Some(look) = looks.get(semantic_id)
        {
            attrs.push_str(" data-look=\"");
            escape_attr_into(&mut attrs, look);
            attrs.push('"');
        }
        if let Some(color_id) = color_ids.and_then(|ids| ids.get(semantic_id)) {
            attrs.push_str(" data-color-id=\"");
            escape_attr_into(&mut attrs, color_id);
            attrs.push('"');
        }
        if let SvgStructureBody::Er(body) = self.svg_body {
            if !body.data_look.is_empty() {
                attrs.push_str(" data-look=\"");
                escape_attr_into(&mut attrs, body.data_look.as_str());
                attrs.push('"');
            }
            if let Some(dom_id) = body.dom_ids.get(semantic_id)
                && semantic_id.ends_with(".label")
            {
                attrs.push_str(" data-id=\"");
                escape_attr_into(&mut attrs, dom_id);
                attrs.push('"');
            }
        }
        attrs
    }

    fn end_semantic_group(&mut self) -> Result<()> {
        match self.groups.pop() {
            Some(GroupKind::Semantic {
                linked,
                emitted,
                semantic_id,
                projected_transform,
            }) => {
                if projected_transform != Transform::IDENTITY {
                    self.state.transform =
                        multiply_transform(projected_transform, self.state.transform);
                }
                if emitted {
                    self.output.push_str("</g>")?;
                }
                if linked {
                    self.output.push_str("</a>")?;
                }
                if matches!(self.svg_body, SvgStructureBody::Mindmap(_))
                    && semantic_id == "mindmap.document"
                {
                    let mut shadow_defs = String::new();
                    super::mindmap::push_mindmap_shadow_defs(
                        &mut shadow_defs,
                        self.diagram_id.as_str(),
                        self.effective_config,
                    );
                    self.output.push_str(&shadow_defs)?;
                }
                Ok(())
            }
            Some(other) => {
                self.groups.push(other);
                Err(invalid(
                    "SVG semantic group end does not match the open group",
                ))
            }
            None => Err(invalid("SVG semantic group end has no matching group")),
        }
    }

    fn emit_path(&mut self, path_id: &ResourceId, style: &PathStyle) -> Result<()> {
        if self.emit_venn_path(path_id, style)? {
            return Ok(());
        }
        if matches!(self.svg_body, SvgStructureBody::Sankey(_)) {
            self.write_sankey_referenced_gradient(style)?;
            if self.emit_compact_sankey_link(path_id, style)? {
                return Ok(());
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Error(_)) {
            return super::error::write_error_path(&mut self.output, path_id);
        }
        if matches!(self.svg_body, SvgStructureBody::Mindmap(_)) {
            return self.emit_mindmap_path(path_id, style);
        }
        if matches!(self.svg_body, SvgStructureBody::Zenuml(_)) {
            return self.emit_zenuml_path(path_id, style);
        }
        if self.is_er_marker_path(path_id.as_str()) {
            // ER cardinality markers are expanded into public DrawingList paths, while Mermaid's
            // canonical SVG contract uses marker definitions referenced from the relationship
            // route. Keep the geometry in the renderer-neutral document and project the source
            // DOM shape from the sidecar below.
            return Ok(());
        }
        if self.is_wardley_marker_path(path_id.as_str()) {
            // DrawingList expands markers into ordinary paths for renderer-neutral hosts.  The
            // canonical Wardley SVG projection uses Mermaid's marker DOM instead, so omit only
            // these adapter-owned helper paths and attach equivalent marker references to their
            // source lines below.  No public DrawingList geometry is discarded.
            return Ok(());
        }
        #[cfg(feature = "layout-cytoscape")]
        if matches!(self.svg_body, SvgStructureBody::Architecture(_)) {
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if raw_id.contains(".arrow.")
                && let Some(points) = polygon_from_path(path)
            {
                return self.emit_polygon(path_id, &points, style);
            }
            if (raw_id.starts_with("architecture.group.") && raw_id.ends_with(".outline")
                || raw_id.starts_with("architecture.node.") && raw_id.ends_with(".junction")
                || raw_id.ends_with(".icon.background"))
                && let Some(bounds) = rectangle_from_path(path)
            {
                return self.emit_rect(path_id, bounds, style);
            }
            if raw_id.ends_with(".icon.case")
                && let Some((bounds, radius)) = rounded_rectangle_from_path(path)
            {
                return self.emit_rounded_rect(path_id, bounds, radius, style);
            }
            if (raw_id.contains(".icon.side.")
                || raw_id.contains(".icon.divider.")
                || raw_id.contains(".icon.axis."))
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
            if (raw_id.ends_with(".icon.top")
                || raw_id.contains(".icon.light.")
                || raw_id.contains(".icon.screw.")
                || raw_id.ends_with(".icon.platter")
                || raw_id.ends_with(".icon.hub")
                || raw_id.ends_with(".icon.globe"))
                && let Some((center, radius_x, radius_y)) = ellipse_from_path(path)
            {
                return self.emit_ellipse(path_id, center, radius_x, radius_y, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::QuadrantChart(_)) {
            let raw_id = path_id.as_str();
            if raw_id.starts_with("quadrantchart.quadrant.") && raw_id.ends_with(".shape") {
                let path = self.path_resource(path_id)?;
                if let Some(bounds) = rectangle_from_path(path) {
                    return self.emit_rect(path_id, bounds, style);
                }
            }
            if raw_id.starts_with("quadrantchart.point.") && raw_id.ends_with(".shape") {
                let path = self.path_resource(path_id)?;
                if let Some((center, radius)) = circle_from_path(path) {
                    return self.emit_circle(path_id, center, radius, style);
                }
            }
            if raw_id.starts_with("quadrantchart.border.") {
                let path = self.path_resource(path_id)?;
                if let Some((start, end)) = line_from_path(path) {
                    return self.emit_line(path_id, start, end, style);
                }
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Pie(_)) {
            if self.emit_pie_path(path_id, style)? {
                return Ok(());
            }
            let raw_id = path_id.as_str();
            if raw_id == "pie.outer" {
                let path = self.path_resource(path_id)?;
                if let Some((center, radius)) = circle_from_path(path) {
                    return self.emit_circle(path_id, center, radius, style);
                }
            }
            if raw_id.starts_with("pie.legend.") && raw_id.ends_with(".swatch") {
                let path = self.path_resource(path_id)?;
                if let Some(bounds) = rectangle_from_path(path) {
                    return self.emit_rect(path_id, bounds, style);
                }
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Timeline(_))
            && path_id.as_str().ends_with(".line")
        {
            let path = self.path_resource(path_id)?;
            if let Some((start, end)) = line_from_path(path) {
                return self.emit_line(path_id, start, end, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Journey(_)) {
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if (raw_id.ends_with(".line") || raw_id == "journey.activity.line")
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
            if (raw_id.ends_with(".circle")
                || raw_id.ends_with(".face")
                || raw_id.ends_with(".left_eye")
                || raw_id.ends_with(".right_eye"))
                && let Some((center, radius)) = circle_from_path(path)
            {
                return self.emit_circle(path_id, center, radius, style);
            }
            if raw_id.ends_with(".background")
                && let Some((bounds, radius)) = rounded_rectangle_from_path(path)
            {
                return self.emit_rounded_rect(path_id, bounds, radius, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Treemap(_)) {
            let path = self.path_resource(path_id)?;
            if let Some(bounds) = rectangle_from_path(path) {
                return self.emit_rect(path_id, bounds, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Kanban(_)) {
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if raw_id.ends_with(".background")
                && let Some((bounds, radius)) = rounded_rectangle_from_path(path)
            {
                return self.emit_rounded_rect(path_id, bounds, radius, style);
            }
            if raw_id.ends_with(".priority")
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Sankey(_))
            && path_id.as_str().ends_with(".shape")
        {
            let path = self.path_resource(path_id)?;
            if let Some(bounds) = rectangle_from_path(path) {
                return self.emit_sankey_node_rect(bounds, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::XyChart(_))
            && path_id.as_str() == "xychart.background"
        {
            let path = self.path_resource(path_id)?;
            if let Some(bounds) = rectangle_from_path(path) {
                return self.emit_xychart_background(bounds, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::XyChart(_))
            && path_id.as_str().contains(".rect.")
            && path_id.as_str().ends_with(".shape")
        {
            let path = self.path_resource(path_id)?;
            if let Some(bounds) = rectangle_from_path(path) {
                return self.emit_rect(path_id, bounds, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Packet(_))
            && path_id.as_str().ends_with(".shape")
        {
            let path = self.path_resource(path_id)?;
            if let Some(bounds) = rectangle_from_path(path) {
                if self
                    .packet_styles
                    .as_ref()
                    .is_some_and(|styles| styles.compact_block(style))
                {
                    write!(
                        self.output,
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" class=\"packetBlock\"",
                        fmt(bounds.x),
                        fmt(bounds.y),
                        fmt(bounds.width),
                        fmt(bounds.height)
                    )?;
                    self.write_state_attrs()?;
                    self.output.push_str("/>")?;
                    return Ok(());
                }
                return self.emit_rect(path_id, bounds, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Ishikawa(_)) {
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if raw_id.ends_with(".path")
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
            if raw_id.ends_with(".label-box")
                && let Some(bounds) = rectangle_from_path(path)
            {
                return self.emit_rect(path_id, bounds, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Cynefin(_)) {
            if self.emit_compact_cynefin_path(path_id, style)? {
                return Ok(());
            }
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if raw_id.starts_with("cynefin.item.")
                && raw_id.ends_with(".shape")
                && let Some((bounds, radius)) = rounded_rectangle_from_path(path)
            {
                return self.emit_rounded_rect(path_id, bounds, radius, style);
            }
            if (raw_id == "cynefin.background"
                || raw_id.starts_with("cynefin.domain.") && raw_id.ends_with(".background"))
                && let Some(bounds) = rectangle_from_path(path)
            {
                return self.emit_rect(path_id, bounds, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::TreeView(_)) {
            let raw_id = path_id.as_str();
            if raw_id.starts_with("treeView.node.") && raw_id.ends_with(".icon") {
                let path = self.path_resource(path_id)?.clone();
                return self.emit_tree_view_icon(path_id, &path, style);
            }
            let path = self.path_resource(path_id)?;
            if raw_id.ends_with(".path")
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
            if raw_id == "treeView.background"
                && let Some(bounds) = rectangle_from_path(path)
            {
                return self.emit_rect(path_id, bounds, style);
            }
            if raw_id.ends_with(".highlight")
                && let Some((bounds, radius)) = rounded_rectangle_from_path(path)
            {
                return self.emit_rounded_rect(path_id, bounds, radius, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Gantt(_)) {
            if self.emit_gantt_row(path_id, style)? {
                return Ok(());
            }
            if self.emit_gantt_axis_path(path_id, style)? {
                return Ok(());
            }
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if raw_id == "gantt.background"
                || raw_id.starts_with("gantt.exclude.")
                || raw_id.starts_with("gantt.row.")
                || raw_id.starts_with("gantt.task.") && raw_id.ends_with(".bar")
            {
                if let Some((bounds, rx, ry)) = elliptical_rounded_rectangle_from_path(path) {
                    return self.emit_elliptical_rounded_rect(path_id, bounds, rx, ry, style);
                }
                if let Some(bounds) = rectangle_from_path(path) {
                    return self.emit_rect(path_id, bounds, style);
                }
                if raw_id.starts_with("gantt.task.")
                    && style.fill.is_none()
                    && style.stroke.is_none()
                    && let Some(bounds) = rectangle_path_bounds(path)
                {
                    // The adapter retains collapsed task identity with no paint. Only that
                    // unpainted path is equivalent to SVG's non-rendering zero-extent rect;
                    // an edited stroke must retain its drawable path geometry.
                    return self.emit_rect(path_id, bounds, style);
                }
            }
            if (raw_id == "gantt.today.line" || raw_id.contains(".tick."))
                && let Some((start, end)) = line_from_path(path)
            {
                if raw_id.contains(".tick.") && start == Point::new(0.0, 0.0) && end.x == 0.0 {
                    return self.emit_gantt_tick_line(path_id, end.y, style);
                }
                return self.emit_line(path_id, start, end, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::GitGraph(_)) {
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if (raw_id.ends_with(".line") || raw_id.ends_with(".stem"))
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
            if (raw_id.ends_with(".circle")
                || raw_id.ends_with(".outer")
                || raw_id.ends_with(".inner")
                || raw_id.ends_with(".left")
                || raw_id.ends_with(".right")
                || raw_id.ends_with(".hole"))
                && let Some((center, radius)) = cubic_circle_from_path(path)
            {
                return self.emit_circle(path_id, center, radius, style);
            }
            if (raw_id.ends_with(".background")
                || raw_id.ends_with(".highlight.outer")
                || raw_id.ends_with(".highlight.inner"))
                && let Some((bounds, radius)) = rounded_rectangle_from_path(path)
            {
                if radius > 0.0 {
                    return self.emit_rounded_rect(path_id, bounds, radius, style);
                }
                return self.emit_rect(path_id, bounds, style);
            }
            if (raw_id.ends_with(".background")
                || raw_id.ends_with(".highlight.outer")
                || raw_id.ends_with(".highlight.inner"))
                && let Some(bounds) = rectangle_from_path(path)
            {
                return self.emit_rect(path_id, bounds, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::State(_)) {
            let raw_id = path_id.as_str();
            if raw_id.starts_with("state.node.") && raw_id.ends_with(".shape") {
                let path = self.path_resource(path_id)?;
                if let Some((center, radius)) = circle_from_path(path) {
                    return self.emit_circle(path_id, center, radius, style);
                }
                if let Some((bounds, radius)) = rounded_rectangle_from_path(path) {
                    return self.emit_rounded_rect(path_id, bounds, radius, style);
                }
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Er(_)) {
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if (raw_id.ends_with(".box") || raw_id.ends_with(".background"))
                && let Some(bounds) = rectangle_from_path(path)
            {
                return self.emit_rect(path_id, bounds, style);
            }
            if raw_id.ends_with(".divider")
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Block(_)) {
            let raw_id = path_id.as_str();
            if raw_id.starts_with("block.edge.") && raw_id.ends_with(".label.background") {
                // Block's HTML label shell owns the same resolved background paint in SVG. Native
                // hosts still receive the public rectangle command.
                return Ok(());
            }
            if raw_id.starts_with("block.node.") {
                let path = self.path_resource(path_id)?;
                if let Some((center, radius)) = circle_from_path(path) {
                    return self.emit_circle(path_id, center, radius, style);
                }
                if let Some((bounds, radius)) = rounded_rectangle_from_path(path) {
                    return self.emit_rounded_rect(path_id, bounds, radius, style);
                }
                if let Some(bounds) = rectangle_from_path(path) {
                    return self.emit_rect(path_id, bounds, style);
                }
            }
        }
        if matches!(self.svg_body, SvgStructureBody::C4(_)) {
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if raw_id.ends_with(".route")
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
            if (raw_id.ends_with(".box") || raw_id.ends_with(".shape") || raw_id.ends_with(".body"))
                && let Some((bounds, radius)) = rounded_rectangle_from_path(path)
            {
                return self.emit_rounded_rect(path_id, bounds, radius, style);
            }
            if raw_id.ends_with(".head")
                && let Some((center, radius)) = circle_from_path(path)
            {
                return self.emit_circle(path_id, center, radius, style);
            }
            if (raw_id.ends_with(".shape") || raw_id.contains(".marker."))
                && let Some(points) = polygon_from_path(path)
            {
                return self.emit_polygon(path_id, &points, style);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Wardley(_)) {
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if (raw_id.ends_with(".line")
                || raw_id.contains(".axis.")
                || raw_id.contains(".grid.")
                || raw_id.ends_with(".divider")
                || raw_id.ends_with(".link")
                || raw_id.ends_with(".inertia")
                || raw_id.contains(".connector.")
                || raw_id.contains(".segment."))
                && let Some((start, end)) = line_from_path(path)
            {
                return self.emit_line(path_id, start, end, style);
            }
            if (raw_id.contains(".overlay.")
                || raw_id.contains(".point.")
                || raw_id.ends_with(".shape"))
                && let Some((center, radius)) = circle_from_path(path)
            {
                return self.emit_circle(path_id, center, radius, style);
            }
            if (raw_id == "wardley.background"
                || raw_id.ends_with(".box")
                || raw_id.ends_with(".shape"))
                && let Some((bounds, radius)) = rounded_rectangle_from_path(path)
            {
                if radius > 0.0 {
                    return self.emit_rounded_rect(path_id, bounds, radius, style);
                }
                return self.emit_rect(path_id, bounds, style);
            }
            if (raw_id == "wardley.background"
                || raw_id.ends_with(".box")
                || raw_id.ends_with(".shape"))
                && let Some(bounds) = rectangle_from_path(path)
            {
                return self.emit_rect(path_id, bounds, style);
            }
        }
        self.output.push_str("<path d=\"")?;
        {
            let path = self.path_resource(path_id)?;
            if matches!(self.svg_body, SvgStructureBody::Pie(_)) {
                output::escape_attr(
                    &mut self.output,
                    super::curve::drawing_path_segments_d_unrounded(&path.segments),
                )?;
            } else {
                output::escape_attr(&mut self.output, path_d(&path.segments))?;
            }
        };
        self.output.push('"')?;
        self.write_gantt_dom_id(path_id.as_str())?;
        self.write_journey_dom_id(path_id.as_str())?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        self.write_er_edge_attrs(path_id.as_str())?;
        self.write_block_edge_attrs(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_zenuml_path_attrs(path_id.as_str())?;
        self.write_block_inline_path_style(path_id, style)?;
        self.write_c4_shape_inline_style(path_id, style, None)?;
        if !self.write_gantt_path_presentation(path_id, style, true)? {
            self.write_path_style(style)?;
            self.write_state_attrs()?;
        }
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_zenuml_path(&mut self, path_id: &ResourceId, style: &PathStyle) -> Result<()> {
        let raw_id = path_id.as_str();
        let path = self.path_resource(path_id)?.clone();
        if (raw_id == "zenuml.frame.outer" || raw_id == "zenuml.frame.inner")
            && let Some((bounds, radius)) = rounded_rectangle_from_path(&path)
        {
            return self.emit_rounded_rect(path_id, bounds, radius, style);
        }
        if (raw_id.ends_with(".line")
            || raw_id.ends_with(".header")
            || raw_id.ends_with(".separator"))
            && let Some((start, end)) = line_from_path(&path)
        {
            return self.emit_line(path_id, start, end, style);
        }
        if raw_id.ends_with(".box")
            || raw_id.ends_with(".bar")
            || raw_id.ends_with(".border")
            || raw_id.ends_with(".background")
        {
            if let Some((bounds, radius)) = rounded_rectangle_from_path(&path) {
                if radius > 0.0 {
                    return self.emit_rounded_rect(path_id, bounds, radius, style);
                }
                return self.emit_rect(path_id, bounds, style);
            }
            if let Some(bounds) = rectangle_from_path(&path) {
                return self.emit_rect(path_id, bounds, style);
            }
        }
        if raw_id.contains(".return.")
            && raw_id.ends_with(".icon.circle")
            && let Some((center, radius)) = circle_from_path(&path)
        {
            return self.emit_circle(path_id, center, radius, style);
        }
        self.emit_path_as_standard(path_id, &path, style)
    }

    fn emit_mindmap_path(&mut self, path_id: &ResourceId, _style: &PathStyle) -> Result<()> {
        let semantic_id = self
            .current_semantic_id()
            .ok_or_else(|| invalid("Mindmap path is outside a semantic group"))?
            .to_string();
        let path = self.path_resource(path_id)?.clone();
        let body = match self.svg_body {
            SvgStructureBody::Mindmap(body) => body,
            _ => return Err(invalid("Mindmap path requires a Mindmap sidecar")),
        };

        if let Some(edge) = body.edges.get(semantic_id.as_str()) {
            let visible = self
                .semantics
                .get(semantic_id.as_str())
                .is_some_and(|semantic| self.debug_visibility(semantic.role));
            let data_points = STANDARD.encode(super::util::json_stringify_display_points(
                edge.points.as_slice(),
            ));
            write!(
                self.output,
                "<path d=\"{}\" id=\"{}-{}\" class=\"{}\" data-look=\"{}\" data-edge=\"true\" data-et=\"edge\" data-id=\"{}\" data-points=\"{}\"{}",
                escaped_attr(path_d(&path.segments)),
                escaped_attr(self.diagram_id.as_str()),
                escaped_attr(edge.dom_id.as_str()),
                escaped_attr(edge.class.as_str()),
                escaped_attr(edge.look.as_str()),
                escaped_attr(edge.data_id.as_str()),
                escaped_attr(data_points.as_str()),
                if visible { "" } else { " display=\"none\"" },
            )?;
            write_semantic_metadata(&mut self.output, self.debug, semantic_id.as_str())?;
            write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
            self.output.push_str("/>")?;
            return Ok(());
        }

        let node = body.nodes.get(semantic_id.as_str()).ok_or_else(|| {
            invalid(format!(
                "Mindmap path {path_id:?} has no node or edge sidecar"
            ))
        })?;
        let local_segments = offset_path_segments(&path.segments, -node.origin.x, -node.origin.y);
        let local_path = PathResource {
            id: path.id.clone(),
            segments: local_segments,
        };
        let raw_id = path_id.as_str();
        if raw_id.ends_with(".divider") {
            let (start, end) = line_from_path(&local_path)
                .ok_or_else(|| invalid(format!("Mindmap divider {raw_id} is not a line")))?;
            write!(
                self.output,
                "<line class=\"node-line-\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
                fmt(start.x),
                fmt(start.y),
                fmt(end.x),
                fmt(end.y),
            )?;
            write_resource_metadata(&mut self.output, self.debug, raw_id)?;
            self.output.push_str("/>")?;
            return Ok(());
        }
        if !raw_id.ends_with(".shape.outer") {
            return Err(invalid(format!(
                "Mindmap SVG serializer does not recognize path {raw_id}"
            )));
        }

        match node.shape.as_str() {
            "defaultMindmapNode" => {
                write!(
                    self.output,
                    "<path id=\"{}-{}\" class=\"node-bkg node-0\" style=\"\" d=\"{}\"",
                    escaped_attr(self.diagram_id.as_str()),
                    escaped_attr(node.dom_id.as_str()),
                    escaped_attr(path_d(&local_path.segments)),
                )?;
            }
            "rect" => {
                let bounds = rectangle_from_path(&local_path)
                    .ok_or_else(|| invalid("Mindmap rect node is not rectangular"))?;
                write!(
                    self.output,
                    "<rect class=\"basic label-container\" style=\"\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                    fmt(bounds.x),
                    fmt(bounds.y),
                    fmt(bounds.width),
                    fmt(bounds.height),
                )?;
            }
            "rounded" => {
                let (bounds, radius) = rounded_rectangle_from_path(&local_path)
                    .ok_or_else(|| invalid("Mindmap rounded node is not a rounded rectangle"))?;
                write!(
                    self.output,
                    "<rect class=\"basic label-container\" style=\"\" rx=\"{}\" ry=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                    fmt(radius),
                    fmt(radius),
                    fmt(bounds.x),
                    fmt(bounds.y),
                    fmt(bounds.width),
                    fmt(bounds.height),
                )?;
            }
            "mindmapCircle" => {
                let (center, radius) = circle_from_path(&local_path)
                    .ok_or_else(|| invalid("Mindmap circle node is not circular"))?;
                write!(
                    self.output,
                    "<circle class=\"basic label-container\" style=\"\" r=\"{}\" cx=\"{}\" cy=\"{}\"",
                    fmt(radius),
                    fmt(center.x),
                    fmt(center.y),
                )?;
            }
            "hexagon" => {
                let points = polygon_from_path(&local_path)
                    .ok_or_else(|| invalid("Mindmap hexagon node is not polygonal"))?;
                self.output.push_str("<polygon points=\"")?;
                for (index, point) in points.iter().enumerate() {
                    if index > 0 {
                        self.output.push(' ')?;
                    }
                    write!(self.output, "{},{}", fmt(point.x), fmt(point.y))?;
                }
                self.output.push_str("\" class=\"label-container\"")?;
            }
            "cloud" | "bang" => {
                write!(
                    self.output,
                    "<path class=\"basic label-container\" style=\"\" d=\"{}\"",
                    escaped_attr(path_d(&local_path.segments)),
                )?;
            }
            other => {
                return Err(invalid(format!(
                    "Mindmap SVG serializer does not recognize shape {other}"
                )));
            }
        }
        write_resource_metadata(&mut self.output, self.debug, raw_id)?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_tree_view_icon(
        &mut self,
        path_id: &ResourceId,
        path: &PathResource,
        style: &PathStyle,
    ) -> Result<()> {
        let transform = self.state.transform;
        if transform.b.abs() > 1e-9
            || transform.c.abs() > 1e-9
            || (transform.a - transform.d).abs() > 1e-9
            || !transform.a.is_finite()
            || transform.a <= 0.0
        {
            return self.emit_path_as_standard(path_id, path, style);
        }
        let path_data = path_d(&path.segments);
        self.output
            .push_str("<g class=\"treeView-node-icon\" transform=\"translate(")?;
        write!(self.output, "{},{}\">", fmt(transform.e), fmt(transform.f))?;
        if self.state.opacity != 1.0 || self.state.blend_mode != BlendMode::Normal {
            // The icon group is emitted inside the current state, so preserve state attributes on
            // the nested SVG rather than changing the path's resolved geometry.
            self.output.pop();
            self.output.push(' ')?;
            if self.state.opacity != 1.0 {
                write!(self.output, "opacity=\"{}\"", fmt(self.state.opacity))?;
            }
            write_blend_style(&mut self.output, self.state.blend_mode)?;
            self.output.push('>')?;
        }
        write!(
            self.output,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\"><path d=\"{}\"",
            fmt(crate::tree_view::TREE_VIEW_ICON_SIZE),
            fmt(crate::tree_view::TREE_VIEW_ICON_SIZE),
            escaped_attr(&path_data),
        )?;
        self.write_path_style(style)?;
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        self.output.push_str("</svg></g>")?;
        Ok(())
    }

    fn emit_path_as_standard(
        &mut self,
        path_id: &ResourceId,
        path: &PathResource,
        style: &PathStyle,
    ) -> Result<()> {
        let path_data = path_d(&path.segments);
        self.output.push_str("<path d=\"")?;
        output::escape_attr(&mut self.output, &path_data)?;
        self.output.push('"')?;
        self.write_gantt_dom_id(path_id.as_str())?;
        self.write_journey_dom_id(path_id.as_str())?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_zenuml_path_attrs(path_id.as_str())?;
        self.write_block_inline_path_style(path_id, style)?;
        self.write_c4_shape_inline_style(path_id, style, None)?;
        if !self.write_gantt_path_presentation(path_id, style, true)? {
            self.write_path_style(style)?;
            self.write_state_attrs()?;
        }
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_xychart_background(&mut self, bounds: Rect, style: &PathStyle) -> Result<()> {
        let Some(fill) = style.fill.as_ref() else {
            return Err(invalid("XYChart background is missing its fill"));
        };
        write!(
            self.output,
            "<rect width=\"{}\" height=\"{}\" class=\"background\"",
            fmt(bounds.width),
            fmt(bounds.height),
        )?;
        self.write_paint("fill", fill)?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_rect(&mut self, path_id: &ResourceId, bounds: Rect, style: &PathStyle) -> Result<()> {
        self.output.push_str("<rect x=\"")?;
        write!(
            self.output,
            "{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
        )?;
        self.write_gantt_dom_id(path_id.as_str())?;
        self.write_journey_dom_id(path_id.as_str())?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_block_inline_path_style(path_id, style)?;
        self.write_c4_shape_inline_style(path_id, style, None)?;
        if !self.write_gantt_path_presentation(path_id, style, false)? {
            self.write_fill_stroke_style(style)?;
            self.write_state_attrs()?;
        }
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_rounded_rect(
        &mut self,
        path_id: &ResourceId,
        bounds: Rect,
        radius: f64,
        style: &PathStyle,
    ) -> Result<()> {
        self.emit_elliptical_rounded_rect(path_id, bounds, radius, radius, style)
    }

    fn emit_elliptical_rounded_rect(
        &mut self,
        path_id: &ResourceId,
        bounds: Rect,
        radius_x: f64,
        radius_y: f64,
        style: &PathStyle,
    ) -> Result<()> {
        self.output.push_str("<rect x=\"")?;
        write!(
            self.output,
            "{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\"",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(radius_x),
            fmt(radius_y),
        )?;
        self.write_gantt_dom_id(path_id.as_str())?;
        self.write_journey_dom_id(path_id.as_str())?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_zenuml_path_attrs(path_id.as_str())?;
        let inline_radius = path_id.as_str().ends_with(".shape").then_some(radius_x);
        self.write_block_inline_path_style(path_id, style)?;
        self.write_c4_shape_inline_style(path_id, style, inline_radius)?;
        if !self.write_gantt_path_presentation(path_id, style, false)? {
            self.write_fill_stroke_style(style)?;
            self.write_state_attrs()?;
        }
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_circle(
        &mut self,
        path_id: &ResourceId,
        center: Point,
        radius: f64,
        style: &PathStyle,
    ) -> Result<()> {
        write!(
            self.output,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"",
            fmt(center.x),
            fmt(center.y),
            fmt(radius),
        )?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_zenuml_path_attrs(path_id.as_str())?;
        self.write_block_inline_path_style(path_id, style)?;
        self.write_c4_shape_inline_style(path_id, style, None)?;
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs()?;
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    #[cfg(feature = "layout-cytoscape")]
    fn emit_ellipse(
        &mut self,
        path_id: &ResourceId,
        center: Point,
        radius_x: f64,
        radius_y: f64,
        style: &PathStyle,
    ) -> Result<()> {
        write!(
            self.output,
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"",
            fmt(center.x),
            fmt(center.y),
            fmt(radius_x),
            fmt(radius_y),
        )?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_zenuml_path_attrs(path_id.as_str())?;
        let inline_radius = path_id.as_str().ends_with(".shape").then_some(12.0);
        self.write_block_inline_path_style(path_id, style)?;
        self.write_c4_shape_inline_style(path_id, style, inline_radius)?;
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs()?;
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_polygon(
        &mut self,
        path_id: &ResourceId,
        points: &[Point],
        style: &PathStyle,
    ) -> Result<()> {
        self.output.push_str("<polygon points=\"")?;
        for (index, point) in points.iter().enumerate() {
            if index > 0 {
                self.output.push(' ')?;
            }
            write!(self.output, "{},{}", fmt(point.x), fmt(point.y))?;
        }
        self.output.push('"')?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_zenuml_path_attrs(path_id.as_str())?;
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs()?;
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_line(
        &mut self,
        path_id: &ResourceId,
        start: Point,
        end: Point,
        style: &PathStyle,
    ) -> Result<()> {
        write!(
            self.output,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
            fmt(start.x),
            fmt(start.y),
            fmt(end.x),
            fmt(end.y),
        )?;
        self.write_gantt_dom_id(path_id.as_str())?;
        self.write_journey_dom_id(path_id.as_str())?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_wardley_marker_attributes(path_id.as_str())?;
        self.write_zenuml_path_attrs(path_id.as_str())?;
        self.write_block_inline_path_style(path_id, style)?;
        if !self.write_gantt_path_presentation(path_id, style, false)? {
            self.write_fill_stroke_style(style)?;
            self.write_state_attrs()?;
        }
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_gantt_tick_line(
        &mut self,
        path_id: &ResourceId,
        y2: f64,
        style: &PathStyle,
    ) -> Result<()> {
        write!(self.output, "<line y2=\"{}\"", fmt(y2))?;
        self.write_gantt_dom_id(path_id.as_str())?;
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_ref())?;
            self.output.push('"')?;
        }
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs()?;
        write_resource_metadata(&mut self.output, self.debug, path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn is_wardley_marker_path(&self, raw_id: &str) -> bool {
        if !matches!(self.svg_body, SvgStructureBody::Wardley(_)) {
            return false;
        }
        (raw_id.starts_with("wardley.link.")
            && (raw_id.ends_with(".start") || raw_id.ends_with(".end")))
            || (raw_id.starts_with("wardley.trend.") && raw_id.ends_with(".end"))
    }

    fn is_er_marker_path(&self, raw_id: &str) -> bool {
        matches!(self.svg_body, SvgStructureBody::Er(_))
            && raw_id.starts_with("er.edge.")
            && (raw_id.ends_with(".marker.start") || raw_id.ends_with(".marker.end"))
    }

    fn write_er_edge_attrs(&mut self, raw_id: &str) -> Result<()> {
        let SvgStructureBody::Er(body) = self.svg_body else {
            return Ok(());
        };
        let Some(metadata) = body.edge_metadata.get(raw_id) else {
            return Ok(());
        };
        self.output.push_str(" id=\"")?;
        output::escape_attr(
            &mut self.output,
            format!("{}-{}", self.diagram_id, metadata.dom_id).as_str(),
        )?;
        self.output.push_str("\" style=\"undefined;;;undefined\"")?;
        self.output
            .push_str(" data-edge=\"true\" data-et=\"edge\" data-id=\"")?;
        output::escape_attr(&mut self.output, metadata.dom_id.as_str())?;
        self.output.push_str("\" data-points=\"")?;
        output::escape_attr(&mut self.output, metadata.data_points.as_str())?;
        self.output.push_str("\" data-look=\"")?;
        output::escape_attr(&mut self.output, body.data_look.as_str())?;
        self.output.push('"')?;
        if let Some(marker) = metadata.start_marker.as_deref()
            && let Some(marker_id) =
                er_marker_svg_id(self.diagram_id.as_str(), body.diagram_type.as_str(), marker)
        {
            self.output.push_str(" marker-start=\"url(#")?;
            output::escape_attr(&mut self.output, marker_id.as_str())?;
            self.output.push_str(")\"")?;
        }
        if let Some(marker) = metadata.end_marker.as_deref()
            && let Some(marker_id) =
                er_marker_svg_id(self.diagram_id.as_str(), body.diagram_type.as_str(), marker)
        {
            self.output.push_str(" marker-end=\"url(#")?;
            output::escape_attr(&mut self.output, marker_id.as_str())?;
            self.output.push_str(")\"")?;
        }
        Ok(())
    }

    fn write_block_edge_attrs(&mut self, raw_id: &str) -> Result<()> {
        let SvgStructureBody::Block(body) = self.svg_body else {
            return Ok(());
        };
        let Some(metadata) = body.edge_metadata.get(raw_id) else {
            return Ok(());
        };
        let data_id = format!("{}-{}", self.diagram_id, metadata.source_id);
        self.output.push_str(" id=\"")?;
        output::escape_attr(
            &mut self.output,
            format!("{}-{data_id}", self.diagram_id).as_str(),
        )?;
        self.output.push_str("\" style=\"undefined;;;undefined\"")?;
        self.output
            .push_str(" data-edge=\"true\" data-et=\"edge\" data-id=\"")?;
        output::escape_attr(&mut self.output, data_id.as_str())?;
        self.output.push_str("\" data-points=\"")?;
        let data_points = STANDARD.encode(super::util::json_stringify_points(
            metadata.points.as_slice(),
        ));
        output::escape_attr(&mut self.output, data_points.as_str())?;
        self.output.push('"')?;
        Ok(())
    }

    fn write_wardley_marker_attributes(&mut self, raw_id: &str) -> Result<()> {
        if !matches!(self.svg_body, SvgStructureBody::Wardley(_)) {
            return Ok(());
        }
        let (prefix, is_link) = if let Some(prefix) = raw_id.strip_suffix(".line") {
            if prefix.starts_with("wardley.link.") {
                (prefix, true)
            } else if prefix.starts_with("wardley.trend.") {
                (prefix, false)
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        };

        let has_marker = |suffix: &str| self.resources.contains_key(&format!("{prefix}{suffix}"));
        if is_link && has_marker(".start") {
            write!(
                self.output,
                " marker-start=\"url(#{})\"",
                escaped_attr(format!("link-arrow-start-{}", self.diagram_id).as_str())
            )?;
        }
        if has_marker(".end") {
            let marker_id = if is_link {
                format!("link-arrow-end-{}", self.diagram_id)
            } else {
                format!("arrow-{}", self.diagram_id)
            };
            write!(
                self.output,
                " marker-end=\"url(#{})\"",
                escaped_attr(marker_id.as_str())
            )?;
        }
        Ok(())
    }

    fn emit_text(&mut self, run: &TextRun) -> Result<()> {
        match &run.obligation {
            TextObligation::HostText { .. } => self.emit_host_text(run),
            TextObligation::Outline { path } => {
                let style = PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(run.style.fill.clone()),
                    stroke: None,
                };
                self.emit_path(path, &style)
            }
            TextObligation::RasterFallback { fallback_id } => {
                self.emit_fallback(fallback_id, Some(run.bounds))
            }
            TextObligation::GlyphRun { .. } => Err(Error::DrawingListUnavailable {
                family: self.family.as_str().to_string(),
                reason: "glyph-run text requires a glyph-aware SVG encoder; refusing a silent host-text substitution".to_string(),
            }),
        }
    }

    fn emit_host_text(&mut self, run: &TextRun) -> Result<()> {
        if matches!(self.svg_body, SvgStructureBody::Error(_)) {
            return super::error::write_error_text(&mut self.output, run);
        }
        let semantic_id = self.current_semantic_id().map(str::to_owned);
        let text_index = self.record_text_index();
        if self.emit_compact_sankey_text(run, semantic_id.as_deref())? {
            return Ok(());
        }
        if self.emit_compact_pie_text(run, semantic_id.as_deref(), text_index)? {
            return Ok(());
        }
        if matches!(self.svg_body, SvgStructureBody::Gantt(_))
            && self.emit_gantt_tick_text(run, semantic_id.as_deref())?
        {
            return Ok(());
        }
        if self.emit_gantt_plain_text(run, semantic_id.as_deref())? {
            return Ok(());
        }
        if let Some(styles) = &self.packet_styles
            && let Some(class) =
                super::packet::packet_text_class(semantic_id.as_deref(), text_index)
            && styles.compact_text(class, run)
        {
            let baseline = match run.baseline {
                TextBaseline::Middle => "middle",
                TextBaseline::Alphabetic => "auto",
                other => text_baseline(other),
            };
            write!(
                self.output,
                "<text x=\"{}\" y=\"{}\" class=\"{}\" dominant-baseline=\"{}\" text-anchor=\"{}\"",
                fmt(run.origin.x),
                fmt(run.origin.y),
                class,
                baseline,
                text_anchor(run.anchor)
            )?;
            self.write_state_attrs()?;
            self.output.push('>')?;
            output::escape_xml(&mut self.output, &run.text)?;
            self.output.push_str("</text>")?;
            return Ok(());
        }
        if self.emit_compact_info_text(run)? {
            return Ok(());
        }
        if self.emit_compact_cynefin_text(run, semantic_id.as_deref())? {
            return Ok(());
        }
        if self.emit_compact_venn_text(run, semantic_id.as_deref())? {
            return Ok(());
        }
        if matches!(self.svg_body, SvgStructureBody::Mindmap(_)) {
            return self.emit_mindmap_html_text(run, semantic_id.as_deref());
        }
        if matches!(self.svg_body, SvgStructureBody::Block(_)) {
            return self.emit_block_html_text(run, semantic_id.as_deref(), text_index);
        }
        if matches!(self.svg_body, SvgStructureBody::C4(_)) {
            return self.emit_c4_text(run, text_index);
        }
        if let SvgStructureBody::Er(body) = self.svg_body
            && semantic_id.as_deref() != Some("er.title")
        {
            let relationship_label = semantic_id
                .as_deref()
                .is_some_and(|id| id.starts_with("er.edge.") && id.ends_with(".label"));
            if (!relationship_label && body.entity_html_labels)
                || (relationship_label && body.relationship_html_labels)
            {
                return self.emit_er_html_text(run, semantic_id.as_deref(), text_index);
            }
        }
        if matches!(self.svg_body, SvgStructureBody::Zenuml(_)) {
            return self.emit_zenuml_text(run, semantic_id.as_deref(), text_index);
        }

        self.output.push_str("<text")?;
        self.write_gantt_text_space_attr(run, semantic_id.as_deref())?;
        self.write_gantt_title_name(run)?;
        if let Some(semantic_id) = semantic_id.as_deref() {
            self.write_gantt_dom_id(semantic_id)?;
            #[cfg(feature = "layout-cytoscape")]
            if !matches!(self.svg_body, SvgStructureBody::Architecture(_)) {
                self.write_sidecar_dom_id(semantic_id)?;
            }
            #[cfg(not(feature = "layout-cytoscape"))]
            self.write_sidecar_dom_id(semantic_id)?;
        }
        if let Some(class) = self.text_class(run, text_index) {
            let class = class.into_owned();
            self.output.push_str(" class=\"")?;
            self.output.push_str(class.as_str())?;
            self.output.push('"')?;
        }
        let font_size = if matches!(self.svg_body, SvgStructureBody::Error(_)) {
            format!("{}px", fmt(run.style.font_size))
        } else {
            fmt(run.style.font_size).to_string()
        };
        let baseline = match self.svg_body {
            SvgStructureBody::Packet(_) => match run.baseline {
                TextBaseline::Middle => "middle",
                TextBaseline::Alphabetic => "auto",
                other => text_baseline(other),
            },
            SvgStructureBody::XyChart(_) => xychart_text_baseline(run.baseline),
            SvgStructureBody::Wardley(_) => wardley_text_baseline(run.baseline),
            SvgStructureBody::Cynefin(_) => match run.baseline {
                TextBaseline::Middle => "middle",
                other => text_baseline(other),
            },
            _ => text_baseline(run.baseline),
        };
        let gitgraph_branch_label = matches!(self.svg_body, SvgStructureBody::GitGraph(body)
        if self.current_semantic_id().is_some_and(|id| {
            body.text_classes
                .get(id)
                .is_some_and(|class| class.split_whitespace().any(|token| token.starts_with("branch-label")))
        }));
        if gitgraph_branch_label {
            write!(
                self.output,
                " x=\"0\" y=\"{}\" transform=\"translate({}, 0)\" text-anchor=\"{}\" direction=\"{}\" font-size=\"{}\" letter-spacing=\"{}\"",
                fmt(run.origin.y - run.style.line_height),
                fmt(run.origin.x),
                text_anchor(run.anchor),
                text_direction(run.direction),
                font_size,
                fmt(run.style.letter_spacing),
            )?;
        } else {
            write!(
                self.output,
                " x=\"{}\" y=\"{}\" text-anchor=\"{}\" dominant-baseline=\"{}\" direction=\"{}\" font-size=\"{}\" letter-spacing=\"{}\"",
                fmt(run.origin.x),
                fmt(run.origin.y),
                text_anchor(run.anchor),
                baseline,
                text_direction(run.direction),
                font_size,
                fmt(run.style.letter_spacing),
            )?;
        }
        let families = self.font_families(&run.style.font)?;
        self.output.push_str(" font-family=\"")?;
        output::escape_attr(&mut self.output, families.as_str())?;
        self.output.push('"')?;
        write!(
            self.output,
            " font-weight=\"{}\" font-style=\"{}\"",
            run.style.font.weight,
            font_style(run.style.font.style),
        )?;
        write_text_metadata(&mut self.output, self.debug, run)?;
        if let Some(language) = run.language.as_deref() {
            self.output.push_str(" xml:lang=\"")?;
            output::escape_attr(&mut self.output, language)?;
            self.output.push('"')?;
        }
        self.write_gantt_task_text_height()?;
        self.write_paint("fill", &run.style.fill)?;
        if let Some(stroke) = &run.style.stroke {
            self.write_stroke_style(Some(stroke))?;
            let order = match run.style.paint_order {
                merman_display_list::TextPaintOrder::FillThenStroke => "fill stroke",
                merman_display_list::TextPaintOrder::StrokeThenFill => "stroke fill",
            };
            write!(self.output, " paint-order=\"{order}\"")?;
        }
        self.write_state_attrs()?;
        self.output.push('>')?;

        let mut lines = run.text.split('\n');
        if gitgraph_branch_label {
            if let Some(first) = lines.next() {
                self.output
                    .push_str(r#"<tspan xml:space="preserve" dy="1em" x="0" class="row">"#)?;
                output::escape_xml(&mut self.output, first)?;
                self.output.push_str("</tspan>")?;
            }
            for line in lines {
                write!(
                    self.output,
                    "<tspan x=\"0\" dy=\"{}\" class=\"row\">",
                    fmt(run.style.line_height),
                )?;
                output::escape_xml(&mut self.output, line)?;
                self.output.push_str("</tspan>")?;
            }
        } else {
            if let Some(first) = lines.next() {
                output::escape_xml(&mut self.output, first)?;
            }
            for line in lines {
                write!(
                    self.output,
                    "<tspan x=\"{}\" dy=\"{}\">",
                    fmt(run.origin.x),
                    fmt(run.style.line_height),
                )?;
                output::escape_xml(&mut self.output, line)?;
                self.output.push_str("</tspan>")?;
            }
        }
        self.output.push_str("</text>")?;
        Ok(())
    }

    fn emit_zenuml_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
        text_index: Option<usize>,
    ) -> Result<()> {
        self.output.push_str("<text")?;
        let families = self.font_families(&run.style.font)?;
        write!(
            self.output,
            " x=\"{}\" y=\"{}\" text-anchor=\"{}\" dominant-baseline=\"{}\" direction=\"{}\" font-size=\"{}\" letter-spacing=\"{}\" font-family=\"{}\" font-weight=\"{}\" font-style=\"{}\"",
            fmt(run.origin.x),
            fmt(run.origin.y),
            text_anchor(run.anchor),
            text_baseline(run.baseline),
            text_direction(run.direction),
            fmt(run.style.font_size),
            fmt(run.style.letter_spacing),
            escaped_attr(families.as_str()),
            run.style.font.weight,
            font_style(run.style.font.style),
        )?;
        write_text_metadata(&mut self.output, self.debug, run)?;
        if let Some(language) = run.language.as_deref() {
            self.output.push_str(" xml:lang=\"")?;
            output::escape_attr(&mut self.output, language)?;
            self.output.push('"')?;
        }
        if let Some(semantic_id) = semantic_id
            && let SvgStructureBody::Zenuml(body) = self.svg_body
            && let Some(statement_id) = body.semantic_data_statements.get(semantic_id)
        {
            self.output.push_str(" data-statement=\"")?;
            output::escape_attr(&mut self.output, statement_id)?;
            self.output.push('"')?;
        }
        self.write_paint("fill", &run.style.fill)?;
        self.write_state_attrs()?;
        if let Some(class) = self
            .text_class(run, text_index)
            .map(|class| class.into_owned())
        {
            self.output.push_str(" class=\"")?;
            output::escape_attr(&mut self.output, class.as_str())?;
            self.output.push('"')?;
        }
        self.output.push('>')?;
        let mut lines = run.text.split('\n');
        if let Some(first) = lines.next() {
            output::escape_xml(&mut self.output, first)?;
        }
        for line in lines {
            write!(
                self.output,
                "<tspan x=\"{}\" dy=\"{}\">",
                fmt(run.origin.x),
                fmt(run.style.line_height),
            )?;
            output::escape_xml(&mut self.output, line)?;
            self.output.push_str("</tspan>")?;
        }
        self.output.push_str("</text>")?;
        Ok(())
    }

    fn emit_mindmap_html_text(&mut self, run: &TextRun, semantic_id: Option<&str>) -> Result<()> {
        let semantic_id =
            semantic_id.ok_or_else(|| invalid("Mindmap text is outside a node semantic group"))?;
        let (node, label_max_width) = match self.svg_body {
            SvgStructureBody::Mindmap(body) => (
                body.nodes.get(semantic_id).cloned().ok_or_else(|| {
                    invalid(format!(
                        "Mindmap SVG sidecar is missing text node {semantic_id}"
                    ))
                })?,
                body.label_max_width,
            ),
            _ => return Err(invalid("Mindmap text requires a Mindmap sidecar")),
        };
        let max_width = if label_max_width.is_finite() && label_max_width > 0.0 {
            label_max_width
        } else {
            200.0
        };
        let bounds = Rect::new(
            run.bounds.x - node.origin.x,
            run.bounds.y - node.origin.y,
            run.bounds.width.max(1.0),
            run.bounds.height.max(1.0),
        );
        let wrap_container = bounds.width >= max_width - 1e-3;
        write!(
            self.output,
            "<g class=\"label\" style=\"\" transform=\"translate({}, {})\"",
            fmt(bounds.x),
            fmt(bounds.y),
        )?;
        write_text_metadata(&mut self.output, self.debug, run)?;
        write!(
            self.output,
            "><rect/><foreignObject width=\"{}\" height=\"{}\"><div xmlns=\"http://www.w3.org/1999/xhtml\" style=\"",
            fmt(bounds.width),
            fmt(bounds.height),
        )?;
        if wrap_container {
            write!(
                self.output,
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: {}px; text-align: center; width: {}px;",
                fmt(max_width),
                fmt(max_width),
            )?;
        } else {
            write!(
                self.output,
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                fmt(max_width),
            )?;
        }
        self.output
            .push_str(r#""><span class="nodeLabel markdown-node-label">"#)?;
        self.output.push_str(&super::mindmap::mindmap_label_xhtml(
            node.label_source.as_str(),
            self.mermaid_config
                .as_ref()
                .ok_or_else(|| invalid("Mindmap SVG serializer is missing effective config"))?,
            None,
        )?)?;
        self.output.push_str("</span></div></foreignObject></g>")?;
        Ok(())
    }

    fn emit_c4_text(&mut self, run: &TextRun, text_index: Option<usize>) -> Result<()> {
        self.output.push_str("<text")?;
        if let Some(class) = self
            .text_class(run, text_index)
            .map(|class| class.into_owned())
        {
            self.output.push_str(" class=\"")?;
            output::escape_attr(&mut self.output, class.as_str())?;
            self.output.push('"')?;
        }
        write!(
            self.output,
            " x=\"{}\" y=\"{}\" dominant-baseline=\"middle\" direction=\"{}\" font-style=\"{}\"",
            fmt(run.origin.x),
            fmt(run.origin.y),
            text_direction(run.direction),
            font_style(run.style.font.style),
        )?;
        write_text_metadata(&mut self.output, self.debug, run)?;
        if let Some(language) = run.language.as_deref() {
            self.output.push_str(" xml:lang=\"")?;
            output::escape_attr(&mut self.output, language)?;
            self.output.push('"')?;
        }
        let families = self.font_families(&run.style.font)?;
        self.output.push_str(" style=\"")?;
        write!(
            self.output,
            "text-anchor: {}; font-size: {}px; font-weight: {}; font-family: {};",
            text_anchor(run.anchor),
            fmt(run.style.font_size),
            run.style.font.weight,
            escaped_attr(families.as_str()),
        )?;
        if self
            .current_semantic_id()
            .is_some_and(|id| id.starts_with("c4.shape."))
            && let Paint::Solid { color } = &run.style.fill
        {
            write!(self.output, " fill: {} !important;", color_css(*color),)?;
        }
        self.output.push('"')?;
        self.write_paint("fill", &run.style.fill)?;
        self.write_state_attrs()?;
        self.output.push('>')?;
        for (index, line) in run.text.split('\n').enumerate() {
            self.output.push_str("<tspan")?;
            if index > 0 {
                write!(
                    self.output,
                    " x=\"{}\" dy=\"{}\"",
                    fmt(run.origin.x),
                    fmt(run.style.line_height),
                )?;
            }
            self.output
                .push_str(" alignment-baseline=\"mathematical\">")?;
            output::escape_xml(&mut self.output, line)?;
            self.output.push_str("</tspan>")?;
        }
        self.output.push_str("</text>")?;
        Ok(())
    }

    fn emit_er_html_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
        text_index: Option<usize>,
    ) -> Result<()> {
        let class = self
            .text_class(run, text_index)
            .unwrap_or(Cow::Borrowed("nodeLabel"))
            .into_owned();
        let wrapper_class = if semantic_id.is_some_and(|id| id.ends_with(".label")) {
            "label edgeLabel"
        } else {
            "label"
        };
        let bounds = run.bounds;
        write!(
            self.output,
            "<g class=\"{}\" transform=\"translate({}, {})\"",
            wrapper_class,
            fmt(bounds.x),
            fmt(bounds.y),
        )?;
        write_text_metadata(&mut self.output, self.debug, run)?;
        write!(
            self.output,
            "><foreignObject x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"><div xmlns=\"http://www.w3.org/1999/xhtml\" class=\"labelBkg\" style=\"display: table-cell; white-space: nowrap; line-height: 1.5; text-align: center;\"><span class=\"{}\">",
            fmt(bounds.width.max(0.0)),
            fmt(bounds.height.max(0.0)),
            escaped_attr(class.as_str()),
        )?;
        for (index, line) in run.text.split('\n').enumerate() {
            if index > 0 {
                self.output.push_str("<br/>")?;
            }
            output::escape_xml(&mut self.output, line)?;
        }
        self.output.push_str("</span></div></foreignObject></g>")?;
        Ok(())
    }

    fn emit_block_html_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
        text_index: Option<usize>,
    ) -> Result<()> {
        let Some(semantic_id) = semantic_id else {
            return Err(invalid("Block HTML text is outside a semantic group"));
        };
        let class: String = self
            .text_class(run, text_index)
            .map(Cow::into_owned)
            .unwrap_or_else(|| "nodeLabel".to_string());
        let (max_width, text_style, div_prefix, data_id) = {
            let SvgStructureBody::Block(body) = self.svg_body else {
                return Err(invalid("Block HTML text requires a Block sidecar"));
            };
            let max_width = body
                .label_max_widths
                .get(semantic_id)
                .copied()
                .unwrap_or(run.bounds.width.max(0.0));
            let (_, text_style, div_prefix) = body
                .label_inline_styles
                .get(semantic_id)
                .map(|styles| super::block::block_inline_styles(styles))
                .unwrap_or_default();
            (
                max_width,
                text_style,
                div_prefix,
                body.label_data_ids.get(semantic_id).cloned(),
            )
        };
        let bounds = run.bounds;

        if semantic_id.starts_with("block.edge.") && semantic_id.ends_with(".label") {
            self.output.push_str("<g class=\"label\"")?;
            if let Some(data_id) = data_id.as_deref() {
                self.output.push_str(" data-id=\"")?;
                output::escape_attr(&mut self.output, data_id)?;
                self.output.push('"')?;
            }
            write!(
                self.output,
                " transform=\"translate({}, {})\"><foreignObject width=\"{}\" height=\"{}\"><div xmlns=\"http://www.w3.org/1999/xhtml\" class=\"labelBkg\" style=\"display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;\"><span class=\"{}\"><p>",
                fmt(bounds.x),
                fmt(bounds.y),
                fmt(bounds.width.max(0.0)),
                fmt(bounds.height.max(0.0)),
                fmt(max_width),
                escaped_attr(class.as_str()),
            )?;
            output::escape_xml(&mut self.output, run.text.as_str())?;
            self.output
                .push_str("</p></span></div></foreignObject></g>")?;
            return Ok(());
        }

        self.output.push_str("<g class=\"label\" style=\"")?;
        output::escape_attr(&mut self.output, text_style.as_str())?;
        write!(
            self.output,
            "\" transform=\"translate({}, {})\"><rect/><foreignObject width=\"{}\" height=\"{}\"><div xmlns=\"http://www.w3.org/1999/xhtml\" style=\"",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width.max(0.0)),
            fmt(bounds.height.max(0.0)),
        )?;
        output::escape_attr(&mut self.output, div_prefix.as_str())?;
        write!(
            self.output,
            "display: table-cell; white-space: nowrap; line-height: 1.5;\"><span class=\"{}\"",
            escaped_attr(class.as_str()),
        )?;
        if !text_style.is_empty() {
            self.output.push_str(" style=\"")?;
            output::escape_attr(&mut self.output, text_style.as_str())?;
            self.output.push('"')?;
        }
        self.output.push_str("><p>")?;
        output::escape_xml(&mut self.output, run.text.as_str())?;
        self.output
            .push_str("</p></span></div></foreignObject></g>")?;
        Ok(())
    }

    fn path_class(&self, path_id: &ResourceId) -> Option<Cow<'static, str>> {
        if matches!(self.svg_body, SvgStructureBody::Error(_))
            && path_id.as_str().starts_with("error.icon.")
        {
            return Some(Cow::Borrowed("error-icon"));
        }
        if let SvgStructureBody::Pie(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Timeline(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Sankey(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Railroad(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::EventModeling(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Ishikawa(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Cynefin(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::TreeView(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Gantt(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Journey(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Kanban(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::GitGraph(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Treemap(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Requirement(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::State(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Er(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Block(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::C4(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        #[cfg(feature = "layout-cytoscape")]
        if let SvgStructureBody::Architecture(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Wardley(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if let SvgStructureBody::Zenuml(body) = self.svg_body
            && let Some(class) = body.path_classes.get(path_id.as_str())
        {
            return Some(Cow::Owned(class.clone()));
        }
        if matches!(self.svg_body, SvgStructureBody::Packet(_))
            && path_id.as_str().ends_with(".shape")
        {
            return Some(Cow::Borrowed("packetBlock"));
        }
        if matches!(self.svg_body, SvgStructureBody::Radar(_)) {
            let raw_id = path_id.as_str();
            if raw_id.starts_with("radar.graticule.") {
                return Some(Cow::Borrowed("radarGraticule"));
            }
            if raw_id.starts_with("radar.axis.") {
                return Some(Cow::Borrowed("radarAxisLine"));
            }
            if let Some(index) = raw_id
                .strip_prefix("radar.curve.")
                .and_then(|value| value.strip_suffix(".shape"))
            {
                return Some(Cow::Owned(format!("radarCurve-{index}")));
            }
            if let Some(index) = raw_id
                .strip_prefix("radar.legend.")
                .and_then(|value| value.strip_suffix(".box"))
            {
                return Some(Cow::Owned(format!("radarLegendBox-{index}")));
            }
        }
        None
    }

    fn write_zenuml_path_attrs(&mut self, path_id: &str) -> Result<()> {
        let SvgStructureBody::Zenuml(body) = self.svg_body else {
            return Ok(());
        };
        if let Some(icon) = body.path_data_icons.get(path_id) {
            self.output.push_str(" data-icon=\"")?;
            output::escape_attr(&mut self.output, icon)?;
            self.output.push('"')?;
        }
        Ok(())
    }

    fn write_gantt_dom_id(&mut self, key: &str) -> Result<()> {
        let Some(raw_id) = (match self.svg_body {
            SvgStructureBody::Gantt(body) => body.dom_ids.get(key),
            _ => None,
        })
        .cloned() else {
            return Ok(());
        };
        self.output.push_str(" id=\"")?;
        output::escape_attr(&mut self.output, self.diagram_id.as_str())?;
        self.output.push('-')?;
        output::escape_attr(&mut self.output, raw_id.as_str())?;
        self.output.push('"')?;
        self.write_gantt_task_semantics()?;
        Ok(())
    }

    fn write_journey_dom_id(&mut self, key: &str) -> Result<()> {
        let Some(raw_id) = (match self.svg_body {
            SvgStructureBody::Journey(body) => body.dom_ids.get(key),
            _ => None,
        })
        .cloned() else {
            return Ok(());
        };
        self.output.push_str(" id=\"")?;
        output::escape_attr(&mut self.output, self.diagram_id.as_str())?;
        self.output.push('-')?;
        output::escape_attr(&mut self.output, raw_id.as_str())?;
        self.output.push('"')?;
        Ok(())
    }

    fn write_sidecar_dom_id(&mut self, key: &str) -> Result<()> {
        let raw_id = match self.svg_body {
            SvgStructureBody::GitGraph(body) => body.dom_ids.get(key),
            SvgStructureBody::Er(body) => body.dom_ids.get(key),
            SvgStructureBody::Block(body) => body.dom_ids.get(key),
            #[cfg(feature = "layout-cytoscape")]
            SvgStructureBody::Architecture(body) => body.dom_ids.get(key),
            _ => None,
        };
        let Some(raw_id) = raw_id else {
            return Ok(());
        };
        self.output.push_str(" id=\"")?;
        output::escape_attr(&mut self.output, self.diagram_id.as_str())?;
        self.output.push('-')?;
        output::escape_attr(&mut self.output, raw_id.as_str())?;
        self.output.push('"')?;
        Ok(())
    }

    fn write_block_inline_path_style(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<()> {
        let properties = match self.svg_body {
            SvgStructureBody::Block(body) => {
                body.path_inline_properties.get(path_id.as_str()).cloned()
            }
            _ => None,
        };
        let Some(properties) = properties else {
            return Ok(());
        };

        let paint_css = |encoder: &mut Self, paint: Option<&Paint>| -> Result<String> {
            match paint {
                Some(Paint::Solid { color }) => Ok(color_css(*color)),
                Some(Paint::Resource { id }) => {
                    Ok(format!("url(#{})", encoder.svg_resource_id(id.as_str())?))
                }
                None => Ok("none".to_string()),
            }
        };
        let paint_opacity = |paint: Option<&Paint>| match paint {
            Some(Paint::Solid { color }) => f64::from(color.alpha) / 255.0,
            Some(Paint::Resource { .. }) | None => 1.0,
        };

        let mut declarations = String::new();
        for property in properties {
            if !declarations.is_empty() {
                declarations.push(';');
            }
            match property {
                BlockInlinePathProperty::BackgroundColor(value) => {
                    write!(declarations, "background-color:{} !important", value)
                }
                BlockInlinePathProperty::Fill => write!(
                    declarations,
                    "fill:{} !important",
                    paint_css(self, style.fill.as_ref())?
                ),
                BlockInlinePathProperty::Stroke => write!(
                    declarations,
                    "stroke:{} !important",
                    paint_css(self, style.stroke.as_ref().map(|stroke| &stroke.paint))?
                ),
                BlockInlinePathProperty::StrokeWidth => write!(
                    declarations,
                    "stroke-width:{}px !important",
                    fmt(style.stroke.as_ref().map_or(0.0, |stroke| stroke.width))
                ),
                BlockInlinePathProperty::StrokeDashArray => {
                    declarations.push_str("stroke-dasharray:");
                    if let Some(stroke) = style.stroke.as_ref()
                        && !stroke.dash_array.is_empty()
                    {
                        for (index, value) in stroke.dash_array.iter().enumerate() {
                            if index > 0 {
                                declarations.push(',');
                            }
                            write!(declarations, "{}", fmt(*value))
                                .map_err(|_| invalid("SVG component formatting failed"))?;
                        }
                    } else {
                        declarations.push_str("none");
                    }
                    declarations.push_str(" !important");
                    Ok(())
                }
                BlockInlinePathProperty::StrokeDashOffset => write!(
                    declarations,
                    "stroke-dashoffset:{}px !important",
                    fmt(style
                        .stroke
                        .as_ref()
                        .map_or(0.0, |stroke| stroke.dash_offset))
                ),
                BlockInlinePathProperty::StrokeLineCap => write!(
                    declarations,
                    "stroke-linecap:{} !important",
                    style
                        .stroke
                        .as_ref()
                        .map_or("butt", |stroke| line_cap(stroke.line_cap))
                ),
                BlockInlinePathProperty::StrokeLineJoin => write!(
                    declarations,
                    "stroke-linejoin:{} !important",
                    style
                        .stroke
                        .as_ref()
                        .map_or("miter", |stroke| line_join(stroke.line_join))
                ),
                BlockInlinePathProperty::StrokeMiterLimit => write!(
                    declarations,
                    "stroke-miterlimit:{} !important",
                    fmt(style
                        .stroke
                        .as_ref()
                        .map_or(4.0, |stroke| stroke.miter_limit))
                ),
                BlockInlinePathProperty::FillRule => write!(
                    declarations,
                    "fill-rule:{} !important",
                    fill_rule_name(style.fill_rule)
                ),
                BlockInlinePathProperty::Opacity => write!(
                    declarations,
                    "opacity:{} !important",
                    fmt(self.state.opacity)
                ),
                BlockInlinePathProperty::FillOpacity => write!(
                    declarations,
                    "fill-opacity:{} !important",
                    fmt(paint_opacity(style.fill.as_ref()))
                ),
                BlockInlinePathProperty::StrokeOpacity => write!(
                    declarations,
                    "stroke-opacity:{} !important",
                    fmt(paint_opacity(
                        style.stroke.as_ref().map(|stroke| &stroke.paint)
                    ))
                ),
            }
            .map_err(|_| invalid("SVG component formatting failed"))?;
        }

        self.output.push_str(" style=\"")?;
        output::escape_attr(&mut self.output, declarations.as_str())?;
        self.output.push('"')?;
        Ok(())
    }

    fn write_c4_shape_inline_style(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
        radius: Option<f64>,
    ) -> Result<()> {
        // Mermaid's unified C4 renderer compiles node cssStyles into inline `!important`
        // declarations so per-element/config colors win over provider and host CSS.
        if !matches!(self.svg_body, SvgStructureBody::C4(_))
            || !path_id.as_str().starts_with("c4.shape.")
        {
            return Ok(());
        }

        let paint_css = |paint: &Paint| -> Result<String> {
            match paint {
                Paint::Solid { color } => Ok(color_css(*color)),
                Paint::Resource { id } => {
                    Ok(format!("url(#{})", self.svg_resource_id(id.as_str())?))
                }
            }
        };
        let fill = style
            .fill
            .as_ref()
            .ok_or_else(|| invalid(format!("C4 shape {path_id:?} is missing its fill")))?;
        let stroke = style
            .stroke
            .as_ref()
            .ok_or_else(|| invalid(format!("C4 shape {path_id:?} is missing its stroke")))?;
        let fill = paint_css(fill)?;
        let stroke = paint_css(&stroke.paint)?;

        self.output.push_str(" style=\"fill:")?;
        output::escape_attr(&mut self.output, fill.as_str())?;
        self.output.push_str(" !important;stroke:")?;
        output::escape_attr(&mut self.output, stroke.as_str())?;
        self.output.push_str(" !important")?;
        if let Some(radius) = radius {
            write!(
                self.output,
                ";rx:{}px !important;ry:{}px !important",
                fmt(radius),
                fmt(radius),
            )?;
        }
        self.output.push('"')?;
        Ok(())
    }

    fn text_class(&self, _run: &TextRun, text_index: Option<usize>) -> Option<Cow<'_, str>> {
        match self.svg_body {
            SvgStructureBody::Error(_) => Some(Cow::Borrowed("error-text")),
            SvgStructureBody::Info(_) => Some(Cow::Borrowed("version")),
            SvgStructureBody::Sankey(_) => self
                .current_semantic_id()
                .filter(|id| id.starts_with("sankey.label."))
                .and_then(|id| self.semantic_extra_class(id))
                .map(Cow::Borrowed),
            SvgStructureBody::Packet(_) => {
                super::packet::packet_text_class(self.current_semantic_id(), text_index)
                    .map(Cow::Borrowed)
            }
            SvgStructureBody::Pie(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Borrowed(class.as_str())),
            SvgStructureBody::Venn(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Railroad(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::EventModeling(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Ishikawa(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Cynefin(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::TreeView(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Gantt(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Journey(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Kanban(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::GitGraph(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Treemap(body) => {
                let semantic_id = self.current_semantic_id()?;
                let class = text_index
                    .and_then(|index| body.text_classes.get(&format!("{semantic_id}#{index}")))
                    .or_else(|| body.text_classes.get(semantic_id))?;
                Some(Cow::Owned(class.clone()))
            }
            SvgStructureBody::Requirement(body) => self
                .current_semantic_id()
                .and_then(|id| {
                    text_index
                        .and_then(|index| body.text_classes.get(&format!("{id}.label.{index}")))
                        .or_else(|| body.text_classes.get(id))
                })
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::State(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Er(body) => {
                let semantic_id = self.current_semantic_id()?;
                let class = text_index
                    .and_then(|index| body.text_classes.get(&format!("{semantic_id}#{index}")))
                    .or_else(|| body.text_classes.get(semantic_id))?;
                Some(Cow::Owned(class.clone()))
            }
            SvgStructureBody::Block(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::C4(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            #[cfg(feature = "layout-cytoscape")]
            SvgStructureBody::Architecture(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Wardley(body) => self
                .current_semantic_id()
                .and_then(|id| body.text_classes.get(id))
                .map(|class| Cow::Owned(class.clone())),
            SvgStructureBody::Zenuml(body) => {
                let semantic_id = self.current_semantic_id()?;
                let class = text_index
                    .and_then(|index| body.text_classes.get(&format!("{semantic_id}#{index}")))?;
                Some(Cow::Owned(class.clone()))
            }
            SvgStructureBody::Radar(_) => match self.current_semantic_id() {
                Some(id) if id.starts_with("radar.axis.") => Some(Cow::Borrowed("radarAxisLabel")),
                Some(id) if id.starts_with("radar.legend.") => {
                    Some(Cow::Borrowed("radarLegendText"))
                }
                Some("radar.title") => Some(Cow::Borrowed("radarTitle")),
                _ => None,
            },
            _ => None,
        }
    }

    fn current_semantic_id(&self) -> Option<&str> {
        self.groups.iter().rev().find_map(|group| match group {
            GroupKind::Semantic { semantic_id, .. } => Some(semantic_id.as_str()),
            _ => None,
        })
    }

    fn record_text_index(&mut self) -> Option<usize> {
        let semantic_id = self.current_semantic_id()?.to_owned();
        let count = self.semantic_text_counts.entry(semantic_id).or_default();
        let index = *count;
        *count = count.saturating_add(1);
        Some(index)
    }

    fn emit_image(
        &mut self,
        image_id: &ResourceId,
        bounds: Rect,
        opacity: f64,
        fallback: Option<&merman_display_list::RasterFallback>,
    ) -> Result<()> {
        let image = self.image_resource(image_id)?;
        let uri = data_uri(&image.image.media_type, &image.image.data);
        write!(
            self.output,
            "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" opacity=\"{}\" preserveAspectRatio=\"none\"",
            escaped_attr(uri),
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(self.state.opacity * opacity),
        )?;
        if let Some(fallback) = fallback {
            write!(
                self.output,
                " data-merman-fallback-reason=\"{}\" data-merman-fallback-family=\"{}\" data-merman-fallback-effect=\"{}\"",
                fallback_reason(fallback.reason),
                escaped_attr(fallback.source.family.as_str()),
                escaped_attr(fallback.source.effect.as_str()),
            )?;
        }
        self.write_transform_and_blend()?;
        self.output.push_str("/>")?;
        Ok(())
    }

    fn emit_fallback(&mut self, fallback_id: &str, override_bounds: Option<Rect>) -> Result<()> {
        let fallback = self
            .fallbacks
            .get(fallback_id)
            .ok_or_else(|| invalid(format!("SVG fallback references unknown id {fallback_id}")))?;
        self.emit_image(
            &fallback.image,
            override_bounds.unwrap_or(fallback.bounds),
            1.0,
            Some(fallback),
        )
    }

    fn write_path_style(&mut self, style: &PathStyle) -> Result<()> {
        self.output.push_str(" fill-rule=\"")?;
        self.output.push_str(fill_rule_name(style.fill_rule))?;
        self.output.push('"')?;
        self.write_fill_stroke_style(style)
    }

    fn write_fill_stroke_style(&mut self, style: &PathStyle) -> Result<()> {
        if let Some(fill) = style.fill.as_ref() {
            self.write_paint("fill", fill)?;
        } else {
            self.output.push_str(" fill=\"none\"")?;
        }
        self.write_stroke_style(style.stroke.as_ref())
    }

    fn write_stroke_style(
        &mut self,
        stroke: Option<&merman_display_list::StrokeStyle>,
    ) -> Result<()> {
        if let Some(stroke) = stroke {
            self.write_paint("stroke", &stroke.paint)?;
            write!(
                self.output,
                " stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\" stroke-miterlimit=\"{}\"",
                fmt(stroke.width),
                line_cap(stroke.line_cap),
                line_join(stroke.line_join),
                fmt(stroke.miter_limit),
            )?;
            if !stroke.dash_array.is_empty() {
                self.output.push_str(" stroke-dasharray=\"")?;
                for (index, value) in stroke.dash_array.iter().enumerate() {
                    if index > 0 {
                        self.output.push(',')?;
                    }
                    write!(self.output, "{}", fmt(*value))?;
                }
                self.output.push('"')?;
            }
            if stroke.dash_offset != 0.0 {
                write!(
                    self.output,
                    " stroke-dashoffset=\"{}\"",
                    fmt(stroke.dash_offset)
                )?;
            }
        } else {
            self.output.push_str(" stroke=\"none\"")?;
        }
        Ok(())
    }

    fn write_paint(&mut self, attribute: &str, paint: &Paint) -> Result<()> {
        self.output.push(' ')?;
        self.output.push_str(attribute)?;
        self.output.push_str("=\"")?;
        match paint {
            Paint::Solid { color } => {
                self.output.push_str(color_css(*color).as_str())?;
                self.output.push('"')?;
                if color.alpha != u8::MAX {
                    write!(
                        self.output,
                        " {}-opacity=\"{}\"",
                        attribute,
                        fmt(f64::from(color.alpha) / 255.0),
                    )?;
                }
            }
            Paint::Resource { id } => {
                let svg_id = self.svg_resource_id(id.as_str())?;
                write!(self.output, "url(#{})\"", escaped_attr(svg_id.as_str()))?;
            }
        }
        Ok(())
    }

    fn write_state_attrs(&mut self) -> Result<()> {
        self.write_transform_and_blend()?;
        if self.state.opacity != 1.0 {
            write!(self.output, " opacity=\"{}\"", fmt(self.state.opacity))?;
        }
        Ok(())
    }

    fn write_transform_and_blend(&mut self) -> Result<()> {
        let family_text = self
            .current_semantic_id()
            .is_some_and(|id| match self.svg_body {
                SvgStructureBody::Pie(_) => id.starts_with("pie."),
                SvgStructureBody::QuadrantChart(_) => id.starts_with("quadrantchart."),
                SvgStructureBody::XyChart(_) => id.starts_with("xychart.text."),
                SvgStructureBody::GitGraph(_) => id.starts_with("gitgraph."),
                SvgStructureBody::Treemap(_) => id.starts_with("treemap."),
                #[cfg(feature = "layout-cytoscape")]
                SvgStructureBody::Architecture(_) => id.starts_with("architecture."),
                _ => false,
            });
        if family_text
            && self.state.transform != Transform::IDENTITY
            && is_rigid_transform(self.state.transform)
        {
            let transform = self.state.transform;
            let rotation = transform.b.atan2(transform.a).to_degrees();
            write!(
                self.output,
                " transform=\"translate({}, {}) rotate({})\"",
                fmt(transform.e),
                fmt(transform.f),
                fmt(rotation),
            )?;
        } else if self.state.transform != Transform::IDENTITY {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(self.state.transform)
            )?;
        }
        write_blend_style(&mut self.output, self.state.blend_mode)?;
        Ok(())
    }

    fn font_families(&self, font: &merman_display_list::FontDescriptor) -> Result<String> {
        let mut families = String::new();
        if let Some(resource) = font.resource.as_ref()
            && let Some(svg_id) = self.resource_svg_ids.get(resource.as_str())
        {
            families.push('"');
            families.push_str(svg_id);
            families.push_str("\",");
        }
        let family_css = crate::portable_font::PortableFontFamilies::from_resolved(&font.families)
            .map_err(|error| {
                invalid(format!(
                    "text font family is not portable to the SVG projection: {error}"
                ))
            })?
            .to_css();
        if !families.is_empty() && !families.ends_with(',') {
            families.push(',');
        }
        families.push_str(&family_css);
        Ok(families)
    }

    fn debug_visibility(&self, role: SemanticRole) -> bool {
        match role {
            SemanticRole::Node => self.debug.include_nodes,
            SemanticRole::Edge => self.debug.include_edges,
            SemanticRole::Group => self.debug.include_clusters,
            SemanticRole::Document | SemanticRole::Label => true,
        }
    }

    fn path_resource(&self, id: &ResourceId) -> Result<&'a PathResource> {
        match self.resources.get(id.as_str()) {
            Some(DrawingResource::Path(path)) => Ok(path),
            Some(_) => Err(invalid(format!("resource {} is not a path", id.as_str()))),
            None => Err(invalid(format!("unknown path resource {}", id.as_str()))),
        }
    }

    fn image_resource(&self, id: &ResourceId) -> Result<&'a merman_display_list::ImageResource> {
        match self.resources.get(id.as_str()) {
            Some(DrawingResource::Image(image)) => Ok(image),
            Some(_) => Err(invalid(format!("resource {} is not an image", id.as_str()))),
            None => Err(invalid(format!("unknown image resource {}", id.as_str()))),
        }
    }

    fn svg_resource_id(&self, id: &str) -> Result<String> {
        if matches!(self.svg_body, SvgStructureBody::GitGraph(_)) && id == "gitgraph.gradient" {
            return Ok(format!("{}-gradient", self.diagram_id));
        }
        self.resource_svg_ids
            .get(id)
            .cloned()
            .ok_or_else(|| invalid(format!("unknown SVG resource id {id}")))
    }

    fn semantic_svg_id(&self, id: &str) -> Result<String> {
        if let SvgStructureBody::Kanban(body) = self.svg_body
            && let Some(raw_id) = body.dom_ids.get(id)
        {
            return Ok(format!("{}-{}", self.diagram_id, raw_id));
        }
        if let SvgStructureBody::Requirement(body) = self.svg_body
            && let Some(raw_id) = body.dom_ids.get(id)
        {
            return Ok(format!("{}-{}", self.diagram_id, raw_id));
        }
        if let SvgStructureBody::Er(body) = self.svg_body
            && let Some(raw_id) = body.dom_ids.get(id)
        {
            return Ok(format!("{}-{}", self.diagram_id, raw_id));
        }
        if let SvgStructureBody::State(body) = self.svg_body
            && let Some(raw_id) = body.dom_ids.get(id)
        {
            return Ok(format!("{}-{}", self.diagram_id, raw_id));
        }
        if let SvgStructureBody::Block(body) = self.svg_body
            && let Some(raw_id) = body.dom_ids.get(id)
        {
            return Ok(format!("{}-{}", self.diagram_id, raw_id));
        }
        if let SvgStructureBody::C4(body) = self.svg_body
            && let Some(raw_id) = body.dom_ids.get(id)
        {
            return Ok(format!("{}-{}", self.diagram_id, raw_id));
        }
        #[cfg(feature = "layout-cytoscape")]
        if let SvgStructureBody::Architecture(body) = self.svg_body
            && let Some(raw_id) = body.dom_ids.get(id)
        {
            return Ok(format!("{}-{}", self.diagram_id, raw_id));
        }
        self.semantic_svg_ids
            .get(id)
            .cloned()
            .ok_or_else(|| invalid(format!("unknown SVG semantic id {id}")))
    }

    fn clip_svg_id(&self, id: &str) -> Result<String> {
        let resource = self.svg_resource_id(id)?;
        Ok(format!("{resource}-clip"))
    }
}

fn is_er_container_semantic(semantic_id: &str) -> bool {
    matches!(
        semantic_id,
        "er.root" | "er.clusters" | "er.edges" | "er.edgeLabels" | "er.nodes"
    )
}

fn default_diagram_id(family: RenderFamilyKind) -> &'static str {
    match family {
        RenderFamilyKind::Mindmap => "mindmap",
        RenderFamilyKind::Packet => "merman",
        RenderFamilyKind::State
        | RenderFamilyKind::Sequence
        | RenderFamilyKind::Class
        | RenderFamilyKind::C4
        | RenderFamilyKind::Er
        | RenderFamilyKind::Flowchart
        | RenderFamilyKind::Swimlane
        | RenderFamilyKind::Block
        | RenderFamilyKind::Error => "merman",
        RenderFamilyKind::QuadrantChart => "quadrantchart",
        RenderFamilyKind::Pie => "merman",
        RenderFamilyKind::Timeline => "merman",
        _ => family.as_str(),
    }
}

fn is_rigid_transform(transform: Transform) -> bool {
    let epsilon = 1e-12;
    (transform.a * transform.a + transform.b * transform.b - 1.0).abs() <= epsilon
        && (transform.c + transform.b).abs() <= epsilon
        && (transform.d - transform.a).abs() <= epsilon
        && transform.a.is_finite()
        && transform.b.is_finite()
        && transform.c.is_finite()
        && transform.d.is_finite()
        && transform.e.is_finite()
        && transform.f.is_finite()
}

fn rectangle_from_path(path: &PathResource) -> Option<Rect> {
    rectangle_path_bounds(path).filter(|bounds| bounds.width > 0.0 && bounds.height > 0.0)
}

fn rectangle_path_bounds(path: &PathResource) -> Option<Rect> {
    let [
        PathSegment::MoveTo { to: first },
        PathSegment::LineTo { to: second },
        PathSegment::LineTo { to: third },
        PathSegment::LineTo { to: fourth },
        PathSegment::Close,
    ] = path.segments.as_slice()
    else {
        return None;
    };
    let epsilon = f64::EPSILON * 8.0;
    let horizontal = |left: Point, right: Point| (left.y - right.y).abs() <= epsilon;
    let vertical = |top: Point, bottom: Point| (top.x - bottom.x).abs() <= epsilon;
    if !horizontal(*first, *second)
        || !vertical(*second, *third)
        || !horizontal(*third, *fourth)
        || !vertical(*fourth, *first)
        || (first.x - fourth.x).abs() > epsilon
        || (second.x - third.x).abs() > epsilon
        || (first.y - second.y).abs() > epsilon
        || (third.y - fourth.y).abs() > epsilon
    {
        return None;
    }
    let x = first.x.min(second.x).min(third.x).min(fourth.x);
    let y = first.y.min(second.y).min(third.y).min(fourth.y);
    let max_x = first.x.max(second.x).max(third.x).max(fourth.x);
    let max_y = first.y.max(second.y).max(third.y).max(fourth.y);
    Some(Rect::new(x, y, max_x - x, max_y - y))
}

fn rounded_rectangle_from_path(path: &PathResource) -> Option<(Rect, f64)> {
    let (bounds, rx, ry) = elliptical_rounded_rectangle_from_path(path)?;
    ((rx - ry).abs() <= 1e-9).then_some((bounds, rx))
}

fn elliptical_rounded_rectangle_from_path(path: &PathResource) -> Option<(Rect, f64, f64)> {
    let [
        PathSegment::MoveTo { to: p0 },
        PathSegment::LineTo { to: p1 },
        PathSegment::ArcTo {
            radius_x: r1x,
            radius_y: r1y,
            x_axis_rotation_degrees: rot1,
            large_arc: large1,
            sweep_clockwise: sweep1,
            to: p2,
        },
        PathSegment::LineTo { to: p3 },
        PathSegment::ArcTo {
            radius_x: r2x,
            radius_y: r2y,
            x_axis_rotation_degrees: rot2,
            large_arc: large2,
            sweep_clockwise: sweep2,
            to: p4,
        },
        PathSegment::LineTo { to: p5 },
        PathSegment::ArcTo {
            radius_x: r3x,
            radius_y: r3y,
            x_axis_rotation_degrees: rot3,
            large_arc: large3,
            sweep_clockwise: sweep3,
            to: p6,
        },
        PathSegment::LineTo { to: p7 },
        PathSegment::ArcTo {
            radius_x: r4x,
            radius_y: r4y,
            x_axis_rotation_degrees: rot4,
            large_arc: large4,
            sweep_clockwise: sweep4,
            to: p8,
        },
        PathSegment::Close,
    ] = path.segments.as_slice()
    else {
        return None;
    };
    let epsilon = 1e-9;
    let same = |left: f64, right: f64| (left - right).abs() <= epsilon;
    let rx = *r1x;
    let ry = *r1y;
    if !rx.is_finite()
        || !ry.is_finite()
        || rx <= 0.0
        || ry <= 0.0
        || !same(*r1x, *r2x)
        || !same(*r1y, *r2y)
        || !same(*r1x, *r3x)
        || !same(*r1y, *r3y)
        || !same(*r1x, *r4x)
        || !same(*r1y, *r4y)
        || [*rot1, *rot2, *rot3, *rot4]
            .into_iter()
            .any(|value| value.abs() > epsilon)
        || [*large1, *large2, *large3, *large4]
            .into_iter()
            .any(|value| value)
        || [*sweep1, *sweep2, *sweep3, *sweep4]
            .into_iter()
            .any(|value| !value)
    {
        return None;
    }
    let left = p7.x;
    let right = p2.x;
    let top = p0.y;
    let bottom = p5.y;
    if [left, right, top, bottom]
        .into_iter()
        .any(|value| !value.is_finite())
        || !same(p0.x, left + rx)
        || !same(p1.x, right - rx)
        || !same(p1.y, top)
        || !same(p2.x, right)
        || !same(p2.y, top + ry)
        || !same(p3.x, right)
        || !same(p3.y, bottom - ry)
        || !same(p4.x, right - rx)
        || !same(p4.y, bottom)
        || !same(p5.x, left + rx)
        || !same(p6.x, left)
        || !same(p6.y, bottom - ry)
        || !same(p7.x, left)
        || !same(p7.y, top + ry)
        || !same(p8.x, left + rx)
        || !same(p8.y, top)
    {
        return None;
    }
    let bounds = Rect::new(left, top, right - left, bottom - top);
    // Subtracting absolute endpoints can round a saturated diameter down by a few ulps.
    // Use the same tolerance as the corner-coordinate checks, not a stricter second gate.
    (bounds.width > 0.0
        && bounds.height > 0.0
        && (rx <= bounds.width / 2.0 || same(rx, bounds.width / 2.0))
        && (ry <= bounds.height / 2.0 || same(ry, bounds.height / 2.0)))
    .then_some((bounds, rx, ry))
}

fn line_from_path(path: &PathResource) -> Option<(Point, Point)> {
    let [
        PathSegment::MoveTo { to: start },
        PathSegment::LineTo { to: end },
    ] = path.segments.as_slice()
    else {
        return None;
    };
    Some((*start, *end))
}

fn polygon_from_path(path: &PathResource) -> Option<Vec<Point>> {
    let (first, rest) = path.segments.split_first()?;
    let PathSegment::MoveTo { to: first } = first else {
        return None;
    };
    let (last, lines) = rest.split_last()?;
    if !matches!(last, PathSegment::Close) || lines.len() < 2 {
        return None;
    }
    let mut points = Vec::with_capacity(lines.len() + 1);
    points.push(*first);
    for segment in lines {
        let PathSegment::LineTo { to } = segment else {
            return None;
        };
        points.push(*to);
    }
    Some(points)
}

fn circle_from_path(path: &PathResource) -> Option<(Point, f64)> {
    let [
        PathSegment::MoveTo { to: start },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees,
            large_arc,
            sweep_clockwise,
            to: opposite,
        },
        PathSegment::ArcTo {
            radius_x: second_radius_x,
            radius_y: second_radius_y,
            x_axis_rotation_degrees: second_rotation,
            large_arc: second_large_arc,
            sweep_clockwise: second_sweep_clockwise,
            to: end,
        },
        PathSegment::Close,
    ] = path.segments.as_slice()
    else {
        return None;
    };

    let epsilon = f64::EPSILON * 32.0;
    let approx_eq = |left: f64, right: f64| (left - right).abs() <= epsilon;
    if !radius_x.is_finite()
        || !radius_y.is_finite()
        || *radius_x <= 0.0
        || (*radius_x - *radius_y).abs() > epsilon
        || (*radius_x - *second_radius_x).abs() > epsilon
        || (*radius_y - *second_radius_y).abs() > epsilon
        || *x_axis_rotation_degrees != 0.0
        || *second_rotation != 0.0
        || *large_arc
        || *second_large_arc
        || !*sweep_clockwise
        || !*second_sweep_clockwise
        || !approx_eq(start.x, end.x)
        || !approx_eq(start.y, end.y)
    {
        return None;
    }
    let center = Point::new((start.x + opposite.x) / 2.0, (start.y + opposite.y) / 2.0);
    if !center.x.is_finite() || !center.y.is_finite() {
        return None;
    }
    Some((center, *radius_x))
}

#[cfg(feature = "layout-cytoscape")]
fn ellipse_from_path(path: &PathResource) -> Option<(Point, f64, f64)> {
    let [
        PathSegment::MoveTo { to: start },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees,
            large_arc,
            sweep_clockwise,
            to: opposite,
        },
        PathSegment::ArcTo {
            radius_x: second_radius_x,
            radius_y: second_radius_y,
            x_axis_rotation_degrees: second_rotation,
            large_arc: second_large_arc,
            sweep_clockwise: second_sweep_clockwise,
            to: end,
        },
        PathSegment::Close,
    ] = path.segments.as_slice()
    else {
        return None;
    };

    let epsilon = 1e-9;
    let approx_eq = |left: f64, right: f64| (left - right).abs() <= epsilon;
    if !radius_x.is_finite()
        || !radius_y.is_finite()
        || *radius_x <= 0.0
        || *radius_y <= 0.0
        || !approx_eq(*radius_x, *second_radius_x)
        || !approx_eq(*radius_y, *second_radius_y)
        || !approx_eq(*x_axis_rotation_degrees, 0.0)
        || !approx_eq(*second_rotation, 0.0)
        || *large_arc
        || *second_large_arc
        || !*sweep_clockwise
        || !*second_sweep_clockwise
        || !approx_eq(start.x, end.x)
        || !approx_eq(start.y, end.y)
        || !approx_eq(start.y, opposite.y)
        || !approx_eq((start.x - opposite.x).abs(), 2.0 * radius_x)
    {
        return None;
    }
    let center = Point::new((start.x + opposite.x) / 2.0, start.y);
    if !center.x.is_finite() || !center.y.is_finite() {
        return None;
    }
    Some((center, *radius_x, *radius_y))
}

fn cubic_circle_from_path(path: &PathResource) -> Option<(Point, f64)> {
    let [
        PathSegment::MoveTo { to: start },
        PathSegment::CubicTo {
            control1,
            control2,
            to: first,
        },
        PathSegment::CubicTo {
            control1: second_control1,
            control2: second_control2,
            to: second,
        },
        PathSegment::CubicTo {
            control1: third_control1,
            control2: third_control2,
            to: third,
        },
        PathSegment::CubicTo {
            control1: fourth_control1,
            control2: fourth_control2,
            to: end,
        },
        PathSegment::Close,
    ] = path.segments.as_slice()
    else {
        return None;
    };

    let points = [
        *start,
        *control1,
        *control2,
        *first,
        *second_control1,
        *second_control2,
        *second,
        *third_control1,
        *third_control2,
        *third,
        *fourth_control1,
        *fourth_control2,
        *end,
    ];
    if points
        .iter()
        .any(|point| !point.x.is_finite() || !point.y.is_finite())
    {
        return None;
    }
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    let width = max_x - min_x;
    let height = max_y - min_y;
    let epsilon = 1e-8_f64.max(width.max(height) * 1e-8);
    if width <= epsilon || (width - height).abs() > epsilon {
        return None;
    }
    let center = Point::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    let radius = width / 2.0;
    if (start.x - end.x).abs() > epsilon || (start.y - end.y).abs() > epsilon {
        return None;
    }
    Some((center, radius))
}

fn scoped_id(diagram_id: &str, prefix: &str, index: usize, raw: &str) -> String {
    let scope = sanitize_svg_id(diagram_id);
    let sanitized = sanitize_svg_id(raw);
    let suffix = sanitized.chars().take(48).collect::<String>();
    format!("merman-{prefix}-{scope}-{index}-{suffix}")
}

fn er_marker_svg_id(diagram_id: &str, diagram_type: &str, marker: &str) -> Option<String> {
    let marker = marker.trim();
    let (base, suffix) = if let Some(base) = marker.strip_suffix("_START") {
        (base, "Start")
    } else if let Some(base) = marker.strip_suffix("_END") {
        (base, "End")
    } else {
        return None;
    };
    let marker_type = match base {
        "ONLY_ONE" => "onlyOne",
        "ZERO_OR_ONE" => "zeroOrOne",
        "ONE_OR_MORE" => "oneOrMore",
        "ZERO_OR_MORE" => "zeroOrMore",
        "MD_PARENT" => "mdParent",
        _ => return None,
    };
    Some(format!("{diagram_id}_{diagram_type}-{marker_type}{suffix}"))
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn escaped_attr(value: impl std::fmt::Display) -> impl std::fmt::Display {
    super::util::escape_attr_display(value)
}

fn escaped_css_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('<', "\\3c ")
        .replace('>', "\\3e ")
}

fn color_css(color: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}

fn opacity_attr(alpha: u8) -> String {
    if alpha == u8::MAX {
        String::new()
    } else {
        format!(" stop-opacity=\"{}\"", fmt(f64::from(alpha) / 255.0))
    }
}

fn offset_path_segments(segments: &[PathSegment], dx: f64, dy: f64) -> Vec<PathSegment> {
    let offset = |point: &Point| Point::new(point.x + dx, point.y + dy);
    segments
        .iter()
        .map(|segment| match segment {
            PathSegment::MoveTo { to } => PathSegment::MoveTo { to: offset(to) },
            PathSegment::LineTo { to } => PathSegment::LineTo { to: offset(to) },
            PathSegment::QuadTo { control, to } => PathSegment::QuadTo {
                control: offset(control),
                to: offset(to),
            },
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => PathSegment::CubicTo {
                control1: offset(control1),
                control2: offset(control2),
                to: offset(to),
            },
            PathSegment::ArcTo {
                radius_x,
                radius_y,
                x_axis_rotation_degrees,
                large_arc,
                sweep_clockwise,
                to,
            } => PathSegment::ArcTo {
                radius_x: *radius_x,
                radius_y: *radius_y,
                x_axis_rotation_degrees: *x_axis_rotation_degrees,
                large_arc: *large_arc,
                sweep_clockwise: *sweep_clockwise,
                to: offset(to),
            },
            PathSegment::Close => PathSegment::Close,
        })
        .collect()
}

fn path_d(segments: &[PathSegment]) -> PathData<'_> {
    PathData {
        segments,
        lowercase_close: false,
    }
}

struct PathData<'a> {
    segments: &'a [PathSegment],
    lowercase_close: bool,
}

impl PathData<'_> {
    fn with_lowercase_close(mut self) -> Self {
        self.lowercase_close = true;
        self
    }
}

impl std::fmt::Display for PathData<'_> {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, segment) in self.segments.iter().enumerate() {
            if index != 0 {
                output.write_str(" ")?;
            }
            match segment {
                PathSegment::MoveTo { to } => write!(output, "M {} {}", fmt(to.x), fmt(to.y)),
                PathSegment::LineTo { to } => write!(output, "L {} {}", fmt(to.x), fmt(to.y)),
                PathSegment::QuadTo { control, to } => write!(
                    output,
                    "Q {} {} {} {}",
                    fmt(control.x),
                    fmt(control.y),
                    fmt(to.x),
                    fmt(to.y)
                ),
                PathSegment::CubicTo {
                    control1,
                    control2,
                    to,
                } => write!(
                    output,
                    "C {} {} {} {} {} {}",
                    fmt(control1.x),
                    fmt(control1.y),
                    fmt(control2.x),
                    fmt(control2.y),
                    fmt(to.x),
                    fmt(to.y)
                ),
                PathSegment::ArcTo {
                    radius_x,
                    radius_y,
                    x_axis_rotation_degrees,
                    large_arc,
                    sweep_clockwise,
                    to,
                } => write!(
                    output,
                    "A {} {} {} {} {} {} {}",
                    fmt(*radius_x),
                    fmt(*radius_y),
                    fmt(*x_axis_rotation_degrees),
                    u8::from(*large_arc),
                    u8::from(*sweep_clockwise),
                    fmt(to.x),
                    fmt(to.y)
                ),
                PathSegment::Close => {
                    output.write_str(if self.lowercase_close { "z" } else { "Z" })
                }
            }?;
        }
        Ok(())
    }
}

fn matrix_attr(transform: Transform) -> String {
    format!(
        "{} {} {} {} {} {}",
        fmt(transform.a),
        fmt(transform.b),
        fmt(transform.c),
        fmt(transform.d),
        fmt(transform.e),
        fmt(transform.f),
    )
}

fn multiply_transform(left: Transform, right: Transform) -> Transform {
    Transform {
        a: left.a * right.a + left.c * right.b,
        b: left.b * right.a + left.d * right.b,
        c: left.a * right.c + left.c * right.d,
        d: left.b * right.c + left.d * right.d,
        e: left.a * right.e + left.c * right.f + left.e,
        f: left.b * right.e + left.d * right.f + left.f,
    }
}

fn fill_rule_name(rule: FillRule) -> &'static str {
    match rule {
        FillRule::NonZero => "nonzero",
        FillRule::EvenOdd => "evenodd",
    }
}

fn line_cap(cap: LineCap) -> &'static str {
    match cap {
        LineCap::Butt => "butt",
        LineCap::Round => "round",
        LineCap::Square => "square",
    }
}

fn line_join(join: LineJoin) -> &'static str {
    match join {
        LineJoin::Miter => "miter",
        LineJoin::Round => "round",
        LineJoin::Bevel => "bevel",
    }
}

fn font_style(style: FontStyle) -> &'static str {
    match style {
        FontStyle::Normal => "normal",
        FontStyle::Italic => "italic",
        FontStyle::Oblique => "oblique",
    }
}

fn text_anchor(anchor: TextAnchor) -> &'static str {
    match anchor {
        TextAnchor::Start => "start",
        TextAnchor::Middle => "middle",
        TextAnchor::End => "end",
    }
}

fn text_baseline(baseline: TextBaseline) -> &'static str {
    match baseline {
        TextBaseline::Alphabetic => "alphabetic",
        TextBaseline::Hanging => "hanging",
        TextBaseline::Ideographic => "ideographic",
        TextBaseline::Middle => "central",
        TextBaseline::Central => "central",
        TextBaseline::TextBeforeEdge => "text-before-edge",
        TextBaseline::TextAfterEdge => "text-after-edge",
    }
}

fn xychart_text_baseline(baseline: TextBaseline) -> &'static str {
    match baseline {
        TextBaseline::Alphabetic => "auto",
        TextBaseline::Hanging => "hanging",
        TextBaseline::Middle => "middle",
        TextBaseline::Central => "central",
        TextBaseline::TextBeforeEdge => "text-before-edge",
        TextBaseline::Ideographic => "ideographic",
        TextBaseline::TextAfterEdge => "text-after-edge",
    }
}

fn wardley_text_baseline(baseline: TextBaseline) -> &'static str {
    match baseline {
        // Wardley's source renderer distinguishes SVG's `middle` from `central`; the latter is
        // a different baseline keyword with subtly different font metrics.
        TextBaseline::Middle => "middle",
        TextBaseline::Central => "central",
        TextBaseline::Alphabetic => "auto",
        TextBaseline::Hanging => "hanging",
        TextBaseline::Ideographic => "ideographic",
        TextBaseline::TextBeforeEdge => "text-before-edge",
        TextBaseline::TextAfterEdge => "text-after-edge",
    }
}

fn text_direction(direction: TextDirection) -> &'static str {
    match direction {
        TextDirection::Auto => "auto",
        TextDirection::Ltr => "ltr",
        TextDirection::Rtl => "rtl",
    }
}

fn semantic_role_class(role: SemanticRole) -> &'static str {
    match role {
        SemanticRole::Document => "semantic-document",
        SemanticRole::Node => "semantic-node",
        SemanticRole::Edge => "semantic-edge",
        SemanticRole::Group => "semantic-group",
        SemanticRole::Label => "semantic-label",
    }
}

fn blend_css(blend_mode: BlendMode) -> Option<&'static str> {
    match blend_mode {
        BlendMode::Normal => None,
        BlendMode::Multiply => Some("multiply"),
        BlendMode::Screen => Some("screen"),
        BlendMode::Overlay => Some("overlay"),
        BlendMode::Darken => Some("darken"),
        BlendMode::Lighten => Some("lighten"),
        BlendMode::ColorDodge => Some("color-dodge"),
        BlendMode::ColorBurn => Some("color-burn"),
        BlendMode::HardLight => Some("hard-light"),
        BlendMode::SoftLight => Some("soft-light"),
        BlendMode::Difference => Some("difference"),
        BlendMode::Exclusion => Some("exclusion"),
    }
}

/// Diagnostic copies of public identifiers never determine rendering or accessibility.
fn write_resource_metadata(
    output: &mut SvgOutput<'_>,
    debug: &SvgDebugOptions,
    id: &str,
) -> Result<()> {
    if debug.include_drawing_list_metadata {
        output.push_str(" data-merman-resource=\"")?;
        output::escape_attr(output, id)?;
        output.push('"')?;
    }
    Ok(())
}

fn write_semantic_metadata(
    output: &mut SvgOutput<'_>,
    debug: &SvgDebugOptions,
    id: &str,
) -> Result<()> {
    if debug.include_drawing_list_metadata {
        output.push_str(" data-merman-semantic-id=\"")?;
        output::escape_attr(output, id)?;
        output.push('"')?;
    }
    Ok(())
}

fn write_text_metadata(
    output: &mut SvgOutput<'_>,
    debug: &SvgDebugOptions,
    run: &TextRun,
) -> Result<()> {
    if debug.include_drawing_list_metadata {
        write!(
            output,
            " data-merman-bounds=\"{},{},{},{}\" data-merman-text-obligation=\"host_text\"",
            fmt(run.bounds.x),
            fmt(run.bounds.y),
            fmt(run.bounds.width),
            fmt(run.bounds.height),
        )?;
    }
    Ok(())
}

fn write_blend_style(output: &mut SvgOutput<'_>, blend_mode: BlendMode) -> Result<()> {
    if let Some(value) = blend_css(blend_mode) {
        write!(output, " style=\"mix-blend-mode: {};\"", value)?;
    }
    Ok(())
}

fn spread_method(spread: GradientSpread) -> &'static str {
    match spread {
        GradientSpread::Pad => "pad",
        GradientSpread::Repeat => "repeat",
        GradientSpread::Reflect => "reflect",
    }
}

fn fallback_reason(reason: merman_display_list::FallbackReason) -> &'static str {
    match reason {
        merman_display_list::FallbackReason::ForeignObject => "foreign_object",
        merman_display_list::FallbackReason::Filter => "filter",
        merman_display_list::FallbackReason::Mask => "mask",
        merman_display_list::FallbackReason::UnsupportedEffect => "unsupported_effect",
        merman_display_list::FallbackReason::TextShaping => "text_shaping",
        merman_display_list::FallbackReason::EmbeddedGraphic => "embedded_graphic",
        merman_display_list::FallbackReason::HostResource => "host_resource",
        merman_display_list::FallbackReason::Pattern => "pattern",
        merman_display_list::FallbackReason::Math => "math",
        merman_display_list::FallbackReason::Icon => "icon",
        merman_display_list::FallbackReason::HandDrawn => "hand_drawn",
    }
}

fn data_uri<'a>(media_type: &str, data: &'a [u8]) -> impl std::fmt::Display + 'a {
    struct DataUri<'a> {
        media_type: String,
        data: &'a [u8],
    }
    impl std::fmt::Display for DataUri<'_> {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "data:{};base64,{}",
                self.media_type,
                base64::display::Base64Display::new(self.data, &STANDARD)
            )
        }
    }
    DataUri {
        media_type: safe_media_type(media_type),
        data,
    }
}

fn safe_media_type(media_type: &str) -> String {
    let main = media_type.split(';').next().unwrap_or(media_type).trim();
    if main.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '/' | '+' | '-' | '.')
    }) {
        main.to_string()
    } else {
        "application/octet-stream".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SvgOutput, data_uri, escaped_attr, matrix_attr, multiply_transform, path_d,
        wardley_text_baseline,
    };
    use merman_display_list::{PathSegment, Point, TextBaseline, Transform};

    #[test]
    fn nested_path_and_base64_formatters_preserve_output_resource_errors() {
        use crate::environment::RenderEnvironment;
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        let path = [PathSegment::MoveTo {
            to: Point::new(12345.0, 67890.0),
        }];
        for encoding in ["path", "pie-path", "image"] {
            let environment = RenderEnvironment::deterministic().with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(
                        ResourceLimitId::MaxSvgBytes,
                        if encoding == "image" { 32 } else { 8 },
                    )
                    .unwrap(),
            );
            let session = environment.begin_session().unwrap();
            let mut output = SvgOutput::new(&session);
            let error = if encoding == "image" {
                output.write_fmt(format_args!(
                    "{}",
                    escaped_attr(data_uri("image/png", &[42; 64]))
                ))
            } else if encoding == "pie-path" {
                output.write_fmt(format_args!(
                    "{}",
                    escaped_attr(super::super::curve::drawing_path_segments_d_unrounded(
                        &path
                    ))
                ))
            } else {
                output.write_fmt(format_args!("{}", escaped_attr(path_d(&path))))
            }
            .unwrap_err();
            assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
            assert!(matches!(
                output.finish(),
                Err(crate::Error::ResourceLimitExceeded(_))
            ));
        }
    }

    #[test]
    fn path_encoder_preserves_all_protocol_segments() {
        let d = path_d(&[
            PathSegment::MoveTo {
                to: Point::new(1.0, 2.0),
            },
            PathSegment::LineTo {
                to: Point::new(3.0, 4.0),
            },
            PathSegment::QuadTo {
                control: Point::new(5.0, 6.0),
                to: Point::new(7.0, 8.0),
            },
            PathSegment::Close,
        ])
        .to_string();
        assert_eq!(d, "M 1 2 L 3 4 Q 5 6 7 8 Z");
    }

    #[test]
    fn affine_transform_composition_matches_svg_matrix_order() {
        let translate = Transform {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 10.0,
            f: 20.0,
        };
        let scale = Transform {
            a: 2.0,
            b: 0.0,
            c: 0.0,
            d: 3.0,
            e: 0.0,
            f: 0.0,
        };
        let combined = multiply_transform(translate, scale);
        assert_eq!(matrix_attr(combined), "2 0 0 3 10 20");
    }

    #[test]
    fn wardley_svg_preserves_middle_and_central_baselines() {
        assert_eq!(wardley_text_baseline(TextBaseline::Middle), "middle");
        assert_eq!(wardley_text_baseline(TextBaseline::Central), "central");
    }
}
