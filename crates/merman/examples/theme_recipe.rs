use merman::diagram_theme::{
    DiagramThemeCompiler, ThemeDesignTokenValueV1, ThemePreset, ThemeRecipeV1, ThemeValueV1,
};
use merman::svg::SvgPipeline;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
use std::path::PathBuf;

const SOURCE: &str = r#"flowchart LR
    Preset[Paper Cut] --> Recipe[Editable JSON]
    Recipe --> Tokens[Blue face sheet]
    Tokens --> Preview[Custom preview]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let output_dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/theme-recipe"));
    if args.next().is_some() {
        return Err("usage: theme_recipe [output-directory]".into());
    }

    let compiler = DiagramThemeCompiler::new();
    let mut recipe = compiler.export_preset(ThemePreset::PaperCut)?;
    std::fs::create_dir_all(&output_dir)?;
    // Save the complete schema-versioned recipe, including its editable bindings.
    std::fs::write(
        output_dir.join("paper-cut.json"),
        serde_json::to_string_pretty(&recipe)?,
    )?;

    let ThemeRecipeV1::Design { design } = &mut recipe else {
        return Err("Paper Cut must export an editable design recipe".into());
    };
    let surface = design
        .tokens
        .get_mut("surface")
        .ok_or("Paper Cut is missing its surface token")?;
    let ThemeDesignTokenValueV1::Color(color) = &mut surface.value else {
        return Err("Paper Cut's surface token must be a color".into());
    };
    // Every consumer referencing this token picks up the new color.
    *color = ThemeValueV1::Literal("#DCEAF7".to_owned());
    surface.label = Some("Blue face sheet".to_owned());
    let recipe_path = output_dir.join("theme.json");
    std::fs::write(&recipe_path, serde_json::to_string_pretty(&recipe)?)?;

    // Import the saved document as a fresh consumer would, with raw JSON preflight first.
    let raw = std::fs::read_to_string(&recipe_path)?;
    compiler.check_design_json_input(raw.as_bytes())?;
    let imported: ThemeRecipeV1 = serde_json::from_str(&raw)?;
    let theme = compiler.compile_recipe(imported)?;
    let mut request = SvgRequest::default();
    request.options.diagram_id = Some("theme-recipe-example".to_owned());
    request.pipeline = Some(SvgPipeline::resvg_safe());
    let output = Renderer::new()
        .render(RenderRequest::svg(SOURCE, OperationControl::new(), request).with_theme(theme))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no Mermaid diagram detected".into());
    };
    std::fs::write(output_dir.join("diagram.svg"), svg.svg())?;
    eprintln!(
        "wrote paper-cut.json, theme.json, and diagram.svg to {}",
        output_dir.display()
    );
    Ok(())
}
