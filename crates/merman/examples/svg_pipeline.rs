use merman::render::{
    HeadlessRenderer, RenderResult, SvgPipeline, SvgPostprocessContext, SvgPostprocessor,
};
use std::borrow::Cow;
use std::io::Read;

struct AddExampleMetadata;

impl SvgPostprocessor for AddExampleMetadata {
    fn name(&self) -> &'static str {
        "add-example-metadata"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> RenderResult<Cow<'a, str>> {
        let metadata = format!(
            r#"<metadata data-merman-example="svg_pipeline" data-preset="{:?}"/>"#,
            ctx.preset()
        );

        let Some(idx) = svg.rfind("</svg>") else {
            return Ok(Cow::Owned(format!("{svg}{metadata}")));
        };

        Ok(Cow::Owned(format!(
            "{}{}{}",
            &svg[..idx],
            metadata,
            &svg[idx..]
        )))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    if input.trim().is_empty() {
        input = r#"flowchart TD
    L7["Layer 7\nHTTP"]
    L6["Layer 6\nEncryption"]
    L7 --> L6
"#
        .to_string();
    }

    let renderer = HeadlessRenderer::new();
    let pipeline = SvgPipeline::resvg_safe().with_postprocessor(AddExampleMetadata);
    let Some(svg) = renderer.render_svg_with_pipeline_sync(&input, &pipeline)? else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{svg}");
    Ok(())
}
