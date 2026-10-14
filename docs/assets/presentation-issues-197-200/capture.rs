use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("usage: pr201-capture SOURCE OPTIONS OUTPUT_STEM".into());
    }
    let source = fs::read(&args[0])?;
    let options = fs::read(&args[1])?;
    let mut svg_options: serde_json::Value = serde_json::from_slice(&options)?;
    svg_options
        .as_object_mut()
        .ok_or("options must be a JSON object")?
        .remove("raster");
    let output = PathBuf::from(&args[2]);
    fs::write(
        output.with_extension("svg"),
        merman_bindings_core::render_svg(&source, &serde_json::to_vec(&svg_options)?)
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?,
    )?;
    fs::write(
        output.with_extension("png"),
        merman_bindings_core::render_png(&source, &options)
            .map_err(|error| std::io::Error::other(format!("{error:?}")))?,
    )?;
    Ok(())
}
