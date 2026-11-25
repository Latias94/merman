//! Exclusive phase scopes for the native workload harnesses.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Preprocess,
    Parse,
    ThemePrepare,
    Measure,
    LayoutPrepare,
    LayoutSolve,
    SvgEmit,
    CssEmit,
    Postprocess,
}

impl Stage {
    pub const ALL: [Self; 9] = [
        Self::Preprocess,
        Self::Parse,
        Self::ThemePrepare,
        Self::Measure,
        Self::LayoutPrepare,
        Self::LayoutSolve,
        Self::SvgEmit,
        Self::CssEmit,
        Self::Postprocess,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Preprocess => "preprocess",
            Self::Parse => "parse",
            Self::ThemePrepare => "theme_prepare",
            Self::Measure => "measure",
            Self::LayoutPrepare => "layout_prepare",
            Self::LayoutSolve => "layout_solve",
            Self::SvgEmit => "svg_emit",
            Self::CssEmit => "css_emit",
            Self::Postprocess => "postprocess",
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct StageTiming {
    pub elapsed: Duration,
    pub calls: usize,
}

pub struct Interval {
    pub stage: Stage,
    pub offset: Duration,
    pub elapsed: Duration,
}

pub struct Timing {
    pub stages: [StageTiming; Stage::ALL.len()],
    pub end_to_end: Duration,
    pub trace: Vec<Interval>,
}

struct ActiveTiming {
    start: Instant,
    cursor: Instant,
    stage: Option<Stage>,
    stages: [StageTiming; Stage::ALL.len()],
    trace: Option<Vec<Interval>>,
}

impl ActiveTiming {
    fn switch(&mut self, next: Option<Stage>) -> Option<Stage> {
        let boundary = Instant::now();
        self.close_interval(boundary);
        let previous = std::mem::replace(&mut self.stage, next);
        // Trace storage, including allocation, belongs to the unattributed observer overhead.
        self.cursor = if self.trace.is_some() {
            Instant::now()
        } else {
            boundary
        };
        previous
    }

    fn close_interval(&mut self, end: Instant) {
        if let Some(stage) = self.stage {
            let elapsed = end - self.cursor;
            self.stages[stage as usize].elapsed += elapsed;
            if let Some(trace) = &mut self.trace {
                trace.push(Interval {
                    stage,
                    offset: self.cursor - self.start,
                    elapsed,
                });
            }
        }
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "Capturing scopes must not allocate"
)]
enum ActiveCapture {
    Timing(ActiveTiming),
    Observing {
        stage: Option<Stage>,
        on_change: fn(Option<Stage>),
    },
}

impl ActiveCapture {
    fn switch(&mut self, next: Option<Stage>) -> Option<Stage> {
        match self {
            Self::Timing(timing) => timing.switch(next),
            Self::Observing { stage, on_change } => {
                let previous = std::mem::replace(stage, next);
                if next != previous {
                    on_change(next);
                }
                previous
            }
        }
    }
}

thread_local! {
    static ACTIVE: RefCell<Option<ActiveCapture>> = const { RefCell::new(None) };
}

struct RestoreCapture(Option<ActiveCapture>);

impl Drop for RestoreCapture {
    fn drop(&mut self) {
        ACTIVE.set(self.0.take());
    }
}

/// Records instrumented scopes on the current thread during one operation or ordered batch.
pub fn capture<R>(trace: bool, render: impl FnOnce() -> R) -> (R, Timing) {
    let start = Instant::now();
    let restore = RestoreCapture(ACTIVE.replace(Some(ActiveCapture::Timing(ActiveTiming {
        start,
        cursor: start,
        stage: None,
        stages: [StageTiming::default(); Stage::ALL.len()],
        trace: trace.then(Vec::new),
    }))));
    let response = render();
    let end = Instant::now();
    let Some(ActiveCapture::Timing(mut active)) = ACTIVE.take() else {
        unreachable!("phase timing capture is active");
    };
    active.close_interval(end);
    drop(restore);
    (
        response,
        Timing {
            stages: active.stages,
            end_to_end: end - start,
            trace: active.trace.unwrap_or_default(),
        },
    )
}

/// Observes phase changes on this thread without recording time or trace intervals.
/// The operation begins outside any phase. A change to `None` resumes unscoped work.
/// The observer runs at the boundary and must not enter a phase itself.
pub fn observe_stage_changes<R>(on_change: fn(Option<Stage>), operation: impl FnOnce() -> R) -> R {
    let _restore = RestoreCapture(ACTIVE.replace(Some(ActiveCapture::Observing {
        stage: None,
        on_change,
    })));
    operation()
}

#[must_use = "keep the scope alive for the work it measures"]
pub struct Scope {
    previous: Option<Option<Stage>>,
    _thread_bound: PhantomData<Rc<()>>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(previous) = self.previous {
            ACTIVE.with_borrow_mut(|active| {
                if let Some(active) = active {
                    active.switch(previous);
                }
            });
        }
    }
}

/// Attributes this scope exclusively while a capture is active.
pub fn enter(stage: Stage) -> Scope {
    let previous = ACTIVE.with_borrow_mut(|active| {
        let active = active.as_mut()?;
        if let ActiveCapture::Timing(timing) = active {
            timing.stages[stage as usize].calls += 1;
        }
        Some(active.switch(Some(stage)))
    });
    Scope {
        previous,
        _thread_bound: PhantomData,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_scopes_report_disjoint_intervals() {
        let (_, timing) = capture(true, || {
            let _layout = enter(Stage::LayoutSolve);
            {
                let _labels = enter(Stage::Measure);
            }
        });

        assert_eq!(timing.stages[Stage::LayoutSolve as usize].calls, 1);
        assert_eq!(timing.stages[Stage::Measure as usize].calls, 1);
        assert_eq!(
            timing
                .trace
                .iter()
                .map(|span| span.stage.id())
                .collect::<Vec<_>>(),
            ["layout_solve", "measure", "layout_solve"]
        );
        assert!(
            timing
                .stages
                .iter()
                .map(|row| row.elapsed)
                .sum::<Duration>()
                <= timing.end_to_end
        );
    }
}
