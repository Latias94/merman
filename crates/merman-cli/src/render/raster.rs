use super::executor::{RenderRequest, RenderedArtifact};
use super::plan::{RenderMode, RenderPlan};
use super::svg_pipeline::svg_metadata;
use crate::cli::{EmbeddedImageCliArgs, PdfCliArgs, RasterCliArgs, RenderFormat, ResourceProfile};
use crate::error::CliError;
use std::sync::{Condvar, Mutex};

const MMDC_DEFAULT_PDF_WIDTH_PT: f32 = 612.0;
const MMDC_DEFAULT_PDF_HEIGHT_PT: f32 = 792.0;
const DEFAULT_MARKDOWN_ENCODING_PARALLEL_BUDGET_MIB: u64 = 512;
const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy)]
pub(super) struct RasterCliOptions {
    fit_width: Option<u32>,
    fit_height: Option<u32>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    max_pixels: Option<u64>,
    parallel_budget_bytes: u64,
    unbounded: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PdfCliOptions {
    filter_scale: Option<f32>,
    max_filter_image_pixels: Option<u64>,
    filter_images_unbounded: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct EmbeddedImageCliOptions {
    max_bytes_per_image: Option<u64>,
    max_total_bytes: Option<u64>,
    max_pixels_per_image: Option<u64>,
    max_total_pixels: Option<u64>,
    unbounded: bool,
}

impl EmbeddedImageCliOptions {
    pub(super) const fn from_args(args: &EmbeddedImageCliArgs) -> Self {
        Self {
            max_bytes_per_image: args.max_image_bytes,
            max_total_bytes: args.max_total_bytes,
            max_pixels_per_image: args.max_image_pixels,
            max_total_pixels: args.max_total_pixels,
            unbounded: args.embedded_images_unbounded,
        }
    }

    fn limit(self) -> merman::render::raster::EmbeddedImageLimit {
        if self.unbounded {
            return merman::render::raster::EmbeddedImageLimit::unbounded();
        }

        let default = merman::render::raster::EmbeddedImageLimit::default();
        merman::render::raster::EmbeddedImageLimit::new(
            self.max_bytes_per_image.or(default.max_bytes_per_image),
            self.max_total_bytes.or(default.max_total_bytes),
            self.max_pixels_per_image.or(default.max_pixels_per_image),
            self.max_total_pixels.or(default.max_total_pixels),
        )
    }
}

impl PdfCliOptions {
    pub(super) const fn from_args(args: &PdfCliArgs) -> Self {
        Self {
            filter_scale: args.filter_scale,
            max_filter_image_pixels: args.max_filter_image_pixels,
            filter_images_unbounded: args.filter_images_unbounded,
        }
    }

    fn apply_to(
        self,
        mut options: merman::render::raster::PdfOptions,
    ) -> merman::render::raster::PdfOptions {
        if let Some(filter_scale) = self.filter_scale {
            options.filter_scale = filter_scale;
        }
        if self.filter_images_unbounded {
            options.filter_image_limit = merman::render::raster::PdfFilterImageLimit::unbounded();
        } else if let Some(max_filter_image_pixels) = self.max_filter_image_pixels {
            options.filter_image_limit =
                merman::render::raster::PdfFilterImageLimit::new(Some(max_filter_image_pixels));
        }
        options
    }
}

impl Default for RasterCliOptions {
    fn default() -> Self {
        Self {
            fit_width: None,
            fit_height: None,
            max_width: None,
            max_height: None,
            max_pixels: None,
            parallel_budget_bytes: DEFAULT_MARKDOWN_ENCODING_PARALLEL_BUDGET_MIB * MIB,
            unbounded: false,
        }
    }
}

impl RasterCliOptions {
    pub(super) fn from_args(args: &RasterCliArgs) -> Result<Self, CliError> {
        if args.raster_unbounded
            && (args.raster_max_width.is_some()
                || args.raster_max_height.is_some()
                || args.raster_max_pixels.is_some())
        {
            return Err(CliError::InvalidInput(
                "--raster-unbounded cannot be combined with --raster-max-* limits".to_string(),
            ));
        }

        let parallel_budget_mib = args
            .encoding_parallel_budget_mib
            .unwrap_or(DEFAULT_MARKDOWN_ENCODING_PARALLEL_BUDGET_MIB);
        let parallel_budget_bytes = parallel_budget_mib.checked_mul(MIB).ok_or_else(|| {
            CliError::InvalidInput("--encoding-parallel-budget-mib is too large".to_string())
        })?;

        Ok(Self {
            fit_width: args.raster_fit_width,
            fit_height: args.raster_fit_height,
            max_width: args.raster_max_width,
            max_height: args.raster_max_height,
            max_pixels: args.raster_max_pixels,
            parallel_budget_bytes,
            unbounded: args.raster_unbounded,
        })
    }

    pub(super) const fn encoding_parallel_budget_bytes(self) -> u64 {
        self.parallel_budget_bytes
    }
}

pub(super) struct EncodingParallelBudget {
    capacity: u64,
    in_flight: Mutex<u64>,
    capacity_changed: Condvar,
}

impl EncodingParallelBudget {
    pub(super) fn new(capacity: u64) -> Self {
        debug_assert!(capacity > 0);
        Self {
            capacity,
            in_flight: Mutex::new(0),
            capacity_changed: Condvar::new(),
        }
    }

    fn acquire(&self, requested: u64) -> EncodingParallelPermit<'_> {
        let weight = requested.min(self.capacity);
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while in_flight.saturating_add(weight) > self.capacity {
            in_flight = self
                .capacity_changed
                .wait(in_flight)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *in_flight += weight;
        EncodingParallelPermit {
            budget: self,
            weight,
        }
    }
}

struct EncodingParallelPermit<'a> {
    budget: &'a EncodingParallelBudget,
    weight: u64,
}

impl Drop for EncodingParallelPermit<'_> {
    fn drop(&mut self) {
        let mut in_flight = self
            .budget
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *in_flight = in_flight.saturating_sub(self.weight);
        self.budget.capacity_changed.notify_all();
    }
}

impl RenderPlan {
    fn conversion_limits(&self) -> merman::render::raster::SvgConversionLimits {
        if matches!(
            self.render.resource_profile,
            ResourceProfile::UnboundedForTrustedInput
        ) {
            merman::render::raster::SvgConversionLimits::unbounded()
        } else {
            merman::render::raster::SvgConversionLimits::default()
        }
    }

    pub(super) fn raster_options(&self) -> merman::render::raster::RasterOptions {
        let mut options = merman::render::raster::RasterOptions {
            scale: self.scale,
            background: self.background.clone(),
            ..Default::default()
        };

        if self.raster.fit_width.is_some() || self.raster.fit_height.is_some() {
            options.fit_to = Some(merman::render::raster::RasterFitBox::new(
                self.raster.fit_width,
                self.raster.fit_height,
            ));
        }

        if self.raster.unbounded {
            options.size_limit = merman::render::raster::RasterSizeLimit::unbounded();
        } else if self.raster.max_width.is_some()
            || self.raster.max_height.is_some()
            || self.raster.max_pixels.is_some()
        {
            let default = merman::render::raster::RasterSizeLimit::default();
            options.size_limit = merman::render::raster::RasterSizeLimit::new(
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
        svg: &merman::render::ResvgCompatibleSvg,
    ) -> Result<RenderedArtifact, CliError> {
        let metadata = svg_metadata(svg.as_str());
        let bytes = match self.plan.format {
            RenderFormat::Svg | RenderFormat::Ascii | RenderFormat::Unicode => {
                return Err(CliError::InvalidOutput(
                    "encoded SVG output requested for an unsupported format".to_string(),
                ));
            }
            RenderFormat::Png => {
                let options = self.plan.raster_options();
                let prepared = merman::render::raster::prepare_raster(svg, &options)?;
                self.report_raster_plan(prepared.plan());
                let _permit = self
                    .encoding_parallel_budget
                    .as_ref()
                    .map(|budget| budget.acquire(prepared.png_scheduling_weight_bytes()));
                prepared.encode_png()?
            }
            RenderFormat::Jpeg => {
                let options = self.plan.raster_options();
                let prepared = merman::render::raster::prepare_raster(svg, &options)?;
                self.report_raster_plan(prepared.plan());
                let _permit = self
                    .encoding_parallel_budget
                    .as_ref()
                    .map(|budget| budget.acquire(prepared.jpeg_scheduling_weight_bytes()));
                prepared.encode_jpeg()?
            }
            RenderFormat::Pdf => {
                let page_policy = self.pdf_page_policy()?;
                let mut options = self.plan.pdf.apply_to(
                    merman::render::raster::PdfOptions::default().with_page_policy(page_policy),
                );
                if let Some(background) = self.plan.background.as_deref() {
                    options = options.with_background(background);
                }
                options.embedded_image_limit = self.plan.embedded_images.limit();
                options.conversion_limits = self.plan.conversion_limits();
                let prepared = merman::render::raster::prepare_pdf(svg, &options)?;
                self.report_pdf_filter_plan(prepared.filter_plan());
                let _permit = self
                    .encoding_parallel_budget
                    .as_ref()
                    .map(|budget| budget.acquire(prepared.scheduling_weight_bytes()));
                prepared.encode()?
            }
        };
        Ok(RenderedArtifact {
            bytes,
            title: metadata.0,
            desc: metadata.1,
        })
    }

    fn pdf_page_policy(&self) -> Result<merman::render::raster::PdfPagePolicy, CliError> {
        if !matches!(self.plan.mode, RenderMode::MmdcCompat) {
            return Ok(merman::render::raster::PdfPagePolicy::FitSvg);
        }
        if !self.plan.pdf_fit {
            return Ok(merman::render::raster::PdfPagePolicy::Fixed {
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
        Ok(merman::render::raster::PdfPagePolicy::FitCssWidth { max_width_px })
    }

    fn report_raster_plan(&self, plan: merman::render::raster::RasterPlan) {
        if plan.limited {
            self.info(&format!(
                "Raster output was constrained from {:.0}x{:.0} to {}x{} pixels.",
                plan.requested_width_px, plan.requested_height_px, plan.width_px, plan.height_px
            ));
        }
    }

    fn report_pdf_filter_plan(&self, plan: merman::render::raster::PdfFilterImagePlan) {
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
    use std::sync::Arc;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

    #[test]
    fn embedded_image_options_preserve_unspecified_safe_defaults() {
        let options = EmbeddedImageCliOptions {
            max_pixels_per_image: Some(123),
            ..Default::default()
        };

        assert_eq!(
            options.limit(),
            merman::render::raster::EmbeddedImageLimit::new(
                Some(merman::render::raster::DEFAULT_MAX_EMBEDDED_IMAGE_BYTES),
                Some(merman::render::raster::DEFAULT_MAX_TOTAL_EMBEDDED_IMAGE_BYTES),
                Some(123),
                Some(merman::render::raster::DEFAULT_MAX_TOTAL_DECODED_IMAGE_PIXELS),
            )
        );
    }

    #[test]
    fn embedded_image_unbounded_mode_removes_all_limits() {
        let options = EmbeddedImageCliOptions {
            max_bytes_per_image: Some(12),
            max_total_bytes: Some(34),
            max_pixels_per_image: Some(123),
            max_total_pixels: Some(456),
            unbounded: true,
        };

        assert_eq!(
            options.limit(),
            merman::render::raster::EmbeddedImageLimit::unbounded()
        );
    }

    #[test]
    fn encoding_parallel_budget_blocks_until_weight_is_released() {
        let budget = Arc::new(EncodingParallelBudget::new(10));
        let first = budget.acquire(8);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let worker_budget = Arc::clone(&budget);
        let worker = std::thread::spawn(move || {
            ready_tx.send(()).unwrap();
            let _second = worker_budget.acquire(6);
            acquired_tx.send(()).unwrap();
        });

        ready_rx.recv().unwrap();
        assert!(matches!(
            acquired_rx.recv_timeout(Duration::from_millis(50)),
            Err(RecvTimeoutError::Timeout)
        ));
        drop(first);
        acquired_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("waiting encoding job should resume after capacity is released");
        worker.join().unwrap();
    }

    #[test]
    fn oversized_encoding_job_uses_the_budget_exclusively() {
        let budget = EncodingParallelBudget::new(10);
        let permit = budget.acquire(100);
        assert_eq!(permit.weight, 10);
        assert_eq!(*budget.in_flight.lock().unwrap(), 10);
    }
}
