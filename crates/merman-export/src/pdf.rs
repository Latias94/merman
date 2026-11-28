//! Bounded preparation and encoding for vector PDF.

use merman_model::OperationControl;
use merman_render::svg::ResvgCompatibleSvg;

use crate::conversion::{
    plan_embedded_data_resource_bytes, plan_embedded_images, plan_pdf_filter_images,
    plan_svg_conversion, validate_embedded_image_limit,
};
use crate::operation::{
    RECURSIVE_SVG_BACKEND_STACK_BYTES, export_checkpoint, run_recursive_svg_backend,
};
use crate::resource_metadata::DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS;
use crate::viewport::parse_root_svg_metadata;
use crate::{
    EmbeddedImageLimit, EmbeddedImagePlan, ExportError, ExportFontPlan, ExportRgbaColor,
    ParsedExportSvg, Result, SvgConversionLimits, SvgConversionPlan, SvgExportFonts,
    parse_export_color, parse_svg_tree,
};
#[cfg(feature = "native-fonts")]
use crate::{NativeFontContext, NativeFontError};

const PDF_POINTS_PER_CSS_PIXEL: f32 = 72.0 / 96.0;

/// Controls how vector content is placed on a PDF page.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PdfPagePolicy {
    /// Use the SVG's intrinsic dimensions as the PDF page dimensions.
    #[default]
    FitSvg,
    /// Scale the SVG uniformly to fit a fixed page and center it without cropping.
    Fixed { width_pt: f32, height_pt: f32 },
    /// Match browser PDF sizing: constrain responsive SVG width in CSS pixels, then convert
    /// CSS pixels to PDF points at 96 CSS pixels per inch.
    FitCssWidth { max_width_px: f32 },
}

/// Aggregate pixel budget for localized filter images retained in a vector PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfFilterImageLimit {
    pub max_total_pixels: Option<u64>,
}

impl PdfFilterImageLimit {
    pub const fn new(max_total_pixels: Option<u64>) -> Self {
        Self { max_total_pixels }
    }

    pub const fn default_safe() -> Self {
        Self {
            max_total_pixels: Some(DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS),
        }
    }

    pub const fn unbounded() -> Self {
        Self {
            max_total_pixels: None,
        }
    }
}

impl Default for PdfFilterImageLimit {
    fn default() -> Self {
        Self::default_safe()
    }
}

/// Vector PDF conversion options, intentionally separate from pixel allocation limits.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfOptions {
    /// PDF page sizing and content placement policy.
    pub page_policy: PdfPagePolicy,
    /// Optional solid paint behind SVG content on the PDF page.
    pub page_paint: Option<String>,
    /// Requested sampling scale for SVG filters embedded in the PDF.
    pub filter_scale: f32,
    /// Aggregate budget for localized filter images retained in the PDF.
    pub filter_image_limit: PdfFilterImageLimit,
    /// Header-derived pixel budget for embedded PNG/JPEG/GIF/WebP images.
    pub embedded_image_limit: EmbeddedImageLimit,
    /// Structural budget shared with PNG/JPEG conversion.
    pub conversion_limits: SvgConversionLimits,
    #[cfg(feature = "native-fonts")]
    pub font_context: Option<NativeFontContext>,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_policy: PdfPagePolicy::FitSvg,
            page_paint: None,
            filter_scale: 4.0,
            filter_image_limit: PdfFilterImageLimit::default(),
            embedded_image_limit: EmbeddedImageLimit::default(),
            conversion_limits: SvgConversionLimits::default(),
            #[cfg(feature = "native-fonts")]
            font_context: None,
        }
    }
}

impl PdfOptions {
    #[cfg(feature = "native-fonts")]
    pub fn with_font_context(mut self, font_context: NativeFontContext) -> Self {
        self.font_context = Some(font_context);
        self
    }

    pub fn with_page_policy(mut self, page_policy: PdfPagePolicy) -> Self {
        self.page_policy = page_policy;
        self
    }

    pub fn with_page_paint(mut self, page_paint: impl Into<String>) -> Self {
        self.page_paint = Some(page_paint.into());
        self
    }

    pub fn with_filter_scale(mut self, filter_scale: f32) -> Self {
        self.filter_scale = filter_scale;
        self
    }

    pub fn with_filter_image_limit(mut self, filter_image_limit: PdfFilterImageLimit) -> Self {
        self.filter_image_limit = filter_image_limit;
        self
    }

    pub fn with_unbounded_filter_images(mut self) -> Self {
        self.filter_image_limit = PdfFilterImageLimit::unbounded();
        self
    }

    pub fn with_embedded_image_limit(mut self, embedded_image_limit: EmbeddedImageLimit) -> Self {
        self.embedded_image_limit = embedded_image_limit;
        self
    }

    pub fn with_conversion_limits(mut self, conversion_limits: SvgConversionLimits) -> Self {
        self.conversion_limits = conversion_limits;
        self
    }
}

/// Plan for localized SVG filter images embedded in a vector PDF.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfFilterImagePlan {
    pub filtered_groups: usize,
    pub requested_scale: f32,
    pub effective_scale: f32,
    pub requested_image_pixels: u64,
    pub effective_image_pixels: u64,
    pub limited: bool,
}

/// PDF page geometry frozen before filter planning and encoding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfPagePlan {
    page_width_pt: f32,
    page_height_pt: f32,
    drawing_width_pt: f32,
    drawing_height_pt: f32,
    offset_x_pt: f32,
    offset_y_pt: f32,
}

impl PdfPagePlan {
    pub const fn page_width_pt(self) -> f32 {
        self.page_width_pt
    }

    pub const fn page_height_pt(self) -> f32 {
        self.page_height_pt
    }

    pub const fn drawing_width_pt(self) -> f32 {
        self.drawing_width_pt
    }

    pub const fn drawing_height_pt(self) -> f32 {
        self.drawing_height_pt
    }

    pub const fn offset_x_pt(self) -> f32 {
        self.offset_x_pt
    }

    pub const fn offset_y_pt(self) -> f32 {
        self.offset_y_pt
    }
}

/// Frozen resource, page, filter, font, and compositing evidence for one PDF export.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfExportReport {
    directional_filter_transform_residual: bool,
    implicit_viewport_clipping_residual: bool,
    fallback_css_residual: bool,
    page: PdfPagePlan,
    filters: PdfFilterImagePlan,
    embedded_images: EmbeddedImagePlan,
    conversion: SvgConversionPlan,
    fonts: ExportFontPlan,
    page_paint: Option<ExportRgbaColor>,
}

impl PdfExportReport {
    /// Returns whether offsets inside a localized filter image may paint in a different direction.
    ///
    /// Vector ancestor transforms remain supported. A transform on the rasterized group or its
    /// descendants can expose the raster backend's offset limitation. Best-effort export still
    /// produces the diagram and reports this paint-quality residual.
    pub const fn directional_filter_transform_residual(self) -> bool {
        self.directional_filter_transform_residual
    }

    /// Returns whether a dimensionless SVG's derived PDF viewport can clip its layer bounds.
    ///
    /// This reports best-effort output without changing the sealed SVG. An explicit viewBox or
    /// width and height define a caller-owned viewport and do not produce this residual.
    pub const fn implicit_viewport_clipping_residual(self) -> bool {
        self.implicit_viewport_clipping_residual
    }

    /// Returns whether converted HTML label styles can differ in the native CSS backend.
    ///
    /// This quality observation is retained from the sealed SVG's terminal validation. It does
    /// not authorize resources, relax SVG safety, or prevent best-effort PDF encoding.
    pub const fn fallback_css_residual(self) -> bool {
        self.fallback_css_residual
    }

    pub const fn page(self) -> PdfPagePlan {
        self.page
    }

    pub const fn filters(self) -> PdfFilterImagePlan {
        self.filters
    }

    pub const fn embedded_images(self) -> EmbeddedImagePlan {
        self.embedded_images
    }

    pub const fn conversion(self) -> SvgConversionPlan {
        self.conversion
    }

    pub const fn fonts(self) -> ExportFontPlan {
        self.fonts
    }

    pub const fn page_paint(self) -> Option<ExportRgbaColor> {
        self.page_paint
    }
}

/// Parsed vector PDF input shared by page planning, memory scheduling, and encoding.
pub struct PreparedPdf {
    tree: usvg::Tree,
    layout: PdfPageLayout,
    filter_scale: f32,
    page_paint: Option<ExportRgbaColor>,
    report: PdfExportReport,
    control: OperationControl,
    #[cfg(feature = "native-fonts")]
    font_context: Option<NativeFontContext>,
}

impl PreparedPdf {
    /// Returns the complete report frozen before PDF encoding.
    pub const fn report(&self) -> PdfExportReport {
        self.report
    }

    /// Returns the localized filter allocation plan computed before PDF encoding.
    pub const fn filter_plan(&self) -> PdfFilterImagePlan {
        self.report.filters
    }

    /// Returns the embedded raster image plan computed from image headers.
    pub const fn embedded_image_plan(&self) -> EmbeddedImagePlan {
        self.report.embedded_images
    }

    pub const fn conversion_plan(&self) -> SvgConversionPlan {
        self.report.conversion
    }

    /// Returns an advisory weight for scheduling parallel PDF jobs.
    ///
    /// This is not a hard memory bound for krilla-svg or resvg internals.
    pub fn scheduling_weight_bytes(&self) -> u64 {
        const PDF_ENCODER_OVERHEAD_BYTES: u64 = 1024 * 1024;
        self.report
            .filters
            .effective_image_pixels
            .saturating_mul(8)
            .saturating_add(self.report.embedded_images.total_pixels.saturating_mul(8))
            .saturating_add(PDF_ENCODER_OVERHEAD_BYTES)
            .saturating_add(RECURSIVE_SVG_BACKEND_STACK_BYTES as u64)
    }

    /// Encodes the prepared tree as vector PDF, rasterizing only SVG filter regions.
    pub fn encode(self) -> Result<Vec<u8>> {
        self.encode_with_report().map(|(bytes, _)| bytes)
    }

    /// Encodes PDF together with the exact resources, page, and filter evidence used.
    pub fn encode_with_report(self) -> Result<(Vec<u8>, PdfExportReport)> {
        let control = self.control.clone();
        #[cfg(feature = "native-fonts")]
        let font_context = self.font_context.clone();
        run_recursive_svg_backend(&control, move |control| {
            let bytes = svg_tree_to_pdf(
                &self.tree,
                self.layout,
                self.filter_scale,
                self.page_paint,
                control,
            )?;
            #[cfg(feature = "native-fonts")]
            if let Some(context) = &font_context {
                context.check()?;
            }
            Ok((bytes, self.report))
        })
    }
}

/// Parses a sealed SVG once and prepares vector PDF page and filter allocation policy.
pub fn prepare_pdf(svg: &ResvgCompatibleSvg, options: &PdfOptions) -> Result<PreparedPdf> {
    prepare_pdf_controlled(svg, options, OperationControl::new())
}

/// Parses a sealed SVG for PDF using caller-owned cooperative cancellation/deadline state.
pub fn prepare_pdf_controlled(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
) -> Result<PreparedPdf> {
    let fonts = SvgExportFonts::from_options(
        svg,
        #[cfg(feature = "native-fonts")]
        options.font_context.as_ref(),
    );
    prepare_pdf_with_fonts(svg, options, control, fonts)
}

/// Prepares PDF export with a caller-supplied font policy instead of `options.font_context`.
///
/// The loader runs at most once and receives the same actual text-demand decision as raster
/// export. It may return `None` only when the successfully parsed SVG needs no fonts.
#[cfg(feature = "native-fonts")]
pub fn prepare_pdf_with_font_loader_controlled(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
    load: impl FnOnce(
        bool,
        &OperationControl,
    ) -> std::result::Result<Option<NativeFontContext>, NativeFontError>
    + Send,
) -> Result<PreparedPdf> {
    prepare_pdf_with_fonts(
        svg,
        options,
        control,
        SvgExportFonts::Deferred(Box::new(load)),
    )
}

fn prepare_pdf_with_fonts(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
    fonts: SvgExportFonts<'_>,
) -> Result<PreparedPdf> {
    export_checkpoint(&control)?;
    let backend_control = control.clone();
    run_recursive_svg_backend(&control, move |control| {
        prepare_pdf_on_backend_stack(svg, options, fonts, control).map(|mut prepared| {
            prepared.control = backend_control;
            prepared
        })
    })
}

fn prepare_pdf_on_backend_stack(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    fonts: SvgExportFonts<'_>,
    control: &OperationControl,
) -> Result<PreparedPdf> {
    export_checkpoint(control)?;
    let source = svg.as_str();
    validate_pdf_options(options)?;
    let page_paint = options
        .page_paint
        .as_deref()
        .map(parse_export_color)
        .transpose()
        .map_err(|_| ExportError::InvalidPdfPagePaint)?;
    let data_plan = plan_embedded_data_resource_bytes(
        svg.inline_raster_sources(),
        options.embedded_image_limit,
        control,
    )?;
    let ParsedExportSvg {
        tree,
        font_plan,
        #[cfg(feature = "native-fonts")]
        font_context,
    } = parse_svg_tree(source, usvg::Options::default(), fonts, control)?;
    let fallback_css_residual = svg.finalization_report().fallback_css_residual();
    let metadata = parse_root_svg_metadata(source, control)?;
    let implicit_viewport_clipping_residual = !metadata.has_view_box
        && !metadata.has_explicit_size
        && tree.root().abs_layer_bounding_box().is_some_and(|layer| {
            let viewport = tree.size();
            layer.left() < 0.0
                || layer.top() < 0.0
                || layer.right() > viewport.width()
                || layer.bottom() > viewport.height()
        });
    let conversion_plan = plan_svg_conversion(&tree, options.conversion_limits, control)?;
    let embedded_image_plan =
        plan_embedded_images(&tree, options.embedded_image_limit, data_plan, control)?;
    let svg_size = pdf_svg_size(&tree)?;
    let layout = pdf_page_layout(svg_size, options.page_policy)?;
    let (filter_plan, directional_filter_transform_residual) = plan_pdf_filter_images(
        &tree,
        layout.drawing_size.width() / svg_size.width(),
        options.filter_scale,
        options.filter_image_limit,
        control,
    )?;
    let report = PdfExportReport {
        directional_filter_transform_residual,
        implicit_viewport_clipping_residual,
        fallback_css_residual,
        page: layout.plan(),
        filters: filter_plan,
        embedded_images: embedded_image_plan,
        conversion: conversion_plan,
        fonts: font_plan,
        page_paint,
    };

    Ok(PreparedPdf {
        tree,
        layout,
        filter_scale: filter_plan.effective_scale,
        page_paint,
        report,
        control: control.clone(),
        #[cfg(feature = "native-fonts")]
        font_context,
    })
}

fn validate_pdf_options(options: &PdfOptions) -> Result<()> {
    if !(options.filter_scale.is_finite() && options.filter_scale > 0.0) {
        return Err(ExportError::InvalidSizing(
            "PDF filter_scale must be finite and positive",
        ));
    }
    if options.filter_image_limit.max_total_pixels == Some(0) {
        return Err(ExportError::InvalidSizing(
            "PDF filter max_total_pixels must be positive",
        ));
    }
    validate_embedded_image_limit(options.embedded_image_limit)?;
    Ok(())
}

pub fn svg_to_pdf(svg: &ResvgCompatibleSvg) -> Result<Vec<u8>> {
    svg_to_pdf_controlled(svg, &PdfOptions::default(), OperationControl::new())
}

pub fn svg_to_pdf_with_options(svg: &ResvgCompatibleSvg, options: &PdfOptions) -> Result<Vec<u8>> {
    svg_to_pdf_controlled(svg, options, OperationControl::new())
}

/// Encodes a sealed SVG as PDF using caller-owned cooperative cancellation/deadline state.
pub fn svg_to_pdf_controlled(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
) -> Result<Vec<u8>> {
    prepare_pdf_controlled(svg, options, control)?.encode()
}

/// Encodes a sealed SVG as PDF and returns the filter-image plan used by the encoder.
pub fn svg_to_pdf_with_plan_controlled(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
    control: OperationControl,
) -> Result<(Vec<u8>, PdfFilterImagePlan)> {
    let prepared = prepare_pdf_controlled(svg, options, control)?;
    let plan = prepared.filter_plan();
    let bytes = prepared.encode()?;
    Ok((bytes, plan))
}

/// Encodes a sealed SVG as PDF and returns its filter-image plan using a fresh control.
pub fn svg_to_pdf_with_plan(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
) -> Result<(Vec<u8>, PdfFilterImagePlan)> {
    svg_to_pdf_with_plan_controlled(svg, options, OperationControl::new())
}

pub fn svg_to_pdf_with_report(
    svg: &ResvgCompatibleSvg,
    options: &PdfOptions,
) -> Result<(Vec<u8>, PdfExportReport)> {
    prepare_pdf(svg, options)?.encode_with_report()
}

fn svg_tree_to_pdf(
    svg_tree: &usvg::Tree,
    layout: PdfPageLayout,
    filter_scale: f32,
    page_paint: Option<ExportRgbaColor>,
    control: &OperationControl,
) -> Result<Vec<u8>> {
    use krilla_svg::SurfaceExt;

    export_checkpoint(control)?;

    let mut document = krilla::Document::new();
    let mut page = document.start_page_with(krilla::page::PageSettings::new(layout.page_size));
    let mut surface = page.surface();
    draw_pdf_page_paint(&mut surface, layout.page_size, page_paint);
    if layout.offset.0 != 0.0 || layout.offset.1 != 0.0 {
        surface.push_transform(&krilla::geom::Transform::from_translate(
            layout.offset.0,
            layout.offset.1,
        ));
    }
    // krilla-svg performs one opaque synchronous draw. Cooperative cancellation is observed at
    // the call boundaries; hard interruption requires host-level worker/process isolation.
    export_checkpoint(control)?;
    surface.draw_svg(
        svg_tree,
        layout.drawing_size,
        krilla_svg::SvgSettings {
            filter_scale,
            ..krilla_svg::SvgSettings::default()
        },
    );
    export_checkpoint(control)?;
    if layout.offset.0 != 0.0 || layout.offset.1 != 0.0 {
        surface.pop();
    }
    surface.finish();
    page.finish();

    export_checkpoint(control)?;
    let pdf = document.finish().map_err(|_| ExportError::PdfConvert);
    export_checkpoint(control)?;
    pdf
}

fn pdf_svg_size(svg_tree: &usvg::Tree) -> Result<krilla::geom::Size> {
    krilla::geom::Size::from_wh(svg_tree.size().width(), svg_tree.size().height())
        .ok_or(ExportError::SvgDocSize)
}

#[derive(Clone, Copy)]
struct PdfPageLayout {
    page_size: krilla::geom::Size,
    drawing_size: krilla::geom::Size,
    offset: (f32, f32),
}

impl PdfPageLayout {
    fn plan(self) -> PdfPagePlan {
        PdfPagePlan {
            page_width_pt: self.page_size.width(),
            page_height_pt: self.page_size.height(),
            drawing_width_pt: self.drawing_size.width(),
            drawing_height_pt: self.drawing_size.height(),
            offset_x_pt: self.offset.0,
            offset_y_pt: self.offset.1,
        }
    }
}

fn pdf_page_layout(
    svg_size: krilla::geom::Size,
    page_policy: PdfPagePolicy,
) -> Result<PdfPageLayout> {
    match page_policy {
        PdfPagePolicy::FitSvg => Ok(PdfPageLayout {
            page_size: svg_size,
            drawing_size: svg_size,
            offset: (0.0, 0.0),
        }),
        PdfPagePolicy::Fixed {
            width_pt,
            height_pt,
        } => {
            let Some(page_size) = krilla::geom::Size::from_wh(width_pt, height_pt) else {
                return Err(ExportError::InvalidSizing(
                    "fixed PDF page dimensions must be finite and positive",
                ));
            };
            let scale = (width_pt / svg_size.width()).min(height_pt / svg_size.height());
            let Some(drawing_size) =
                krilla::geom::Size::from_wh(svg_size.width() * scale, svg_size.height() * scale)
            else {
                return Err(ExportError::SvgDocSize);
            };
            let offset = (
                (width_pt - drawing_size.width()) / 2.0,
                (height_pt - drawing_size.height()) / 2.0,
            );
            Ok(PdfPageLayout {
                page_size,
                drawing_size,
                offset,
            })
        }
        PdfPagePolicy::FitCssWidth { max_width_px } => {
            if !(max_width_px.is_finite() && max_width_px > 0.0) {
                return Err(ExportError::InvalidSizing(
                    "PDF CSS viewport width must be finite and positive",
                ));
            }
            let displayed_width_px = svg_size.width().min(max_width_px);
            let page_width_pt = displayed_width_px * PDF_POINTS_PER_CSS_PIXEL;
            let page_height_pt = svg_size.height() / svg_size.width() * page_width_pt;
            let Some(page_size) = krilla::geom::Size::from_wh(page_width_pt, page_height_pt) else {
                return Err(ExportError::SvgDocSize);
            };
            Ok(PdfPageLayout {
                page_size,
                drawing_size: page_size,
                offset: (0.0, 0.0),
            })
        }
    }
}

fn draw_pdf_page_paint(
    surface: &mut krilla::surface::Surface<'_>,
    page_size: krilla::geom::Size,
    page_paint: Option<ExportRgbaColor>,
) {
    let Some(color) = page_paint.filter(|color| color.alpha() != 0) else {
        return;
    };
    let Some(opacity) = krilla::num::NormalizedF32::new(f32::from(color.alpha()) / 255.0) else {
        return;
    };
    let mut path = krilla::geom::PathBuilder::new();
    let Some(rect) = krilla::geom::Rect::from_xywh(0.0, 0.0, page_size.width(), page_size.height())
    else {
        return;
    };
    path.push_rect(rect);
    let Some(path) = path.finish() else {
        return;
    };
    surface.set_fill(Some(krilla::paint::Fill {
        paint: krilla::color::rgb::Color::new(color.red(), color.green(), color.blue()).into(),
        opacity,
        rule: Default::default(),
    }));
    surface.draw_path(&path);
    surface.set_fill(None);
}
