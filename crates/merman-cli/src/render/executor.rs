use super::plan::RenderPlan;
#[cfg(feature = "markdown")]
use super::svg_pipeline::svg_metadata;
use super::svg_pipeline::svg_output_policy;
use crate::cli::{RenderFormat, SvgPipelineKind};
use crate::config::renderer_for;
use crate::error::CliError;
use crate::input::InputLimit;
use crate::io::write_output;
#[cfg(feature = "markdown")]
use crate::resources::{ByteLedgerKind, CheckedBytes};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use crate::resources::{CheckedSchedulingWeight, ResourceLedgerError};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman::svg::ResvgCompatibleSvg;
use merman::svg::{HeadlessRenderer, SvgPipeline};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use std::sync::Condvar;
#[cfg(any(
    feature = "markdown",
    feature = "png",
    feature = "jpeg",
    feature = "pdf"
))]
use std::sync::Mutex;

pub(super) struct RenderRequest<'a> {
    pub(super) plan: &'a RenderPlan,
    pub(super) renderer: HeadlessRenderer,
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(super) scheduling_weight_budget: Option<SchedulingWeightBudget>,
    #[cfg(feature = "markdown")]
    pub(super) staged_bytes: Mutex<CheckedBytes>,
    pipeline: SvgPipeline,
}

pub(super) struct RenderedArtifact {
    pub(super) bytes: Vec<u8>,
    #[cfg(feature = "markdown")]
    pub(super) title: Option<String>,
    #[cfg(feature = "markdown")]
    pub(super) desc: Option<String>,
}

pub(crate) fn run_render(plan: RenderPlan) -> Result<(), CliError> {
    plan.warn_for_accepted_compat_options();
    #[cfg(feature = "markdown")]
    let input_limit = if plan.is_markdown_input() {
        InputLimit::new(
            crate::resources::CliResourceLimitId::MaxMarkdownDocumentBytes.as_str(),
            plan.resources.files().markdown_document_bytes,
        )
    } else if plan.input_kind_is_raw_svg() {
        InputLimit::new(
            merman::svg::ResourceLimitId::MaxSvgBytes.as_str(),
            plan.resources
                .render_policy()
                .value(merman::svg::ResourceLimitId::MaxSvgBytes),
        )
    } else {
        InputLimit::new(
            merman::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
            plan.resources
                .input_policy()
                .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
        )
    };
    #[cfg(not(feature = "markdown"))]
    let input_limit = if plan.input_kind_is_raw_svg() {
        InputLimit::new(
            merman::svg::ResourceLimitId::MaxSvgBytes.as_str(),
            plan.resources
                .render_policy()
                .value(merman::svg::ResourceLimitId::MaxSvgBytes),
        )
    } else {
        InputLimit::new(
            merman::resources::InputResourceLimitId::MaxSourceBytes.as_str(),
            plan.resources
                .input_policy()
                .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
        )
    };
    let text = crate::io::read_input(
        plan.input.as_deref(),
        !plan.warn_on_implicit_stdin,
        input_limit,
    )?;
    let renderer = renderer_for(
        &plan.parse,
        &plan.render,
        plan.icon_registry(),
        &plan.resources,
    )?;
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    let request = RenderRequest::new(&plan, renderer, scheduling_weight_budget(&plan));
    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    let request = RenderRequest::new(&plan, renderer);

    #[cfg(feature = "markdown")]
    if plan.is_markdown_input() {
        return request.render_markdown(&text);
    }

    request.render(&text)
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn scheduling_weight_budget(plan: &RenderPlan) -> Option<SchedulingWeightBudget> {
    if !plan.format.requires_svg_encoding() {
        return None;
    }
    Some(SchedulingWeightBudget::new(
        plan.resources.checked_scheduling_weight(),
    ))
}

fn pipeline_kind(plan: &RenderPlan) -> SvgPipelineKind {
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    if plan.format.requires_svg_encoding() {
        return SvgPipelineKind::ResvgSafe;
    }
    plan.svg_pipeline.unwrap_or(SvgPipelineKind::Parity)
}

impl<'a> RenderRequest<'a> {
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(super) fn new(
        plan: &'a RenderPlan,
        renderer: HeadlessRenderer,
        scheduling_weight_budget: Option<SchedulingWeightBudget>,
    ) -> Self {
        let kind = pipeline_kind(plan);
        let pipeline =
            svg_output_policy(kind, plan.background.as_deref(), plan.css.as_deref()).pipeline();
        Self {
            plan,
            renderer,
            scheduling_weight_budget,
            #[cfg(feature = "markdown")]
            staged_bytes: Mutex::new(plan.resources.checked_bytes(ByteLedgerKind::StagedOutput)),
            pipeline,
        }
    }

    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    pub(super) fn new(plan: &'a RenderPlan, renderer: HeadlessRenderer) -> Self {
        let kind = pipeline_kind(plan);
        let pipeline =
            svg_output_policy(kind, plan.background.as_deref(), plan.css.as_deref()).pipeline();
        Self {
            plan,
            renderer,
            #[cfg(feature = "markdown")]
            staged_bytes: Mutex::new(plan.resources.checked_bytes(ByteLedgerKind::StagedOutput)),
            pipeline,
        }
    }

    fn render(&self, text: &str) -> Result<(), CliError> {
        let artifact = self.render_artifact(text)?;
        write_output(
            self.plan.output.as_ref(),
            &artifact.bytes,
            &self.plan.publications,
        )
    }

    pub(super) fn render_artifact(&self, text: &str) -> Result<RenderedArtifact, CliError> {
        #[cfg(feature = "ascii")]
        if self.plan.format.is_text() {
            return self.render_text(text);
        }

        #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
        if self.plan.input_kind == crate::cli::RenderInputKind::Svg
            && self.plan.format.requires_svg_encoding()
        {
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
            .with_resource_policy(*self.plan.resources.input_policy());
        let Some(rendered) = ascii_renderer.render_ascii_sync(text)? else {
            return Err(CliError::NoDiagram);
        };
        Ok(RenderedArtifact {
            bytes: rendered.into_bytes(),
            #[cfg(feature = "markdown")]
            title: None,
            #[cfg(feature = "markdown")]
            desc: None,
        })
    }

    #[cfg(any(
        feature = "markdown",
        feature = "png",
        feature = "jpeg",
        feature = "pdf"
    ))]
    pub(super) fn info(&self, message: &str) {
        crate::diagnostics::DiagnosticSink::new(self.plan.quiet).info(message);
    }

    #[cfg(feature = "markdown")]
    pub(super) fn charge_staged_bytes(&self, bytes: usize) -> Result<(), CliError> {
        let bytes = u64::try_from(bytes)
            .map_err(|_| CliError::InvalidOutput("staged output size overflow".to_string()))?;
        self.staged_bytes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_add(bytes)?;
        Ok(())
    }
}

impl RenderedArtifact {
    fn from_svg(svg: String) -> Self {
        #[cfg(feature = "markdown")]
        let (title, desc) = svg_metadata(&svg);
        Self {
            bytes: svg.into_bytes(),
            #[cfg(feature = "markdown")]
            title,
            #[cfg(feature = "markdown")]
            desc,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(super) struct SchedulingWeightBudget {
    in_flight: Mutex<CheckedSchedulingWeight>,
    capacity_changed: Condvar,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl SchedulingWeightBudget {
    pub(super) fn new(in_flight: CheckedSchedulingWeight) -> Self {
        Self {
            in_flight: Mutex::new(in_flight),
            capacity_changed: Condvar::new(),
        }
    }

    pub(super) fn acquire(
        &self,
        requested: u64,
    ) -> Result<SchedulingWeightPermit<'_>, ResourceLedgerError> {
        let mut in_flight = self
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        in_flight.check_single(requested)?;
        loop {
            match in_flight.try_acquire(requested) {
                Ok(()) => break,
                Err(ResourceLedgerError::LimitExceeded { .. }) => {
                    in_flight = self
                        .capacity_changed
                        .wait(in_flight)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(SchedulingWeightPermit {
            budget: self,
            weight: requested,
        })
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(super) struct SchedulingWeightPermit<'a> {
    budget: &'a SchedulingWeightBudget,
    #[cfg(test)]
    pub(super) weight: u64,
    #[cfg(not(test))]
    weight: u64,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
impl Drop for SchedulingWeightPermit<'_> {
    fn drop(&mut self) {
        let mut in_flight = self
            .budget
            .in_flight
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        in_flight
            .release(self.weight)
            .expect("a live scheduling permit must own its charged weight");
        self.budget.capacity_changed.notify_all();
    }
}

#[cfg(all(test, any(feature = "png", feature = "jpeg", feature = "pdf")))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::time::Duration;

    #[test]
    fn scheduling_weight_budget_blocks_until_weight_is_released() {
        let mut policy = crate::resources::ResolvedResourcePolicy::for_profile(
            merman::resources::ResourceProfile::Constrained,
        );
        policy
            .apply_override("max_scheduling_weight_bytes", 10)
            .unwrap();
        let budget = Arc::new(SchedulingWeightBudget::new(
            policy.checked_scheduling_weight(),
        ));
        let first = budget.acquire(8).unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let worker_budget = Arc::clone(&budget);
        let worker = std::thread::spawn(move || {
            ready_tx.send(()).unwrap();
            let _second = worker_budget.acquire(6).unwrap();
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
    fn oversized_scheduling_job_is_rejected() {
        let mut policy = crate::resources::ResolvedResourcePolicy::for_profile(
            merman::resources::ResourceProfile::Constrained,
        );
        policy
            .apply_override("max_scheduling_weight_bytes", 10)
            .unwrap();
        let budget = SchedulingWeightBudget::new(policy.checked_scheduling_weight());
        let error = match budget.acquire(100) {
            Ok(_) => panic!("oversized work item should be rejected"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ResourceLedgerError::LimitExceeded {
                limit: "max_scheduling_weight_bytes",
                actual: 100,
                max: 10,
                ..
            }
        ));
    }
}
