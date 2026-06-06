use crate::cli::{ExportArgs, ParseCliArgs, RenderArgs, RenderCliArgs, RenderFormat};
use crate::config::{engine_for, layout_options, math_renderer, parse_options};
use crate::error::CliError;
use crate::io::{
    OutputTarget, read_input, read_named_text_file, read_optional_text_file, write_file,
    write_output,
};
use crate::markdown::{self, MarkdownImage};
use merman::render::{
    IconRegistry, MathRenderer, RootBackgroundPostprocessor, ScopedCssPostprocessor, SvgPipeline,
    SvgRenderOptions,
};
use merman::{Engine, ParseOptions};
use rayon::prelude::*;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const MMDC_DEFAULT_PDF_WIDTH_PT: f32 = 612.0;
const MMDC_DEFAULT_PDF_HEIGHT_PT: f32 = 792.0;

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
    raster: RasterCliOptions,
    background: Option<String>,
    css: Option<String>,
    icon_registry: Option<Arc<IconRegistry>>,
    artefacts: Option<PathBuf>,
    jobs: usize,
    pdf_fit: bool,
    quiet: bool,
    sequence_mirror_actors: bool,
    mode: RenderMode,
}

#[derive(Debug, Clone, Copy, Default)]
struct RasterCliOptions {
    fit_width: Option<u32>,
    fit_height: Option<u32>,
    max_width: Option<u32>,
    max_height: Option<u32>,
    max_pixels: Option<u64>,
    unbounded: bool,
}

struct RenderRequest<'a> {
    plan: &'a RenderPlan,
    engine: &'a Engine,
    parse_options: ParseOptions,
    math_renderer: Option<Arc<dyn MathRenderer + Send + Sync>>,
}

struct RenderedArtifact {
    bytes: Vec<u8>,
    title: Option<String>,
    desc: Option<String>,
}

pub(crate) fn render_plan_for_mmdc(
    positional_input: Option<String>,
    export: ExportArgs,
) -> Result<RenderPlan, CliError> {
    let input = merge_input(export.input_file.clone(), positional_input)?;
    let artefacts = prepare_artefacts_dir(export.artefacts.as_deref(), input.as_deref())?;
    validate_mmdc_output_path(export.output.as_deref())?;
    let icon_registry = load_icon_registry(&export.icon_packs, &export.icon_packs_names_and_urls)?;
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
        fixed_today: export.fixed_today,
        fixed_local_offset_minutes: export.fixed_local_offset_minutes,
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
    validate_puppeteer_config_file(export.puppeteer_config_file.as_deref())?;

    Ok(RenderPlan {
        input,
        output,
        format,
        parse,
        render,
        scale: export.scale.unwrap_or(1.0),
        raster: RasterCliOptions::from_export(&export)?,
        background: Some(
            export
                .background_color
                .clone()
                .unwrap_or_else(|| "white".to_string()),
        ),
        css: read_optional_text_file(export.css_file.as_deref(), "CSS file")?,
        icon_registry,
        artefacts,
        jobs: export.jobs.unwrap_or_else(default_jobs),
        pdf_fit: export.pdf_fit,
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
    let icon_registry = load_icon_registry(
        &args.export.icon_packs,
        &args.export.icon_packs_names_and_urls,
    )?;
    validate_puppeteer_config_file(args.export.puppeteer_config_file.as_deref())?;

    Ok(RenderPlan {
        input,
        output,
        format,
        parse: ParseCliArgs {
            suppress_errors: args.export.suppress_errors,
            config_file: args.export.config_file.clone(),
            theme: args.export.theme.clone(),
            fixed_today: args.export.fixed_today,
            fixed_local_offset_minutes: args.export.fixed_local_offset_minutes,
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
        raster: RasterCliOptions::from_export(&args.export)?,
        background: args.export.background_color.clone(),
        css: read_optional_text_file(args.export.css_file.as_deref(), "CSS file")?,
        icon_registry,
        artefacts: None,
        jobs: args.export.jobs.unwrap_or_else(default_jobs),
        pdf_fit: true,
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

    if plan.is_mmdc_markdown_input() {
        request.render_markdown(&text)
    } else {
        request.render(&text)
    }
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
            icon_registry: self.plan.icon_registry.clone(),
            ..Default::default()
        }
    }

    fn render(&self, text: &str) -> Result<(), CliError> {
        let artifact = self.render_artifact(text)?;
        write_output(self.plan.output.as_ref(), &artifact.bytes)
    }

    fn render_markdown(&self, text: &str) -> Result<(), CliError> {
        if self.plan.format.is_text() {
            return Err(CliError::InvalidOutput(
                "Markdown input does not support ASCII/Unicode output".to_string(),
            ));
        }

        let output_path = match self.plan.output.as_ref() {
            Some(OutputTarget::File(path)) => path.as_path(),
            None | Some(OutputTarget::Stdout) => {
                return Err(CliError::InvalidOutput(
                    "Cannot use `stdout` with markdown input".to_string(),
                ));
            }
        };

        let charts = markdown::extract_charts(text);

        if charts.is_empty() {
            self.info("No mermaid charts found in Markdown input");
        } else {
            self.info(&format!(
                "Found {} mermaid charts in Markdown input",
                charts.len()
            ));
        }

        let images = self.render_markdown_charts(output_path, &charts)?;

        if markdown::is_markdown_path(output_path) {
            let rewritten = markdown::replace_charts_with_images(text, &images);
            write_file(output_path, rewritten.as_bytes())?;
            self.info(&format!(" ✅ {}", output_path.display()));
        }

        Ok(())
    }

    fn render_markdown_charts(
        &self,
        output_path: &Path,
        charts: &[markdown::MarkdownChart],
    ) -> Result<Vec<MarkdownImage>, CliError> {
        if charts.len() <= 1 || self.plan.jobs == 1 {
            return charts
                .iter()
                .enumerate()
                .map(|(index, chart)| self.render_markdown_chart(output_path, index, chart))
                .collect();
        }

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.plan.jobs)
            .build()
            .map_err(|err| {
                CliError::InvalidInput(format!("failed to configure Markdown render jobs: {err}"))
            })?;

        pool.install(|| {
            charts
                .par_iter()
                .enumerate()
                .map(|(index, chart)| self.render_markdown_chart(output_path, index, chart))
                .collect()
        })
    }

    fn render_markdown_chart(
        &self,
        output_path: &Path,
        index: usize,
        chart: &markdown::MarkdownChart,
    ) -> Result<MarkdownImage, CliError> {
        let output_file = markdown::numbered_output_path(
            output_path,
            index + 1,
            self.plan.format,
            self.plan.artefacts.as_deref(),
        );
        let artifact = self.render_artifact(&chart.definition)?;
        write_file(&output_file, &artifact.bytes)?;

        let url = markdown::relative_markdown_url(output_path, &output_file)?;
        self.info(&format!(" ✅ {url}"));
        Ok(MarkdownImage {
            url,
            title: artifact.title,
            alt: artifact.desc.unwrap_or_else(|| "diagram".to_string()),
        })
    }

    fn render_artifact(&self, text: &str) -> Result<RenderedArtifact, CliError> {
        if self.plan.format.is_text() {
            return self.render_text(text);
        }

        if text.trim_start().starts_with("<svg") && self.plan.format.is_raster() {
            let svg = self.postprocess_raw_svg_for_raster(text)?;
            return self.rasterize_svg(&svg);
        }

        let pipeline = self.postprocess_pipeline();
        let Some(svg) = merman::render::render_svg_with_pipeline_sync(
            self.engine,
            text,
            self.parse_options,
            &self.layout_options(),
            &self.svg_options(),
            &pipeline,
        )?
        else {
            return Err(CliError::NoDiagram);
        };

        match self.plan.format {
            RenderFormat::Svg => Ok(RenderedArtifact::from_svg(svg)),
            RenderFormat::Ascii | RenderFormat::Unicode => unreachable!("handled above"),
            RenderFormat::Png | RenderFormat::Jpeg | RenderFormat::Pdf => self.rasterize_svg(&svg),
        }
    }

    fn postprocess_pipeline(&self) -> SvgPipeline {
        svg_postprocess_pipeline(
            SvgPipeline::parity(),
            self.plan.background.as_deref(),
            self.plan.css.as_deref(),
        )
    }

    fn raw_svg_raster_pipeline(&self) -> SvgPipeline {
        svg_postprocess_pipeline(
            SvgPipeline::resvg_safe(),
            self.plan.background.as_deref(),
            self.plan.css.as_deref(),
        )
    }

    fn postprocess_raw_svg_for_raster(&self, svg: &str) -> Result<String, CliError> {
        let pipeline = self.raw_svg_raster_pipeline();
        Ok(merman::render::apply_svg_pipeline(svg, &pipeline)?)
    }

    fn rasterize_svg(&self, svg: &str) -> Result<RenderedArtifact, CliError> {
        let metadata = svg_metadata(svg);
        let svg = merman::render::svg_resvg_safe(svg)?;
        let options = self.plan.raster_options();
        let bytes = match self.plan.format {
            RenderFormat::Svg | RenderFormat::Ascii | RenderFormat::Unicode => {
                return Err(CliError::InvalidOutput(
                    "raster output requested for a non-raster format".to_string(),
                ));
            }
            RenderFormat::Png => merman::render::raster::svg_to_png(&svg, &options)?,
            RenderFormat::Jpeg => merman::render::raster::svg_to_jpeg(&svg, &options)?,
            RenderFormat::Pdf => {
                merman::render::raster::validate_svg_pdf_size(&svg, &options)?;
                let pdf_svg = self.pdf_svg_source(&svg);
                merman::render::raster::svg_to_pdf_with_options(pdf_svg.as_ref(), &options)?
            }
        };
        Ok(RenderedArtifact {
            bytes,
            title: metadata.0,
            desc: metadata.1,
        })
    }

    #[cfg(feature = "ascii")]
    fn render_text(&self, text: &str) -> Result<RenderedArtifact, CliError> {
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
        Ok(RenderedArtifact {
            bytes: rendered.into_bytes(),
            title: None,
            desc: None,
        })
    }

    #[cfg(not(feature = "ascii"))]
    fn render_text(&self, _text: &str) -> Result<RenderedArtifact, CliError> {
        let _ = self.plan.sequence_mirror_actors;
        Err(CliError::InvalidOutput(
            "ASCII/Unicode output requires building merman-cli with --features ascii.".to_string(),
        ))
    }

    fn info(&self, message: &str) {
        if !self.plan.quiet {
            println!("{message}");
        }
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

fn svg_postprocess_pipeline(
    mut pipeline: SvgPipeline,
    background: Option<&str>,
    css: Option<&str>,
) -> SvgPipeline {
    if let Some(background) = background {
        pipeline.push_postprocessor(RootBackgroundPostprocessor::new(background));
    }
    if let Some(css) = css {
        pipeline.push_postprocessor(ScopedCssPostprocessor::new(css));
    }
    pipeline
}

impl RenderPlan {
    fn is_mmdc_markdown_input(&self) -> bool {
        matches!(self.mode, RenderMode::MmdcCompat)
            && self
                .input
                .as_deref()
                .filter(|path| *path != "-")
                .map(|path| markdown::is_markdown_path(Path::new(path)))
                .unwrap_or(false)
    }

    fn raster_options(&self) -> merman::render::raster::RasterOptions {
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

impl RasterCliOptions {
    fn from_export(export: &ExportArgs) -> Result<Self, CliError> {
        if export.raster_unbounded
            && (export.raster_max_width.is_some()
                || export.raster_max_height.is_some()
                || export.raster_max_pixels.is_some())
        {
            return Err(CliError::InvalidInput(
                "--raster-unbounded cannot be combined with --raster-max-* limits".to_string(),
            ));
        }

        Ok(Self {
            fit_width: export.raster_fit_width,
            fit_height: export.raster_fit_height,
            max_width: export.raster_max_width,
            max_height: export.raster_max_height,
            max_pixels: export.raster_max_pixels,
            unbounded: export.raster_unbounded,
        })
    }
}

impl RenderedArtifact {
    fn from_svg(svg: String) -> Self {
        let (title, desc) = svg_metadata(&svg);
        Self {
            bytes: svg.into_bytes(),
            title,
            desc,
        }
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

fn prepare_artefacts_dir(
    artefacts: Option<&str>,
    input: Option<&str>,
) -> Result<Option<PathBuf>, CliError> {
    let Some(raw_path) = artefacts else {
        return Ok(None);
    };

    let is_markdown_input = input
        .filter(|path| *path != "-")
        .map(|path| markdown::is_markdown_path(Path::new(path)))
        .unwrap_or(false);
    if !is_markdown_input {
        return Err(CliError::InvalidInput(
            "Artefacts [-a|--artefacts] path can only be used with Markdown input file".to_string(),
        ));
    }

    let path = PathBuf::from(raw_path);
    std::fs::create_dir_all(&path)?;
    Ok(Some(path))
}

fn validate_puppeteer_config_file(path: Option<&str>) -> Result<(), CliError> {
    let Some(path) = path else {
        return Ok(());
    };

    let text = read_named_text_file(path, "Puppeteer configuration file")?;
    let _: serde_json::Value = serde_json::from_str(&text)?;
    Ok(())
}

fn load_icon_registry(
    icon_packs: &[String],
    icon_packs_names_and_urls: &[String],
) -> Result<Option<Arc<IconRegistry>>, CliError> {
    if icon_packs.is_empty() && icon_packs_names_and_urls.is_empty() {
        return Ok(None);
    }

    let cwd = std::env::current_dir()?;
    let mut registry = IconRegistry::new();

    for icon_pack in icon_packs {
        let prefix = icon_pack_package_prefix(icon_pack)?;
        let source = local_icon_pack_path(icon_pack, &cwd)
            .map(IconPackSource::LocalPath)
            .unwrap_or_else(|| {
                IconPackSource::RemoteUrl(format!("https://unpkg.com/{icon_pack}/icons.json"))
            });
        let json = read_icon_pack_source(&source)?;
        register_icon_pack_json(&mut registry, &json, Some(&prefix), icon_pack)?;
    }

    for icon_pack_info in icon_packs_names_and_urls {
        let (prefix, source) = icon_pack_info.split_once('#').ok_or_else(|| {
            CliError::InvalidInput(format!(
                "Invalid --iconPacksNamesAndUrls value `{icon_pack_info}`; expected prefix#url"
            ))
        })?;
        let prefix = prefix.trim();
        let source = source.trim();
        if prefix.is_empty() || source.is_empty() {
            return Err(CliError::InvalidInput(format!(
                "Invalid --iconPacksNamesAndUrls value `{icon_pack_info}`; expected non-empty prefix and URL"
            )));
        }

        let source = icon_pack_source_from_cli(source, &cwd);
        let json = read_icon_pack_source(&source)?;
        register_icon_pack_json(&mut registry, &json, Some(prefix), icon_pack_info)?;
    }

    Ok((!registry.is_empty()).then(|| Arc::new(registry)))
}

enum IconPackSource {
    LocalPath(PathBuf),
    RemoteUrl(String),
}

fn register_icon_pack_json(
    registry: &mut IconRegistry,
    json: &str,
    prefix_override: Option<&str>,
    label: &str,
) -> Result<(), CliError> {
    registry
        .register_iconify_json_str(json, prefix_override)
        .map_err(|err| {
            CliError::InvalidInput(format!("Invalid icon pack JSON for `{label}`: {err}"))
        })
}

fn icon_pack_package_prefix(icon_pack: &str) -> Result<String, CliError> {
    let icon_pack = icon_pack.trim().trim_end_matches('/');
    let prefix = icon_pack.rsplit('/').next().unwrap_or(icon_pack).trim();
    if prefix.is_empty() || prefix.starts_with('@') {
        return Err(CliError::InvalidInput(format!(
            "Invalid --iconPacks value `{icon_pack}`; expected an Iconify package such as @iconify-json/logos"
        )));
    }
    Ok(prefix.to_string())
}

fn local_icon_pack_path(icon_pack: &str, cwd: &Path) -> Option<PathBuf> {
    if looks_like_path(icon_pack) {
        let path = resolve_cli_path(icon_pack, cwd);
        if path.exists() {
            return Some(path);
        }
    }

    let mut current = Some(cwd);
    while let Some(dir) = current {
        let candidate = dir.join("node_modules").join(icon_pack).join("icons.json");
        if candidate.exists() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn icon_pack_source_from_cli(source: &str, cwd: &Path) -> IconPackSource {
    if source.starts_with("http://") || source.starts_with("https://") {
        IconPackSource::RemoteUrl(source.to_string())
    } else if let Some(path) = file_url_to_path(source) {
        IconPackSource::LocalPath(path)
    } else {
        IconPackSource::LocalPath(resolve_cli_path(source, cwd))
    }
}

fn read_icon_pack_source(source: &IconPackSource) -> Result<String, CliError> {
    match source {
        IconPackSource::LocalPath(path) => std::fs::read_to_string(path).map_err(|err| {
            CliError::InvalidInput(format!(
                "Failed to read icon pack JSON `{}`: {err}",
                path.display()
            ))
        }),
        IconPackSource::RemoteUrl(url) => fetch_icon_pack_json(url),
    }
}

fn fetch_icon_pack_json(url: &str) -> Result<String, CliError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| {
            CliError::InvalidInput(format!("Failed to create icon pack HTTP client: {err}"))
        })?;
    let response = client.get(url).send().map_err(|err| {
        CliError::InvalidInput(format!("Failed to fetch icon pack JSON `{url}`: {err}"))
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(CliError::InvalidInput(format!(
            "Failed to fetch icon pack JSON `{url}`: HTTP {status}"
        )));
    }
    response.text().map_err(|err| {
        CliError::InvalidInput(format!("Failed to read icon pack JSON `{url}`: {err}"))
    })
}

fn looks_like_path(value: &str) -> bool {
    value.ends_with(".json")
        || value.starts_with('.')
        || value.contains('\\')
        || Path::new(value).is_absolute()
}

fn resolve_cli_path(value: &str, cwd: &Path) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn file_url_to_path(value: &str) -> Option<PathBuf> {
    let raw = value.strip_prefix("file://")?;
    let decoded = raw.replace("%20", " ");
    #[cfg(windows)]
    {
        let trimmed = decoded.strip_prefix('/').unwrap_or(&decoded);
        return Some(PathBuf::from(trimmed));
    }
    #[cfg(not(windows))]
    {
        Some(PathBuf::from(decoded))
    }
}

fn default_jobs() -> usize {
    std::thread::available_parallelism()
        .map(|count| (count.get() / 2).max(1))
        .unwrap_or(1)
}

fn svg_metadata(svg: &str) -> (Option<String>, Option<String>) {
    (
        first_svg_element_text(svg, "title"),
        first_svg_element_text(svg, "desc"),
    )
}

fn first_svg_element_text(svg: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = svg.find(&open)?;
    let content_start = svg[start..].find('>')? + start + 1;
    let content_end = svg[content_start..].find(&close)? + content_start;
    let value = svg[content_start..content_end].trim();
    (!value.is_empty()).then(|| decode_basic_xml_entities(value))
}

fn decode_basic_xml_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
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

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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

fn validate_mmdc_output_path(output: Option<&str>) -> Result<(), CliError> {
    let Some(output) = output else {
        return Ok(());
    };
    if output == "-" {
        return Ok(());
    }

    let Some(ext) = Path::new(output)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
    else {
        return Err(invalid_mmdc_output_extension());
    };

    if matches!(
        ext.as_str(),
        "md" | "markdown" | "svg" | "png" | "pdf" | "jpg" | "jpeg" | "txt" | "ascii"
    ) {
        Ok(())
    } else {
        Err(invalid_mmdc_output_extension())
    }
}

fn invalid_mmdc_output_extension() -> CliError {
    CliError::InvalidOutput(
        "Output file must end with \".md\"/\".markdown\", \".svg\", \".png\", \".pdf\", \
         \".jpg\"/\".jpeg\", \".txt\" or \".ascii\""
            .to_string(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_svg_raster_pipeline_sanitizes_before_cli_postprocessors() {
        let pipeline = svg_postprocess_pipeline(
            SvgPipeline::resvg_safe(),
            Some("#f8fafc"),
            Some(".node { fill: red; }"),
        );
        let svg = r#"<svg id="raw" xmlns="http://www.w3.org/2000/svg"><style>@keyframes bad { to { opacity: .5; } } .node { animation: bad 1s; }</style><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

        let out = pipeline.process_to_string(svg).unwrap();

        assert!(!out.contains("<foreignObject"));
        assert!(!out.contains("@keyframes bad"));
        assert!(!out.contains("animation: bad"));
        assert!(out.contains(r#"style="background-color: #f8fafc;""#));
        assert!(out.contains(r#"data-merman-postprocess="scoped-css""#));
        assert!(out.contains("#raw .node { fill: red; }"));
    }
}
