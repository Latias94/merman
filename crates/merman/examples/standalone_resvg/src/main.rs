//! Writes a standalone SVG and paints it with the downstream application's resvg.

use std::{
    collections::HashSet,
    env,
    error::Error,
    fs,
    path::PathBuf,
    sync::{Arc, OnceLock},
};

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
    let (svg, tree, stack) = render_class(font_bytes)?;
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

fn render_class(
    font_bytes: Vec<Vec<u8>>,
) -> Result<(merman::SvgOutput, usvg::Tree, String), Box<dyn Error>> {
    let control = OperationControl::new();
    let fonts = NativeFontContext::from_font_bytes(
        font_bytes.iter().cloned(),
        Default::default(),
        &control,
    )?
    .with_strict_glyph_coverage(true);
    let fallback = OnceLock::new();
    let mut paint = usvg::Options::default();
    let db = paint.fontdb_mut();
    let mut sources = HashSet::new();
    let mut families = Vec::new();
    for bytes in font_bytes {
        let bytes = Arc::new(bytes);
        if !sources.insert(Arc::clone(&bytes)) {
            continue;
        }
        for id in db.load_font_source(usvg::fontdb::Source::Binary(bytes)) {
            let (family, _) = db
                .face(id)
                .and_then(|face| face.families.first())
                .ok_or("missing font family name")?;
            if !families.contains(family) {
                families.push(family.clone());
            }
        }
    }
    if fonts.face_count() != db.len() {
        return Err("The font databases admitted different faces. Supply one complete face per family, style and weight, including faces inside collections.".into());
    }
    let stack = FontStack::new(families)?.as_css();
    // The two resolvers can substitute different faces for the same missing glyph.
    paint.font_resolver.select_fallback = Box::new(|ch, _, _| {
        fallback.get_or_init(|| ch);
        None
    });
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
    if let Some(ch) = fallback.get() {
        return Err(format!(
            "resvg needs a fallback for {ch:?}. Put a text family covering the Class labels first."
        )
        .into());
    }
    Ok((svg, tree, stack))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(files: &[&str]) -> Result<(merman::SvgOutput, usvg::Tree, String), Box<dyn Error>> {
        let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../merman-export/tests/fonts");
        render_class(
            files
                .iter()
                .map(|file| fs::read(fixtures.join(file)))
                .collect::<Result<_, _>>()?,
        )
    }

    #[test]
    fn repeated_source_and_separate_weights_paint_with_the_measured_faces() {
        let (svg, tree, stack) = render(&[
            "Cousine-Regular.ttf",
            "Cousine-Regular.ttf",
            "Cousine-Bold.ttf",
        ])
        .unwrap();
        assert_eq!(stack, "Cousine");
        let measured = svg
            .evidence()
            .native_font_diagnostics()
            .unwrap()
            .selected_faces();
        let mut groups = vec![tree.root()];
        let mut labels = Vec::new();
        while let Some(group) = groups.pop() {
            for node in group.children() {
                match node {
                    usvg::Node::Group(group) => groups.push(group),
                    usvg::Node::Text(text) => {
                        let label = text.chunks()[0].text();
                        let expected = match label {
                            "Animal" | "Vache" => "Cousine-Bold",
                            "+String nom" | "+manger()" => "Cousine-Regular",
                            _ => continue,
                        };
                        assert!(measured.contains(&format!("{expected}#0")));
                        let glyphs = text
                            .layouted()
                            .iter()
                            .flat_map(|span| &span.positioned_glyphs);
                        for glyph in glyphs {
                            assert_eq!(
                                tree.fontdb().face(glyph.font).unwrap().post_script_name,
                                expected
                            );
                        }
                        assert!(text.flattened().bounding_box().is_some());
                        labels.push(label);
                    }
                    _ => {}
                }
            }
        }
        labels.sort_unstable();
        assert_eq!(labels, ["+String nom", "+manger()", "Animal", "Vache"]);
    }

    #[test]
    fn overlapping_collection_and_implicit_glyph_substitution_explain_the_fix() {
        for (files, message) in [
            (
                &["Cousine-test-collection.ttc", "Cousine-Regular.ttf"][..],
                "one complete face",
            ),
            (
                &[
                    "NotoSans-Latin-subset.ttf",
                    "MermanSymbolicTest.ttf",
                    "Cousine-Regular.ttf",
                ][..],
                "covering the Class labels first",
            ),
        ] {
            let error = render(files).err().expect("ambiguous inputs must fail");
            assert!(error.to_string().contains(message), "{error}");
        }
    }
}
