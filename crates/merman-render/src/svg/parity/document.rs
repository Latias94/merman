//! SVG encoder for the renderer-neutral [`RenderDocument`].
//!
//! This module deliberately knows nothing about a Mermaid family model.  Family adapters own
//! geometry and resolved style; this encoder only turns the validated public command stream into
//! SVG.  The private sidecar is used for the root's family/a11y role, never as a second geometry
//! or paint source.

use super::root_svg;
use super::util::{escape_attr_into, escape_xml_into, fmt};
use super::{SvgDebugOptions, SvgRenderOptions, sanitize_svg_id};
use crate::drawing_list::{RenderDocument, SvgStructureBody};
use crate::environment::RenderSession;
use crate::family::RenderFamilyKind;
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

/// Serializes one validated canonical document to SVG.
pub(crate) fn render_document_svg(
    document: &RenderDocument,
    options: &SvgRenderOptions,
    debug: &SvgDebugOptions,
    effective_config: &Value,
    session: &RenderSession,
) -> Result<String> {
    let svg =
        DocumentSvgEncoder::new(document, options, debug, effective_config, session)?.render()?;
    super::apply_theme_css(svg, effective_config, session)
}

struct DocumentSvgEncoder<'a> {
    document: &'a DrawingListDocument,
    svg_body: &'a SvgStructureBody,
    family: RenderFamilyKind,
    diagram_type: &'static str,
    options: &'a SvgRenderOptions,
    debug: &'a SvgDebugOptions,
    effective_config: &'a Value,
    session: &'a RenderSession,
    diagram_id: String,
    resources: BTreeMap<String, &'a DrawingResource>,
    resource_svg_ids: BTreeMap<String, String>,
    semantics: BTreeMap<String, &'a SemanticAnnotation>,
    semantic_svg_ids: BTreeMap<String, String>,
    fallbacks: BTreeMap<String, &'a merman_display_list::RasterFallback>,
    state: GraphicsState,
    saves: Vec<SavePoint>,
    groups: Vec<GroupKind>,
    semantic_text_counts: BTreeMap<String, usize>,
    output: String,
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
    Semantic { linked: bool, semantic_id: String },
    Layer,
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
        document
            .public
            .validate()
            .map_err(Error::DrawingListContract)?;

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
                scoped_id("resource", index, raw_id.as_str()),
            );
        }

        let mut semantics = BTreeMap::new();
        let mut semantic_svg_ids = BTreeMap::new();
        for (index, semantic) in document.public.semantics.iter().enumerate() {
            let raw_id = semantic.id.clone();
            semantics.insert(raw_id.clone(), semantic);
            semantic_svg_ids.insert(raw_id.clone(), scoped_id("semantic", index, &raw_id));
        }

        let fallbacks = document
            .public
            .fallbacks
            .iter()
            .map(|fallback| (fallback.id.clone(), fallback))
            .collect();

        Ok(Self {
            document: &document.public,
            svg_body: &document.svg.body,
            family,
            diagram_type: document.svg.diagram_type(),
            options,
            debug,
            effective_config,
            session,
            diagram_id,
            resources,
            resource_svg_ids,
            semantics,
            semantic_svg_ids,
            fallbacks,
            state: GraphicsState::default(),
            saves: Vec::new(),
            groups: Vec::new(),
            semantic_text_counts: BTreeMap::new(),
            output: String::new(),
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

        let title = self
            .document_semantic()
            .and_then(|semantic| semantic.title.clone());
        let description = self
            .document_semantic()
            .and_then(|semantic| semantic.description.clone());
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
        if matches!(self.svg_body, SvgStructureBody::Journey(_)) {
            chrome.extra_attrs = &journey_extra_attrs;
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
            chrome.dom.responsive_height_placement =
                root_svg::RootResponsiveHeightPlacement::AfterExtraAttrs;
        }
        if matches!(self.svg_body, SvgStructureBody::Kanban(_)) {
            chrome.dom.fixed_height_placement = root_svg::SvgRootFixedHeightPlacement::AfterXmlns;
            chrome.dom.fixed_style_placement = root_svg::RootStylePlacement::Tail;
        }
        if matches!(self.svg_body, SvgStructureBody::Sankey(_)) {
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
            SvgStructureBody::Error(_) | SvgStructureBody::Packet(_)
        ) {
            chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
        }
        if matches!(self.svg_body, SvgStructureBody::XyChart(_)) {
            chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
        }
        let root_document = root_context.write_open(&mut self.output, root_spec, chrome)?;

        self.write_family_style()?;
        self.write_defs()?;
        self.write_accessibility_metadata(
            title.as_deref(),
            description.as_deref(),
            title_id.as_deref(),
            description_id.as_deref(),
        );
        for command in &self.document.commands {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_command(command)?;
        }

        if !self.saves.is_empty() || !self.groups.is_empty() {
            return Err(invalid(
                "canonical SVG command stream ended with unbalanced state",
            ));
        }
        self.output.push_str("</svg>");
        root_document
            .complete(self.output)?
            .into_string_for(self.family)
    }

    fn root_spec(
        &self,
        viewport_bounds: root_svg::DiagramBounds,
        padded_bounds: root_svg::DiagramBounds,
    ) -> Result<root_svg::RootViewportSpec> {
        match self.svg_body {
            SvgStructureBody::Info(_) => Ok(
                root_svg::RootViewportSpec::responsive_without_view_box(400.0),
            ),
            SvgStructureBody::Error(body) => {
                if !body.max_width_px.is_finite() || body.max_width_px <= 0.0 {
                    return Err(invalid("canonical error SVG max-width is invalid"));
                }
                Ok(root_svg::RootViewportSpec::responsive(viewport_bounds)
                    .with_max_width(root_svg::RootMaxWidth::SvgNumber(body.max_width_px))
                    .without_background())
            }
            SvgStructureBody::Packet(_) => Ok(root_svg::RootViewportSpec::responsive(
                viewport_bounds,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::QuadrantChart(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            SvgStructureBody::Pie(body) => {
                let max_width = body.max_width_px.map_or_else(
                    || root_svg::RootMaxWidth::CssSixSignificant(viewport_bounds.width),
                    root_svg::RootMaxWidth::SvgNumber,
                );
                Ok(
                    root_svg::RootViewportSpec::mermaid(viewport_bounds, body.use_max_width)
                        .with_max_width(max_width),
                )
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
            SvgStructureBody::Kanban(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(
                viewport_bounds.width,
            ))),
            SvgStructureBody::Sankey(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )
            .with_max_width(root_svg::RootMaxWidth::SvgNumber(viewport_bounds.width))),
            SvgStructureBody::Venn(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
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
            SvgStructureBody::Cynefin(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            SvgStructureBody::TreeView(body) => Ok(root_svg::RootViewportSpec::mermaid(
                viewport_bounds,
                body.use_max_width,
            )),
            SvgStructureBody::Gantt(_body) => {
                Ok(root_svg::RootViewportSpec::responsive(viewport_bounds)
                    .with_max_width(root_svg::RootMaxWidth::SvgNumber(viewport_bounds.width)))
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
            _ => Ok(
                root_svg::RootViewportSpec::responsive(padded_bounds).with_max_width(
                    root_svg::RootMaxWidth::CssSixSignificant(padded_bounds.width),
                ),
            ),
        }
    }

    fn write_family_style(&mut self) -> Result<()> {
        let css = match self.svg_body {
            SvgStructureBody::Info(_) => Some((
                false,
                super::info_css_with_config(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::Error(_) => Some((
                true,
                super::info_css_with_config(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::Packet(_) => Some((
                false,
                super::packet::packet_css(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::QuadrantChart(_) => Some((
                false,
                super::info_css_with_config(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::Pie(_) => {
                let mut css = String::new();
                super::PieCss::new(self.effective_config)
                    .write_for_canonical_id(&mut css, self.diagram_id.as_str())
                    .map_err(|_| invalid("failed to write Pie stylesheet"))?;
                Some((false, css))
            }
            SvgStructureBody::Timeline(_) => Some((
                false,
                super::timeline::canonical_timeline_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            SvgStructureBody::Journey(_) => Some((
                false,
                super::journey::canonical_journey_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                ),
            )),
            SvgStructureBody::Kanban(_) => Some((
                false,
                super::kanban::canonical_kanban_css(
                    self.diagram_id.as_str(),
                    self.effective_config,
                )?,
            )),
            SvgStructureBody::Sankey(_) => Some((
                false,
                super::sankey_css(self.diagram_id.as_str(), self.effective_config),
            )),
            SvgStructureBody::Venn(_) => Some((
                false,
                super::venn::canonical_venn_css(self.diagram_id.as_str(), self.effective_config)
                    .map_err(|_| invalid("failed to write Venn stylesheet"))?,
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
            SvgStructureBody::Cynefin(_) => Some((
                false,
                super::cynefin::canonical_cynefin_css(
                    self.diagram_id.as_str(),
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
            SvgStructureBody::Gantt(_) => Some((
                false,
                super::gantt::canonical_gantt_css(self.diagram_id.as_str(), self.effective_config),
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
            _ => None,
        };
        let Some((xhtml_namespace, css)) = css else {
            return Ok(());
        };
        if xhtml_namespace {
            self.output
                .push_str(r#"<style xmlns="http://www.w3.org/1999/xhtml">"#);
        } else {
            self.output.push_str("<style>");
        }
        self.session
            .work_meter()
            .preflight_svg_byte_count(
                css.len(),
                crate::resources::ResourceLimitPhase::SvgOutput,
                OperationPhase::Emit,
            )
            .map_err(Error::from)?;
        self.output.push_str(&css);
        self.output.push_str("</style>");
        if matches!(
            self.svg_body,
            SvgStructureBody::Packet(_)
                | SvgStructureBody::QuadrantChart(_)
                | SvgStructureBody::Pie(_)
                | SvgStructureBody::Timeline(_)
                | SvgStructureBody::Journey(_)
                | SvgStructureBody::Kanban(_)
                | SvgStructureBody::Sankey(_)
                | SvgStructureBody::Venn(_)
                | SvgStructureBody::Railroad(_)
                | SvgStructureBody::EventModeling(_)
                | SvgStructureBody::Ishikawa(_)
                | SvgStructureBody::Cynefin(_)
                | SvgStructureBody::TreeView(_)
                | SvgStructureBody::Gantt(_)
                | SvgStructureBody::GitGraph(_)
                | SvgStructureBody::XyChart(_)
        ) {
            self.output.push_str("<g/>");
        }
        Ok(())
    }

    fn root_class(&self) -> Option<&'static str> {
        match self.svg_body {
            SvgStructureBody::Info(_) | SvgStructureBody::Error(_) => Some(self.family.as_str()),
            SvgStructureBody::Railroad(_) => Some("railroad-diagram"),
            SvgStructureBody::GitGraph(_) => Some("gitGraph"),
            _ => None,
        }
    }

    fn accessibility_id(&self, suffix: &str) -> String {
        match self.svg_body {
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
            | SvgStructureBody::XyChart(_) => {
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
    ) {
        if let (Some(title), Some(id)) = (title, title_id) {
            self.output.push_str("<title id=\"");
            escape_attr_into(&mut self.output, id);
            self.output.push_str("\">");
            escape_xml_into(&mut self.output, title);
            self.output.push_str("</title>");
        }
        if let (Some(description), Some(id)) = (description, description_id) {
            self.output.push_str("<desc id=\"");
            escape_attr_into(&mut self.output, id);
            self.output.push_str("\">");
            escape_xml_into(&mut self.output, description);
            self.output.push_str("</desc>");
        }
    }

    fn write_defs(&mut self) -> Result<()> {
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
        self.output.push_str("<defs>");
        let resources = self
            .resources
            .iter()
            .map(|(raw_id, resource)| (raw_id.clone(), *resource))
            .collect::<Vec<_>>();
        for (raw_id, resource) in resources {
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
            .filter_map(|(raw_id, resource)| match resource {
                DrawingResource::Path(path) if clip_paths.contains(raw_id) => {
                    Some((raw_id.clone(), path.segments.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (raw_id, segments) in paths {
            let clip_id = self.clip_svg_id(raw_id.as_str())?;
            self.output.push_str("<clipPath id=\"");
            escape_attr_into(&mut self.output, clip_id.as_str());
            self.output.push_str("\" clipPathUnits=\"userSpaceOnUse\">");
            self.output.push_str("<path d=\"");
            let path_data = path_d(&segments);
            escape_attr_into(&mut self.output, path_data.as_str());
            self.output.push_str("\"/>");
            self.output.push_str("</clipPath>");
        }
        self.output.push_str("</defs>");
        Ok(())
    }

    fn write_linear_gradient(
        &mut self,
        id: &str,
        gradient: &merman_display_list::LinearGradientResource,
    ) -> Result<()> {
        write!(
            self.output,
            "<linearGradient id=\"{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" gradientTransform=\"{}\" spreadMethod=\"{}\">",
            escaped_attr(id),
            fmt(gradient.start.x),
            fmt(gradient.start.y),
            fmt(gradient.end.x),
            fmt(gradient.end.y),
            matrix_attr(gradient.transform),
            spread_method(gradient.spread),
        )
        .map_err(|_| invalid("failed to write linear gradient"))?;
        self.write_stops(&gradient.stops);
        self.output.push_str("</linearGradient>");
        Ok(())
    }

    fn write_radial_gradient(
        &mut self,
        id: &str,
        gradient: &merman_display_list::RadialGradientResource,
    ) -> Result<()> {
        write!(
            self.output,
            "<radialGradient id=\"{}\" cx=\"{}\" cy=\"{}\" fx=\"{}\" fy=\"{}\" r=\"{}\" gradientTransform=\"{}\" spreadMethod=\"{}\">",
            escaped_attr(id),
            fmt(gradient.center.x),
            fmt(gradient.center.y),
            fmt(gradient.focal.x),
            fmt(gradient.focal.y),
            fmt(gradient.radius),
            matrix_attr(gradient.transform),
            spread_method(gradient.spread),
        )
        .map_err(|_| invalid("failed to write radial gradient"))?;
        self.write_stops(&gradient.stops);
        self.output.push_str("</radialGradient>");
        Ok(())
    }

    fn write_stops(&mut self, stops: &[merman_display_list::GradientStop]) {
        for stop in stops {
            let color = color_css(stop.color);
            write!(
                self.output,
                "<stop offset=\"{}\" stop-color=\"{}\"{} />",
                fmt(stop.offset),
                color,
                opacity_attr(stop.color.alpha),
            )
            .expect("writing to String cannot fail");
        }
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
            "<pattern id=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" patternUnits=\"userSpaceOnUse\" patternContentUnits=\"userSpaceOnUse\" patternTransform=\"{}\">",
            escaped_attr(id),
            fmt(pattern.tile.x),
            fmt(pattern.tile.y),
            fmt(width),
            fmt(height),
            matrix_attr(pattern.transform),
        )
        .map_err(|_| invalid("failed to write pattern"))?;
        write!(
            self.output,
            "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\"/>",
            escaped_attr(image_uri.as_str()),
            fmt(pattern.tile.x),
            fmt(pattern.tile.y),
            fmt(f64::from(pixel_width)),
            fmt(f64::from(pixel_height)),
        )
        .map_err(|_| invalid("failed to write pattern image"))?;
        self.output.push_str("</pattern>");
        Ok(())
    }

    fn write_font(&mut self, id: &str, font: &merman_display_list::FontResource) -> Result<()> {
        let media_type = safe_media_type(&font.font.media_type);
        let encoded = STANDARD.encode(&font.font.data);
        write!(
            self.output,
            "<style>@font-face{{font-family:\"{}\";src:url(\"data:{};base64,{}\");}}</style>",
            escaped_css_string(id),
            escaped_css_string(media_type.as_str()),
            encoded,
        )
        .map_err(|_| invalid("failed to write font resource"))?;
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
                Some(GroupKind::Clip) => self.output.push_str("</g>"),
                Some(GroupKind::Semantic { .. }) | Some(GroupKind::Layer) => {
                    return Err(invalid(
                        "SVG restore crossed an open semantic or layer group",
                    ));
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
        write!(
            self.output,
            "<g class=\"merman-layer\" data-bounds=\"{},{},{},{}\" opacity=\"{}\"",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(opacity),
        )
        .map_err(|_| invalid("failed to write SVG layer"))?;
        write_blend_style(&mut self.output, blend_mode);
        self.output.push('>');
        self.groups.push(GroupKind::Layer);
        Ok(())
    }

    fn end_layer(&mut self) -> Result<()> {
        match self.groups.pop() {
            Some(GroupKind::Layer) => {
                self.output.push_str("</g>");
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
            "<g clip-path=\"url(#{})\" data-fill-rule=\"{}\">",
            escaped_attr(clip_id.as_str()),
            fill_rule_name(fill_rule),
        )
        .map_err(|_| invalid("failed to write SVG clip group"))?;
        self.groups.push(GroupKind::Clip);
        Ok(())
    }

    fn begin_semantic_group(&mut self, semantic_id: &str) -> Result<()> {
        let semantic = self.semantics.get(semantic_id).ok_or_else(|| {
            invalid(format!(
                "SVG semantic group references unknown id {semantic_id}"
            ))
        })?;
        let svg_id = self.semantic_svg_id(semantic_id)?;
        let role_class = semantic_role_class(semantic.role);
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
        let kanban_ticket_href = match self.svg_body {
            SvgStructureBody::Kanban(body) if semantic_id.starts_with("kanban.item.") => {
                body.ticket_links.get(semantic_id).cloned()
            }
            _ => None,
        };
        let kanban_ticket_anchor = kanban_ticket_href.is_some();
        let linked = link.is_some() || kanban_ticket_anchor;
        if kanban_ticket_anchor {
            self.output.push_str("<a class=\"kanban-ticket-link\"");
            if let Some(href) = kanban_ticket_href.flatten() {
                self.output.push_str(" xlink:href=\"");
                // KanbanPreparedTicketLink already owns the serialized SVG attribute value.
                self.output.push_str(href.as_str());
                self.output.push('"');
            }
            if security == MermaidNavigationSecurity::Loose {
                self.output.push_str(" target=\"_blank\"");
            }
            self.output.push('>');
        } else if let Some(link) = link {
            self.output.push_str("<a href=\"");
            escape_attr_into(&mut self.output, link.as_str());
            self.output.push_str("\">");
        }
        write!(self.output, "<g id=\"{}\"", escaped_attr(svg_id.as_str()))
            .map_err(|_| invalid("failed to write semantic group"))?;
        if let SvgStructureBody::Kanban(body) = self.svg_body
            && semantic_id.starts_with("kanban.section.")
        {
            // Preserve Mermaid's stable section attribute ordering for DOM consumers.
            self.output.push_str(" data-look=\"");
            escape_attr_into(&mut self.output, body.look.as_str());
            self.output.push('"');
        }
        let semantic_extra_class = self.semantic_extra_class(semantic_id).map(str::to_owned);
        if matches!(self.svg_body, SvgStructureBody::Timeline(_))
            && let Some(class) = semantic_extra_class.as_deref()
        {
            // Timeline's current Mermaid DOM uses the family class as the complete group class;
            // retain that stable selector while the semantic identity remains explicit in the
            // data attribute and ARIA role.
            write!(
                self.output,
                " role=\"group\" data-merman-semantic-id=\"{}\" class=\"{}\"",
                escaped_attr(semantic.id.as_str()),
                class,
            )
            .map_err(|_| invalid("failed to write Timeline semantic group"))?;
        } else {
            write!(
                self.output,
                " class=\"merman-semantic {}{}\" role=\"group\" data-merman-semantic-id=\"{}\"",
                role_class,
                semantic_extra_class
                    .as_deref()
                    .map_or_else(String::new, |class| format!(" {class}")),
                escaped_attr(semantic.id.as_str()),
            )
            .map_err(|_| invalid("failed to write semantic group"))?;
        }
        if let SvgStructureBody::Venn(body) = self.svg_body
            && let Some(sets) = body.semantic_data_sets.get(semantic_id)
        {
            self.output.push_str(" data-venn-sets=\"");
            escape_attr_into(&mut self.output, sets);
            self.output.push('"');
        }
        if !visible {
            self.output.push_str(" display=\"none\"");
        }
        self.output.push('>');
        if let Some(title) = semantic.title.as_deref() {
            self.output.push_str("<title>");
            escape_xml_into(&mut self.output, title);
            self.output.push_str("</title>");
        }
        if let Some(description) = semantic.description.as_deref() {
            self.output.push_str("<desc>");
            escape_xml_into(&mut self.output, description);
            self.output.push_str("</desc>");
        }
        self.groups.push(GroupKind::Semantic {
            linked,
            semantic_id: semantic_id.to_owned(),
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
            _ => None,
        }
    }

    fn end_semantic_group(&mut self) -> Result<()> {
        match self.groups.pop() {
            Some(GroupKind::Semantic { linked, .. }) => {
                self.output.push_str("</g>");
                if linked {
                    self.output.push_str("</a>");
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
                return self.emit_rect(path_id, bounds, style);
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
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
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
            let raw_id = path_id.as_str();
            let path = self.path_resource(path_id)?;
            if raw_id == "gantt.background"
                || raw_id.starts_with("gantt.exclude.")
                || raw_id.starts_with("gantt.row.")
                || raw_id.starts_with("gantt.task.") && raw_id.ends_with(".bar")
            {
                if let Some((bounds, radius)) = rounded_rectangle_from_path(path) {
                    if radius > 0.0 {
                        return self.emit_rounded_rect(path_id, bounds, radius, style);
                    }
                    return self.emit_rect(path_id, bounds, style);
                }
                if let Some(bounds) = rectangle_from_path(path) {
                    return self.emit_rect(path_id, bounds, style);
                }
            }
            if (raw_id == "gantt.today.line" || raw_id.contains(".tick."))
                && let Some((start, end)) = line_from_path(path)
            {
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
        let path_data = {
            let path = self.path_resource(path_id)?;
            path_d(&path.segments)
        };
        self.output.push_str("<path d=\"");
        escape_attr_into(&mut self.output, path_data.as_str());
        self.output.push_str("\"");
        self.write_gantt_dom_id(path_id.as_str());
        self.write_journey_dom_id(path_id.as_str());
        self.write_sidecar_dom_id(path_id.as_str());
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"");
            self.output.push_str(class.as_ref());
            self.output.push_str("\"");
        }
        self.write_path_style(style)?;
        self.write_state_attrs();
        self.output.push_str(" data-merman-resource=\"");
        escape_attr_into(&mut self.output, path_id.as_str());
        self.output.push_str("\"/>");
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
            .push_str("<g class=\"treeView-node-icon\" transform=\"translate(");
        write!(self.output, "{},{}\">", fmt(transform.e), fmt(transform.f))
            .map_err(|_| invalid("failed to write TreeView icon transform"))?;
        if self.state.opacity != 1.0 || self.state.blend_mode != BlendMode::Normal {
            // The icon group is emitted inside the current state, so preserve state attributes on
            // the nested SVG rather than changing the path's resolved geometry.
            self.output.pop();
            self.output.push(' ');
            if self.state.opacity != 1.0 {
                write!(self.output, "opacity=\"{}\"", fmt(self.state.opacity))
                    .map_err(|_| invalid("failed to write TreeView icon opacity"))?;
            }
            write_blend_style(&mut self.output, self.state.blend_mode);
            self.output.push('>');
        }
        write!(
            self.output,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\"><path d=\"{}\"",
            fmt(crate::tree_view::TREE_VIEW_ICON_SIZE),
            fmt(crate::tree_view::TREE_VIEW_ICON_SIZE),
            escaped_attr(path_data.as_str()),
        )
        .map_err(|_| invalid("failed to write TreeView icon SVG"))?;
        self.write_path_style(style)?;
        self.output.push_str(" data-merman-resource=\"");
        escape_attr_into(&mut self.output, path_id.as_str());
        self.output.push_str("\"/>");
        self.output.push_str("</svg></g>");
        Ok(())
    }

    fn emit_path_as_standard(
        &mut self,
        path_id: &ResourceId,
        path: &PathResource,
        style: &PathStyle,
    ) -> Result<()> {
        let path_data = path_d(&path.segments);
        self.output.push_str("<path d=\"");
        escape_attr_into(&mut self.output, path_data.as_str());
        self.output.push_str("\"");
        self.write_gantt_dom_id(path_id.as_str());
        self.write_journey_dom_id(path_id.as_str());
        self.write_sidecar_dom_id(path_id.as_str());
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"");
            self.output.push_str(class.as_ref());
            self.output.push_str("\"");
        }
        self.write_path_style(style)?;
        self.write_state_attrs();
        self.output.push_str(" data-merman-resource=\"");
        escape_attr_into(&mut self.output, path_id.as_str());
        self.output.push_str("\"/>");
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
        )
        .map_err(|_| invalid("failed to write XYChart background"))?;
        self.write_paint("fill", fill)?;
        self.output.push_str("/>");
        Ok(())
    }

    fn emit_rect(&mut self, path_id: &ResourceId, bounds: Rect, style: &PathStyle) -> Result<()> {
        self.output.push_str("<rect x=\"");
        write!(
            self.output,
            "{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
        )
        .map_err(|_| invalid("failed to write SVG rectangle"))?;
        self.write_gantt_dom_id(path_id.as_str());
        self.write_journey_dom_id(path_id.as_str());
        self.write_sidecar_dom_id(path_id.as_str());
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"");
            self.output.push_str(class.as_ref());
            self.output.push_str("\"");
        }
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs();
        self.output.push_str(" data-merman-resource=\"");
        escape_attr_into(&mut self.output, path_id.as_str());
        self.output.push_str("\"/>");
        Ok(())
    }

    fn emit_rounded_rect(
        &mut self,
        path_id: &ResourceId,
        bounds: Rect,
        radius: f64,
        style: &PathStyle,
    ) -> Result<()> {
        self.output.push_str("<rect x=\"");
        write!(
            self.output,
            "{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\"",
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(radius),
            fmt(radius),
        )
        .map_err(|_| invalid("failed to write rounded SVG rectangle"))?;
        self.write_gantt_dom_id(path_id.as_str());
        self.write_journey_dom_id(path_id.as_str());
        self.write_sidecar_dom_id(path_id.as_str());
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"");
            self.output.push_str(class.as_ref());
            self.output.push('"');
        }
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs();
        self.output.push_str(" data-merman-resource=\"");
        escape_attr_into(&mut self.output, path_id.as_str());
        self.output.push_str("\"/>");
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
        )
        .map_err(|_| invalid("failed to write SVG circle"))?;
        self.write_sidecar_dom_id(path_id.as_str());
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"");
            self.output.push_str(class.as_ref());
            self.output.push('"');
        }
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs();
        self.output.push_str(" data-merman-resource=\"");
        escape_attr_into(&mut self.output, path_id.as_str());
        self.output.push_str("\"/>");
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
        )
        .map_err(|_| invalid("failed to write SVG line"))?;
        self.write_gantt_dom_id(path_id.as_str());
        self.write_journey_dom_id(path_id.as_str());
        self.write_sidecar_dom_id(path_id.as_str());
        if let Some(class) = self.path_class(path_id) {
            self.output.push_str(" class=\"");
            self.output.push_str(class.as_ref());
            self.output.push('"');
        }
        self.write_fill_stroke_style(style)?;
        self.write_state_attrs();
        self.output.push_str(" data-merman-resource=\"");
        escape_attr_into(&mut self.output, path_id.as_str());
        self.output.push_str("\"/>");
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
        self.output.push_str("<text");
        let semantic_id = self.current_semantic_id().map(str::to_owned);
        if let Some(semantic_id) = semantic_id.as_deref() {
            self.write_gantt_dom_id(semantic_id);
            self.write_sidecar_dom_id(semantic_id);
        }
        let text_index = self.record_text_index();
        if let Some(class) = self.text_class(run, text_index) {
            let class = class.into_owned();
            self.output.push_str(" class=\"");
            self.output.push_str(class.as_str());
            self.output.push_str("\"");
        }
        let font_size = if matches!(self.svg_body, SvgStructureBody::Error(_)) {
            format!("{}px", fmt(run.style.font_size))
        } else {
            fmt(run.style.font_size).to_string()
        };
        let baseline = if matches!(self.svg_body, SvgStructureBody::XyChart(_)) {
            xychart_text_baseline(run.baseline)
        } else {
            text_baseline(run.baseline)
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
            )
            .map_err(|_| invalid("failed to write GitGraph branch label style"))?;
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
            )
            .map_err(|_| invalid("failed to write text style"))?;
        }
        let families = self.font_families(&run.style.font);
        self.output.push_str(" font-family=\"");
        escape_attr_into(&mut self.output, families.as_str());
        self.output.push_str("\"");
        write!(
            self.output,
            " font-weight=\"{}\" font-style=\"{}\" data-merman-bounds=\"{},{},{},{}\" data-merman-text-obligation=\"host_text\"",
            run.style.font.weight,
            font_style(run.style.font.style),
            fmt(run.bounds.x),
            fmt(run.bounds.y),
            fmt(run.bounds.width),
            fmt(run.bounds.height),
        )
        .map_err(|_| invalid("failed to write text metadata"))?;
        if let Some(language) = run.language.as_deref() {
            self.output.push_str(" xml:lang=\"");
            escape_attr_into(&mut self.output, language);
            self.output.push_str("\"");
        }
        if let SvgStructureBody::Gantt(body) = self.svg_body
            && self
                .effective_config
                .get("securityLevel")
                .and_then(Value::as_str)
                == Some("loose")
            && self
                .current_semantic_id()
                .is_some_and(|id| id.starts_with("gantt.task."))
        {
            write!(self.output, " text-height=\"{}\"", fmt(body.bar_height))
                .map_err(|_| invalid("failed to write Gantt task text height"))?;
        }
        self.write_paint("fill", &run.style.fill)?;
        self.write_state_attrs();
        self.output.push('>');

        let mut lines = run.text.split('\n');
        if gitgraph_branch_label {
            if let Some(first) = lines.next() {
                self.output
                    .push_str(r#"<tspan xml:space="preserve" dy="1em" x="0" class="row">"#);
                escape_xml_into(&mut self.output, first);
                self.output.push_str("</tspan>");
            }
            for line in lines {
                write!(
                    self.output,
                    "<tspan x=\"0\" dy=\"{}\" class=\"row\">",
                    fmt(run.style.line_height),
                )
                .map_err(|_| invalid("failed to write GitGraph multiline text"))?;
                escape_xml_into(&mut self.output, line);
                self.output.push_str("</tspan>");
            }
        } else {
            if let Some(first) = lines.next() {
                escape_xml_into(&mut self.output, first);
            }
            for line in lines {
                write!(
                    self.output,
                    "<tspan x=\"{}\" dy=\"{}\">",
                    fmt(run.origin.x),
                    fmt(run.style.line_height),
                )
                .map_err(|_| invalid("failed to write multiline text"))?;
                escape_xml_into(&mut self.output, line);
                self.output.push_str("</tspan>");
            }
        }
        self.output.push_str("</text>");
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

    fn write_gantt_dom_id(&mut self, key: &str) {
        let Some(raw_id) = (match self.svg_body {
            SvgStructureBody::Gantt(body) => body.dom_ids.get(key),
            _ => None,
        })
        .cloned() else {
            return;
        };
        self.output.push_str(" id=\"");
        escape_attr_into(&mut self.output, self.diagram_id.as_str());
        self.output.push('-');
        escape_attr_into(&mut self.output, raw_id.as_str());
        self.output.push('"');
    }

    fn write_journey_dom_id(&mut self, key: &str) {
        let Some(raw_id) = (match self.svg_body {
            SvgStructureBody::Journey(body) => body.dom_ids.get(key),
            _ => None,
        })
        .cloned() else {
            return;
        };
        self.output.push_str(" id=\"");
        escape_attr_into(&mut self.output, self.diagram_id.as_str());
        self.output.push('-');
        escape_attr_into(&mut self.output, raw_id.as_str());
        self.output.push('"');
    }

    fn write_sidecar_dom_id(&mut self, key: &str) {
        let raw_id = match self.svg_body {
            SvgStructureBody::GitGraph(body) => body.dom_ids.get(key),
            _ => None,
        };
        let Some(raw_id) = raw_id else {
            return;
        };
        self.output.push_str(" id=\"");
        escape_attr_into(&mut self.output, self.diagram_id.as_str());
        self.output.push('-');
        escape_attr_into(&mut self.output, raw_id.as_str());
        self.output.push('"');
    }

    fn text_class(&self, _run: &TextRun, text_index: Option<usize>) -> Option<Cow<'_, str>> {
        match self.svg_body {
            SvgStructureBody::Error(_) => Some(Cow::Borrowed("error-text")),
            SvgStructureBody::Info(_) => Some(Cow::Borrowed("version")),
            SvgStructureBody::Packet(_) => match self.current_semantic_id() {
                Some("packet.title") => Some(Cow::Borrowed("packetTitle")),
                Some(id) if id.starts_with("packet.block.") => match text_index {
                    Some(0) => Some(Cow::Borrowed("packetLabel")),
                    Some(1) => Some(Cow::Borrowed("packetByte start")),
                    Some(2) => Some(Cow::Borrowed("packetByte end")),
                    _ => None,
                },
                _ => None,
            },
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
            escaped_attr(uri.as_str()),
            fmt(bounds.x),
            fmt(bounds.y),
            fmt(bounds.width),
            fmt(bounds.height),
            fmt(self.state.opacity * opacity),
        )
        .map_err(|_| invalid("failed to write SVG image"))?;
        if let Some(fallback) = fallback {
            write!(
                self.output,
                " data-merman-fallback-reason=\"{}\" data-merman-fallback-family=\"{}\" data-merman-fallback-effect=\"{}\"",
                fallback_reason(fallback.reason),
                escaped_attr(fallback.source.family.as_str()),
                escaped_attr(fallback.source.effect.as_str()),
            )
            .map_err(|_| invalid("failed to write fallback metadata"))?;
        }
        self.write_transform_and_blend();
        self.output.push_str("/>");
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
        self.output.push_str(" fill-rule=\"");
        self.output.push_str(fill_rule_name(style.fill_rule));
        self.output.push_str("\"");
        self.write_fill_stroke_style(style)
    }

    fn write_fill_stroke_style(&mut self, style: &PathStyle) -> Result<()> {
        if let Some(fill) = style.fill.as_ref() {
            self.write_paint("fill", fill)?;
        } else {
            self.output.push_str(" fill=\"none\"");
        }
        if let Some(stroke) = style.stroke.as_ref() {
            self.write_paint("stroke", &stroke.paint)?;
            write!(
                self.output,
                " stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\" stroke-miterlimit=\"{}\"",
                fmt(stroke.width),
                line_cap(stroke.line_cap),
                line_join(stroke.line_join),
                fmt(stroke.miter_limit),
            )
            .map_err(|_| invalid("failed to write stroke style"))?;
            if !stroke.dash_array.is_empty() {
                self.output.push_str(" stroke-dasharray=\"");
                for (index, value) in stroke.dash_array.iter().enumerate() {
                    if index > 0 {
                        self.output.push(',');
                    }
                    write!(self.output, "{}", fmt(*value))
                        .map_err(|_| invalid("failed to write stroke dash"))?;
                }
                self.output.push_str("\"");
            }
            if stroke.dash_offset != 0.0 {
                write!(
                    self.output,
                    " stroke-dashoffset=\"{}\"",
                    fmt(stroke.dash_offset)
                )
                .map_err(|_| invalid("failed to write stroke dash offset"))?;
            }
        } else {
            self.output.push_str(" stroke=\"none\"");
        }
        Ok(())
    }

    fn write_paint(&mut self, attribute: &str, paint: &Paint) -> Result<()> {
        self.output.push(' ');
        self.output.push_str(attribute);
        self.output.push_str("=\"");
        match paint {
            Paint::Solid { color } => {
                self.output.push_str(color_css(*color).as_str());
                self.output.push('"');
                if color.alpha != u8::MAX {
                    write!(
                        self.output,
                        " {}-opacity=\"{}\"",
                        attribute,
                        fmt(f64::from(color.alpha) / 255.0),
                    )
                    .map_err(|_| invalid("failed to write paint opacity"))?;
                }
            }
            Paint::Resource { id } => {
                let svg_id = self.svg_resource_id(id.as_str())?;
                write!(self.output, "url(#{})\"", escaped_attr(svg_id.as_str()))
                    .map_err(|_| invalid("failed to write resource paint"))?;
            }
        }
        Ok(())
    }

    fn write_state_attrs(&mut self) {
        self.write_transform_and_blend();
        if self.state.opacity != 1.0 {
            write!(self.output, " opacity=\"{}\"", fmt(self.state.opacity))
                .expect("writing to String cannot fail");
        }
    }

    fn write_transform_and_blend(&mut self) {
        let family_text = self
            .current_semantic_id()
            .is_some_and(|id| match self.svg_body {
                SvgStructureBody::Pie(_) => id.starts_with("pie."),
                SvgStructureBody::QuadrantChart(_) => id.starts_with("quadrantchart."),
                SvgStructureBody::XyChart(_) => id.starts_with("xychart.text."),
                SvgStructureBody::GitGraph(_) => id.starts_with("gitgraph."),
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
            )
            .expect("writing to String cannot fail");
        } else if self.state.transform != Transform::IDENTITY {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(self.state.transform)
            )
            .expect("writing to String cannot fail");
        }
        write_blend_style(&mut self.output, self.state.blend_mode);
    }

    fn font_families(&self, font: &merman_display_list::FontDescriptor) -> String {
        let mut families = String::new();
        if let Some(resource) = font.resource.as_ref()
            && let Some(svg_id) = self.resource_svg_ids.get(resource.as_str())
        {
            families.push('"');
            families.push_str(svg_id);
            families.push_str("\",");
        }
        for (index, family) in font.families.iter().enumerate() {
            if index > 0 || !families.is_empty() {
                if !families.ends_with(',') {
                    families.push(',');
                }
            }
            families.push_str(family);
        }
        if families.is_empty() {
            families.push_str("sans-serif");
        }
        families
    }

    fn debug_visibility(&self, role: SemanticRole) -> bool {
        match role {
            SemanticRole::Node => self.debug.include_nodes,
            SemanticRole::Edge => self.debug.include_edges,
            SemanticRole::Group => self.debug.include_clusters,
            SemanticRole::Document | SemanticRole::Label => true,
        }
    }

    fn path_resource(&self, id: &ResourceId) -> Result<&PathResource> {
        match self.resources.get(id.as_str()) {
            Some(DrawingResource::Path(path)) => Ok(path),
            Some(_) => Err(invalid(format!("resource {} is not a path", id.as_str()))),
            None => Err(invalid(format!("unknown path resource {}", id.as_str()))),
        }
    }

    fn image_resource(&self, id: &ResourceId) -> Result<&merman_display_list::ImageResource> {
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
    let bounds = Rect::new(x, y, max_x - x, max_y - y);
    (bounds.width > 0.0 && bounds.height > 0.0).then_some(bounds)
}

fn rounded_rectangle_from_path(path: &PathResource) -> Option<(Rect, f64)> {
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
    let radius = *r1x;
    if !radius.is_finite()
        || radius <= 0.0
        || !same(*r1x, *r1y)
        || !same(*r1x, *r2x)
        || !same(*r1x, *r2y)
        || !same(*r1x, *r3x)
        || !same(*r1x, *r3y)
        || !same(*r1x, *r4x)
        || !same(*r1x, *r4y)
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
        || !same(p0.x, left + radius)
        || !same(p1.x, right - radius)
        || !same(p1.y, top)
        || !same(p2.x, right)
        || !same(p2.y, top + radius)
        || !same(p3.x, right)
        || !same(p3.y, bottom - radius)
        || !same(p4.x, right - radius)
        || !same(p4.y, bottom)
        || !same(p5.x, left + radius)
        || !same(p6.x, left)
        || !same(p6.y, bottom - radius)
        || !same(p7.x, left)
        || !same(p7.y, top + radius)
        || !same(p8.x, left + radius)
        || !same(p8.y, top)
    {
        return None;
    }
    let bounds = Rect::new(left, top, right - left, bottom - top);
    (bounds.width > 0.0
        && bounds.height > 0.0
        && radius <= bounds.width / 2.0
        && radius <= bounds.height / 2.0)
        .then_some((bounds, radius))
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

fn scoped_id(prefix: &str, index: usize, raw: &str) -> String {
    let sanitized = sanitize_svg_id(raw);
    let suffix = sanitized.chars().take(48).collect::<String>();
    format!("merman-{prefix}-{index}-{suffix}")
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn escaped_attr(value: &str) -> String {
    let mut output = String::new();
    escape_attr_into(&mut output, value);
    output
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

fn path_d(segments: &[PathSegment]) -> String {
    let mut output = String::new();
    for segment in segments {
        if !output.is_empty() {
            output.push(' ');
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
            PathSegment::Close => write!(output, "Z"),
        }
        .expect("writing to String cannot fail");
    }
    output
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
        TextBaseline::TextBeforeEdge => "text-before-edge",
        TextBaseline::TextAfterEdge => "text-after-edge",
    }
}

fn xychart_text_baseline(baseline: TextBaseline) -> &'static str {
    match baseline {
        TextBaseline::Alphabetic => "auto",
        TextBaseline::Hanging => "hanging",
        TextBaseline::Middle => "middle",
        TextBaseline::TextBeforeEdge => "text-before-edge",
        TextBaseline::Ideographic => "ideographic",
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

fn write_blend_style(output: &mut String, blend_mode: BlendMode) {
    if let Some(value) = blend_css(blend_mode) {
        write!(output, " style=\"mix-blend-mode: {};\"", value)
            .expect("writing to String cannot fail");
    }
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

fn data_uri(media_type: &str, data: &[u8]) -> String {
    format!(
        "data:{};base64,{}",
        safe_media_type(media_type),
        STANDARD.encode(data)
    )
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
    use super::{matrix_attr, multiply_transform, path_d};
    use merman_display_list::{PathSegment, Point, Transform};

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
        ]);
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
}
