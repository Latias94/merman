use std::borrow::Cow;

use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::{DataPalette, FamilyTheme};

use super::TimelineConfigView;

#[derive(Debug, Default)]
struct TimelineSectionPaint {
    fill: Option<String>,
    text: Option<String>,
    accent: Option<String>,
}

#[derive(Debug)]
pub(crate) struct TimelineNativeStyle {
    palette: DataPalette,
    section_overrides: Vec<TimelineSectionPaint>,
    pub(crate) text: String,
    pub(crate) title: String,
    pub(crate) connector: String,
    pub(crate) connector_width: f64,
    pub(crate) border: String,
    pub(crate) border_width: String,
    pub(crate) radius: f64,
    pub(crate) container_radius: f64,
    pub(crate) accent_width: f64,
    pub(crate) node_weight: String,
    pub(crate) gradient_stops: Option<[Option<String>; 2]>,
    event_fill: String,
}

impl TimelineNativeStyle {
    pub(super) fn new(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        config_view: &TimelineConfigView<'_>,
        is_redux: bool,
        node_weight: String,
    ) -> Self {
        let roles = &theme.roles;
        let source = |path: &str| {
            config
                .get_str(path)
                .filter(|_| config.explicit_config_owns_path(path))
                .map(str::to_owned)
        };
        let text = source("themeVariables.textColor");
        let title = source("themeVariables.titleColor")
            .or_else(|| text.clone())
            .unwrap_or_else(|| roles.text_title.to_string());
        let mut section_overrides = Vec::new();
        if config.explicit_config_owns_path("themeVariables") {
            for slot in 0..super::timeline_theme_color_limit(config.as_value()) {
                let paint = TimelineSectionPaint {
                    fill: source(&format!("themeVariables.cScale{slot}")),
                    text: source(&format!("themeVariables.cScaleLabel{slot}")),
                    accent: source(&format!("themeVariables.cScaleInv{slot}")),
                };
                if paint.fill.is_some() || paint.text.is_some() || paint.accent.is_some() {
                    section_overrides.resize_with(slot, TimelineSectionPaint::default);
                    section_overrides.push(paint);
                }
            }
        }
        let border = source("themeVariables.nodeBorder");
        Self {
            palette: theme.data.clone(),
            section_overrides,
            text: text.unwrap_or_else(|| roles.text_primary.to_string()),
            title,
            connector: source("themeVariables.lineColor")
                .or_else(|| if is_redux { border.clone() } else { None })
                .unwrap_or_else(|| roles.stroke_connector.first_color().to_string()),
            connector_width: theme.form.connector_width,
            border: border.unwrap_or_else(|| roles.stroke_border.first_color().to_string()),
            border_width: if config.explicit_config_owns_path("themeVariables.strokeWidth") {
                merman_render_core::config::config_css_number_or_string(
                    config,
                    &["themeVariables", "strokeWidth"],
                )
                .unwrap_or_else(|| "1".into())
            } else {
                theme.form.stroke_width.to_string()
            },
            radius: theme.form.radius,
            container_radius: theme.form.container_radius,
            accent_width: theme.form.stroke_width * 2.0,
            node_weight,
            gradient_stops: (config.explicit_config_owns_path("themeVariables.useGradient")
                && config_view.uses_neo_gradient())
            .then(|| {
                ["gradientStart", "gradientStop"].map(|key| {
                    merman_render_core::config::config_string(config, &["themeVariables", key])
                })
            }),
            event_fill: source("themeVariables.mainBkg")
                .unwrap_or_else(|| roles.surface_node.first_color().to_string()),
        }
    }

    pub(crate) fn card_paint(
        &self,
        section_class: &str,
        ordinal: usize,
        event: bool,
    ) -> (Cow<'_, str>, Cow<'_, str>, Option<Cow<'_, str>>) {
        let source = section_class
            .strip_prefix("section-")
            .and_then(|index| index.parse::<i64>().ok())
            .and_then(|index| index.checked_add(1))
            .and_then(|slot| usize::try_from(slot).ok())
            .and_then(|slot| self.section_overrides.get(slot));
        let fill = if let Some(fill) = source.and_then(|source| source.fill.as_deref()) {
            Cow::Borrowed(fill)
        } else if event {
            Cow::Borrowed(self.event_fill.as_str())
        } else {
            Cow::Owned(self.palette.fill(ordinal).to_string())
        };
        let text = if let Some(text) = source.and_then(|source| source.text.as_deref()) {
            Cow::Borrowed(text)
        } else if event {
            Cow::Borrowed(self.text.as_str())
        } else {
            Cow::Owned(self.palette.fill_ink(ordinal).to_string())
        };
        let accent = source
            .and_then(|source| source.accent.as_deref())
            .map(Cow::Borrowed)
            .or_else(|| event.then(|| Cow::Owned(self.palette.ink(ordinal).to_string())));
        (fill, text, accent)
    }
}
