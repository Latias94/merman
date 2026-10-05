//! Native worker example: keep a control handle, then cancel superseded preview jobs.
//! Run normally to print SVG; pass `--cancel` to cancel the job before execution.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use merman::svg::{RenderResourcePolicy, SvgPipeline};
    use merman::{
        OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgEnvironment,
        SvgRequest,
    };
    use std::{thread, time::Duration};

    let source = String::from("flowchart LR\n  Edit --> Preview --> Display\n");
    let control = OperationControl::new();

    // An editor keeps this handle with its job and calls cancel() when the source changes.
    // This switch demonstrates a superseded job that has not started running yet.
    if std::env::args().any(|argument| argument == "--cancel") {
        control.cancel();
    }
    let worker_control = control.clone();
    let worker = thread::spawn(move || -> Result<String, RenderError> {
        // Start the execution deadline inside the worker, after any host queue wait.
        let control = worker_control.with_deadline(Duration::from_secs(2));
        let request = SvgRequest {
            environment: SvgEnvironment::deterministic()
                .with_resource_policy(RenderResourcePolicy::interactive()),
            pipeline: Some(SvgPipeline::resvg_safe()),
            ..SvgRequest::default()
        };
        let output = Renderer::new().render(RenderRequest::svg(&source, control, request))?;
        match output {
            RenderOutput::Svg(Some(svg)) => Ok(svg.into_parts().0),
            _ => Err(RenderError::NoDiagram),
        }
    });

    // A GUI receives completion asynchronously and checks its document revision before display.
    // The command-line example joins one worker; it does not create a hidden scheduler.
    let result = worker
        .join()
        .map_err(|_| std::io::Error::other("render worker panicked"))?;
    match result {
        Ok(svg) => print!("{svg}"),
        Err(RenderError::Cancelled(cancelled)) => {
            eprintln!("preview cancelled: {cancelled}");
        }
        Err(error @ RenderError::ResourceLimitExceeded(_)) => {
            eprintln!("preview exceeded its resource policy: {error}");
            return Err(error.into());
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn main() {
    eprintln!("render_cancellable demonstrates native threads; use a host Worker on the web");
}
