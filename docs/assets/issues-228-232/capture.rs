use std::{collections::BTreeSet, env, error::Error, fs, path::PathBuf};

use merman_core::OperationControl;
use merman_export::NativeFontOptions;
use serde_json::json;

fn text_paints(tree: &usvg::Tree, id: &str) -> BTreeSet<String> {
    let Some(usvg::Node::Text(text)) = tree.node_by_id(id) else {
        return BTreeSet::new();
    };
    text.chunks()
        .iter()
        .flat_map(|chunk| chunk.spans())
        .filter_map(|span| match span.fill()?.paint() {
            usvg::Paint::Color(color) => Some(format!(
                "#{:02x}{:02x}{:02x}",
                color.red, color.green, color.blue
            )),
            _ => None,
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    let lane = &args[0];
    let inputs = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let fixtures = PathBuf::from(&args[3]);
    fs::create_dir_all(&output)?;
    for family in [
        "c4", "gantt", "wardley", "zenuml", "quadrant", "er", "timeline",
    ] {
        let source = fs::read(inputs.join(format!("{family}.mmd")))?;
        let mut options = json!({"svg": {
            "diagram_id": format!("review-{family}"), "pipeline": "resvg-safe",
            "root_background_color": "#1e1f22"
        }});
        if lane == "main" {
            options["theme"] = json!({"preset":"editor-dark"});
        } else {
            options["presentation"] = json!({"theme":{"preset":"editor-dark"}});
        }
        let options = serde_json::to_vec_pretty(&options)?;
        fs::write(output.join(format!("{family}.json")), &options)?;
        fs::write(
            output.join(format!("{family}.svg")),
            merman_bindings_core::render_svg(&source, &options)
                .map_err(|error| format!("{family}/svg: {error:?}"))?,
        )?;
        fs::write(
            output.join(format!("{family}.png")),
            merman_bindings_core::render_png(&source, &options)
                .map_err(|error| format!("{family}/png: {error:?}"))?,
        )?;
        println!("{lane}/{family}: public binding SVG + PNG");
        if family == "gantt" {
            let context = NativeFontOptions::default()
                .with_font_file(fixtures.join("Cousine-Regular.ttf"))
                .load(&OperationControl::new())?;
            let mut native = usvg::Options::default();
            context.apply_to_usvg_options(&mut native);
            let svg = fs::read(output.join("gantt.svg"))?;
            let tree = usvg::Tree::from_data(&svg, &native)?;
            let paints: std::collections::BTreeMap<_, _> = (1..=5)
                .map(|task| {
                    let id = format!("review-gantt-t{task}-text");
                    let paints = text_paints(&tree, &id);
                    (id, paints)
                })
                .collect();
            fs::write(
                output.join("gantt-native-paints.json"),
                serde_json::to_vec_pretty(&paints)?,
            )?;
            println!("{lane}/gantt: usvg text paints {paints:?}");
        }
    }
    for (name, fonts, family, label) in [
        (
            "font-generic",
            vec!["MermanSymbolicTest.ttf", "Cousine-Regular.ttf"],
            "serif",
            "Hello serif / manage call",
        ),
        (
            "font-glyph",
            vec![
                "NotoSans-Latin-subset.ttf",
                "MermanSymbolicTest.ttf",
                "Cousine-Regular.ttf",
            ],
            "Noto Sans",
            "Aam / manage call",
        ),
    ] {
        let mut source = NativeFontOptions::default();
        for font in &fonts {
            source = source.with_font_file(fixtures.join(font));
        }
        let context = source.load(&OperationControl::new())?;
        let mut options = usvg::Options::default();
        context.apply_to_usvg_options(&mut options);
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="660" height="130"><rect width="660" height="130" fill="#fff"/><text id="probe" x="24" y="78" font-family="{family}" font-size="36" fill="#17212f">{label}</text></svg>"##
        );
        let tree = usvg::Tree::from_str(&svg, &options)?;
        let Some(usvg::Node::Text(text)) = tree.node_by_id("probe") else {
            return Err("missing text".into());
        };
        let selected: BTreeSet<_> = text
            .layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .map(|glyph| {
                tree.fontdb()
                    .face(glyph.font)
                    .unwrap()
                    .post_script_name
                    .clone()
            })
            .collect();
        let mut pixmap = tiny_skia::Pixmap::new(660, 130).ok_or("pixmap allocation failed")?;
        resvg::render(
            &tree,
            tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        pixmap.save_png(output.join(format!("{name}.png")))?;
        fs::write(output.join(format!("{name}.svg")), svg)?;
        fs::write(
            output.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&json!({
                "family":family, "text":label, "font_fixtures":fonts, "painted_faces":selected,
                "path":"NativeFontContext::apply_to_usvg_options -> usvg -> resvg"
            }))?,
        )?;
        context.check()?;
        println!("{lane}/{name}: painted {selected:?}");
    }
    Ok(())
}
