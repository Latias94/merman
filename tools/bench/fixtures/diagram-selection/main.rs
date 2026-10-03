use merman::{OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgRequest};
use std::collections::BTreeSet;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args().nth(1).ok_or("expected a source path or --families")?;
    if input == "--families" {
        let families: BTreeSet<_> = merman::diagram_family_capabilities()
            .iter()
            .filter(|family| family.has_semantic_parser && family.logical_family_kind != "error")
            .map(|family| family.logical_family_kind)
            .collect();
        for family in families {
            println!("{family}");
        }
        return Ok(());
    }
    let source = std::fs::read_to_string(input)?;
    match Renderer::new().render(RenderRequest::svg(
        &source,
        OperationControl::new(),
        SvgRequest::default(),
    )) {
        Ok(RenderOutput::Svg(Some(svg))) => print!("{}", svg.svg()),
        Ok(_) => return Err("expected a rendered SVG".into()),
        Err(RenderError::Parse(error)) => {
            eprintln!("{}", error.terminal_diagnostic_details().code);
            std::process::exit(2);
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
