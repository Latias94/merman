use merman_model::families::gantt::GanttDiagramRenderModel;
use merman_model::presentation::PresentationConfig;
use merman_model::{OperationControl, OperationControlResult};

use merman_render_core::config::ConfigLookup;
use merman_theme_contract::v2::FamilyTheme;

use super::GanttNativeStyle;

mod css_binding;
mod font_size;
pub(crate) use css_binding::GanttCssBinding;

/// Reports a consumed Gantt font size that the host cannot resolve for native output.
pub fn unsupported_native_font_size(
    model: &GanttDiagramRenderModel,
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    control.checkpoint()?;
    if model.tasks.is_empty() {
        return Ok(None);
    }
    // Admission shares both the emitted root size and the exact attribute resolver with layout.
    let css = GanttCssBinding::new(config, "");
    control.checkpoint()?;
    if css.section_font_size.used_px().is_some() {
        return Ok(None);
    }
    let compact =
        model.display_mode == "compact" || config.get_str("gantt.displayMode") == Some("compact");
    for task in &model.tasks {
        control.checkpoint()?;
        if !task.vert {
            let section = if compact {
                &task.section
            } else {
                &task.task_type
            };
            let text =
                merman_render_core::text::line_break::mermaid_html_breaks_to_newlines(section);
            if native_text_is_nonempty(&text, control)? {
                return Ok(Some(
                    "gantt.sectionFontSize (unresolved CSS value)".to_owned(),
                ));
            }
        }
    }
    control.checkpoint()?;
    Ok(None)
}

fn native_text_is_nonempty(text: &str, control: &OperationControl) -> OperationControlResult<bool> {
    let mut next_checkpoint = 0;
    for (offset, character) in text.char_indices() {
        if offset >= next_checkpoint {
            control.checkpoint()?;
            next_checkpoint = offset.saturating_add(1024);
        }
        if !character.is_whitespace() {
            return Ok(true);
        }
    }
    control.checkpoint()?;
    Ok(false)
}

/// Resolved Gantt layout typography and SVG presentation.
#[derive(Debug)]
pub struct GanttStyle {
    css: GanttCssBinding,
    native: Option<GanttNativeStyle>,
}

impl GanttStyle {
    pub(crate) fn resolve(config: &PresentationConfig, native: Option<&FamilyTheme>) -> Self {
        match native {
            Some(theme) => {
                let (native, css) = GanttCssBinding::for_native(theme, config);
                Self {
                    css,
                    native: Some(native),
                }
            }
            None => Self {
                css: GanttCssBinding::new(config, &gantt_config_font_family_css(config)),
                native: None,
            },
        }
    }

    pub(crate) fn native(&self) -> Option<&GanttNativeStyle> {
        self.native.as_ref()
    }

    pub(crate) const fn css_binding(&self) -> &GanttCssBinding {
        &self.css
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.css.common.font_family()
    }

    pub(super) fn radius_px(&self) -> f64 {
        self.native().map_or(3.0, |style| style.bar_radius)
    }
}

fn gantt_config_font_family_css(config: &dyn ConfigLookup) -> String {
    let value = config
        .get("gantt")
        .and_then(|gantt| gantt.get("fontFamily"))
        .and_then(serde_json::Value::as_str)
        .and_then(merman_render_core::config::normalized_config_font_family);
    value.unwrap_or_else(|| merman_render_core::config::config_font_family_css(config))
}
