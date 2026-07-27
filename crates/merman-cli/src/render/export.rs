use super::executor::{RenderRequest, RenderedArtifact};
#[cfg(feature = "pdf")]
use super::plan::RenderMode;
use super::plan::RenderPlan;
#[cfg(feature = "markdown")]
use super::svg_pipeline::svg_metadata;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::cli::{RenderFormat, ResourceProfile};
use crate::error::CliError;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::invocation::ResolvedEmbeddedImageOptions;
#[cfg(feature = "pdf")]
use crate::invocation::ResolvedPdfOptions;

#[cfg(feature = "pdf")]
const MMDC_DEFAULT_PDF_WIDTH_PT: f32 = 612.0;
#[cfg(feature = "pdf")]
const MMDC_DEFAULT_PDF_HEIGHT_PT: f32 = 792.0;
impl ResolvedEmbeddedImageOptions {
    fn limit(self) -> merman::svg::export::EmbeddedImageLimit {
        if self.unbounded {
            return merman::svg::export::EmbeddedImageLimit::unbounded();
        }
        let default = merman::svg::export::EmbeddedImageLimit::default();
        merman::svg::export::EmbeddedImageLimit::new(
            self.max_bytes_per_image.or(default.max_bytes_per_image),
            self.max_total_bytes.or(default.max_total_bytes),
            self.max_pixels_per_image.or(default.max_pixels_per_image),
            self.max_total_pixels.or(default.max_total_pixels),
        )
    }
}

#[cfg(feature = "pdf")]
impl ResolvedPdfOptions {
    fn apply_to(
        self,
        mut options: merman::svg::export::PdfOptions,
    ) -> merman::svg::export::PdfOptions {
        if let Some(filter_scale) = self.filter_scale {
            options.filter_scale = filter_scale;
        }
        if self.filter_images_unbounded {
            options.filter_image_limit = merman::svg::export::PdfFilterImageLimit::unbounded();
        } else if let Some(max_filter_image_pixels) = self.max_filter_image_pixels {
            options.filter_image_limit =
                merman::svg::export::PdfFilterImageLimit::new(Some(max_filter_image_pixels));
        }
        options
    }
}

impl RenderPlan {
    fn conversion_limits(&self) -> merman::svg::export::SvgConversionLimits {
        if matches!(
            self.render.resource_profile,
            ResourceProfile::UnboundedForTrustedInput
        ) {
            merman::svg::export::SvgConversionLimits::unbounded()
        } else {
            merman::svg::export::SvgConversionLimits::default()
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    fn raster_options(&self) -> merman::svg::export::RasterOptions {
        let mut options = merman::svg::export::RasterOptions {
            scale: self.scale,
            background: self.background.clone(),
            ..Default::default()
        };
        if self.raster.fit_width.is_some() || self.raster.fit_height.is_some() {
            options.fit_to = Some(merman::svg::export::RasterFitBox::new(
                self.raster.fit_width,
                self.raster.fit_height,
            ));
        }
        if self.raster.unbounded {
            options.size_limit = merman::svg::export::RasterSizeLimit::unbounded();
        } else if self.raster.max_width.is_some()
            || self.raster.max_height.is_some()
            || self.raster.max_pixels.is_some()
        {
            let default = merman::svg::export::RasterSizeLimit::default();
            options.size_limit = merman::svg::export::RasterSizeLimit::new(
                self.raster.max_width.or(default.max_width),
                self.raster.max_height.or(default.max_height),
                self.raster.max_pixels.or(default.max_pixels),
            );
        }
        options.embedded_image_limit = self.embedded_images.limit();
        options.conversion_limits = self.conversion_limits();
        options
    }
}

impl<'a> RenderRequest<'a> {
    pub(super) fn encode_prepared_svg(
        &self,
        svg: &merman::svg::ResvgCompatibleSvg,
    ) -> Result<RenderedArtifact, CliError> {
        #[cfg(feature = "markdown")]
        let metadata = svg_metadata(svg.as_str());
        let bytes = match self.plan.format {
            #[cfg(feature = "png")]
            RenderFormat::Png => {
                let prepared =
                    merman::svg::export::prepare_raster(svg, &self.plan.raster_options())?;
                self.report_raster_plan(prepared.plan());
                #[cfg(feature = "parallel-markdown")]
                let _permit = self
                    .encoding_parallel_budget
                    .as_ref()
                    .map(|budget| budget.acquire(prepared.png_scheduling_weight_bytes()));
                prepared.encode_png()?
            }
            #[cfg(feature = "jpeg")]
            RenderFormat::Jpeg => {
                let prepared =
                    merman::svg::export::prepare_raster(svg, &self.plan.raster_options())?;
                self.report_raster_plan(prepared.plan());
                #[cfg(feature = "parallel-markdown")]
                let _permit = self
                    .encoding_parallel_budget
                    .as_ref()
                    .map(|budget| budget.acquire(prepared.jpeg_scheduling_weight_bytes()));
                prepared.encode_jpeg()?
            }
            #[cfg(feature = "pdf")]
            RenderFormat::Pdf => {
                let mut options = self.plan.pdf.apply_to(
                    merman::svg::export::PdfOptions::default()
                        .with_page_policy(self.pdf_page_policy()?),
                );
                if let Some(background) = self.plan.background.as_deref() {
                    options = options.with_background(background);
                }
                options.embedded_image_limit = self.plan.embedded_images.limit();
                options.conversion_limits = self.plan.conversion_limits();
                let prepared = merman::svg::export::prepare_pdf(svg, &options)?;
                self.report_pdf_filter_plan(prepared.filter_plan());
                #[cfg(feature = "parallel-markdown")]
                let _permit = self
                    .encoding_parallel_budget
                    .as_ref()
                    .map(|budget| budget.acquire(prepared.scheduling_weight_bytes()));
                prepared.encode()?
            }
            _ => {
                return Err(CliError::InvalidOutput(
                    "encoded SVG output requested for an unsupported format".to_string(),
                ));
            }
        };
        Ok(RenderedArtifact {
            bytes,
            #[cfg(feature = "markdown")]
            title: metadata.0,
            #[cfg(feature = "markdown")]
            desc: metadata.1,
        })
    }

    #[cfg(feature = "pdf")]
    fn pdf_page_policy(&self) -> Result<merman::svg::export::PdfPagePolicy, CliError> {
        if !matches!(self.plan.mode, RenderMode::MmdcCompat) {
            return Ok(merman::svg::export::PdfPagePolicy::FitSvg);
        }
        if !self.plan.pdf_fit {
            return Ok(merman::svg::export::PdfPagePolicy::Fixed {
                width_pt: MMDC_DEFAULT_PDF_WIDTH_PT,
                height_pt: MMDC_DEFAULT_PDF_HEIGHT_PT,
            });
        }
        let max_width_px = self.plan.render.container_width.unwrap_or(800.0) as f32;
        if !(max_width_px.is_finite() && max_width_px > 0.0) {
            return Err(CliError::InvalidInput(
                "PDF viewport width is outside the supported numeric range".to_string(),
            ));
        }
        Ok(merman::svg::export::PdfPagePolicy::FitCssWidth { max_width_px })
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    fn report_raster_plan(&self, plan: merman::svg::export::RasterPlan) {
        if plan.limited {
            self.info(&format!(
                "Raster output was constrained from {:.0}x{:.0} to {}x{} pixels.",
                plan.requested_width_px, plan.requested_height_px, plan.width_px, plan.height_px
            ));
        }
    }

    #[cfg(feature = "pdf")]
    fn report_pdf_filter_plan(&self, plan: merman::svg::export::PdfFilterImagePlan) {
        if plan.limited {
            self.info(&format!(
                "PDF filter sampling was constrained from {} to {} retained image pixels (scale {:.3} -> {:.3}).",
                plan.requested_image_pixels,
                plan.effective_image_pixels,
                plan.requested_scale,
                plan.effective_scale
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_image_options_preserve_unspecified_safe_defaults() {
        let options = ResolvedEmbeddedImageOptions {
            max_pixels_per_image: Some(123),
            ..Default::default()
        };
        assert_eq!(
            options.limit(),
            merman::svg::export::EmbeddedImageLimit::new(
                Some(merman::svg::export::DEFAULT_MAX_EMBEDDED_IMAGE_BYTES),
                Some(merman::svg::export::DEFAULT_MAX_TOTAL_EMBEDDED_IMAGE_BYTES),
                Some(123),
                Some(merman::svg::export::DEFAULT_MAX_TOTAL_DECODED_IMAGE_PIXELS),
            )
        );
    }

    #[test]
    fn embedded_image_unbounded_mode_removes_all_limits() {
        let options = ResolvedEmbeddedImageOptions {
            max_bytes_per_image: Some(12),
            max_total_bytes: Some(34),
            max_pixels_per_image: Some(123),
            max_total_pixels: Some(456),
            unbounded: true,
        };
        assert_eq!(
            options.limit(),
            merman::svg::export::EmbeddedImageLimit::unbounded()
        );
    }
}
