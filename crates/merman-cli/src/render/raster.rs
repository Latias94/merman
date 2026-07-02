use super::executor::{RenderRequest, RenderedArtifact};
use super::plan::{RenderMode, RenderPlan};
use super::svg_pipeline::{escape_xml_attr, svg_metadata};
use crate::cli::{RasterCliArgs, RenderFormat};
use crate::error::CliError;
use std::borrow::Cow;

const MMDC_DEFAULT_PDF_WIDTH_PT: f32 = 612.0;
const MMDC_DEFAULT_PDF_HEIGHT_PT: f32 = 792.0;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RasterCliOptions {
    fit_width: Option<u32>,
    fit_height: Option<u32>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    max_pixels: Option<u64>,
    unbounded: bool,
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

        Ok(Self {
            fit_width: args.raster_fit_width,
            fit_height: args.raster_fit_height,
            max_width: args.raster_max_width,
            max_height: args.raster_max_height,
            max_pixels: args.raster_max_pixels,
            unbounded: args.raster_unbounded,
        })
    }
}

impl RenderPlan {
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

        options
    }
}

impl<'a> RenderRequest<'a> {
    pub(super) fn rasterize_prepared_svg(&self, svg: &str) -> Result<RenderedArtifact, CliError> {
        let metadata = svg_metadata(svg);
        let options = self.plan.raster_options();
        let bytes = match self.plan.format {
            RenderFormat::Svg | RenderFormat::Ascii | RenderFormat::Unicode => {
                return Err(CliError::InvalidOutput(
                    "raster output requested for a non-raster format".to_string(),
                ));
            }
            RenderFormat::Png => merman::render::raster::svg_to_png(svg, &options)?,
            RenderFormat::Jpeg => merman::render::raster::svg_to_jpeg(svg, &options)?,
            RenderFormat::Pdf => {
                merman::render::raster::validate_svg_pdf_size(svg, &options)?;
                let pdf_svg = self.pdf_svg_source(svg);
                merman::render::raster::svg_to_pdf_with_options(pdf_svg.as_ref(), &options)?
            }
        };
        Ok(RenderedArtifact {
            bytes,
            title: metadata.0,
            desc: metadata.1,
        })
    }

    fn pdf_svg_source<'svg>(&self, svg: &'svg str) -> Cow<'svg, str> {
        if matches!(self.plan.mode, RenderMode::MmdcCompat) && !self.plan.pdf_fit {
            Cow::Owned(wrap_svg_for_mmdc_default_pdf_page(
                svg,
                self.plan.background.as_deref(),
            ))
        } else {
            Cow::Borrowed(svg)
        }
    }
}

fn wrap_svg_for_mmdc_default_pdf_page(svg: &str, background: Option<&str>) -> String {
    let background_rect = background
        .filter(|value| !value.eq_ignore_ascii_case("transparent"))
        .map(|value| {
            format!(
                r#"<rect width="100%" height="100%" fill="{}"/>"#,
                escape_xml_attr(value)
            )
        })
        .unwrap_or_default();

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{MMDC_DEFAULT_PDF_WIDTH_PT}" height="{MMDC_DEFAULT_PDF_HEIGHT_PT}" viewBox="0 0 {MMDC_DEFAULT_PDF_WIDTH_PT} {MMDC_DEFAULT_PDF_HEIGHT_PT}">{background_rect}{svg}</svg>"#
    )
}
