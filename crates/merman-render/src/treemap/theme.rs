use std::sync::Arc;

use merman_model::OperationPhase;
use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::DataPalette;

use crate::model::TreemapDiagramLayout;
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation};
use crate::svg::PreparedCommonCss;
use crate::theme::{Color, FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags};

use super::css_binding::TreemapCssBinding;

mod font_size;
mod node_visual;

use font_size::TreemapInlineFontSize;
use node_visual::TreemapNodeVisual;

pub(crate) struct TreemapThemeAdapter;

impl FamilyThemeAdapter for TreemapThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::TREEMAP;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::CONTAINER)
        .union(ThemeCapabilityFlags::DATA_FILL)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::PATTERN_ENCODING)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug, Clone, Default)]
enum TreemapSourceTextOverride<T> {
    #[default]
    Generated,
    Inherited,
    Value(T),
    Unverified,
}

#[derive(Debug, Clone, Default)]
struct TreemapSourceTextStyle {
    visual: node_visual::TreemapNodeVisual,
    font_family: TreemapSourceTextOverride<Box<str>>,
    font_size_px: TreemapSourceTextOverride<f64>,
    inline_font_size: TreemapInlineFontSize,
    font_weight: TreemapSourceTextOverride<Box<str>>,
    font_style: TreemapSourceTextOverride<Box<str>>,
}

impl TreemapSourceTextStyle {
    fn resolve(
        &self,
        root_font_family_css: &str,
        root_font_size_px: f64,
        generated_font_size_px: f64,
        generated_font_weight: Option<&str>,
        generated_font_style: Option<&str>,
    ) -> TreemapResolvedTextStyle {
        let family = match &self.font_family {
            TreemapSourceTextOverride::Value(font_family) => font_family.as_ref(),
            TreemapSourceTextOverride::Generated
            | TreemapSourceTextOverride::Inherited
            | TreemapSourceTextOverride::Unverified => root_font_family_css,
        };
        let font_size = match &self.font_size_px {
            TreemapSourceTextOverride::Generated => generated_font_size_px,
            TreemapSourceTextOverride::Inherited => root_font_size_px,
            TreemapSourceTextOverride::Value(font_size_px) => *font_size_px,
            TreemapSourceTextOverride::Unverified => generated_font_size_px,
        };
        let font_weight = match &self.font_weight {
            TreemapSourceTextOverride::Generated => generated_font_weight.map(str::to_owned),
            TreemapSourceTextOverride::Inherited => None,
            TreemapSourceTextOverride::Value(font_weight) => Some(font_weight.to_string()),
            TreemapSourceTextOverride::Unverified => generated_font_weight.map(str::to_owned),
        };
        let font_style = match &self.font_style {
            TreemapSourceTextOverride::Generated => generated_font_style.map(str::to_owned),
            TreemapSourceTextOverride::Inherited => None,
            TreemapSourceTextOverride::Value(font_style) => Some(font_style.to_string()),
            TreemapSourceTextOverride::Unverified => generated_font_style.map(str::to_owned),
        };

        TreemapResolvedTextStyle {
            style: crate::text::TextStyle {
                font_family: Some(family.to_owned()),
                font_size,
                font_weight,
                font_style,
            },
            inline_font_size: self.inline_font_size,
        }
    }

    fn retained_bytes(&self) -> Option<usize> {
        fn override_bytes(value: &TreemapSourceTextOverride<Box<str>>) -> usize {
            match value {
                TreemapSourceTextOverride::Value(value) => value.len(),
                TreemapSourceTextOverride::Generated
                | TreemapSourceTextOverride::Inherited
                | TreemapSourceTextOverride::Unverified => 0,
            }
        }

        std::mem::size_of::<Self>()
            .checked_add(override_bytes(&self.font_family))?
            .checked_add(override_bytes(&self.font_weight))?
            .checked_add(override_bytes(&self.font_style))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TreemapResolvedTextStyle {
    style: crate::text::TextStyle,
    inline_font_size: TreemapInlineFontSize,
}

impl TreemapResolvedTextStyle {
    pub(crate) const fn style(&self) -> &crate::text::TextStyle {
        &self.style
    }

    pub(crate) fn with_font_size_px(&self, font_size_px: f64) -> Self {
        let mut resolved = self.clone();
        resolved.style.font_size = font_size_px;
        resolved.inline_font_size = TreemapInlineFontSize::Generated;
        resolved
    }

    pub(crate) fn fitting_font_size(&self) -> Option<f64> {
        self.inline_font_size.integer(self.style.font_size)
    }

    pub(crate) fn inline_font_size_number(&self) -> Option<f64> {
        self.inline_font_size.number(self.style.font_size)
    }

    pub(crate) fn accepts_font_size_assignment(&self, size: f64) -> bool {
        size.is_finite() && size >= 0.0 && self.inline_font_size.accepts_px_assignment()
    }
}

#[derive(Debug)]
pub(crate) struct TreemapStyle {
    css: TreemapCssBinding,
    title_font_size_px: f64,
    pub(crate) native_radius: Option<f64>,
    pub(crate) native_base_size: Option<f64>,
    pub(crate) native_patterns: Option<DataPalette>,
    section_source_styles: Box<[TreemapSourceTextStyle]>,
    leaf_source_styles: Box<[TreemapSourceTextStyle]>,
    _retained_reservation: PreparedTextRetainedReservation,
    _visual_retained_reservation: Option<PreparedTextRetainedReservation>,
}

impl TreemapStyle {
    pub(crate) fn resolve(
        config: &PresentationConfig,
        layout: &TreemapDiagramLayout,
        native: Option<&FamilyTheme>,
        background: Option<Color>,
        work: &Arc<OperationWorkMeter>,
    ) -> crate::Result<Self> {
        let font = crate::config::config_font_family_css(config);
        let mut css = TreemapCssBinding::new(config.as_value(), &font)?;
        let root_font_size_px = css.root_font_size_px();
        let mut title_font_size_px = 14.0;
        if layout
            .title
            .as_deref()
            .is_some_and(|title| !title.trim().is_empty())
        {
            work.charge(1usize.saturating_add(css.title_font_size.len().div_ceil(64)))?;
            let declaration = format!("font-size:{}", css.title_font_size);
            title_font_size_px = crate::mermaid_style::parse_style_declaration(&declaration)
                .and_then(|declaration| {
                    if declaration.font_size_is_literal_zero() {
                        return Some(0.0);
                    }
                    if declaration.analysis().depends_on_root_font_size() {
                        return None;
                    }
                    declaration.resolve_font_size_px(
                        crate::mermaid_style::CssFontSizeContext::uniform(root_font_size_px),
                    )
                })
                .unwrap_or(14.0);
        }
        let native_base_size = native.map(|theme| theme.typography.base_size_px);
        if let Some(theme) = native {
            let typography = &theme.typography;
            let size = typography.base_size_px;
            let font = crate::theme::font_family_css(typography.stack(typography.body_family));
            css.common = PreparedCommonCss::with_resolved_typography(
                config.as_value(),
                font,
                format!("{size}px"),
            )
            .with_text_color(&theme.roles.text_primary.to_string());
            css.set_root_font_size_px(size);
            title_font_size_px = size * 1.35;
            css.title_font_size = format!("{title_font_size_px}px");
            css.title_color = theme.roles.text_title.to_string();
        }

        let font_bytes = css.common.font_family().len();
        let retained_upper_bound =
            treemap_source_style_retained_upper_bound(layout, font_bytes, work)?;
        let mut retained_reservation =
            work.reserve_prepared_text_retained_bytes(retained_upper_bound)?;
        let mut section_source_styles = Vec::with_capacity(layout.sections.len());
        for section in &layout.sections {
            section_source_styles.push(resolve_treemap_source_text_style(
                section.css_compiled_styles.as_deref().unwrap_or_default(),
                root_font_size_px,
                work,
            )?);
        }
        let mut leaf_source_styles = Vec::with_capacity(layout.leaves.len());
        for leaf in &layout.leaves {
            leaf_source_styles.push(resolve_treemap_source_text_style(
                leaf.css_compiled_styles.as_deref().unwrap_or_default(),
                root_font_size_px,
                work,
            )?);
        }
        let retained_bytes = section_source_styles
            .iter()
            .chain(&leaf_source_styles)
            .try_fold(font_bytes, |retained, style| {
                retained.checked_add(style.retained_bytes()?)
            })
            .ok_or_else(|| crate::Error::from(work.arithmetic_overflow()))?;
        retained_reservation.reconcile_downward(retained_bytes);
        Self {
            css,
            title_font_size_px,
            native_radius: native.map(|theme| theme.form.radius),
            native_base_size,
            native_patterns: native
                .filter(|theme| theme.data.pattern_encoding)
                .map(|theme| theme.data.clone()),
            section_source_styles: section_source_styles.into_boxed_slice(),
            leaf_source_styles: leaf_source_styles.into_boxed_slice(),
            _retained_reservation: retained_reservation,
            _visual_retained_reservation: None,
        }
        .prepare_node_visuals(layout, work, native, background)
    }

    fn font_family_css(&self) -> &str {
        self.css.common.font_family()
    }

    pub(crate) fn css_binding(&self) -> &TreemapCssBinding {
        &self.css
    }

    pub(crate) fn section_visual(&self, index: usize) -> Option<&TreemapNodeVisual> {
        self.section_source_styles
            .get(index)
            .map(|source| &source.visual)
    }

    pub(crate) fn leaf_visual(&self, index: usize) -> Option<&TreemapNodeVisual> {
        self.leaf_source_styles
            .get(index)
            .map(|source| &source.visual)
    }

    pub(crate) fn title_text_style(&self) -> crate::text::TextStyle {
        crate::text::TextStyle {
            font_family: Some(self.font_family_css().to_owned()),
            font_size: self.title_font_size_px,
            ..crate::text::TextStyle::default()
        }
    }

    pub(crate) fn section_text_style(
        &self,
        index: usize,
        generated_font_size_px: f64,
        generated_font_weight: Option<&str>,
        generated_font_style: Option<&str>,
    ) -> TreemapResolvedTextStyle {
        self.section_source_styles
            .get(index)
            .map(|style| {
                style.resolve(
                    self.font_family_css(),
                    self.css.root_font_size_px(),
                    generated_font_size_px,
                    generated_font_weight,
                    generated_font_style,
                )
            })
            .unwrap_or_else(|| TreemapResolvedTextStyle {
                style: crate::text::TextStyle {
                    font_family: Some(self.font_family_css().to_string()),
                    font_size: generated_font_size_px,
                    font_weight: generated_font_weight.map(str::to_owned),
                    font_style: generated_font_style.map(str::to_owned),
                },
                inline_font_size: TreemapInlineFontSize::Unverified,
            })
    }

    pub(crate) fn leaf_text_style(
        &self,
        index: usize,
        generated_font_size_px: f64,
    ) -> TreemapResolvedTextStyle {
        self.leaf_source_styles
            .get(index)
            .map(|style| {
                style.resolve(
                    self.font_family_css(),
                    self.css.root_font_size_px(),
                    generated_font_size_px,
                    None,
                    None,
                )
            })
            .unwrap_or_else(|| TreemapResolvedTextStyle {
                style: crate::text::TextStyle {
                    font_family: Some(self.font_family_css().to_string()),
                    font_size: generated_font_size_px,
                    font_weight: None,
                    font_style: None,
                },
                inline_font_size: TreemapInlineFontSize::Unverified,
            })
    }
}

fn treemap_source_style_retained_upper_bound(
    layout: &TreemapDiagramLayout,
    root_font_family_bytes: usize,
    work_meter: &OperationWorkMeter,
) -> crate::Result<usize> {
    let item_count = layout
        .sections
        .len()
        .checked_add(layout.leaves.len())
        .ok_or_else(|| crate::Error::from(work_meter.arithmetic_overflow()))?;
    let mut retained_bytes = item_count
        .checked_mul(std::mem::size_of::<TreemapSourceTextStyle>())
        .and_then(|bytes| bytes.checked_add(root_font_family_bytes))
        .ok_or_else(|| crate::Error::from(work_meter.arithmetic_overflow()))?;

    for declaration_lists in layout
        .sections
        .iter()
        .map(|section| section.css_compiled_styles.as_deref().unwrap_or_default())
        .chain(
            layout
                .leaves
                .iter()
                .map(|leaf| leaf.css_compiled_styles.as_deref().unwrap_or_default()),
        )
    {
        work_meter.charge(1)?;
        for declaration_list in declaration_lists {
            work_meter.charge(1)?;
            retained_bytes = retained_bytes
                .checked_add(declaration_list.len())
                .ok_or_else(|| crate::Error::from(work_meter.arithmetic_overflow()))?;
        }
    }

    Ok(retained_bytes)
}

fn resolve_treemap_source_text_style(
    declaration_lists: &[String],
    root_font_size_px: f64,
    work_meter: &OperationWorkMeter,
) -> crate::Result<TreemapSourceTextStyle> {
    let mut style = TreemapSourceTextStyle::default();
    let font_size_context =
        crate::mermaid_style::CssFontSizeContext::new(root_font_size_px, root_font_size_px);

    for declaration_list in declaration_lists {
        work_meter.charge(1usize.saturating_add(declaration_list.len().div_ceil(64)))?;
        let mut checkpoint = || work_meter.checkpoint(OperationPhase::Layout);
        crate::mermaid_style::visit_style_declaration_boundaries_with_checkpoints(
            declaration_list,
            &mut checkpoint,
            |boundary| {
                work_meter.charge(1)?;
                let raw = boundary.raw();
                let Some(declaration) = crate::mermaid_style::parse_style_declaration(raw) else {
                    let property = raw
                        .split_once(':')
                        .map(|(property, _)| property.trim())
                        .unwrap_or_else(|| raw.trim());
                    match property {
                        "font-family" => style.font_family = TreemapSourceTextOverride::Unverified,
                        "font-size" => {
                            style.font_size_px = TreemapSourceTextOverride::Unverified;
                            style.inline_font_size = TreemapInlineFontSize::Unverified;
                        }
                        "font-weight" => style.font_weight = TreemapSourceTextOverride::Unverified,
                        "font-style" => style.font_style = TreemapSourceTextOverride::Unverified,
                        _ => {}
                    }
                    return Ok(true);
                };

                // Mermaid's Treemap style splitter uses exact, case-sensitive keys before the
                // declarations reach the browser. Keep that boundary instead of treating this as
                // a general CSS cascade.
                match declaration.property_source().trim() {
                    "font-family" => {
                        style.font_family = if declaration.important() {
                            // Upstream appends its own `!important`; accepting an authored one
                            // would produce an invalid doubled priority token.
                            TreemapSourceTextOverride::Unverified
                        } else if declaration.inherits_property_value() {
                            TreemapSourceTextOverride::Inherited
                        } else if crate::mermaid_style::is_static_css_font_family_list(
                            declaration.value(),
                        ) {
                            TreemapSourceTextOverride::Value(declaration.value().into())
                        } else {
                            TreemapSourceTextOverride::Unverified
                        };
                    }
                    "font-size" => {
                        if !declaration
                            .svg_number_or_px()
                            .is_some_and(|size| size.is_finite() && size >= 0.0)
                            && crate::mermaid_style::css_font_size_value_override(
                                declaration.source_value(),
                            )
                            .is_none()
                        {
                            // Mermaid already folded this exact property to its final value.
                            // Invalid CSS leaves the generated inline size in effect.
                            style.font_size_px = TreemapSourceTextOverride::Generated;
                            style.inline_font_size = TreemapInlineFontSize::Generated;
                            return Ok(true);
                        }
                        style.font_size_px = if declaration.important() {
                            TreemapSourceTextOverride::Unverified
                        } else if declaration.inherits_property_value() {
                            TreemapSourceTextOverride::Inherited
                        } else if crate::mermaid_style::css_font_size_is_zero(declaration.value()) {
                            // The shared positive-size resolver intentionally excludes zero.
                            // Treemap's fitting pass must retain it so the label stays hidden.
                            TreemapSourceTextOverride::Value(0.0)
                        } else if let Some(font_size_px) =
                            declaration.resolve_font_size_px(font_size_context)
                        {
                            if font_size_px.is_finite() && font_size_px >= 0.0 {
                                TreemapSourceTextOverride::Value(font_size_px)
                            } else {
                                TreemapSourceTextOverride::Unverified
                            }
                        } else {
                            TreemapSourceTextOverride::Unverified
                        };
                        style.inline_font_size = if matches!(
                            style.font_size_px,
                            TreemapSourceTextOverride::Unverified
                        ) {
                            TreemapInlineFontSize::Unverified
                        } else {
                            TreemapInlineFontSize::authored(&declaration)
                        };
                    }
                    property @ ("font-weight" | "font-style") => {
                        let (target, supported) = if property == "font-weight" {
                            (
                                &mut style.font_weight,
                                crate::mermaid_style::is_supported_css_font_weight_value(
                                    declaration.value(),
                                ),
                            )
                        } else {
                            (
                                &mut style.font_style,
                                crate::mermaid_style::is_supported_css_font_style_value(
                                    declaration.value(),
                                ),
                            )
                        };
                        *target = if declaration.important() {
                            TreemapSourceTextOverride::Unverified
                        } else if declaration.inherits_property_value() {
                            TreemapSourceTextOverride::Inherited
                        } else if supported {
                            TreemapSourceTextOverride::Value(declaration.value().into())
                        } else {
                            // Shared longhand semantics resolve initial weight/style resets.
                            // Keep the authored value in the separate emitted style suffix.
                            let inherited = crate::text::TextStyle::default();
                            let mut cascade = crate::mermaid_style::TextStyleCascade::new(
                                &inherited,
                                font_size_context,
                            );
                            cascade.observe_declaration(&declaration);
                            let resolved = cascade.finish();
                            let reset = if property == "font-weight" {
                                resolved.font_weight.as_deref()
                            } else {
                                resolved.font_style.as_deref()
                            };
                            reset.map_or(TreemapSourceTextOverride::Unverified, |value| {
                                TreemapSourceTextOverride::Value(value.into())
                            })
                        };
                    }
                    _ => {}
                }
                Ok(true)
            },
        )?;
    }
    Ok(style)
}
