use super::*;
use crate::DiagramFamilyId;
use crate::diagram_theme::{
    BlendMode, CanvasPaint, GradientStop, LinearGradient, RadialGradient, RootThemeApplication,
    RootThemeMechanismKey, RootThemePlan, RootThemeReport, ThemeCapability, ThemeLength,
    paint_capabilities,
};
use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
use std::ops::Range;

const DEFERRED_ROOT_ATTRIBUTE_VALUE: &str = "";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DiagramBounds {
    pub(super) min_x: f64,
    pub(super) min_y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

impl DiagramBounds {
    pub(super) fn from_view_box(min_x: f64, min_y: f64, width: f64, height: f64) -> Self {
        Self {
            min_x,
            min_y,
            width,
            height,
        }
    }

    pub(super) fn from_extents(
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        padding: f64,
    ) -> Self {
        let padding = if padding.is_finite() {
            padding.max(0.0)
        } else {
            padding
        };
        Self::from_view_box(
            min_x - padding,
            min_y - padding,
            max_x - min_x + 2.0 * padding,
            max_y - min_y + 2.0 * padding,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ViewBox {
    pub(super) min_x: f64,
    pub(super) min_y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

impl ViewBox {
    pub(super) fn new(min_x: f64, min_y: f64, width: f64, height: f64) -> Self {
        Self {
            min_x,
            min_y,
            width,
            height,
        }
    }

    fn from_bounds(bounds: DiagramBounds, resources: &RenderResourcePolicy) -> Result<Self> {
        let min_x = checked_svg_coordinate(bounds.min_x, "viewBox min-x", resources)?;
        let min_y = checked_svg_coordinate(bounds.min_y, "viewBox min-y", resources)?;
        let width = checked_viewport_dimension(bounds.width, "viewBox width", resources)?;
        let height = checked_viewport_dimension(bounds.height, "viewBox height", resources)?;
        checked_svg_coordinate(min_x + width, "viewBox max-x", resources)?;
        checked_svg_coordinate(min_y + height, "viewBox max-y", resources)?;
        Ok(Self::new(min_x, min_y, width, height))
    }

    pub(super) fn attr(self) -> String {
        format!(
            "{} {} {} {}",
            fmt(self.min_x),
            fmt(self.min_y),
            fmt(self.width),
            fmt(self.height)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RootBackground {
    None,
    White,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum RootMaxWidth {
    ViewBox,
    SvgNumber(f64),
    CssSixSignificant(f64),
    Precision { value: f64, significant_digits: u8 },
}

impl RootMaxWidth {
    fn format(self, view_box: Option<ViewBox>, resources: &RenderResourcePolicy) -> Result<String> {
        let value = match self {
            Self::ViewBox => {
                view_box
                    .ok_or_else(|| Error::InvalidModel {
                        message: "root max-width requested without a viewBox".to_string(),
                    })?
                    .width
            }
            Self::SvgNumber(value)
            | Self::CssSixSignificant(value)
            | Self::Precision { value, .. } => value,
        };
        let value = checked_viewport_dimension(value, "root max-width", resources)?;
        Ok(match self {
            Self::ViewBox | Self::SvgNumber(_) => fmt_string(value),
            Self::CssSixSignificant(_) => format_css_max_width(value),
            Self::Precision {
                significant_digits, ..
            } => format_precision_fixed(value, significant_digits),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum RootSizing {
    Responsive,
    Mermaid {
        use_max_width: bool,
    },
    #[cfg(feature = "layout-cytoscape")]
    MermaidOrIntrinsic {
        use_max_width: bool,
    },
    MermaidWithResponsiveHeight {
        use_max_width: bool,
        height: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RootViewportSpec {
    view_box: Option<DiagramBounds>,
    max_width: RootMaxWidth,
    sizing: RootSizing,
    background: RootBackground,
    fixed_size: Option<(f64, f64)>,
}

impl RootViewportSpec {
    pub(super) fn responsive(bounds: DiagramBounds) -> Self {
        Self {
            view_box: Some(bounds),
            max_width: RootMaxWidth::ViewBox,
            sizing: RootSizing::Responsive,
            background: RootBackground::White,
            fixed_size: None,
        }
    }

    pub(super) fn responsive_without_view_box(max_width: f64) -> Self {
        Self {
            view_box: None,
            max_width: RootMaxWidth::SvgNumber(max_width),
            sizing: RootSizing::Responsive,
            background: RootBackground::White,
            fixed_size: None,
        }
    }

    pub(super) fn mermaid(bounds: DiagramBounds, use_max_width: bool) -> Self {
        Self {
            sizing: RootSizing::Mermaid { use_max_width },
            ..Self::responsive(bounds)
        }
    }

    #[cfg(feature = "layout-cytoscape")]
    pub(super) fn mermaid_or_intrinsic(bounds: DiagramBounds, use_max_width: bool) -> Self {
        Self {
            sizing: RootSizing::MermaidOrIntrinsic { use_max_width },
            ..Self::responsive(bounds)
        }
    }

    pub(super) fn with_max_width(mut self, max_width: RootMaxWidth) -> Self {
        self.max_width = max_width;
        self
    }

    pub(super) fn with_mermaid_responsive_height(
        mut self,
        use_max_width: bool,
        height: f64,
    ) -> Self {
        self.sizing = RootSizing::MermaidWithResponsiveHeight {
            use_max_width,
            height,
        };
        self
    }

    pub(super) fn without_background(mut self) -> Self {
        self.background = RootBackground::None;
        self
    }

    pub(super) fn with_fixed_size(mut self, width: f64, height: f64) -> Self {
        self.fixed_size = Some((width, height));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RootStylePlacement {
    Viewport,
    AfterRoleDescription,
    Tail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RootResponsiveHeightPlacement {
    BeforeExtraAttrs,
    AfterExtraAttrs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RootDomProfile {
    pub(super) style_viewbox_order: SvgRootStyleViewBoxOrder,
    pub(super) fixed_height_placement: SvgRootFixedHeightPlacement,
    pub(super) aria_attr_order: SvgRootAriaAttrOrder,
    pub(super) responsive_style_placement: RootStylePlacement,
    pub(super) fixed_style_placement: RootStylePlacement,
    pub(super) responsive_height_placement: RootResponsiveHeightPlacement,
    pub(super) trailing_newline: bool,
}

impl Default for RootDomProfile {
    fn default() -> Self {
        Self {
            style_viewbox_order: SvgRootStyleViewBoxOrder::StyleThenViewBox,
            fixed_height_placement: SvgRootFixedHeightPlacement::BeforeXmlns,
            aria_attr_order: SvgRootAriaAttrOrder::DescribedbyThenLabelledby,
            responsive_style_placement: RootStylePlacement::Viewport,
            fixed_style_placement: RootStylePlacement::Viewport,
            responsive_height_placement: RootResponsiveHeightPlacement::BeforeExtraAttrs,
            trailing_newline: true,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum RootDiagramId<'a> {
    Projected(SvgDiagramId<'a>),
    Raw(&'a str),
}

impl<'a> RootDiagramId<'a> {
    fn as_str(self) -> &'a str {
        match self {
            Self::Projected(diagram_id) => diagram_id.semantic_str(),
            Self::Raw(diagram_id) => diagram_id,
        }
    }

    fn write_escaped(self, out: &mut impl SvgOutput) {
        match self {
            Self::Projected(diagram_id) => {
                let _ = write!(out, "{diagram_id}");
            }
            Self::Raw(diagram_id) => escape_attr_into(out, diagram_id),
        }
    }
}

impl std::fmt::Debug for RootDiagramId<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("RootDiagramId")
            .field(&self.as_str())
            .finish()
    }
}

impl std::fmt::Display for RootDiagramId<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl PartialEq for RootDiagramId<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for RootDiagramId<'_> {}

impl<'a> From<SvgDiagramId<'a>> for RootDiagramId<'a> {
    fn from(diagram_id: SvgDiagramId<'a>) -> Self {
        Self::Projected(diagram_id)
    }
}

impl<'a> From<&'a str> for RootDiagramId<'a> {
    fn from(diagram_id: &'a str) -> Self {
        Self::Raw(diagram_id)
    }
}

pub(super) struct RootChrome<'a> {
    pub(super) diagram_id: RootDiagramId<'a>,
    pub(super) class: Option<&'a str>,
    pub(super) extra_attrs: &'a [(&'a str, &'a str)],
    pub(super) aria_roledescription: &'a str,
    pub(super) aria_labelledby: Option<&'a str>,
    pub(super) aria_describedby: Option<&'a str>,
    pub(super) after_roledescription_attrs: &'a [(&'a str, &'a str)],
    pub(super) tail_attrs: &'a [(&'a str, &'a str)],
    pub(super) custom_properties: &'a [(&'a str, &'a str)],
    pub(super) dom: RootDomProfile,
}

impl<'a> RootChrome<'a> {
    pub(super) fn new(
        diagram_id: impl Into<RootDiagramId<'a>>,
        aria_roledescription: &'a str,
    ) -> Self {
        Self {
            diagram_id: diagram_id.into(),
            class: None,
            extra_attrs: &[],
            aria_roledescription,
            aria_labelledby: None,
            aria_describedby: None,
            after_roledescription_attrs: &[],
            tail_attrs: &[],
            custom_properties: &[],
            dom: RootDomProfile::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DeferredRootSpec {
    sizing: RootSizing,
    background: RootBackground,
}

impl DeferredRootSpec {
    pub(super) fn responsive() -> Self {
        Self {
            sizing: RootSizing::Responsive,
            background: RootBackground::White,
        }
    }

    #[cfg(feature = "layout-cytoscape")]
    pub(super) fn mermaid_or_intrinsic(use_max_width: bool) -> Self {
        Self {
            sizing: RootSizing::MermaidOrIntrinsic { use_max_width },
            background: RootBackground::White,
        }
    }
}

#[derive(Debug)]
enum RootDocumentState {
    Deferred {
        view_box_range: Range<usize>,
        max_width_range: Option<Range<usize>>,
        responsive: bool,
        background: RootBackground,
        root_open_snapshot: String,
        root_open_end: usize,
    },
    Ready {
        root_open: String,
        plan: RootViewportPlan,
    },
}

#[derive(Debug)]
pub(super) struct RootDocument {
    family: DiagramFamilyId,
    diagram_id: String,
    state: RootDocumentState,
}

/// A complete built-in SVG whose root was emitted and finalized by this module.
///
/// The field is intentionally private: sibling family modules may return this type, but only the
/// Root Viewport protocol can construct or unwrap it.
#[derive(Debug)]
pub(super) struct RootedSvg {
    svg: String,
    family: DiagramFamilyId,
    diagram_id: String,
    root_open_end: usize,
    viewport: RootViewportPlan,
}

#[derive(Debug, Clone)]
pub(super) struct RootViewportContext<'a> {
    family: DiagramFamilyId,
    diagram_id: RootDiagramId<'a>,
    resources: RenderResourcePolicy,
}

impl<'a> RootViewportContext<'a> {
    pub(super) fn new(family: DiagramFamilyId, diagram_id: impl Into<RootDiagramId<'a>>) -> Self {
        Self {
            family,
            diagram_id: diagram_id.into(),
            resources: RenderResourcePolicy::unbounded_for_trusted_input(),
        }
    }

    pub(super) fn with_resource_policy(mut self, resources: RenderResourcePolicy) -> Self {
        self.resources = resources;
        self
    }

    pub(super) fn begin_document(
        &self,
        out: &mut impl SvgOutput,
        spec: DeferredRootSpec,
        chrome: RootChrome<'_>,
    ) -> Result<RootDocument> {
        self.require_empty_document(out)?;
        if chrome.diagram_id != self.diagram_id {
            return Err(Error::InvalidModel {
                message: "deferred root chrome belongs to a different render context".to_string(),
            });
        }
        let responsive = match spec.sizing {
            RootSizing::Responsive
            | RootSizing::Mermaid {
                use_max_width: true,
            } => true,
            #[cfg(feature = "layout-cytoscape")]
            RootSizing::MermaidOrIntrinsic {
                use_max_width: true,
            } => true,
            #[cfg(feature = "layout-cytoscape")]
            RootSizing::MermaidOrIntrinsic {
                use_max_width: false,
            } => false,
            RootSizing::Mermaid {
                use_max_width: false,
            }
            | RootSizing::MermaidWithResponsiveHeight { .. } => {
                return Err(Error::InvalidModel {
                    message: "unsupported deferred root sizing mode".to_string(),
                });
            }
        };
        let fixed_style = if responsive {
            None
        } else {
            with_custom_properties(root_style(None, spec.background), chrome.custom_properties)?
        };
        let deferred_style_suffix = with_custom_properties(
            Some(match spec.background {
                RootBackground::None => "px;".into(),
                RootBackground::White => "px; background-color: white;".into(),
            }),
            chrome.custom_properties,
        )?
        .unwrap_or_default();
        let style_placement = if responsive {
            chrome.dom.responsive_style_placement
        } else {
            chrome.dom.fixed_style_placement
        };
        let tracked_ranges = push_svg_root_open(
            out,
            SvgRootAttrs {
                diagram_id: chrome.diagram_id,
                class: chrome.class,
                width: if responsive {
                    SvgRootWidth::Percent100
                } else {
                    SvgRootWidth::None
                },
                height_attr: None,
                style_attr: if responsive {
                    Some(SvgRootAttributeValue::tracked(
                        "max-width: ",
                        DEFERRED_ROOT_ATTRIBUTE_VALUE,
                        &deferred_style_suffix,
                    ))
                } else {
                    fixed_style.as_deref().map(SvgRootAttributeValue::plain)
                },
                viewbox_attr: Some(SvgRootAttributeValue::tracked(
                    "",
                    DEFERRED_ROOT_ATTRIBUTE_VALUE,
                    "",
                )),
                style_viewbox_order: chrome.dom.style_viewbox_order,
                style_placement,
                responsive_height_placement: chrome.dom.responsive_height_placement,
                extra_attrs: chrome.extra_attrs,
                aria_roledescription: chrome.aria_roledescription,
                aria_labelledby: chrome.aria_labelledby,
                aria_describedby: chrome.aria_describedby,
                after_roledescription_attrs: chrome.after_roledescription_attrs,
                tail_attrs: chrome.tail_attrs,
                fixed_height_placement: chrome.dom.fixed_height_placement,
                trailing_newline: chrome.dom.trailing_newline,
                aria_attr_order: chrome.dom.aria_attr_order,
            },
        );
        out.checkpoint()?;
        let view_box_range = tracked_ranges.view_box.ok_or_else(|| Error::InvalidModel {
            message: "deferred SVG root is missing its viewBox placeholder".to_string(),
        })?;
        let max_width_range = if responsive {
            Some(
                tracked_ranges
                    .max_width
                    .ok_or_else(|| Error::InvalidModel {
                        message: "deferred SVG root is missing its max-width placeholder"
                            .to_string(),
                    })?,
            )
        } else {
            None
        };

        Ok(RootDocument {
            family: self.family,
            diagram_id: self.diagram_id.as_str().to_string(),
            state: RootDocumentState::Deferred {
                view_box_range,
                max_width_range,
                responsive,
                background: spec.background,
                root_open_snapshot: out.as_str().to_string(),
                root_open_end: out.len(),
            },
        })
    }

    pub(super) fn finish_document(
        &self,
        out: &mut impl SvgOutput,
        document: RootDocument,
        spec: RootViewportSpec,
    ) -> Result<RootDocument> {
        if document.family != self.family || document.diagram_id != self.diagram_id.as_str() {
            return Err(Error::InvalidModel {
                message: "deferred root document belongs to a different render context".to_string(),
            });
        }
        let RootDocumentState::Deferred {
            view_box_range,
            max_width_range,
            responsive,
            background,
            root_open_snapshot,
            mut root_open_end,
        } = document.state
        else {
            return Err(Error::InvalidModel {
                message: "root document viewport was already finalized".to_string(),
            });
        };
        if background != spec.background {
            return Err(Error::InvalidModel {
                message: "deferred root document belongs to a different render context".to_string(),
            });
        }
        let plan = self.plan(spec)?;
        if plan.responsive != responsive || plan.height.is_some() {
            return Err(Error::InvalidModel {
                message: "deferred root sizing changed between open and finalize".to_string(),
            });
        }
        let view_box = plan.view_box.ok_or_else(|| Error::InvalidModel {
            message: "deferred root viewport did not resolve a viewBox".to_string(),
        })?;
        if out.as_str().get(..root_open_end) != Some(root_open_snapshot.as_str())
            || out.as_str().get(view_box_range.clone()) != Some(DEFERRED_ROOT_ATTRIBUTE_VALUE)
            || max_width_range.as_ref().is_some_and(|range| {
                out.as_str().get(range.clone()) != Some(DEFERRED_ROOT_ATTRIBUTE_VALUE)
            })
        {
            return Err(Error::InvalidModel {
                message: "deferred root document was mutated before viewport finalize".to_string(),
            });
        }
        let mut replacements = vec![(view_box_range, view_box.attr())];
        match (max_width_range, plan.responsive) {
            (Some(range), true) => replacements.push((range, plan.max_width.clone())),
            (None, false) => {}
            _ => {
                return Err(Error::InvalidModel {
                    message: "deferred root max-width state changed during finalize".to_string(),
                });
            }
        }
        replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, replacement) in replacements {
            root_open_end = root_open_end
                .checked_add(replacement.len())
                .and_then(|end| end.checked_sub(range.len()))
                .ok_or_else(|| Error::InvalidModel {
                    message: "deferred root attribute replacement overflowed".to_string(),
                })?;
            out.replace_range(range, replacement.as_str())?;
        }
        let root_open = out
            .as_str()
            .get(..root_open_end)
            .ok_or_else(|| Error::InvalidModel {
                message: "deferred root document was truncated before viewport finalize"
                    .to_string(),
            })?
            .to_string();
        Ok(RootDocument {
            family: self.family,
            diagram_id: self.diagram_id.as_str().to_string(),
            state: RootDocumentState::Ready { root_open, plan },
        })
    }

    pub(super) fn write_open(
        &self,
        out: &mut impl SvgOutput,
        spec: RootViewportSpec,
        chrome: RootChrome<'_>,
    ) -> Result<RootDocument> {
        if chrome.diagram_id != self.diagram_id {
            return Err(Error::InvalidModel {
                message: format!(
                    "root viewport diagram id '{}' does not match root chrome id '{}'",
                    self.diagram_id, chrome.diagram_id
                ),
            });
        }
        let plan = self.plan(spec)?;
        self.write_plan(out, &plan, chrome)
    }

    pub(super) fn write_plan(
        &self,
        out: &mut impl SvgOutput,
        plan: &RootViewportPlan,
        chrome: RootChrome<'_>,
    ) -> Result<RootDocument> {
        self.require_empty_document(out)?;
        if plan.family != self.family
            || plan.diagram_id != self.diagram_id.as_str()
            || chrome.diagram_id != self.diagram_id
        {
            return Err(Error::InvalidModel {
                message: "root viewport plan belongs to a different render context".to_string(),
            });
        }
        let viewbox_attr = plan.view_box.map(ViewBox::attr);
        let style = with_custom_properties(plan.style.clone(), chrome.custom_properties)?;
        let width = match plan.width.as_deref() {
            None => SvgRootWidth::None,
            Some("100%") => SvgRootWidth::Percent100,
            Some(width) => SvgRootWidth::Fixed(width),
        };
        let style_placement = if plan.responsive {
            chrome.dom.responsive_style_placement
        } else {
            chrome.dom.fixed_style_placement
        };

        push_svg_root_open(
            out,
            SvgRootAttrs {
                diagram_id: chrome.diagram_id,
                class: chrome.class,
                width,
                height_attr: plan.height.as_deref(),
                style_attr: style.as_deref().map(SvgRootAttributeValue::plain),
                viewbox_attr: viewbox_attr.as_deref().map(SvgRootAttributeValue::plain),
                style_viewbox_order: chrome.dom.style_viewbox_order,
                style_placement,
                responsive_height_placement: chrome.dom.responsive_height_placement,
                extra_attrs: chrome.extra_attrs,
                aria_roledescription: chrome.aria_roledescription,
                aria_labelledby: chrome.aria_labelledby,
                aria_describedby: chrome.aria_describedby,
                after_roledescription_attrs: chrome.after_roledescription_attrs,
                tail_attrs: chrome.tail_attrs,
                fixed_height_placement: chrome.dom.fixed_height_placement,
                trailing_newline: chrome.dom.trailing_newline,
                aria_attr_order: chrome.dom.aria_attr_order,
            },
        );
        out.checkpoint()?;
        Ok(RootDocument {
            family: self.family,
            diagram_id: self.diagram_id.as_str().to_string(),
            state: RootDocumentState::Ready {
                root_open: out.as_str().to_string(),
                plan: plan.clone(),
            },
        })
    }

    fn complete_document(&self, out: String, document: RootDocument) -> Result<RootedSvg> {
        if document.family != self.family || document.diagram_id != self.diagram_id.as_str() {
            return Err(Error::InvalidModel {
                message: "root document belongs to a different render context".to_string(),
            });
        }
        let RootDocumentState::Ready { root_open, plan } = document.state else {
            return Err(Error::InvalidModel {
                message: "root document viewport was not finalized".to_string(),
            });
        };
        if !out.starts_with(&root_open) {
            return Err(Error::InvalidModel {
                message: "family mutated operation-owned SVG root attributes".to_string(),
            });
        }
        if !out.trim_end().ends_with("</svg>") {
            return Err(Error::InvalidModel {
                message: "family returned an incomplete SVG root document".to_string(),
            });
        }
        Ok(RootedSvg {
            svg: out,
            family: self.family,
            diagram_id: self.diagram_id.as_str().to_string(),
            root_open_end: root_open.len(),
            viewport: plan,
        })
    }

    fn require_empty_document(&self, out: &impl SvgOutput) -> Result<()> {
        if out.len() == 0 {
            return Ok(());
        }
        Err(Error::InvalidModel {
            message: "root SVG emission must begin with an empty document buffer".to_string(),
        })
    }

    pub(super) fn plan(&self, spec: RootViewportSpec) -> Result<RootViewportPlan> {
        let view_box = spec
            .view_box
            .map(|bounds| ViewBox::from_bounds(bounds, &self.resources))
            .transpose()?;
        let max_width = spec.max_width.format(view_box, &self.resources)?;

        let fixed_dimensions = || {
            if let Some((width, height)) = spec.fixed_size {
                return Ok::<_, Error>((
                    fmt_string(checked_viewport_dimension(
                        width,
                        "fixed root width",
                        &self.resources,
                    )?),
                    fmt_string(checked_viewport_dimension(
                        height,
                        "fixed root height",
                        &self.resources,
                    )?),
                ));
            }
            let view_box = view_box.ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "fixed root sizing for {} diagram '{}' requires a viewBox",
                    self.family, self.diagram_id
                ),
            })?;
            Ok::<_, Error>((fmt_string(view_box.width), fmt_string(view_box.height)))
        };
        let (responsive, width, height) = match spec.sizing {
            RootSizing::Responsive => (true, Some("100%".to_string()), None),
            RootSizing::Mermaid {
                use_max_width: true,
            } => (true, Some("100%".to_string()), None),
            #[cfg(feature = "layout-cytoscape")]
            RootSizing::MermaidOrIntrinsic {
                use_max_width: true,
            } => (true, Some("100%".to_string()), None),
            RootSizing::MermaidWithResponsiveHeight {
                use_max_width: true,
                height,
            } => (
                true,
                Some("100%".to_string()),
                Some(fmt_string(checked_viewport_dimension(
                    height,
                    "responsive root height",
                    &self.resources,
                )?)),
            ),
            RootSizing::Mermaid {
                use_max_width: false,
            } => {
                let (width, height) = fixed_dimensions()?;
                (false, Some(width), Some(height))
            }
            RootSizing::MermaidWithResponsiveHeight {
                use_max_width: false,
                ..
            } => {
                let (width, height) = fixed_dimensions()?;
                (false, Some(width), Some(height))
            }
            #[cfg(feature = "layout-cytoscape")]
            RootSizing::MermaidOrIntrinsic {
                use_max_width: false,
            } => (false, None, None),
        };
        let style = root_style(responsive.then_some(max_width.as_str()), spec.background);

        Ok(RootViewportPlan {
            family: self.family,
            diagram_id: self.diagram_id.as_str().to_string(),
            view_box,
            width,
            height,
            style,
            responsive,
            max_width,
        })
    }
}

impl RootedSvg {
    pub(super) fn as_str(&self) -> &str {
        &self.svg
    }

    pub(super) fn apply_root_theme(
        mut self,
        plan: Option<&RootThemePlan>,
        work_meter: &OperationWorkMeter,
    ) -> Result<(Self, RootThemeReport)> {
        let Some(plan) = plan else {
            return Ok((
                self,
                RootThemePlan::default().begin_svg_application().finish(),
            ));
        };
        let mut application = plan.begin_svg_application();
        let resources = work_meter.policy();
        let canvas_prelude = CanvasPreludePlan::new(
            plan,
            self.diagram_id.as_str(),
            self.viewport.view_box(),
            &resources,
        )?;
        let background_edit = (plan.canvas().has_explicit_base()
            && matches!(plan.canvas().base(), CanvasPaint::Transparent))
        .then(|| {
            crate::svg::pipeline::set_root_background_color(&self.svg, "transparent").ok_or_else(
                || Error::InvalidModel {
                    message: "root background edit requires a complete SVG opening tag".to_string(),
                },
            )
        })
        .transpose()?;

        let prelude_len = canvas_prelude.encoded_len(self.viewport.view_box())?;
        let background_growth = background_edit
            .as_ref()
            .map_or(0, |edit| edit.additional_len());
        let total_growth =
            background_growth
                .checked_add(prelude_len)
                .ok_or_else(|| Error::InvalidModel {
                    message: "root theme SVG growth overflowed".to_string(),
                })?;
        work_meter.check_svg_append(self.svg.len(), total_growth)?;
        canvas_prelude.ensure_resource_ids_available(&self.svg, work_meter)?;
        if prelude_len != 0 || background_edit.is_some() {
            let mut prelude = String::new();
            prelude
                .try_reserve_exact(prelude_len)
                .map_err(|_| Error::InvalidModel {
                    message: "failed to reserve bounded root theme SVG prelude".to_string(),
                })?;
            canvas_prelude.write(&mut prelude, self.viewport.view_box())?;
            if prelude.len() != prelude_len {
                return Err(Error::InvalidModel {
                    message: "root theme SVG preflight disagreed with terminal serialization"
                        .to_string(),
                });
            }
            self.svg
                .try_reserve_exact(total_growth)
                .map_err(|_| Error::InvalidModel {
                    message: "failed to reserve bounded root theme SVG".to_string(),
                })?;
            if let Some(edit) = background_edit {
                self.root_open_end =
                    edit.adjusted_end(self.root_open_end)
                        .ok_or_else(|| Error::InvalidModel {
                            message: "root background edit invalidated the SVG opening boundary"
                                .to_string(),
                        })?;
                edit.apply(&mut self.svg);
            }
            if !prelude.is_empty() {
                self.svg.insert_str(self.root_open_end, &prelude);
            }
        }
        canvas_prelude.mark_applied(&mut application)?;
        Ok((self, application.finish()))
    }

    pub(super) fn into_string_for(self, expected_family: DiagramFamilyId) -> Result<String> {
        if self.family != expected_family {
            return Err(Error::InvalidModel {
                message: format!(
                    "{} root document '{}' was returned for {expected_family}",
                    self.family, self.diagram_id
                ),
            });
        }
        Ok(self.svg)
    }
}

fn supported_canvas_paint(paint: &CanvasPaint) -> bool {
    !matches!(paint, CanvasPaint::Pattern(_))
}

fn canvas_paint_has_concrete_svg_geometry(paint: &CanvasPaint, view_box: Option<ViewBox>) -> bool {
    match paint {
        CanvasPaint::LinearGradient(gradient) => linear_gradient_line(gradient, view_box).is_some(),
        CanvasPaint::RadialGradient(gradient) if gradient.is_repeating() => {
            resolved_radial_radius_px(gradient, view_box)
                .is_some_and(|radius_px| gradient.resolved_repeating_radius_is_bounded(radius_px))
        }
        CanvasPaint::Transparent
        | CanvasPaint::Solid(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => true,
    }
}

fn supported_canvas_capabilities(
    paint: &CanvasPaint,
) -> impl Iterator<Item = ThemeCapability> + '_ {
    let supported = supported_canvas_paint(paint);
    paint_capabilities(paint).filter(move |_| supported)
}

fn validate_canvas_paint_svg_geometry(
    paint: &CanvasPaint,
    view_box: Option<ViewBox>,
    resources: &RenderResourcePolicy,
) -> Result<()> {
    match paint {
        CanvasPaint::LinearGradient(gradient) => {
            let line =
                linear_gradient_line(gradient, view_box).ok_or_else(|| Error::InvalidModel {
                    message: "ordinary root linear gradients require concrete paint bounds"
                        .to_string(),
                })?;
            for (field, value) in [
                ("linear gradient x1", line.x1),
                ("linear gradient y1", line.y1),
                ("linear gradient x2", line.x2),
                ("linear gradient y2", line.y2),
            ] {
                checked_svg_coordinate(value, field, resources)?;
            }
        }
        CanvasPaint::RadialGradient(gradient) => {
            for (field, length) in [
                ("radial gradient center-x", gradient.center_x()),
                ("radial gradient center-y", gradient.center_y()),
                ("radial gradient radius", gradient.radius()),
            ] {
                if let ThemeLength::Px(value) = length {
                    checked_svg_coordinate(f64::from(value), field, resources)?;
                }
            }
            if gradient.is_repeating() {
                let radius_px = resolved_radial_radius_px(gradient, view_box).ok_or_else(|| {
                    Error::InvalidModel {
                        message: "repeating radial gradient requires concrete paint bounds"
                            .to_string(),
                    }
                })?;
                if !gradient.resolved_repeating_radius_is_bounded(radius_px) {
                    return Err(Error::InvalidModel {
                        message: "repeating radial gradient resolved outside its bounded period"
                            .to_string(),
                    });
                }
                checked_svg_coordinate(radius_px, "repeating radial gradient radius", resources)?;
            }
        }
        CanvasPaint::Transparent | CanvasPaint::Solid(_) | CanvasPaint::Pattern(_) => {}
    }
    Ok(())
}

#[derive(Debug)]
struct CanvasPreludePlan<'a> {
    scope: String,
    entries: Vec<CanvasPreludeEntry<'a>>,
}

impl<'a> CanvasPreludePlan<'a> {
    fn new(
        plan: &'a RootThemePlan,
        diagram_id: &str,
        view_box: Option<ViewBox>,
        resources: &RenderResourcePolicy,
    ) -> Result<Self> {
        let scope = sanitize_svg_id(diagram_id);
        let mut entries = Vec::with_capacity(plan.canvas().layers().len() + 1);

        if plan.canvas().has_explicit_base()
            && supported_canvas_paint(plan.canvas().base())
            && canvas_paint_has_concrete_svg_geometry(plan.canvas().base(), view_box)
        {
            validate_canvas_paint_svg_geometry(plan.canvas().base(), view_box, resources)?;
            entries.push(CanvasPreludeEntry::Base {
                paint: plan.canvas().base(),
            });
        }

        for (index, layer) in plan.canvas().layers().iter().enumerate() {
            if !supported_canvas_paint(layer.paint())
                || !canvas_paint_has_concrete_svg_geometry(layer.paint(), view_box)
            {
                continue;
            }
            validate_canvas_paint_svg_geometry(layer.paint(), view_box, resources)?;
            let (offset_x, offset_y) = layer.offset();
            checked_svg_coordinate(f64::from(offset_x), "canvas layer offset-x", resources)?;
            checked_svg_coordinate(f64::from(offset_y), "canvas layer offset-y", resources)?;
            entries.push(CanvasPreludeEntry::Layer {
                index,
                paint: layer.paint(),
                opacity: layer.opacity(),
                offset: layer.offset(),
                blend_mode: layer.blend_mode(),
            });
        }

        Ok(Self { scope, entries })
    }

    fn encoded_len(&self, view_box: Option<ViewBox>) -> Result<usize> {
        let mut projected = ProjectedSvgLength::default();
        self.write(&mut projected, view_box)?;
        projected.finish()
    }

    fn ensure_resource_ids_available(
        &self,
        existing_svg: &str,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        if !self
            .entries
            .iter()
            .any(|entry| entry.resource_ids(&self.scope).is_some())
        {
            return Ok(());
        }
        work_meter.charge(existing_svg.len().div_ceil(64))?;
        if svg_contains_canvas_resource_namespace(existing_svg, &self.scope) {
            return Err(Error::InvalidModel {
                message: format!(
                    "root theme SVG resource namespace '{}-merman-theme-canvas-' already exists",
                    self.scope
                ),
            });
        }
        Ok(())
    }

    fn write(&self, out: &mut impl SvgOutput, view_box: Option<ViewBox>) -> Result<()> {
        for entry in &self.entries {
            entry.write(out, view_box, &self.scope);
        }
        out.checkpoint()
    }

    fn mark_applied(&self, application: &mut RootThemeApplication) -> Result<()> {
        for entry in &self.entries {
            entry.mark_applied(application)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
enum CanvasPreludeEntry<'a> {
    Base {
        paint: &'a CanvasPaint,
    },
    Layer {
        index: usize,
        paint: &'a CanvasPaint,
        opacity: f32,
        offset: (f32, f32),
        blend_mode: BlendMode,
    },
}

impl CanvasPreludeEntry<'_> {
    fn resource_ids<'a>(&self, scope: &'a str) -> Option<CanvasResourceIds<'a>> {
        match self {
            Self::Base { paint } => {
                CanvasResourceIds::for_paint(scope, CanvasPaintSlot::Base, paint)
            }
            Self::Layer { index, paint, .. } => {
                CanvasResourceIds::for_paint(scope, CanvasPaintSlot::Layer(*index), paint)
            }
        }
    }

    fn write(&self, out: &mut impl SvgOutput, view_box: Option<ViewBox>, scope: &str) {
        match self {
            Self::Base { paint } => {
                let ids = self.resource_ids(scope);
                push_canvas_paint_defs(out, paint, ids, view_box);
                push_canvas_base(out, view_box, paint, ids);
            }
            Self::Layer {
                index,
                paint,
                opacity,
                offset,
                blend_mode,
            } => {
                let ids = self.resource_ids(scope);
                push_canvas_paint_defs(out, paint, ids, view_box);
                push_canvas_layer(
                    out,
                    view_box,
                    *index,
                    paint,
                    ids,
                    *opacity,
                    *offset,
                    *blend_mode,
                );
            }
        }
    }

    fn mark_applied(&self, application: &mut RootThemeApplication) -> Result<()> {
        match self {
            Self::Base { paint } => mark_root_mechanism_applied(
                application,
                &RootThemeMechanismKey::CanvasBase,
                supported_canvas_capabilities(paint),
            ),
            Self::Layer {
                index,
                paint,
                opacity,
                offset,
                blend_mode,
                ..
            } => {
                let applied = std::iter::once(ThemeCapability::LayeredCanvas)
                    .chain(supported_canvas_capabilities(paint))
                    .chain((*opacity != 1.0).then_some(ThemeCapability::Opacity))
                    .chain((*offset != (0.0, 0.0)).then_some(ThemeCapability::CanvasLayerPlacement))
                    .chain(
                        (!matches!(blend_mode, BlendMode::Normal))
                            .then_some(ThemeCapability::BlendMode),
                    );
                mark_root_mechanism_applied(
                    application,
                    &RootThemeMechanismKey::CanvasLayer { index: *index },
                    applied,
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CanvasPaintSlot {
    Base,
    Layer(usize),
}

#[derive(Debug, Clone, Copy)]
struct CanvasResourceIds<'a> {
    scope: &'a str,
    slot: CanvasPaintSlot,
    tiled: bool,
}

#[derive(Debug, Clone, Copy)]
enum CanvasResourceKind {
    Gradient,
    Pattern,
}

impl<'a> CanvasResourceIds<'a> {
    fn for_paint(scope: &'a str, slot: CanvasPaintSlot, paint: &CanvasPaint) -> Option<Self> {
        if !matches!(
            paint,
            CanvasPaint::LinearGradient(_) | CanvasPaint::RadialGradient(_)
        ) {
            return None;
        }
        let tiled = match paint {
            CanvasPaint::LinearGradient(gradient) => gradient.tile_size_px().is_some(),
            CanvasPaint::RadialGradient(gradient) => gradient.tile_size_px().is_some(),
            CanvasPaint::Transparent | CanvasPaint::Solid(_) | CanvasPaint::Pattern(_) => false,
        };
        Some(Self { scope, slot, tiled })
    }

    fn write(self, out: &mut impl SvgOutput, kind: CanvasResourceKind) {
        escape_attr_into(out, self.scope);
        match self.slot {
            CanvasPaintSlot::Base => out.push_str("-merman-theme-canvas-base"),
            CanvasPaintSlot::Layer(index) => {
                let _ = write!(out, "-merman-theme-canvas-layer-{index}");
            }
        }
        match kind {
            CanvasResourceKind::Gradient => out.push_str("-gradient"),
            CanvasResourceKind::Pattern => out.push_str("-pattern"),
        }
    }
}

fn svg_contains_canvas_resource_namespace(svg: &str, scope: &str) -> bool {
    let mut cursor = 0usize;
    while let Some(relative_start) = svg[cursor..].find('<') {
        let start = cursor + relative_start;
        let Some(end) = crate::svg::scanner::find_tag_end(svg, start) else {
            return false;
        };
        if svg_tag_contains_canvas_resource_namespace(&svg[start..=end], scope) {
            return true;
        }
        cursor = end + 1;
    }
    false
}

fn svg_tag_contains_canvas_resource_namespace(tag: &str, scope: &str) -> bool {
    let bytes = tag.as_bytes();
    let mut cursor = 1usize;
    while cursor < bytes.len() {
        if !bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let name_start = cursor;
        while cursor < bytes.len()
            && !bytes[cursor].is_ascii_whitespace()
            && !matches!(bytes[cursor], b'=' | b'/' | b'>')
        {
            cursor += 1;
        }
        let name_end = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let Some(&quote @ (b'"' | b'\'')) = bytes.get(cursor) else {
            continue;
        };
        let value_start = cursor + 1;
        let Some(value_end) = bytes[value_start..]
            .iter()
            .position(|byte| *byte == quote)
            .map(|relative| value_start + relative)
        else {
            return false;
        };
        if &bytes[name_start..name_end] == b"id"
            && tag[value_start..value_end]
                .strip_prefix(scope)
                .is_some_and(|suffix| suffix.starts_with("-merman-theme-canvas-"))
        {
            return true;
        }
        cursor = value_end + 1;
    }
    false
}

#[derive(Debug, Clone, Copy)]
struct GradientBounds {
    min_x: f64,
    min_y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy)]
struct LinearGradientLine {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

fn gradient_paint_bounds(
    tile_size_px: Option<(f32, f32)>,
    view_box: Option<ViewBox>,
) -> Option<GradientBounds> {
    if let Some((width_px, height_px)) = tile_size_px {
        return Some(GradientBounds {
            min_x: 0.0,
            min_y: 0.0,
            width: f64::from(width_px),
            height: f64::from(height_px),
        });
    }
    view_box.map(|view_box| GradientBounds {
        min_x: view_box.min_x,
        min_y: view_box.min_y,
        width: view_box.width,
        height: view_box.height,
    })
}

fn linear_gradient_line(
    gradient: &LinearGradient,
    view_box: Option<ViewBox>,
) -> Option<LinearGradientLine> {
    let radians = f64::from(gradient.angle_degrees())
        .rem_euclid(360.0)
        .to_radians();
    let direction_x = radians.sin();
    let direction_y = -radians.cos();
    if let Some(period_px) = gradient.repeating_period_px() {
        let period_px = f64::from(period_px);
        return Some(LinearGradientLine {
            x1: 0.0,
            y1: 0.0,
            x2: direction_x * period_px,
            y2: direction_y * period_px,
        });
    }

    let bounds = gradient_paint_bounds(gradient.tile_size_px(), view_box)?;
    let center_x = bounds.min_x + bounds.width / 2.0;
    let center_y = bounds.min_y + bounds.height / 2.0;
    let half_length =
        (bounds.width * direction_x).abs() / 2.0 + (bounds.height * direction_y).abs() / 2.0;
    Some(LinearGradientLine {
        x1: center_x - direction_x * half_length,
        y1: center_y - direction_y * half_length,
        x2: center_x + direction_x * half_length,
        y2: center_y + direction_y * half_length,
    })
}

fn gradient_bounds(gradient: &RadialGradient, view_box: Option<ViewBox>) -> Option<GradientBounds> {
    gradient_paint_bounds(gradient.tile_size_px(), view_box)
}

fn normalized_diagonal(width: f64, height: f64) -> f64 {
    width.hypot(height) / std::f64::consts::SQRT_2
}

fn resolved_radial_radius_px(gradient: &RadialGradient, view_box: Option<ViewBox>) -> Option<f64> {
    match gradient.radius() {
        ThemeLength::Px(value) => Some(f64::from(value)),
        ThemeLength::Percent(_) => {
            let bounds = gradient_bounds(gradient, view_box)?;
            Some(resolve_radius(
                gradient.radius(),
                bounds.width,
                bounds.height,
            ))
        }
    }
}

fn push_canvas_paint_defs(
    out: &mut impl SvgOutput,
    paint: &CanvasPaint,
    ids: Option<CanvasResourceIds<'_>>,
    view_box: Option<ViewBox>,
) {
    let Some(ids) = ids else {
        return;
    };
    out.push_str("<defs>");
    match paint {
        CanvasPaint::LinearGradient(gradient) => {
            push_linear_gradient(out, ids, gradient, view_box);
            if let Some((width_px, height_px)) = gradient.tile_size_px() {
                push_gradient_pattern(out, ids, width_px, height_px, view_box);
            }
        }
        CanvasPaint::RadialGradient(gradient) => {
            push_radial_gradient(out, ids, gradient, view_box);
            if let Some((width_px, height_px)) = gradient.tile_size_px() {
                push_gradient_pattern(out, ids, width_px, height_px, view_box);
            }
        }
        CanvasPaint::Transparent | CanvasPaint::Solid(_) | CanvasPaint::Pattern(_) => {}
    }
    out.push_str("</defs>");
}

fn push_linear_gradient(
    out: &mut impl SvgOutput,
    ids: CanvasResourceIds<'_>,
    gradient: &LinearGradient,
    view_box: Option<ViewBox>,
) {
    let line = linear_gradient_line(gradient, view_box)
        .expect("validated root linear gradient must have concrete geometry");
    out.push_str(r#"<linearGradient id=""#);
    ids.write(out, CanvasResourceKind::Gradient);
    let _ = write!(
        out,
        r#"" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}""#,
        fmt(line.x1),
        fmt(line.y1),
        fmt(line.x2),
        fmt(line.y2),
    );
    if let Some(period_px) = gradient.repeating_period_px() {
        debug_assert!(period_px >= 1.0);
        out.push_str(r#" spreadMethod="repeat">"#);
    } else {
        out.push('>');
    }
    push_gradient_stops(out, gradient.stops());
    out.push_str("</linearGradient>");
}

fn push_radial_gradient(
    out: &mut impl SvgOutput,
    ids: CanvasResourceIds<'_>,
    gradient: &RadialGradient,
    view_box: Option<ViewBox>,
) {
    let bounds = gradient_bounds(gradient, view_box);
    out.push_str(r#"<radialGradient id=""#);
    ids.write(out, CanvasResourceKind::Gradient);
    out.push_str(r#"" gradientUnits="userSpaceOnUse" cx=""#);
    match bounds {
        Some(bounds) => {
            push_resolved_position(out, gradient.center_x(), bounds.min_x, bounds.width)
        }
        None => push_theme_length(out, gradient.center_x()),
    }
    out.push_str(r#"" cy=""#);
    match bounds {
        Some(bounds) => {
            push_resolved_position(out, gradient.center_y(), bounds.min_y, bounds.height)
        }
        None => push_theme_length(out, gradient.center_y()),
    }
    out.push_str(r#"" r=""#);
    match bounds {
        Some(bounds) => push_resolved_radius(out, gradient.radius(), bounds.width, bounds.height),
        None => push_theme_length(out, gradient.radius()),
    }
    if gradient.is_repeating() {
        out.push_str(r#"" spreadMethod="repeat">"#);
    } else {
        out.push_str(r#"">"#);
    }
    push_gradient_stops(out, gradient.stops());
    out.push_str("</radialGradient>");
}

fn push_gradient_pattern(
    out: &mut impl SvgOutput,
    ids: CanvasResourceIds<'_>,
    width_px: f32,
    height_px: f32,
    view_box: Option<ViewBox>,
) {
    debug_assert!(ids.tiled, "tiled gradient must own a pattern id");
    out.push_str(r#"<pattern id=""#);
    ids.write(out, CanvasResourceKind::Pattern);
    let _ = write!(
        out,
        r#"" patternUnits="userSpaceOnUse" x="0" y="0" width="{}" height="{}""#,
        fmt(f64::from(width_px)),
        fmt(f64::from(height_px)),
    );
    if let Some(bounds) = view_box.filter(|bounds| bounds.min_x != 0.0 || bounds.min_y != 0.0) {
        // Move the tile coordinate system once, keeping its content and gradient local.
        let _ = write!(
            out,
            r#" patternTransform="translate({} {})""#,
            fmt(bounds.min_x),
            fmt(bounds.min_y),
        );
    }
    let _ = write!(
        out,
        r#"><rect x="0" y="0" width="{}" height="{}" fill="url(#"#,
        fmt(f64::from(width_px)),
        fmt(f64::from(height_px)),
    );
    ids.write(out, CanvasResourceKind::Gradient);
    out.push_str(r#")"/></pattern>"#);
}

fn push_gradient_stops(out: &mut impl SvgOutput, stops: &[GradientStop]) {
    for stop in stops {
        let _ = write!(
            out,
            r#"<stop offset="{}%" stop-color=""#,
            fmt(f64::from(stop.offset()) * 100.0),
        );
        escape_attr_into(out, stop.color().as_css_cow().as_ref());
        out.push_str(r#""/>"#);
    }
}

fn push_theme_length(out: &mut impl SvgOutput, length: ThemeLength) {
    match length {
        ThemeLength::Px(value) => {
            let _ = write!(out, "{}", fmt(f64::from(value)));
        }
        ThemeLength::Percent(value) => {
            let _ = write!(out, "{}%", fmt(f64::from(value)));
        }
    }
}

fn push_resolved_position(out: &mut impl SvgOutput, length: ThemeLength, origin: f64, extent: f64) {
    let value = match length {
        ThemeLength::Px(value) => f64::from(value),
        ThemeLength::Percent(value) => origin + extent * f64::from(value) / 100.0,
    };
    let _ = write!(out, "{}", fmt(value));
}

fn push_resolved_radius(out: &mut impl SvgOutput, length: ThemeLength, width: f64, height: f64) {
    let value = resolve_radius(length, width, height);
    let _ = write!(out, "{}", fmt(value));
}

fn resolve_radius(length: ThemeLength, width: f64, height: f64) -> f64 {
    match length {
        ThemeLength::Px(value) => f64::from(value),
        ThemeLength::Percent(value) => {
            normalized_diagonal(width, height) * f64::from(value) / 100.0
        }
    }
}

fn push_canvas_base(
    out: &mut impl SvgOutput,
    view_box: Option<ViewBox>,
    paint: &CanvasPaint,
    ids: Option<CanvasResourceIds<'_>>,
) {
    out.push_str(
        r#"<rect class="merman-theme-canvas-base" data-merman-theme-canvas="base" aria-hidden="true" pointer-events="none""#,
    );
    push_canvas_rect_geometry(out, view_box);
    out.push_str(r#" fill=""#);
    push_canvas_fill(out, paint, ids);
    out.push_str(r#""/>"#);
}

fn push_canvas_layer(
    out: &mut impl SvgOutput,
    view_box: Option<ViewBox>,
    index: usize,
    paint: &CanvasPaint,
    ids: Option<CanvasResourceIds<'_>>,
    opacity: f32,
    offset: (f32, f32),
    blend_mode: BlendMode,
) {
    let _ = write!(
        out,
        r#"<g class="merman-theme-canvas-layer" data-merman-theme-canvas-layer="{index}" aria-hidden="true" pointer-events="none""#,
    );
    if opacity != 1.0 {
        let _ = write!(out, r#" opacity="{}""#, fmt(f64::from(opacity)));
    }
    if offset != (0.0, 0.0) {
        let _ = write!(
            out,
            r#" transform="translate({} {})""#,
            fmt(f64::from(offset.0)),
            fmt(f64::from(offset.1)),
        );
    }
    if !matches!(blend_mode, BlendMode::Normal) {
        out.push_str(r#" style="mix-blend-mode:"#);
        out.push_str(blend_mode.as_svg());
        out.push_str(r#"""#);
    }
    out.push('>');
    out.push_str(r#"<rect"#);
    push_canvas_rect_geometry(out, view_box);
    out.push_str(r#" fill=""#);
    push_canvas_fill(out, paint, ids);
    out.push_str(r#""/></g>"#);
}

fn push_canvas_fill(
    out: &mut impl SvgOutput,
    paint: &CanvasPaint,
    ids: Option<CanvasResourceIds<'_>>,
) {
    match paint {
        CanvasPaint::Transparent => out.push_str("none"),
        CanvasPaint::Solid(color) => escape_attr_into(out, color.as_css_cow().as_ref()),
        CanvasPaint::LinearGradient(gradient) => {
            push_canvas_resource_url(out, ids, gradient.tile_size_px().is_some())
        }
        CanvasPaint::RadialGradient(gradient) => {
            push_canvas_resource_url(out, ids, gradient.tile_size_px().is_some())
        }
        CanvasPaint::Pattern(_) => {}
    }
}

fn push_canvas_resource_url(
    out: &mut impl SvgOutput,
    ids: Option<CanvasResourceIds<'_>>,
    uses_pattern: bool,
) {
    let ids = ids.expect("gradient paint must own SVG resource ids");
    let kind = if uses_pattern {
        debug_assert!(ids.tiled, "tiled gradient must own a pattern id");
        CanvasResourceKind::Pattern
    } else {
        CanvasResourceKind::Gradient
    };
    out.push_str("url(#");
    ids.write(out, kind);
    out.push(')');
}

#[derive(Debug, Default)]
struct ProjectedSvgLength {
    len: usize,
    overflowed: bool,
}

impl ProjectedSvgLength {
    fn add(&mut self, additional: usize) -> std::fmt::Result {
        let Some(len) = self.len.checked_add(additional) else {
            self.overflowed = true;
            return Err(std::fmt::Error);
        };
        self.len = len;
        Ok(())
    }

    fn finish(self) -> Result<usize> {
        if self.overflowed {
            return Err(Error::InvalidModel {
                message: "root theme SVG preflight length overflowed".to_string(),
            });
        }
        Ok(self.len)
    }
}

impl std::fmt::Write for ProjectedSvgLength {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.add(value.len())
    }
}

impl SvgOutput for ProjectedSvgLength {
    fn push_str(&mut self, value: &str) {
        let _ = self.add(value.len());
    }

    fn push(&mut self, value: char) {
        let _ = self.add(value.len_utf8());
    }

    fn len(&self) -> usize {
        self.len
    }

    fn as_str(&self) -> &str {
        ""
    }

    fn replace_range(&mut self, _range: Range<usize>, _replacement: &str) -> Result<()> {
        Err(Error::InvalidModel {
            message: "root theme SVG preflight cannot replace retained ranges".to_string(),
        })
    }

    fn checkpoint(&mut self) -> Result<()> {
        if self.overflowed {
            return Err(Error::InvalidModel {
                message: "root theme SVG preflight length overflowed".to_string(),
            });
        }
        Ok(())
    }
}

fn push_canvas_rect_geometry(out: &mut impl SvgOutput, view_box: Option<ViewBox>) {
    match view_box {
        Some(view_box) => {
            let _ = write!(
                out,
                r#" x="{}" y="{}" width="{}" height="{}""#,
                fmt(view_box.min_x),
                fmt(view_box.min_y),
                fmt(view_box.width),
                fmt(view_box.height),
            );
        }
        None => out.push_str(r#" x="0" y="0" width="100%" height="100%""#),
    }
}

fn mark_root_mechanism_applied(
    application: &mut RootThemeApplication,
    key: &RootThemeMechanismKey,
    capabilities: impl IntoIterator<Item = ThemeCapability>,
) -> Result<()> {
    if application.mark_applied(key, capabilities) {
        return Ok(());
    }
    Err(Error::InvalidModel {
        message: format!("root SVG consumer emitted unplanned theme mechanism {key}"),
    })
}

impl std::ops::Deref for RootedSvg {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.svg
    }
}

impl std::fmt::Display for RootedSvg {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.svg)
    }
}

impl RootDocument {
    pub(super) fn complete(self, out: String) -> Result<RootedSvg> {
        let family = self.family;
        let diagram_id = self.diagram_id.clone();
        RootViewportContext::new(family, diagram_id.as_str()).complete_document(out, self)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RootViewportPlan {
    family: DiagramFamilyId,
    diagram_id: String,
    view_box: Option<ViewBox>,
    width: Option<String>,
    height: Option<String>,
    style: Option<String>,
    responsive: bool,
    max_width: String,
}

impl RootViewportPlan {
    pub(super) fn view_box(&self) -> Option<ViewBox> {
        self.view_box
    }
}

fn root_style(max_width: Option<&str>, background: RootBackground) -> Option<String> {
    let mut style = String::new();
    if let Some(max_width) = max_width {
        let _ = write!(style, "max-width: {max_width}px;");
    }
    if background == RootBackground::White {
        if !style.is_empty() {
            style.push(' ');
        }
        style.push_str("background-color: white;");
    }
    (!style.is_empty()).then_some(style)
}

fn with_custom_properties(
    style: Option<String>,
    properties: &[(&str, &str)],
) -> Result<Option<String>> {
    let mut style = style.unwrap_or_default();
    for (key, value) in properties {
        let declaration = format!("{key}:{value}");
        if !key.starts_with("--")
            || crate::mermaid_style::parse_safe_style_decl(&declaration).is_none()
        {
            return Err(Error::InvalidModel {
                message: format!("invalid SVG root custom property {key}"),
            });
        }
        if !style.is_empty() {
            style.push(' ');
        }
        let _ = write!(style, "{key}: {value};");
    }
    Ok((!style.is_empty()).then_some(style))
}

fn format_css_max_width(value: f64) -> String {
    if !value.is_finite() || value.abs() < 0.0005 {
        return "0".to_string();
    }
    let exponent = value.abs().max(0.0005).log10().floor() as i32;
    let decimals = (5 - exponent).clamp(0, 6) as usize;
    let scale = 10f64.powi(decimals as i32);
    let mut rounded = round_ties_to_even(value * scale) / scale;
    if rounded.abs() < 0.0005 {
        rounded = 0.0;
    }
    let mut formatted = format!("{rounded:.decimals$}");
    if formatted.contains('.') {
        while formatted.ends_with('0') {
            formatted.pop();
        }
        if formatted.ends_with('.') {
            formatted.pop();
        }
    }
    formatted
}

fn round_ties_to_even(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    let sign = if value.is_sign_negative() { -1.0 } else { 1.0 };
    let absolute = value.abs();
    let floor = absolute.floor();
    let fraction = absolute - floor;
    let rounded = if fraction < 0.5 {
        floor
    } else if fraction > 0.5 {
        floor + 1.0
    } else if (floor as i64) % 2 == 0 {
        floor
    } else {
        floor + 1.0
    };
    sign * rounded
}

fn format_precision_fixed(value: f64, significant_digits: u8) -> String {
    let precision = i32::from(significant_digits.max(1));
    if !value.is_finite() {
        return "0".to_string();
    }
    if value == 0.0 {
        return format!("{:.*}", (precision - 1) as usize, 0.0);
    }
    let exponent = value.abs().log10().floor() as i32;
    let decimals = (precision - exponent - 1).max(0) as usize;
    format!("{value:.decimals$}")
}

fn checked_svg_coordinate(
    value: f64,
    field: &str,
    resources: &RenderResourcePolicy,
) -> Result<f64> {
    if value.is_finite() {
        resources.check_svg_backend_coordinate_magnitude(value)?;
        return Ok(value);
    }
    Err(Error::InvalidModel {
        message: format!("root SVG {field} must be finite"),
    })
}

fn checked_viewport_dimension(
    value: f64,
    field: &str,
    resources: &RenderResourcePolicy,
) -> Result<f64> {
    Ok(checked_svg_coordinate(value, field, resources)?.max(1.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SvgRootWidth<'a> {
    None,
    Percent100,
    Fixed(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SvgRootStyleViewBoxOrder {
    StyleThenViewBox,
    ViewBoxThenStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SvgRootAriaAttrOrder {
    DescribedbyThenLabelledby,
    LabelledbyThenDescribedby,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SvgRootFixedHeightPlacement {
    BeforeXmlns,
    AfterXmlns,
    AfterClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SvgRootAttributeValue<'a> {
    Plain(&'a str),
    Tracked {
        prefix: &'a str,
        value: &'a str,
        suffix: &'a str,
    },
}

impl<'a> SvgRootAttributeValue<'a> {
    const fn plain(value: &'a str) -> Self {
        Self::Plain(value)
    }

    const fn tracked(prefix: &'a str, value: &'a str, suffix: &'a str) -> Self {
        Self::Tracked {
            prefix,
            value,
            suffix,
        }
    }

    fn write_escaped(self, out: &mut impl SvgOutput) -> Option<Range<usize>> {
        match self {
            Self::Plain(value) => {
                escape_attr_into(out, value);
                None
            }
            Self::Tracked {
                prefix,
                value,
                suffix,
            } => {
                escape_attr_into(out, prefix);
                let start = out.len();
                escape_attr_into(out, value);
                let end = out.len();
                escape_attr_into(out, suffix);
                Some(start..end)
            }
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SvgRootTrackedRanges {
    view_box: Option<Range<usize>>,
    max_width: Option<Range<usize>>,
}

struct SvgRootAttrs<'a> {
    diagram_id: RootDiagramId<'a>,
    class: Option<&'a str>,
    width: SvgRootWidth<'a>,
    height_attr: Option<&'a str>,
    style_attr: Option<SvgRootAttributeValue<'a>>,
    viewbox_attr: Option<SvgRootAttributeValue<'a>>,
    style_viewbox_order: SvgRootStyleViewBoxOrder,
    style_placement: RootStylePlacement,
    responsive_height_placement: RootResponsiveHeightPlacement,
    extra_attrs: &'a [(&'a str, &'a str)],
    aria_roledescription: &'a str,
    aria_labelledby: Option<&'a str>,
    aria_describedby: Option<&'a str>,
    after_roledescription_attrs: &'a [(&'a str, &'a str)],
    tail_attrs: &'a [(&'a str, &'a str)],
    fixed_height_placement: SvgRootFixedHeightPlacement,
    trailing_newline: bool,
    aria_attr_order: SvgRootAriaAttrOrder,
}

fn push_svg_root_attribute(
    out: &mut impl SvgOutput,
    name: &str,
    value: SvgRootAttributeValue<'_>,
) -> Option<Range<usize>> {
    out.push(' ');
    out.push_str(name);
    out.push_str(r#"=""#);
    let tracked_range = value.write_escaped(out);
    out.push('"');
    tracked_range
}

fn push_svg_root_open(out: &mut impl SvgOutput, attrs: SvgRootAttrs<'_>) -> SvgRootTrackedRanges {
    let SvgRootAttrs {
        diagram_id,
        class,
        width,
        height_attr,
        style_attr,
        viewbox_attr,
        style_viewbox_order,
        style_placement,
        responsive_height_placement,
        extra_attrs,
        aria_roledescription,
        aria_labelledby,
        aria_describedby,
        after_roledescription_attrs,
        tail_attrs,
        fixed_height_placement,
        trailing_newline,
        aria_attr_order,
    } = attrs;

    // Keep attribute order stable (helps strict-mode diffs) and match existing renderers:
    // id, width/height (with configurable fixed-height placement), xmlns, class?,
    // style?/viewBox (configurable), extra-attrs..., role, aria-roledescription, aria-*, tail-attrs..., >\n?
    let mut deferred_height_after_class: Option<&str> = None;
    let mut tracked_ranges = SvgRootTrackedRanges::default();
    out.push_str(r#"<svg id=""#);
    diagram_id.write_escaped(out);
    let responsive_height_attr = matches!(width, SvgRootWidth::Percent100)
        .then_some(height_attr)
        .flatten();
    match width {
        SvgRootWidth::None => {
            out.push_str(r#"" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#);
        }
        SvgRootWidth::Percent100 => {
            out.push_str(
                r#"" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
            );
        }
        SvgRootWidth::Fixed(w) => {
            out.push_str(r#"" width=""#);
            escape_attr_into(out, w);
            out.push('"');
            match fixed_height_placement {
                SvgRootFixedHeightPlacement::BeforeXmlns => {
                    out.push_str(r#" height=""#);
                    escape_attr_into(out, height_attr.unwrap_or("0"));
                    out.push('"');
                    out.push_str(
                        r#" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
                    );
                }
                SvgRootFixedHeightPlacement::AfterXmlns => {
                    out.push_str(
                        r#" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
                    );
                    out.push_str(r#" height=""#);
                    escape_attr_into(out, height_attr.unwrap_or("0"));
                    out.push('"');
                }
                SvgRootFixedHeightPlacement::AfterClass => {
                    out.push_str(
                        r#" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink""#,
                    );
                    deferred_height_after_class = Some(height_attr.unwrap_or("0"));
                }
            }
        }
    }

    if let Some(class) = class {
        out.push_str(r#" class=""#);
        escape_attr_into(out, class);
        out.push('"');
    }
    if let Some(h) = deferred_height_after_class.take() {
        out.push_str(r#" height=""#);
        escape_attr_into(out, h);
        out.push('"');
    }
    match style_viewbox_order {
        SvgRootStyleViewBoxOrder::StyleThenViewBox => {
            if style_placement == RootStylePlacement::Viewport
                && let Some(style_attr) = style_attr
            {
                tracked_ranges.max_width = push_svg_root_attribute(out, "style", style_attr);
            }
            if let Some(viewbox_attr) = viewbox_attr {
                tracked_ranges.view_box = push_svg_root_attribute(out, "viewBox", viewbox_attr);
            }
        }
        SvgRootStyleViewBoxOrder::ViewBoxThenStyle => {
            if let Some(viewbox_attr) = viewbox_attr {
                tracked_ranges.view_box = push_svg_root_attribute(out, "viewBox", viewbox_attr);
            }
            if style_placement == RootStylePlacement::Viewport
                && let Some(style_attr) = style_attr
            {
                tracked_ranges.max_width = push_svg_root_attribute(out, "style", style_attr);
            }
        }
    }
    if responsive_height_placement == RootResponsiveHeightPlacement::BeforeExtraAttrs
        && let Some(height_attr) = responsive_height_attr
    {
        out.push_str(r#" height=""#);
        escape_attr_into(out, height_attr);
        out.push('"');
    }
    for (k, v) in extra_attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str(r#"=""#);
        escape_attr_into(out, v);
        out.push('"');
    }
    if responsive_height_placement == RootResponsiveHeightPlacement::AfterExtraAttrs
        && let Some(height_attr) = responsive_height_attr
    {
        out.push_str(r#" height=""#);
        escape_attr_into(out, height_attr);
        out.push('"');
    }

    out.push_str(r#" role="graphics-document document" aria-roledescription=""#);
    escape_attr_into(out, aria_roledescription);
    out.push('"');
    if style_placement == RootStylePlacement::AfterRoleDescription
        && let Some(style_attr) = style_attr
    {
        tracked_ranges.max_width = push_svg_root_attribute(out, "style", style_attr);
    }
    for (k, v) in after_roledescription_attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str(r#"=""#);
        escape_attr_into(out, v);
        out.push('"');
    }
    match aria_attr_order {
        SvgRootAriaAttrOrder::DescribedbyThenLabelledby => {
            if let Some(v) = aria_describedby {
                out.push_str(r#" aria-describedby=""#);
                escape_attr_into(out, v);
                out.push('"');
            }
            if let Some(v) = aria_labelledby {
                out.push_str(r#" aria-labelledby=""#);
                escape_attr_into(out, v);
                out.push('"');
            }
        }
        SvgRootAriaAttrOrder::LabelledbyThenDescribedby => {
            if let Some(v) = aria_labelledby {
                out.push_str(r#" aria-labelledby=""#);
                escape_attr_into(out, v);
                out.push('"');
            }
            if let Some(v) = aria_describedby {
                out.push_str(r#" aria-describedby=""#);
                escape_attr_into(out, v);
                out.push('"');
            }
        }
    }

    if style_placement == RootStylePlacement::Tail
        && let Some(style_attr) = style_attr
    {
        tracked_ranges.max_width = push_svg_root_attribute(out, "style", style_attr);
    }
    for (k, v) in tail_attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str(r#"=""#);
        escape_attr_into(out, v);
        out.push('"');
    }

    out.push('>');
    if trailing_newline {
        out.push('\n');
    }
    tracked_ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root_theme_meter(svg_limit: usize) -> crate::resources::OperationWorkMeter {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, svg_limit)
            .unwrap();
        crate::resources::OperationWorkMeter::new(policy)
    }

    fn rooted_svg_for_theme_test() -> RootedSvg {
        let context = computed_context(DiagramFamilyId::INFO, "info");
        let mut chrome = RootChrome::new("info", "info");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .write_open(
                &mut out,
                RootViewportSpec::responsive_without_view_box(400.0),
                chrome,
            )
            .unwrap();
        out.push_str("<g/></svg>");
        document.complete(out).unwrap()
    }

    fn rooted_svg_for_theme_test_with_view_box(width: f64, height: f64) -> RootedSvg {
        let context = computed_context(DiagramFamilyId::INFO, "info");
        let mut chrome = RootChrome::new("info", "info");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .write_open(
                &mut out,
                RootViewportSpec::responsive(DiagramBounds::from_view_box(0.0, 0.0, width, height)),
                chrome,
            )
            .unwrap();
        out.push_str("<g/></svg>");
        document.complete(out).unwrap()
    }

    fn transparent_root_theme_plan() -> RootThemePlan {
        let theme = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(
                crate::diagram_theme::DiagramThemeSpec::new()
                    .with_canvas(crate::diagram_theme::CanvasSpec::transparent()),
            )
            .unwrap();
        RootThemePlan::from_theme(Some(&theme))
    }

    fn gradient_stops() -> [crate::diagram_theme::GradientStop; 2] {
        [
            crate::diagram_theme::GradientStop::new(
                0.0,
                crate::diagram_theme::ThemeColorValue::parse("#0f172a").unwrap(),
            )
            .unwrap(),
            crate::diagram_theme::GradientStop::new(
                1.0,
                crate::diagram_theme::ThemeColorValue::parse("#f8fafc").unwrap(),
            )
            .unwrap(),
        ]
    }

    fn root_theme_plan(canvas: crate::diagram_theme::CanvasSpec) -> RootThemePlan {
        let theme = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(crate::diagram_theme::DiagramThemeSpec::new().with_canvas(canvas))
            .expect("compile root canvas theme");
        RootThemePlan::from_theme(Some(&theme))
    }

    fn repeating_linear_root_theme_plan() -> RootThemePlan {
        root_theme_plan(
            crate::diagram_theme::CanvasSpec::default().with_base(
                crate::diagram_theme::CanvasPaint::LinearGradient(
                    crate::diagram_theme::LinearGradient::new(90.0, gradient_stops())
                        .unwrap()
                        .with_repeating_period_px(16.0)
                        .unwrap(),
                ),
            ),
        )
    }

    fn repeating_radial_root_theme_plan(radius_percent: f32) -> RootThemePlan {
        root_theme_plan(
            crate::diagram_theme::CanvasSpec::default().with_base(
                crate::diagram_theme::CanvasPaint::RadialGradient(
                    crate::diagram_theme::RadialGradient::new(
                        crate::diagram_theme::ThemeLength::percent(50.0),
                        crate::diagram_theme::ThemeLength::percent(50.0),
                        crate::diagram_theme::ThemeLength::percent(radius_percent),
                        gradient_stops(),
                    )
                    .unwrap()
                    .with_repeating()
                    .unwrap(),
                ),
            ),
        )
    }

    #[test]
    fn non_square_tiled_linear_gradient_preserves_its_authored_angle() {
        let gradient = crate::diagram_theme::LinearGradient::new(135.0, gradient_stops())
            .unwrap()
            .with_tile_px(40.0, 20.0)
            .unwrap();
        let line = linear_gradient_line(&gradient, None).expect("tile supplies concrete bounds");
        let delta_x = line.x2 - line.x1;
        let delta_y = line.y2 - line.y1;

        assert!((delta_x - delta_y).abs() < 1e-9, "{line:?}");
        assert!(
            linear_gradient_line(
                &crate::diagram_theme::LinearGradient::new(135.0, gradient_stops()).unwrap(),
                None,
            )
            .is_none()
        );
        assert!((normalized_diagonal(40.0, 20.0) - 31.622_776_601_683_793).abs() < 1e-12);

        let plan = root_theme_plan(
            crate::diagram_theme::CanvasSpec::default()
                .with_base(crate::diagram_theme::CanvasPaint::LinearGradient(gradient)),
        );
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let (rooted, _) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &meter)
            .unwrap();
        assert!(
            rooted
                .svg
                .contains(r#"gradientUnits="userSpaceOnUse" x1="5" y1="-5" x2="35" y2="25""#),
            "{}",
            rooted.svg
        );
        assert!(!rooted.svg.contains("gradientTransform"));
    }

    #[test]
    fn ordinary_linear_gradient_without_paint_bounds_remains_residual() {
        let plan = root_theme_plan(crate::diagram_theme::CanvasSpec::default().with_base(
            crate::diagram_theme::CanvasPaint::LinearGradient(
                crate::diagram_theme::LinearGradient::new(135.0, gradient_stops()).unwrap(),
            ),
        ));
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        let (rooted, report) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &meter)
            .unwrap();

        assert!(!rooted.svg.contains("<linearGradient"));
        assert_eq!(
            report.verification(),
            crate::diagram_theme::RootThemeVerification::Unverified
        );
    }

    #[test]
    fn repeating_radial_percentage_period_is_admitted_from_resolved_pixels() {
        let plan = repeating_radial_root_theme_plan(50.0);
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        for rooted in [
            rooted_svg_for_theme_test(),
            rooted_svg_for_theme_test_with_view_box(1.0, 1.0),
            rooted_svg_for_theme_test_with_view_box(200_000.0, 200_000.0),
        ] {
            let (rooted, report) = rooted.apply_root_theme(Some(&plan), &meter).unwrap();
            assert!(!rooted.svg.contains("<radialGradient"));
            assert_eq!(
                report.verification(),
                crate::diagram_theme::RootThemeVerification::Unverified
            );
        }

        let (rooted, report) = rooted_svg_for_theme_test_with_view_box(20.0, 20.0)
            .apply_root_theme(Some(&plan), &meter)
            .unwrap();
        assert!(
            rooted.svg.contains(r#"r="10" spreadMethod="repeat""#),
            "{}",
            rooted.svg
        );
        assert_eq!(
            report.verification(),
            crate::diagram_theme::RootThemeVerification::Verified
        );
    }

    fn computed_context(family: DiagramFamilyId, diagram_id: &str) -> RootViewportContext<'_> {
        RootViewportContext::new(family, diagram_id)
    }

    #[test]
    fn root_plan_rejects_non_finite_viewbox_geometry() {
        let err = computed_context(DiagramFamilyId::VENN, "root-id")
            .plan(RootViewportSpec::mermaid(
                DiagramBounds::from_view_box(-2.0, f64::NAN, -42.5, f64::INFINITY),
                false,
            ))
            .unwrap_err();

        assert!(
            err.to_string().contains("viewBox min-y must be finite"),
            "{err}"
        );
    }

    #[test]
    fn root_theme_terminal_growth_admits_the_exact_svg_budget_without_charging_it() {
        let plan = transparent_root_theme_plan();
        let reference_meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let (expected, _) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &reference_meter)
            .unwrap();
        assert_eq!(reference_meter.projected_svg_bytes(), 0);

        let exact_meter = root_theme_meter(expected.svg.len());
        let (actual, report) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &exact_meter)
            .unwrap();

        assert_eq!(actual.svg, expected.svg);
        assert_eq!(
            report.verification(),
            crate::diagram_theme::RootThemeVerification::Verified
        );
        assert_eq!(exact_meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn repeating_linear_root_canvas_emits_gradient_and_pattern_evidence_atomically() {
        let plan = repeating_linear_root_theme_plan();
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        let (rooted, report) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &meter)
            .unwrap();

        assert!(
            rooted.svg.contains(
                r#"<linearGradient id="info-merman-theme-canvas-base-gradient" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="16" y2="0" spreadMethod="repeat">"#
            ),
            "{}",
            rooted.svg
        );
        assert!(
            rooted
                .svg
                .contains(r#"fill="url(#info-merman-theme-canvas-base-gradient)""#),
            "{}",
            rooted.svg
        );
        assert_eq!(
            report.verification(),
            crate::diagram_theme::RootThemeVerification::Verified
        );
        let applied = report
            .applied_capabilities()
            .collect::<std::collections::BTreeSet<_>>();
        assert!(applied.contains(&ThemeCapability::GradientPaint));
        assert!(applied.contains(&ThemeCapability::PatternPaint));
    }

    #[test]
    fn tiled_radial_root_canvas_emits_one_bounded_tile_without_shape_expansion() {
        let radial = crate::diagram_theme::RadialGradient::new(
            crate::diagram_theme::ThemeLength::percent(50.0),
            crate::diagram_theme::ThemeLength::percent(50.0),
            crate::diagram_theme::ThemeLength::percent(50.0),
            gradient_stops(),
        )
        .unwrap()
        .with_tile_px(20.0, 20.0)
        .unwrap();
        let canvas = crate::diagram_theme::CanvasSpec::default()
            .with_layer(crate::diagram_theme::CanvasLayer::new(
                crate::diagram_theme::CanvasPaint::RadialGradient(radial),
            ))
            .unwrap();
        let plan = root_theme_plan(canvas);
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        let (rooted, report) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &meter)
            .unwrap();

        assert!(
            rooted.svg.contains(
                r#"<radialGradient id="info-merman-theme-canvas-layer-0-gradient" gradientUnits="userSpaceOnUse" cx="10" cy="10" r="10">"#
            ),
            "{}",
            rooted.svg
        );
        assert!(
            rooted.svg.contains(
                r#"<pattern id="info-merman-theme-canvas-layer-0-pattern" patternUnits="userSpaceOnUse" x="0" y="0" width="20" height="20"><rect x="0" y="0" width="20" height="20" fill="url(#info-merman-theme-canvas-layer-0-gradient)"/></pattern>"#
            ),
            "{}",
            rooted.svg
        );
        assert_eq!(
            rooted
                .svg
                .matches("info-merman-theme-canvas-layer-0-pattern")
                .count(),
            2,
            "one definition and one fill reference are sufficient: {}",
            rooted.svg
        );
        assert_eq!(
            report.verification(),
            crate::diagram_theme::RootThemeVerification::Verified
        );
    }

    #[test]
    fn tiled_canvas_is_anchored_to_the_canvas_origin() {
        let gradient = LinearGradient::new(180.0, gradient_stops())
            .unwrap()
            .with_tile_px(40.0, 40.0)
            .unwrap();
        let plan = root_theme_plan(
            crate::diagram_theme::CanvasSpec::default()
                .with_base(CanvasPaint::LinearGradient(gradient)),
        );
        let context = computed_context(DiagramFamilyId::INFO, "canvas-origin");
        let mut out = String::new();
        let document = context
            .write_open(
                &mut out,
                RootViewportSpec::responsive(DiagramBounds::from_view_box(-8.0, 7.0, 160.0, 100.0)),
                RootChrome::new("canvas-origin", "info"),
            )
            .unwrap();
        out.push_str("<g/></svg>");
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let (rooted, report) = document
            .complete(out)
            .unwrap()
            .apply_root_theme(Some(&plan), &meter)
            .unwrap();
        let svg = roxmltree::Document::parse(&rooted.svg).unwrap();
        let pattern = svg
            .descendants()
            .find(|node| node.has_tag_name("pattern"))
            .unwrap();
        assert_eq!(
            pattern.attribute("patternTransform"),
            Some("translate(-8 7)")
        );
        assert_eq!(pattern.attribute("x"), Some("0"));
        assert_eq!(pattern.attribute("y"), Some("0"));
        let tile = pattern
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert_eq!(tile.attribute("x"), Some("0"));
        assert_eq!(tile.attribute("y"), Some("0"));
        assert_eq!(tile.attribute("width"), Some("40"));
        assert_eq!(tile.attribute("height"), Some("40"));
        assert_eq!(
            report.verification(),
            crate::diagram_theme::RootThemeVerification::Verified
        );
    }

    #[test]
    fn repeated_gradient_defs_are_preflighted_against_the_exact_svg_budget() {
        let plan = repeating_linear_root_theme_plan();
        let reference_meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let (expected, _) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &reference_meter)
            .unwrap();

        let exact_meter = root_theme_meter(expected.svg.len());
        let (actual, _) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &exact_meter)
            .unwrap();
        assert_eq!(actual.svg, expected.svg);
        assert_eq!(exact_meter.projected_svg_bytes(), 0);

        let short_meter = root_theme_meter(expected.svg.len() - 1);
        let error = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &short_meter)
            .unwrap_err();
        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert_eq!(short_meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn root_canvas_resource_ids_fail_closed_on_existing_svg_definitions() {
        let plan = repeating_linear_root_theme_plan();
        let mut rooted = rooted_svg_for_theme_test();
        rooted.svg.insert_str(
            rooted.root_open_end,
            r#"<defs><linearGradient id = 'info-merman-theme-canvas-base-gradient'/></defs>"#,
        );
        let expected_scan_work = rooted.svg.len().div_ceil(64);
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );

        let error = rooted.apply_root_theme(Some(&plan), &meter).unwrap_err();
        assert!(
            error.to_string().contains(
                "root theme SVG resource namespace 'info-merman-theme-canvas-' already exists"
            ),
            "{error}"
        );
        assert_eq!(meter.used(), expected_scan_work);
    }

    #[test]
    fn root_canvas_resource_namespace_scan_only_matches_id_attributes() {
        let scope = "info";
        assert!(svg_contains_canvas_resource_namespace(
            r#"<svg><linearGradient id = 'info-merman-theme-canvas-base-gradient'/></svg>"#,
            scope,
        ));
        assert!(!svg_contains_canvas_resource_namespace(
            r#"<svg><g data-note="id = 'info-merman-theme-canvas-base-gradient'" data-id="info-merman-theme-canvas-base-gradient"/></svg>"#,
            scope,
        ));
    }

    #[test]
    fn root_theme_terminal_growth_rejects_short_background_and_prelude_budgets() {
        let plan = transparent_root_theme_plan();
        let rooted = rooted_svg_for_theme_test();
        let edit = crate::svg::pipeline::set_root_background_color(&rooted.svg, "transparent")
            .expect("transparent root background edit");
        assert!(edit.additional_len() > 0);
        let background_limit = rooted.svg.len() + edit.additional_len() - 1;
        let background_meter = root_theme_meter(background_limit);
        let error = rooted
            .apply_root_theme(Some(&plan), &background_meter)
            .unwrap_err();
        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert_eq!(background_meter.projected_svg_bytes(), 0);

        let reference_meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let (expected, _) = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &reference_meter)
            .unwrap();
        assert!(expected.svg.len() > background_limit + 1);
        let prelude_meter = root_theme_meter(expected.svg.len() - 1);
        let error = rooted_svg_for_theme_test()
            .apply_root_theme(Some(&plan), &prelude_meter)
            .unwrap_err();
        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert_eq!(prelude_meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn root_theme_without_a_plan_does_not_consume_terminal_svg_budget() {
        let rooted = rooted_svg_for_theme_test();
        let expected = rooted.svg.clone();
        let meter = root_theme_meter(1);

        let (actual, report) = rooted.apply_root_theme(None, &meter).unwrap();

        assert_eq!(actual.svg, expected);
        assert_eq!(
            report.verification(),
            crate::diagram_theme::RootThemeVerification::NotApplicable
        );
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn root_plan_rejects_finite_geometry_beyond_the_svg_backend_coordinate_cap() {
        let maximum = crate::resources::MAX_SVG_BACKEND_COORDINATE_MAGNITUDE as f64;
        let error = computed_context(DiagramFamilyId::STATE, "root-id")
            .plan(RootViewportSpec::responsive(DiagramBounds::from_view_box(
                maximum, 0.0, 2.0, 10.0,
            )))
            .unwrap_err();

        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected backend coordinate resource limit")
        };
        assert_eq!(
            limit.limit,
            crate::resources::SVG_BACKEND_COORDINATE_MAGNITUDE_HARD_CAP_ID
        );
        assert_eq!(
            limit.max,
            crate::resources::MAX_SVG_BACKEND_COORDINATE_MAGNITUDE
        );
        assert_eq!(limit.actual, limit.max + 2);
    }

    #[test]
    fn root_plan_rejects_finite_max_width_beyond_the_svg_backend_coordinate_cap() {
        let maximum = crate::resources::MAX_SVG_BACKEND_COORDINATE_MAGNITUDE as f64;
        let error = computed_context(DiagramFamilyId::INFO, "root-id")
            .plan(RootViewportSpec::responsive_without_view_box(maximum + 1.0))
            .unwrap_err();

        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected backend coordinate resource limit")
        };
        assert_eq!(
            limit.limit,
            crate::resources::SVG_BACKEND_COORDINATE_MAGNITUDE_HARD_CAP_ID
        );
        assert_eq!(limit.actual, limit.max + 1);
    }

    #[test]
    fn responsive_root_emits_declared_dom_order() {
        let context = computed_context(DiagramFamilyId::JOURNEY, "root-id");
        let extra_attrs = [("preserveAspectRatio", "xMinYMin meet")];
        let mut chrome = RootChrome::new("root-id", "journey");
        chrome.extra_attrs = &extra_attrs;
        chrome.dom = RootDomProfile {
            style_viewbox_order: SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
            responsive_height_placement: RootResponsiveHeightPlacement::AfterExtraAttrs,
            trailing_newline: false,
            ..RootDomProfile::default()
        };
        let mut out = String::new();
        context
            .write_open(
                &mut out,
                RootViewportSpec::responsive(DiagramBounds::from_view_box(-2.0, 0.0, 42.0, 24.0))
                    .with_mermaid_responsive_height(true, 30.0),
                chrome,
            )
            .unwrap();

        assert!(out.starts_with(r#"<svg id="root-id" width="100%""#));
        assert!(out.contains(
            r#"viewBox="-2 0 42 24" style="max-width: 42px; background-color: white;" preserveAspectRatio="xMinYMin meet" height="30""#
        ));
        assert!(out.contains(r#"style="max-width: 42px; background-color: white;""#));
    }

    #[test]
    fn responsive_root_can_omit_viewbox() {
        let context = computed_context(DiagramFamilyId::INFO, "info");
        let mut chrome = RootChrome::new("info", "info");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        context
            .write_open(
                &mut out,
                RootViewportSpec::responsive_without_view_box(400.0),
                chrome,
            )
            .unwrap();

        assert!(out.contains(r#"width="100%""#));
        assert!(out.contains(r#"style="max-width: 400px; background-color: white;""#));
        assert!(!out.contains("viewBox="));
    }

    #[test]
    fn bounded_root_open_admits_the_exact_size_and_rejects_before_excess_retention() {
        let context = computed_context(DiagramFamilyId::INFO, "info");
        let spec = RootViewportSpec::responsive_without_view_box(400.0);
        let mut expected = String::new();
        context
            .write_open(&mut expected, spec, RootChrome::new("info", "info"))
            .unwrap();

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(
                crate::resources::ResourceLimitId::MaxSvgBytes,
                expected.len(),
            )
            .unwrap();
        let exact_meter = crate::resources::OperationWorkMeter::new(exact_policy);
        let mut exact = BoundedSvgOutput::new(&exact_meter);
        context
            .write_open(&mut exact, spec, RootChrome::new("info", "info"))
            .unwrap();
        assert_eq!(exact.finish().unwrap(), expected);

        let short_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(
                crate::resources::ResourceLimitId::MaxSvgBytes,
                expected.len() - 1,
            )
            .unwrap();
        let short_meter = crate::resources::OperationWorkMeter::new(short_policy);
        let mut short = BoundedSvgOutput::new(&short_meter);
        let error = context
            .write_open(&mut short, spec, RootChrome::new("info", "info"))
            .unwrap_err();
        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert!(short.as_str().len() < expected.len());
    }

    #[test]
    fn root_chrome_escapes_every_dynamic_attribute_once() {
        let diagram_id = r#"root" onload="alert(1)&"#;
        let context = computed_context(DiagramFamilyId::INFO, diagram_id);
        let extra_attrs = [("data-note", r#""<&"#)];
        let mut chrome = RootChrome::new(diagram_id, r#"info" aria-hidden="true"#);
        chrome.class = Some(r#"diagram" injected="yes"#);
        chrome.extra_attrs = &extra_attrs;
        chrome.aria_labelledby = Some(r#"title" autofocus="true"#);
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        context
            .write_open(
                &mut out,
                RootViewportSpec::responsive_without_view_box(400.0),
                chrome,
            )
            .unwrap();

        assert!(out.contains(r#"id="root&quot; onload=&quot;alert(1)&amp;""#));
        assert!(out.contains(r#"class="diagram&quot; injected=&quot;yes""#));
        assert!(out.contains(r#"data-note="&quot;&lt;&amp;""#));
        assert!(out.contains(r#"aria-roledescription="info&quot; aria-hidden=&quot;true""#));
        assert!(out.contains(r#"aria-labelledby="title&quot; autofocus=&quot;true""#));
        assert!(!out.contains(r#" onload="alert(1)""#));
        assert!(!out.contains(r#" injected="yes""#));
    }

    #[test]
    fn completed_root_document_carries_family_provenance() {
        let context = computed_context(DiagramFamilyId::INFO, "root-id");
        let mut chrome = RootChrome::new("root-id", "info");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .write_open(
                &mut out,
                RootViewportSpec::responsive_without_view_box(400.0),
                chrome,
            )
            .unwrap();
        out.push_str("</svg>");

        let svg = document
            .complete(out)
            .unwrap()
            .into_string_for(DiagramFamilyId::INFO)
            .unwrap();

        assert!(svg.starts_with(r#"<svg id="root-id""#));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn completed_root_document_rejects_root_attribute_mutation() {
        let context = computed_context(DiagramFamilyId::INFO, "root-id");
        let mut chrome = RootChrome::new("root-id", "info");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .write_open(
                &mut out,
                RootViewportSpec::responsive_without_view_box(400.0),
                chrome,
            )
            .unwrap();
        out = out.replacen(r#"id="root-id""#, r#"id="forged""#, 1);
        out.push_str("</svg>");

        let error = document.complete(out).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("mutated operation-owned SVG root attributes")
        );
    }

    #[test]
    fn completed_root_document_rejects_incomplete_and_wrong_family_outputs() {
        let context = computed_context(DiagramFamilyId::INFO, "root-id");
        let mut chrome = RootChrome::new("root-id", "info");
        chrome.dom.trailing_newline = false;
        let mut incomplete = String::new();
        let incomplete_document = context
            .write_open(
                &mut incomplete,
                RootViewportSpec::responsive_without_view_box(400.0),
                chrome,
            )
            .unwrap();
        let error = incomplete_document.complete(incomplete).unwrap_err();
        assert!(error.to_string().contains("incomplete SVG root document"));

        let mut complete = String::new();
        let complete_document = context
            .write_open(
                &mut complete,
                RootViewportSpec::responsive_without_view_box(400.0),
                RootChrome::new("root-id", "info"),
            )
            .unwrap();
        complete.push_str("</svg>");
        let error = complete_document
            .complete(complete)
            .unwrap()
            .into_string_for(DiagramFamilyId::VENN)
            .unwrap_err();
        assert!(error.to_string().contains("was returned for venn"));
    }

    #[test]
    fn deferred_document_finalizes_computed_root_without_leaking_markers() {
        let diagram_id = "stress_state_accdescr_block_and_markdown_labels_049";
        let context = RootViewportContext::new(DiagramFamilyId::STATE, diagram_id);
        let mut chrome = RootChrome::new(diagram_id, "stateDiagram");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .begin_document(&mut out, DeferredRootSpec::responsive(), chrome)
            .unwrap();
        out.push_str("<g/></svg>");
        let document = context
            .finish_document(
                &mut out,
                document,
                RootViewportSpec::responsive(DiagramBounds::from_view_box(0.0, 0.0, 10.0, 10.0))
                    .with_max_width(RootMaxWidth::CssSixSignificant(10.0)),
            )
            .unwrap();

        let rooted = document.complete(out.clone()).unwrap();
        assert_eq!(rooted.family, DiagramFamilyId::STATE);
        assert!(out.contains(r#"viewBox="0 0 10 10""#));
        assert!(out.contains("max-width: 10px"));
        assert!(!out.contains("__MERMAN_ROOT_"));
    }

    #[test]
    fn bounded_deferred_root_admits_the_exact_final_size() {
        let diagram_id = "state";
        let context = RootViewportContext::new(DiagramFamilyId::STATE, diagram_id);
        let final_spec =
            RootViewportSpec::responsive(DiagramBounds::from_view_box(0.0, 0.0, 10.0, 10.0))
                .with_max_width(RootMaxWidth::CssSixSignificant(10.0));

        let mut expected = String::new();
        let expected_document = context
            .begin_document(
                &mut expected,
                DeferredRootSpec::responsive(),
                RootChrome::new(diagram_id, "stateDiagram"),
            )
            .unwrap();
        expected.push_str("<g/>");
        let expected_document = context
            .finish_document(&mut expected, expected_document, final_spec)
            .unwrap();
        expected.push_str("</svg>");
        let expected = expected_document.complete(expected).unwrap().svg;

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(
                crate::resources::ResourceLimitId::MaxSvgBytes,
                expected.len(),
            )
            .unwrap();
        let exact_meter = crate::resources::OperationWorkMeter::new(exact_policy);
        let mut exact = BoundedSvgOutput::new(&exact_meter);
        let exact_document = context
            .begin_document(
                &mut exact,
                DeferredRootSpec::responsive(),
                RootChrome::new(diagram_id, "stateDiagram"),
            )
            .unwrap();
        exact.push_str("<g/>");
        let exact_document = context
            .finish_document(&mut exact, exact_document, final_spec)
            .unwrap();
        exact.push_str("</svg>");
        let exact = exact.finish().unwrap();
        assert_eq!(exact_document.complete(exact).unwrap().svg, expected);

        let short_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(
                crate::resources::ResourceLimitId::MaxSvgBytes,
                expected.len() - 1,
            )
            .unwrap();
        let short_meter = crate::resources::OperationWorkMeter::new(short_policy);
        let mut short = BoundedSvgOutput::new(&short_meter);
        let short_document = context
            .begin_document(
                &mut short,
                DeferredRootSpec::responsive(),
                RootChrome::new(diagram_id, "stateDiagram"),
            )
            .unwrap();
        short.push_str("<g/>");
        context
            .finish_document(&mut short, short_document, final_spec)
            .unwrap();
        short.push_str("</svg>");
        let error = short.finish().unwrap_err();
        assert!(matches!(error, Error::ResourceLimitExceeded(_)));
        assert!(short_meter.projected_svg_bytes() == 0);
    }

    #[test]
    fn deferred_document_tracks_root_attributes_when_a_valid_id_matches_markers() {
        let diagram_id =
            "valid___MERMAN_ROOT_VIEW_BOX_____MERMAN_ROOT_MAX_WIDTH___diagram".to_string();
        let context = RootViewportContext::new(DiagramFamilyId::STATE, diagram_id.as_str());
        let mut chrome = RootChrome::new(diagram_id.as_str(), "stateDiagram");
        let properties = [
            ("--font-family", "\"Arial\", sans-serif"),
            ("--marker-text", "__MERMAN_ROOT_MAX_WIDTH__"),
        ];
        chrome.custom_properties = &properties;
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .begin_document(&mut out, DeferredRootSpec::responsive(), chrome)
            .unwrap();
        out.push_str("</svg>");
        let document = context
            .finish_document(
                &mut out,
                document,
                RootViewportSpec::responsive(DiagramBounds::from_view_box(1.0, 2.0, 30.0, 40.0))
                    .with_max_width(RootMaxWidth::CssSixSignificant(30.0)),
            )
            .unwrap();

        let rooted = document.complete(out).unwrap();
        let document = roxmltree::Document::parse(&rooted).unwrap();
        let root = document.root_element();
        assert_eq!(root.attribute("id"), Some(diagram_id.as_str()));
        assert_eq!(root.attribute("viewBox"), Some("1 2 30 40"));
        assert_eq!(
            root.attribute("style"),
            Some(
                "max-width: 30px; background-color: white; --font-family: \"Arial\", sans-serif; --marker-text: __MERMAN_ROOT_MAX_WIDTH__;"
            )
        );
    }

    #[test]
    fn deferred_document_rejects_prefix_mutation_instead_of_patching_wrong_range() {
        let context = computed_context(DiagramFamilyId::STATE, "state");
        let mut chrome = RootChrome::new("state", "stateDiagram");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .begin_document(&mut out, DeferredRootSpec::responsive(), chrome)
            .unwrap();
        out.insert_str(0, "prefix");

        let error = context
            .finish_document(
                &mut out,
                document,
                RootViewportSpec::responsive(DiagramBounds::from_view_box(0.0, 0.0, 10.0, 10.0)),
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("mutated before viewport finalize")
        );
    }

    #[test]
    fn deferred_document_rejects_untracked_root_attribute_insertion() {
        let context = computed_context(DiagramFamilyId::STATE, "state");
        let mut chrome = RootChrome::new("state", "stateDiagram");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .begin_document(&mut out, DeferredRootSpec::responsive(), chrome)
            .unwrap();
        let root_open_end = out.rfind('>').expect("root open delimiter");
        out.insert_str(root_open_end, r#" data-forged="true""#);

        let error = context
            .finish_document(
                &mut out,
                document,
                RootViewportSpec::responsive(DiagramBounds::from_view_box(0.0, 0.0, 10.0, 10.0)),
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("mutated before viewport finalize")
        );
    }

    #[test]
    fn deferred_document_rejects_same_length_root_tail_mutation() {
        let context = computed_context(DiagramFamilyId::STATE, "state");
        let mut chrome = RootChrome::new("state", "stateDiagram");
        chrome.dom.trailing_newline = false;
        let mut out = String::new();
        let document = context
            .begin_document(&mut out, DeferredRootSpec::responsive(), chrome)
            .unwrap();
        let start = out
            .find("stateDiagram")
            .expect("root aria role description");
        out.replace_range(start..start + "stateDiagram".len(), "stateDiagrax");

        let error = context
            .finish_document(
                &mut out,
                document,
                RootViewportSpec::responsive(DiagramBounds::from_view_box(0.0, 0.0, 10.0, 10.0)),
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("mutated before viewport finalize")
        );
    }

    #[test]
    fn css_max_width_uses_mermaid_six_significant_digit_format() {
        assert_eq!(format_css_max_width(1184.88), "1184.88");
        assert_eq!(format_css_max_width(2019.2), "2019.2");
        assert_eq!(format_css_max_width(658.6762084960938), "658.676");
    }

    #[test]
    fn extents_apply_non_negative_padding_and_support_large_negative_origins() {
        let bounds = DiagramBounds::from_extents(-1_000_000.0, -20.0, 50.0, 80.0, 8.0);
        assert_eq!(
            bounds,
            DiagramBounds::from_view_box(-1_000_008.0, -28.0, 1_000_066.0, 116.0)
        );
    }

    #[test]
    fn root_plan_preserves_f64_get_bbox_extents_and_padding() {
        let (min_x, min_y) = (1.123_456_789, 2.123_456_789);
        let (max_x, max_y) = (111.987_654_321, 222.987_654_321);
        let padding = 40.0;
        let bounds = DiagramBounds::from_extents(min_x, min_y, max_x, max_y, padding);
        let plan = computed_context(DiagramFamilyId::ARCHITECTURE, "architecture")
            .plan(RootViewportSpec::responsive(bounds))
            .unwrap();

        assert_eq!(
            plan.view_box(),
            Some(ViewBox::new(
                min_x - padding,
                min_y - padding,
                max_x - min_x + 2.0 * padding,
                max_y - min_y + 2.0 * padding,
            ))
        );
    }
}
