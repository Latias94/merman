//! Writes a standalone SVG and paints it with the downstream application's resvg.

use std::{env, error::Error, fs, path::PathBuf, sync::Arc};

use merman::svg::export::NativeFontContext;
use merman::svg::{FontStack, SvgPipeline};
use merman::{
    MermaidEngine, OperationControl, RenderOutput, RenderRequest, Renderer,
    SourcePresentationConfig, SvgEnvironment, SvgRequest,
};
use resvg::{tiny_skia, usvg};

const SOURCE: &str = "classDiagram\nclass Animal {\n+String nom\n+manger()\n}\nAnimal <|-- Vache";

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let directory = PathBuf::from(
        args.next()
            .ok_or("usage: standalone-resvg OUTPUT_DIRECTORY FONT_FILE [FONT_FILE ...]")?,
    );
    let font_bytes = args.map(fs::read).collect::<Result<Vec<_>, _>>()?;
    let control = OperationControl::new();
    let fonts = NativeFontContext::from_font_bytes(
        font_bytes.iter().cloned(),
        Default::default(),
        &control,
    )?;

    // Build the CSS stack from the family names resvg reads from the same font files.
    // Regular and bold faces of one family share a single entry in the stack.
    let mut paint = usvg::Options::default();
    let db = paint.fontdb_mut();
    let mut families = Vec::new();
    for bytes in font_bytes {
        let ids = db.load_font_source(usvg::fontdb::Source::Binary(Arc::new(bytes)));
        if ids.is_empty() {
            return Err("resvg could not load a supplied font".into());
        }
        for id in ids {
            let (family, _) = db
                .face(id)
                .and_then(|face| face.families.first())
                .ok_or("missing font family name")?;
            if !families.contains(family) {
                families.push(family.clone());
            }
        }
    }
    let stack = FontStack::new(families)?.as_css();
    let config = SourcePresentationConfig::from_value(serde_json::json!({
        "fontFamily": stack,
        "themeVariables": { "fontFamily": stack }
    }));
    let renderer =
        Renderer::new().with_presentation_engine(MermaidEngine::new().with_site_config(config));
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic().with_native_font_context(fonts)?,
        pipeline: Some(SvgPipeline::resvg_safe()),
        ..Default::default()
    };
    let RenderOutput::Svg(Some(svg)) =
        renderer.render(RenderRequest::svg(SOURCE, control, request))?
    else {
        return Err("no Mermaid diagram detected".into());
    };

    let tree = usvg::Tree::from_str(svg.svg(), &paint)?;
    let size = tree.size().to_int_size();
    let mut pixmap =
        tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("invalid image size")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    fs::create_dir_all(&directory)?;
    fs::write(directory.join("class.svg"), svg.svg())?;
    pixmap.save_png(directory.join("class.png"))?;
    eprintln!(
        "Wrote class.svg and class.png to {} with {stack}",
        directory.display()
    );
    Ok(())
}
