use std::{env, error::Error, fs, path::PathBuf};

use merman::svg::export::NativeFontOptions;
use merman_bindings_core::{BindingEngine, BindingEngineServices, BindingError, OperationControl};

fn binding_error(error: BindingError) -> std::io::Error {
    std::io::Error::other(format!("{error:?}"))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() < 4 {
        return Err("usage: capture MODE SOURCE OPTIONS OUTPUT_STEM [FONT_FILES...]".into());
    }
    let services = match args[0].as_str() {
        "deterministic" if args.len() == 4 => BindingEngineServices::new(),
        "native" if args.len() > 4 => {
            let fonts = args[4..]
                .iter()
                .fold(NativeFontOptions::default(), |options, path| {
                    options.with_font_file(path)
                })
                .load(&OperationControl::new())?;
            BindingEngineServices::new().with_native_font_context(fonts)
        }
        _ => {
            return Err(
                "select deterministic without fonts, or native with explicit font files".into(),
            );
        }
    };
    let source = fs::read(&args[1])?;
    let options = fs::read(&args[2])?;
    let engine =
        BindingEngine::from_options_and_services(&options, services).map_err(binding_error)?;
    let output = PathBuf::from(&args[3]);
    fs::write(
        output.with_extension("svg"),
        engine.render_svg(&source).map_err(binding_error)?,
    )?;
    fs::write(
        output.with_extension("png"),
        engine.render_png(&source).map_err(binding_error)?,
    )?;
    fs::write(
        output.with_extension("pdf"),
        engine.render_pdf(&source).map_err(binding_error)?,
    )?;
    Ok(())
}
