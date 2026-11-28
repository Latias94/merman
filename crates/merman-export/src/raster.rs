//! Bounded preparation and encoding shared by PNG and JPEG.

use merman_model::OperationControl;
use merman_render::svg::ResvgCompatibleSvg;

use crate::conversion::{
    directional_filter_transforms_supported, plan_embedded_data_resource_bytes,
    plan_embedded_images, plan_svg_conversion,
};
use crate::operation::{
    RECURSIVE_SVG_BACKEND_STACK_BYTES, export_checkpoint, run_recursive_svg_backend,
};
use crate::viewport::{
    RasterFitBox, RasterGeometry, RasterSizeLimit, parse_root_svg_metadata,
    raster_geometry_for_svg, raster_plan_for_geometry, raster_usvg_options,
};
use crate::{
    EmbeddedImageLimit, EmbeddedImagePlan, ExportError, ExportFontPlan, ExportRgbaColor,
    ParsedExportSvg, Result, SvgConversionLimits, SvgConversionPlan, SvgExportFonts,
    parse_export_color, parse_svg_tree,
};
#[cfg(feature = "native-fonts")]
use crate::{NativeFontContext, NativeFontError};

#[derive(Debug, Clone)]
pub struct RasterOptions {
    pub scale: f32,
    /// Optional solid color composited behind SVG pixels. This is not the diagram theme canvas.
    pub matte: Option<String>,
    pub jpeg_quality: u8,
    pub fit_to: Option<RasterFitBox>,
    pub size_limit: RasterSizeLimit,
    pub embedded_image_limit: EmbeddedImageLimit,
    pub conversion_limits: SvgConversionLimits,
    #[cfg(feature = "native-fonts")]
    pub font_context: Option<NativeFontContext>,
}

impl Default for RasterOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            matte: None,
            jpeg_quality: 90,
            fit_to: None,
            size_limit: RasterSizeLimit::default(),
            embedded_image_limit: EmbeddedImageLimit::default(),
            conversion_limits: SvgConversionLimits::default(),
            #[cfg(feature = "native-fonts")]
            font_context: None,
        }
    }
}

impl RasterOptions {
    /// The opaque matte used for JPEG when the caller supplies no matte.
    pub const DEFAULT_JPEG_MATTE: ExportRgbaColor = ExportRgbaColor::WHITE;

    #[cfg(feature = "native-fonts")]
    pub fn with_font_context(mut self, font_context: NativeFontContext) -> Self {
        self.font_context = Some(font_context);
        self
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn with_matte(mut self, matte: impl Into<String>) -> Self {
        self.matte = Some(matte.into());
        self
    }

    pub fn with_fit_to(mut self, fit_to: RasterFitBox) -> Self {
        self.fit_to = Some(fit_to);
        self
    }

    pub fn with_size_limit(mut self, size_limit: RasterSizeLimit) -> Self {
        self.size_limit = size_limit;
        self
    }

    pub fn with_unbounded_size(mut self) -> Self {
        self.size_limit = RasterSizeLimit::unbounded();
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterPlan {
    pub requested_width_px: f64,
    pub requested_height_px: f64,
    pub width_px: u32,
    pub height_px: u32,
    pub requested_scale: f64,
    pub effective_scale: f64,
    pub limited: bool,
}

/// Concrete pixel output selected from one prepared raster tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RasterOutputKind {
    Png,
    Jpeg,
}

/// Frozen resource, allocation, font, and compositing evidence for one raster export.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterExportReport {
    directional_filter_transform_residual: bool,
    fallback_css_residual: bool,
    output: RasterOutputKind,
    raster: RasterPlan,
    embedded_images: EmbeddedImagePlan,
    conversion: SvgConversionPlan,
    fonts: ExportFontPlan,
    matte: Option<ExportRgbaColor>,
    matte_defaulted: bool,
}

impl RasterExportReport {
    /// Returns whether directional filter offsets may differ from their SVG transform.
    ///
    /// Raster export supports transforms that preserve or swap the coordinate axes. Best-effort
    /// export reports the residual for other transforms that can resample the filtered layer.
    pub const fn directional_filter_transform_residual(self) -> bool {
        self.directional_filter_transform_residual
    }

    /// Returns whether converted HTML label styles can differ in the native CSS backend.
    ///
    /// This quality observation is retained from the sealed SVG's terminal validation. It does
    /// not authorize resources, relax SVG safety, or prevent best-effort image encoding.
    pub const fn fallback_css_residual(self) -> bool {
        self.fallback_css_residual
    }

    pub const fn output(self) -> RasterOutputKind {
        self.output
    }

    pub const fn raster(self) -> RasterPlan {
        self.raster
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

    /// Returns the effective output matte. JPEG always reports an opaque value.
    pub const fn matte(self) -> Option<ExportRgbaColor> {
        self.matte
    }

    /// Returns whether the output format supplied the matte rather than the caller.
    pub const fn matte_defaulted(self) -> bool {
        self.matte_defaulted
    }
}

#[derive(Debug, Clone, Copy)]
struct RasterReportSeed {
    directional_filter_transform_residual: bool,
    fallback_css_residual: bool,
    raster: RasterPlan,
    embedded_images: EmbeddedImagePlan,
    conversion: SvgConversionPlan,
    fonts: ExportFontPlan,
}

/// Parsed raster input shared by sizing, memory scheduling, and image encoding.
pub struct PreparedRaster {
    tree: usvg::Tree,
    geometry: RasterGeometry,
    translate_min_to_origin: bool,
    report: RasterReportSeed,
    matte: Option<ExportRgbaColor>,
    #[cfg(feature = "jpeg")]
    jpeg_quality: u8,
    control: OperationControl,
    #[cfg(feature = "native-fonts")]
    font_context: Option<NativeFontContext>,
}

impl PreparedRaster {
    /// Returns the allocation plan computed before any output pixmap is created.
    pub const fn plan(&self) -> RasterPlan {
        self.report.raster
    }

    /// Returns the embedded raster image plan computed from image headers.
    pub const fn embedded_image_plan(&self) -> EmbeddedImagePlan {
        self.report.embedded_images
    }

    pub const fn conversion_plan(&self) -> SvgConversionPlan {
        self.report.conversion
    }

    /// Returns the complete report that would be used for the selected raster output.
    ///
    /// The report is frozen during preparation. JPEG's implicit white matte is reflected here;
    /// the encoder still performs its format-specific dimension and opacity checks before bytes
    /// are emitted.
    pub fn report_for_output(&self, output: RasterOutputKind) -> RasterExportReport {
        let matte_defaulted = matches!(output, RasterOutputKind::Jpeg) && self.matte.is_none();
        let matte = if matte_defaulted {
            Some(RasterOptions::DEFAULT_JPEG_MATTE)
        } else {
            self.matte
        };
        self.report_for(output, matte, matte_defaulted)
    }

    fn report_for(
        &self,
        output: RasterOutputKind,
        matte: Option<ExportRgbaColor>,
        matte_defaulted: bool,
    ) -> RasterExportReport {
        RasterExportReport {
            directional_filter_transform_residual: self
                .report
                .directional_filter_transform_residual,
            fallback_css_residual: self.report.fallback_css_residual,
            output,
            raster: self.report.raster,
            embedded_images: self.report.embedded_images,
            conversion: self.report.conversion,
            fonts: self.report.fonts,
            matte,
            matte_defaulted,
        }
    }

    /// Returns an advisory weight for scheduling parallel PNG jobs.
    ///
    /// This is not a hard memory bound for resvg internals.
    pub fn png_scheduling_weight_bytes(&self) -> u64 {
        encoding_scheduling_weight_bytes(self.report.raster, self.report.embedded_images, 8)
    }

    /// Returns an advisory weight for scheduling parallel JPEG jobs.
    ///
    /// This is not a hard memory bound for resvg internals.
    pub fn jpeg_scheduling_weight_bytes(&self) -> u64 {
        encoding_scheduling_weight_bytes(self.report.raster, self.report.embedded_images, 10)
    }

    /// Allocates and encodes the prepared image as PNG.
    #[cfg(feature = "png")]
    pub fn encode_png(self) -> Result<Vec<u8>> {
        self.encode_png_with_report().map(|(bytes, _)| bytes)
    }

    /// Allocates and encodes PNG together with the exact resources and compositing evidence used.
    #[cfg(feature = "png")]
    pub fn encode_png_with_report(self) -> Result<(Vec<u8>, RasterExportReport)> {
        let control = self.control.clone();
        #[cfg(feature = "native-fonts")]
        let font_context = self.font_context.clone();
        run_recursive_svg_backend(&control, move |control| {
            export_checkpoint(control)?;
            let matte = self.matte;
            let report = self.report_for_output(RasterOutputKind::Png);
            let pixmap = self.into_pixmap(matte, control)?;
            export_checkpoint(control)?;
            let bytes = pixmap.encode_png().map_err(|_| ExportError::PngEncode);
            export_checkpoint(control)?;
            let bytes = bytes?;
            #[cfg(feature = "native-fonts")]
            if let Some(context) = &font_context {
                context.check()?;
            }
            Ok((bytes, report))
        })
    }

    /// Allocates and encodes the prepared image as JPEG.
    #[cfg(feature = "jpeg")]
    pub fn encode_jpeg(self) -> Result<Vec<u8>> {
        self.encode_jpeg_with_report().map(|(bytes, _)| bytes)
    }

    /// Allocates and encodes JPEG together with the exact resources and compositing evidence used.
    #[cfg(feature = "jpeg")]
    pub fn encode_jpeg_with_report(self) -> Result<(Vec<u8>, RasterExportReport)> {
        let control = self.control.clone();
        #[cfg(feature = "native-fonts")]
        let font_context = self.font_context.clone();
        run_recursive_svg_backend(&control, move |control| {
            export_checkpoint(control)?;
            if self.report.raster.width_px > u32::from(u16::MAX)
                || self.report.raster.height_px > u32::from(u16::MAX)
            {
                return Err(ExportError::JpegDimensionLimit);
            }
            let matte = self.matte.unwrap_or(RasterOptions::DEFAULT_JPEG_MATTE);
            if !matte.is_opaque() {
                return Err(ExportError::JpegOpaqueMatteRequired);
            }

            let report = self.report_for_output(RasterOutputKind::Jpeg);
            let quality = self.jpeg_quality;
            let pixmap = self.into_pixmap(Some(matte), control)?;
            export_checkpoint(control)?;
            let (w, h) = (pixmap.width(), pixmap.height());
            let rgba = pixmap.data();
            let mut rgb = vec![0u8; (w as usize) * (h as usize) * 3];
            for (src, dst) in rgba
                .as_chunks::<4>()
                .0
                .iter()
                .zip(rgb.as_chunks_mut::<3>().0.iter_mut())
            {
                export_checkpoint(control)?;
                dst[0] = src[0];
                dst[1] = src[1];
                dst[2] = src[2];
            }

            let mut out = Vec::new();
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            export_checkpoint(control)?;
            let encoded = enc
                .encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
                .map_err(|_| ExportError::JpegEncode);
            export_checkpoint(control)?;
            encoded?;
            #[cfg(feature = "native-fonts")]
            if let Some(context) = &font_context {
                context.check()?;
            }
            Ok((out, report))
        })
    }

    fn render_pixmap(
        &self,
        matte: Option<ExportRgbaColor>,
        control: &OperationControl,
    ) -> Result<tiny_skia::Pixmap> {
        export_checkpoint(control)?;
        let mut pixmap =
            tiny_skia::Pixmap::new(self.report.raster.width_px, self.report.raster.height_px)
                .ok_or(ExportError::PixmapAlloc)?;

        if let Some(matte) = matte {
            pixmap.fill(tiny_skia_color(matte));
        }

        let scale = self.report.raster.effective_scale as f32;
        let transform = if self.translate_min_to_origin {
            tiny_skia::Transform::from_row(
                scale,
                0.0,
                0.0,
                scale,
                -self.geometry.min_x * scale,
                -self.geometry.min_y * scale,
            )
        } else {
            tiny_skia::Transform::from_scale(scale, scale)
        };

        // resvg is one opaque synchronous backend call. Cooperative cancellation is observed at
        // its boundaries; hosts that need hard preemption must isolate the worker or process.
        export_checkpoint(control)?;
        resvg::render(&self.tree, transform, &mut pixmap.as_mut());
        export_checkpoint(control)?;
        #[cfg(feature = "native-fonts")]
        if let Some(context) = &self.font_context {
            context.check()?;
        }
        Ok(pixmap)
    }

    fn into_pixmap(
        self,
        matte: Option<ExportRgbaColor>,
        control: &OperationControl,
    ) -> Result<tiny_skia::Pixmap> {
        self.render_pixmap(matte, control)
    }
}

/// Parses a sealed SVG once and prepares its bounded raster allocation plan.
pub fn prepare_raster(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<PreparedRaster> {
    prepare_raster_controlled(svg, options, OperationControl::new())
}

/// Parses a sealed SVG using caller-owned cooperative cancellation/deadline state.
pub fn prepare_raster_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<PreparedRaster> {
    let fonts = SvgExportFonts::from_options(
        svg,
        #[cfg(feature = "native-fonts")]
        options.font_context.as_ref(),
    );
    prepare_raster_with_fonts(svg, options, control, fonts)
}

/// Prepares raster export with a caller-supplied font policy instead of `options.font_context`.
///
/// Calls `load` at most once. A `true` argument requires a context for actual converted text.
/// For a successfully parsed fontless SVG, passes `false` so explicitly supplied sources can
/// still be validated without discovering system fonts. Returning `None` is allowed only then.
#[cfg(feature = "native-fonts")]
pub fn prepare_raster_with_font_loader_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
    load: impl FnOnce(
        bool,
        &OperationControl,
    ) -> std::result::Result<Option<NativeFontContext>, NativeFontError>
    + Send,
) -> Result<PreparedRaster> {
    prepare_raster_with_fonts(
        svg,
        options,
        control,
        SvgExportFonts::Deferred(Box::new(load)),
    )
}

fn prepare_raster_with_fonts(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
    fonts: SvgExportFonts<'_>,
) -> Result<PreparedRaster> {
    export_checkpoint(&control)?;
    let backend_control = control.clone();
    run_recursive_svg_backend(&control, move |control| {
        prepare_raster_on_backend_stack(svg, options, fonts, control).map(|mut prepared| {
            prepared.control = backend_control;
            prepared
        })
    })
}

fn prepare_raster_on_backend_stack(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    fonts: SvgExportFonts<'_>,
    control: &OperationControl,
) -> Result<PreparedRaster> {
    export_checkpoint(control)?;
    let matte = options
        .matte
        .as_deref()
        .map(parse_export_color)
        .transpose()
        .map_err(|_| ExportError::InvalidRasterMatte)?;
    let root_metadata = parse_root_svg_metadata(svg.as_str(), control)?;
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
    } = parse_svg_tree(
        svg.as_str(),
        raster_usvg_options(root_metadata),
        fonts,
        control,
    )?;
    let (geometry, translate_min_to_origin) = raster_geometry_for_svg(root_metadata, &tree);
    let fallback_css_residual = svg.finalization_report().fallback_css_residual();
    let conversion_plan = plan_svg_conversion(&tree, options.conversion_limits, control)?;
    let directional_filter_transform_residual =
        !directional_filter_transforms_supported(tree.root(), control)?;
    let embedded_image_plan =
        plan_embedded_images(&tree, options.embedded_image_limit, data_plan, control)?;
    let plan = raster_plan_for_geometry(geometry, options, control)?;
    export_checkpoint(control)?;

    Ok(PreparedRaster {
        tree,
        geometry,
        translate_min_to_origin,
        report: RasterReportSeed {
            directional_filter_transform_residual,
            fallback_css_residual,
            raster: plan,
            embedded_images: embedded_image_plan,
            conversion: conversion_plan,
            fonts: font_plan,
        },
        matte,
        #[cfg(feature = "jpeg")]
        jpeg_quality: options.jpeg_quality,
        control: control.clone(),
        #[cfg(feature = "native-fonts")]
        font_context,
    })
}

fn encoding_scheduling_weight_bytes(
    plan: RasterPlan,
    embedded_images: EmbeddedImagePlan,
    bytes_per_pixel: u64,
) -> u64 {
    const ENCODER_OVERHEAD_BYTES: u64 = 1024 * 1024;
    u64::from(plan.width_px)
        .saturating_mul(u64::from(plan.height_px))
        .saturating_mul(bytes_per_pixel)
        .saturating_add(embedded_images.total_pixels.saturating_mul(8))
        .saturating_add(ENCODER_OVERHEAD_BYTES)
        .saturating_add(RECURSIVE_SVG_BACKEND_STACK_BYTES as u64)
}

#[cfg(feature = "png")]
pub fn svg_to_png(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<Vec<u8>> {
    svg_to_png_controlled(svg, options, OperationControl::new())
}

/// Encodes a sealed SVG as PNG using caller-owned cooperative cancellation/deadline state.
#[cfg(feature = "png")]
pub fn svg_to_png_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<Vec<u8>> {
    prepare_raster_controlled(svg, options, control)?.encode_png()
}

/// Encodes a sealed SVG as PNG and returns the allocation plan used for the output pixmap.
#[cfg(feature = "png")]
pub fn svg_to_png_with_plan_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<(Vec<u8>, RasterPlan)> {
    let prepared = prepare_raster_controlled(svg, options, control)?;
    let plan = prepared.plan();
    let bytes = prepared.encode_png()?;
    Ok((bytes, plan))
}

/// Encodes a sealed SVG as PNG and returns its allocation plan using a fresh control.
#[cfg(feature = "png")]
pub fn svg_to_png_with_plan(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterPlan)> {
    svg_to_png_with_plan_controlled(svg, options, OperationControl::new())
}

#[cfg(feature = "png")]
pub fn svg_to_png_with_report(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterExportReport)> {
    prepare_raster(svg, options)?.encode_png_with_report()
}

#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<Vec<u8>> {
    svg_to_jpeg_controlled(svg, options, OperationControl::new())
}

/// Encodes a sealed SVG as JPEG using caller-owned cooperative cancellation/deadline state.
#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<Vec<u8>> {
    prepare_raster_controlled(svg, options, control)?.encode_jpeg()
}

/// Encodes a sealed SVG as JPEG and returns the allocation plan used for the output pixmap.
#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg_with_plan_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<(Vec<u8>, RasterPlan)> {
    let prepared = prepare_raster_controlled(svg, options, control)?;
    let plan = prepared.plan();
    let bytes = prepared.encode_jpeg()?;
    Ok((bytes, plan))
}

/// Encodes a sealed SVG as JPEG and returns its allocation plan using a fresh control.
#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg_with_plan(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterPlan)> {
    svg_to_jpeg_with_plan_controlled(svg, options, OperationControl::new())
}

#[cfg(feature = "jpeg")]
pub fn svg_to_jpeg_with_report(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterExportReport)> {
    prepare_raster(svg, options)?.encode_jpeg_with_report()
}

pub fn svg_raster_plan(svg: &ResvgCompatibleSvg, options: &RasterOptions) -> Result<RasterPlan> {
    svg_raster_plan_controlled(svg, options, OperationControl::new())
}

/// Computes a sealed SVG raster plan using caller-owned cooperative cancellation/deadline state.
pub fn svg_raster_plan_controlled(
    svg: &ResvgCompatibleSvg,
    options: &RasterOptions,
    control: OperationControl,
) -> Result<RasterPlan> {
    Ok(prepare_raster_controlled(svg, options, control)?.plan())
}

fn tiny_skia_color(color: ExportRgbaColor) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(color.red(), color.green(), color.blue(), color.alpha())
}
