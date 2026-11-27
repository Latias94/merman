use merman_render_core::{Result, svg_css::SvgId, svg_output::SvgOutput};

use super::super::BlockCssThemeBinding;
use super::BlockSvgContext;

pub(super) fn write_palette_css(
    out: &mut impl SvgOutput,
    id: SvgId<'_>,
    binding: &BlockCssThemeBinding,
    options: &BlockSvgContext<'_>,
) -> Result<()> {
    let look = &binding.palette_look;
    for (index, (border, background)) in binding.palette.iter().enumerate() {
        options.checkpoint_emit()?;
        let _ = write!(
            out,
            r#"#{id} [data-look="{look}"][data-color-id="color-{index}"].node rect.composite{{stroke:{};"#,
            border
        );
        if let Some(background) = background {
            let _ = write!(out, "fill:{background};");
        }
        out.push('}');
        out.checkpoint()?;
    }
    Ok(())
}
