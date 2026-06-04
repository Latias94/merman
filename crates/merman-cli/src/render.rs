use crate::cli::{ExportArgs, ParseCliArgs, RenderArgs, RenderCliArgs, RenderFormat};
use crate::config::{engine_for, layout_options, math_renderer, parse_options};
use crate::error::CliError;
use crate::io::{OutputTarget, read_input, read_optional_text_file, write_output};
use merman::render::{
    MathRenderer, RootBackgroundPostprocessor, ScopedCssPostprocessor, SvgPipeline,
    SvgRenderOptions,
};
use merman::{Engine, ParseOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
enum RenderMode {
    MmdcCompat,
    Subcommand,
}

#[derive(Debug, Clone)]
pub(crate) struct RenderPlan {
    input: Option<String>,
    output: Option<OutputTarget>,
    format: RenderFormat,
    parse: ParseCliArgs,
    render: RenderCliArgs,
    scale: f32,
    background: Option<String>,
    css: Option<String>,
    quiet: bool,
    sequence_mirror_actors: bool,
    mode: RenderMode,
}

struct RenderRequest<'a> {
    plan: &'a RenderPlan,
    engine: &'a Engine,
    parse_options: ParseOptions,
    math_renderer: Option<Arc<dyn MathRenderer + Send + Sync>>,
}

pub(crate) fn render_plan_for_mmdc(
    positional_input: Option<String>,
    export: ExportArgs,
) -> Result<RenderPlan, CliError> {
    let input = merge_input(export.input_file.clone(), positional_input)?;
    let format = infer_output_format(export.output.as_deref(), export.output_format)
        .unwrap_or(RenderFormat::Svg);
    let output = Some(OutputTarget::from_cli(
        export
            .output
            .clone()
            .unwrap_or_else(|| default_mmdc_output_path(input.as_deref(), format)),
    ));

    let mut parse = ParseCliArgs {
        suppress_errors: export.suppress_errors,
        config_file: export.config_file.clone(),
        theme: export.theme.clone(),
    };
    let mut render = RenderCliArgs {
        text_measurer: export.text_measurer,
        math_renderer: export.math_renderer,
        width: export.width,
        height: export.height,
        svg_id: export.svg_id.clone(),
        hand_drawn_seed: export.hand_drawn_seed,
    };

    apply_official_defaults(&mut parse, &mut render);

    Ok(RenderPlan {
        input,
        output,
        format,
        parse,
        render,
        scale: export.scale.unwrap_or(1.0),
        background: Some(
            export
                .background_color
                .clone()
                .unwrap_or_else(|| "white".to_string()),
        ),
        css: read_optional_text_file(export.css_file.as_deref(), "CSS file")?,
        quiet: export.quiet,
        sequence_mirror_actors: export.sequence_mirror_actors,
        mode: RenderMode::MmdcCompat,
    })
}

pub(crate) fn render_plan_for_subcommand(args: RenderArgs) -> Result<RenderPlan, CliError> {
    let input = merge_input(args.export.input_file.clone(), args.input)?;
    let format = infer_output_format(args.export.output.as_deref(), args.export.output_format)
        .unwrap_or(RenderFormat::Svg);
    let output = subcommand_output_target(args.export.output.clone(), input.as_deref(), format);

    Ok(RenderPlan {
        input,
        output,
        format,
        parse: ParseCliArgs {
            suppress_errors: args.export.suppress_errors,
            config_file: args.export.config_file.clone(),
            theme: args.export.theme.clone(),
        },
        render: RenderCliArgs {
            text_measurer: args.export.text_measurer,
            math_renderer: args.export.math_renderer,
            width: args.export.width,
            height: args.export.height,
            svg_id: args.export.svg_id.clone(),
            hand_drawn_seed: args.export.hand_drawn_seed,
        },
        scale: args.export.scale.unwrap_or(1.0),
        background: args.export.background_color.clone(),
        css: read_optional_text_file(args.export.css_file.as_deref(), "CSS file")?,
        quiet: args.export.quiet,
        sequence_mirror_actors: args.export.sequence_mirror_actors,
        mode: RenderMode::Subcommand,
    })
}

pub(crate) fn run_render(plan: RenderPlan) -> Result<(), CliError> {
    warn_for_accepted_compat_options(&plan);
    let text = read_input(plan.input.as_deref(), plan.quiet)?;

    let engine = engine_for(&plan.parse, &plan.render)?;
    let math_renderer = math_renderer(plan.render.math_renderer)?;
    let request = RenderRequest {
        plan: &plan,
        engine: &engine,
        parse_options: parse_options(&plan.parse),
        math_renderer,
    };

    request.render(&text)
}

impl<'a> RenderRequest<'a> {
    fn layout_options(&self) -> merman::render::LayoutOptions {
        layout_options(&self.plan.render, self.math_renderer.clone())
    }

    fn svg_options(&self) -> SvgRenderOptions {
        SvgRenderOptions {
            diagram_id: self
                .plan
                .render
                .svg_id
                .as_deref()
                .map(merman::render::sanitize_svg_id),
            math_renderer: self.math_renderer.clone(),
            ..Default::default()
        }
    }

    fn render(&self, text: &str) -> Result<(), CliError> {
        if self.plan.format.is_text() {
            return self.render_text(text);
        }

        if text.trim_start().starts_with("<svg") && self.plan.format.is_raster() {
            let svg = self.postprocess_svg(text)?;
            return self.write_rasterized_svg(&svg);
        }

        let Some(svg) = merman::render::render_svg_sync(
            self.engine,
            text,
            self.parse_options,
            &self.layout_options(),
            &self.svg_options(),
        )?
        else {
            return Err(CliError::NoDiagram);
        };
        let svg = self.postprocess_svg(&svg)?;

        match self.plan.format {
            RenderFormat::Svg => write_output(self.plan.output.as_ref(), svg.as_bytes()),
            RenderFormat::Ascii | RenderFormat::Unicode => unreachable!("handled above"),
            RenderFormat::Png | RenderFormat::Jpeg | RenderFormat::Pdf => {
                self.write_rasterized_svg(&svg)
            }
        }
    }

    fn postprocess_svg(&self, svg: &str) -> Result<String, CliError> {
        let mut pipeline = SvgPipeline::parity();
        if let Some(background) = self.plan.background.as_deref() {
            pipeline.push_postprocessor(RootBackgroundPostprocessor::new(background));
        }
        if let Some(css) = self.plan.css.as_deref() {
            pipeline.push_postprocessor(ScopedCssPostprocessor::new(css));
        }
        Ok(merman::render::apply_svg_pipeline(svg, &pipeline)?)
    }

    fn write_rasterized_svg(&self, svg: &str) -> Result<(), CliError> {
        let svg = merman::render::svg_resvg_safe(svg)?;
        let options = merman::render::raster::RasterOptions {
            scale: self.plan.scale,
            background: self.plan.background.clone(),
            ..Default::default()
        };
        let bytes = match self.plan.format {
            RenderFormat::Svg | RenderFormat::Ascii | RenderFormat::Unicode => {
                return Err(CliError::InvalidOutput(
                    "raster output requested for a non-raster format".to_string(),
                ));
            }
            RenderFormat::Png => merman::render::raster::svg_to_png(&svg, &options)?,
            RenderFormat::Jpeg => merman::render::raster::svg_to_jpeg(&svg, &options)?,
            RenderFormat::Pdf => merman::render::raster::svg_to_pdf(&svg)?,
        };
        write_output(self.plan.output.as_ref(), &bytes)
    }

    #[cfg(feature = "ascii")]
    fn render_text(&self, text: &str) -> Result<(), CliError> {
        let options = match self.plan.format {
            RenderFormat::Ascii => merman::ascii::AsciiRenderOptions::ascii(),
            RenderFormat::Unicode => merman::ascii::AsciiRenderOptions::unicode(),
            _ => {
                return Err(CliError::InvalidOutput(
                    "text output requested for a non-text format".to_string(),
                ));
            }
        }
        .with_sequence_mirror_actors(self.plan.sequence_mirror_actors);
        let Some(rendered) =
            merman::ascii::render_ascii_sync(self.engine, text, self.parse_options, &options)?
        else {
            return Err(CliError::NoDiagram);
        };
        write_output(self.plan.output.as_ref(), rendered.as_bytes())
    }

    #[cfg(not(feature = "ascii"))]
    fn render_text(&self, _text: &str) -> Result<(), CliError> {
        let _ = self.plan.sequence_mirror_actors;
        Err(CliError::InvalidOutput(
            "ASCII/Unicode output requires building merman-cli with --features ascii.".to_string(),
        ))
    }
}

fn apply_official_defaults(parse: &mut ParseCliArgs, render: &mut RenderCliArgs) {
    if parse.theme.is_none() {
        parse.theme = Some("default".to_string());
    }
    if render.width.is_none() {
        render.width = Some(800.0);
    }
    if render.height.is_none() {
        render.height = Some(600.0);
    }
}

fn merge_input(
    option_input: Option<String>,
    positional_input: Option<String>,
) -> Result<Option<String>, CliError> {
    match (option_input, positional_input) {
        (Some(a), Some(b)) if a != b => Err(CliError::InvalidInput(
            "input was provided both positionally and with --input; choose one".to_string(),
        )),
        (Some(a), _) => Ok(Some(a)),
        (_, Some(b)) => Ok(Some(b)),
        (None, None) => Ok(None),
    }
}

fn infer_output_format(
    output: Option<&str>,
    explicit: Option<RenderFormat>,
) -> Option<RenderFormat> {
    explicit.or_else(|| output.and_then(format_from_output_path))
}

fn format_from_output_path(path: &str) -> Option<RenderFormat> {
    if path == "-" {
        return Some(RenderFormat::Svg);
    }
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "svg" => Some(RenderFormat::Svg),
        "png" => Some(RenderFormat::Png),
        "jpg" | "jpeg" => Some(RenderFormat::Jpeg),
        "pdf" => Some(RenderFormat::Pdf),
        "txt" | "ascii" => Some(RenderFormat::Ascii),
        _ => None,
    }
}

fn default_mmdc_output_path(input: Option<&str>, format: RenderFormat) -> String {
    let ext = format.extension();
    match input.filter(|p| *p != "-") {
        Some(path) => format!("{path}.{ext}"),
        None => format!("out.{ext}"),
    }
}

fn subcommand_output_target(
    output: Option<String>,
    input: Option<&str>,
    format: RenderFormat,
) -> Option<OutputTarget> {
    if let Some(output) = output {
        return Some(OutputTarget::from_cli(output));
    }

    if format == RenderFormat::Svg || format.is_text() {
        return None;
    }

    Some(OutputTarget::File(default_raster_out_path(
        input,
        format.extension(),
    )))
}

fn default_raster_out_path(input: Option<&str>, ext: &str) -> PathBuf {
    match input.filter(|p| *p != "-") {
        Some(path) => PathBuf::from(path).with_extension(ext),
        None => PathBuf::from(format!("out.{ext}")),
    }
}

fn warn_for_accepted_compat_options(plan: &RenderPlan) {
    if plan.quiet {
        return;
    }
    if matches!(plan.mode, RenderMode::MmdcCompat) {
        // Kept intentionally quiet for no-op options that are only meaningful in a browser.
    }
}
