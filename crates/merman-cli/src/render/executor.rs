use super::plan::RenderPlan;
#[cfg(feature = "analysis")]
use super::svg_pipeline::svg_metadata;
use super::svg_pipeline::svg_output_policy;
use crate::cli::{RenderFormat, SvgPipelineKind};
use crate::config::renderer_for;
use crate::error::CliError;
use crate::io::write_output;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman::svg::ResvgCompatibleSvg;
use merman::svg::{HeadlessRenderer, SvgPipeline};
#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
use std::sync::{Condvar, Mutex};

#[cfg(all(
    feature = "parallel-markdown",
    feature = "pdf",
    not(feature = "png"),
    not(feature = "jpeg")
))]
const DEFAULT_ENCODING_PARALLEL_BUDGET_BYTES: u64 = 512 * 1024 * 1024;

pub(super) struct RenderRequest<'a> {
    pub(super) plan: &'a RenderPlan,
    pub(super) renderer: HeadlessRenderer,
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    pub(super) encoding_parallel_budget: Option<EncodingParallelBudget>,
    pipeline: SvgPipeline,
}

pub(super) struct RenderedArtifact {
    pub(super) bytes: Vec<u8>,
    #[cfg(feature = "analysis")]
    pub(super) title: Option<String>,
    #[cfg(feature = "analysis")]
    pub(super) desc: Option<String>,
}

pub(crate) fn run_render(plan: RenderPlan) -> Result<(), CliError> {
    plan.warn_for_accepted_compat_options();
    let text = crate::io::read_input(plan.input.as_deref(), plan.quiet)?;
    let renderer = renderer_for(&plan.parse, &plan.render, plan.icon_registry())?;
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    let request = RenderRequest::new(&plan, renderer, encoding_parallel_budget(&plan));
    #[cfg(not(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    )))]
    let request = RenderRequest::new(&plan, renderer);

    #[cfg(feature = "analysis")]
    if plan.is_mmdc_markdown_input() {
        return request.render_markdown(&text);
    }

    request.render(&text)
}

#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
fn encoding_parallel_budget(plan: &RenderPlan) -> Option<EncodingParallelBudget> {
    if !plan.format.requires_svg_encoding() {
        return None;
    }
    #[cfg(any(feature = "png", feature = "jpeg"))]
    {
        Some(EncodingParallelBudget::new(
            plan.raster.encoding_parallel_budget_bytes(),
        ))
    }
    #[cfg(all(not(feature = "png"), not(feature = "jpeg"), feature = "pdf"))]
    {
        Some(EncodingParallelBudget::new(
            DEFAULT_ENCODING_PARALLEL_BUDGET_BYTES,
        ))
    }
}

fn pipeline_kind(plan: &RenderPlan) -> SvgPipelineKind {
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    if plan.format.requires_svg_encoding() {
        return SvgPipelineKind::ResvgSafe;
    }
    plan.svg_pipeline.unwrap_or(SvgPipelineKind::Parity)
}

impl<'a> RenderRequest<'a> {
    #[cfg(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    ))]
    pub(super) fn new(
        plan: &'a RenderPlan,
        renderer: HeadlessRenderer,
        encoding_parallel_budget: Option<EncodingParallelBudget>,
    ) -> Self {
        let kind = pipeline_kind(plan);
        let pipeline =
            svg_output_policy(kind, plan.background.as_deref(), plan.css.as_deref()).pipeline();
        Self {
            plan,
            renderer,
            encoding_parallel_budget,
            pipeline,
        }
    }

    #[cfg(not(all(
        feature = "parallel-markdown",
        any(feature = "png", feature = "jpeg", feature = "pdf")
    )))]
    pub(super) fn new(plan: &'a RenderPlan, renderer: HeadlessRenderer) -> Self {
        let kind = pipeline_kind(plan);
        let pipeline =
            svg_output_policy(kind, plan.background.as_deref(), plan.css.as_deref()).pipeline();
        Self {
            plan,
            renderer,
            pipeline,
        }
    }

    fn render(&self, text: &str) -> Result<(), CliError> {
        let artifact = self.render_artifact(text)?;
        write_output(self.plan.output.as_ref(), &artifact.bytes)
    }

    pub(super) fn render_artifact(&self, text: &str) -> Result<RenderedArtifact, CliError> {
        #[cfg(feature = "ascii")]
        if self.plan.format.is_text() {
            return self.render_text(text);
        }

        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        if text.trim_start().starts_with("<svg") && self.plan.format.requires_svg_encoding() {
            let svg = self.prepare_raw_svg_for_encoding(text)?;
            return self.encode_prepared_svg(&svg);
        }

        match self.plan.format {
            RenderFormat::Svg => {
                let Some(svg) = self
                    .renderer
                    .render_svg_with_pipeline_sync(text, self.postprocess_pipeline())?
                else {
                    return Err(CliError::NoDiagram);
                };
                Ok(RenderedArtifact::from_svg(svg))
            }
            #[cfg(feature = "ascii")]
            RenderFormat::Ascii | RenderFormat::Unicode => unreachable!("handled above"),
            #[cfg(feature = "png")]
            RenderFormat::Png => self.render_encoded_svg(text),
            #[cfg(feature = "jpeg")]
            RenderFormat::Jpeg => self.render_encoded_svg(text),
            #[cfg(feature = "pdf")]
            RenderFormat::Pdf => self.render_encoded_svg(text),
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn render_encoded_svg(&self, text: &str) -> Result<RenderedArtifact, CliError> {
        let Some(svg) = self
            .renderer
            .render_resvg_compatible_svg_with_pipeline_sync(text, self.postprocess_pipeline())?
        else {
            return Err(CliError::NoDiagram);
        };
        self.encode_prepared_svg(&svg)
    }

    pub(super) const fn postprocess_pipeline(&self) -> &SvgPipeline {
        &self.pipeline
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn prepare_raw_svg_for_encoding(&self, svg: &str) -> Result<ResvgCompatibleSvg, CliError> {
        let session = self
            .renderer
            .environment()
            .begin_session()
            .map_err(merman::svg::HeadlessError::from)?;
        Ok(self
            .pipeline
            .process_resvg_compatible(svg, &session)
            .map_err(merman::svg::HeadlessError::from)?)
    }

    #[cfg(feature = "ascii")]
    fn render_text(&self, text: &str) -> Result<RenderedArtifact, CliError> {
        let options = match self.plan.format {
            RenderFormat::Ascii => merman::ascii::AsciiRenderOptions::ascii(),
            RenderFormat::Unicode => merman::ascii::AsciiRenderOptions::unicode(),
            _ => unreachable!("only text formats reach render_text"),
        };
        let options = self.plan.apply_text_options(options)?;
        let ascii_renderer = merman::ascii::HeadlessAsciiRenderer::new()
            .with_engine(self.renderer.engine().clone())
            .with_parse_options(self.renderer.parse_options())
            .with_ascii_options(options)
            .with_resource_profile(self.plan.render.resource_profile);
        let Some(rendered) = ascii_renderer.render_ascii_sync(text)? else {
            return Err(CliError::NoDiagram);
        };
        Ok(RenderedArtifact {
            bytes: rendered.into_bytes(),
            #[cfg(feature = "analysis")]
            title: None,
            #[cfg(feature = "analysis")]
            desc: None,
        })
    }

    #[cfg(any(
        feature = "analysis",
        feature = "png",
        feature = "jpeg",
        feature = "pdf"
    ))]
    pub(super) fn info(&self, message: &str) {
        if !self.plan.quiet {
            eprintln!("{message}");
        }
    }
}

impl RenderedArtifact {
    fn from_svg(svg: String) -> Self {
        #[cfg(feature = "analysis")]
        let (title, desc) = svg_metadata(&svg);
        Self {
            bytes: svg.into_bytes(),
            #[cfg(feature = "analysis")]
            title,
            #[cfg(feature = "analysis")]
            desc,
        }
    }
}

#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
pub(super) struct EncodingParallelBudget {
    capacity: u64,
    in_flight: Mutex<u64>,
    capacity_changed: Condvar,
}

#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
impl EncodingParallelBudget {
    pub(super) fn new(capacity: u64) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            in_flight: Mutex::new(0),
            capacity_changed: Condvar::new(),
        }
    }

    pub(super) fn acquire(&self, requested: u64) -> EncodingParallelPermit<'_> {
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

#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
pub(super) struct EncodingParallelPermit<'a> {
    budget: &'a EncodingParallelBudget,
    #[cfg(test)]
    pub(super) weight: u64,
    #[cfg(not(test))]
    weight: u64,
}

#[cfg(all(
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
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

#[cfg(all(
    test,
    feature = "parallel-markdown",
    any(feature = "png", feature = "jpeg", feature = "pdf")
))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

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

    #[cfg(all(feature = "pdf", not(feature = "png"), not(feature = "jpeg")))]
    #[test]
    fn default_parallel_budget_stays_bounded() {
        assert_eq!(DEFAULT_ENCODING_PARALLEL_BUDGET_BYTES, 512 * 1024 * 1024);
    }
}
