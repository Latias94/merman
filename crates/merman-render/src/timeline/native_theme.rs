use std::borrow::Cow;

use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::DataPalette;

use crate::theme::{FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags};

use super::TimelineCssBinding;

pub(crate) struct TimelineThemeAdapter;

impl FamilyThemeAdapter for TimelineThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::TIMELINE;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::CONTAINER)
        .union(ThemeCapabilityFlags::DATA_FILL)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
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
    pub(crate) source_gradient: bool,
    event_fill: String,
}

impl TimelineNativeStyle {
    pub(super) fn new(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        css: &TimelineCssBinding,
        node_weight: String,
    ) -> Self {
        let roles = &theme.roles;
        let source = |key: &str| {
            let path = format!("themeVariables.{key}");
            config
                .get_str(&path)
                .filter(|_| config.explicit_config_owns_path(&path))
                .map(str::to_owned)
        };
        let text = source("textColor").unwrap_or_else(|| roles.text_primary.to_string());
        let title = source("titleColor")
            .or_else(|| source("textColor"))
            .unwrap_or_else(|| roles.text_title.to_string());
        let section_overrides = (0..css.sections.len())
            .map(|slot| TimelineSectionPaint {
                fill: source(&format!("cScale{slot}")),
                text: source(&format!("cScaleLabel{slot}")),
                accent: source(&format!("cScaleInv{slot}")),
            })
            .collect();
        Self {
            palette: theme.data.clone(),
            section_overrides,
            text,
            title,
            connector: source("lineColor")
                .or_else(|| css.is_redux_theme.then(|| source("nodeBorder")).flatten())
                .unwrap_or_else(|| roles.stroke_connector.first_color().to_string()),
            connector_width: theme.form.connector_width,
            border: source("nodeBorder")
                .unwrap_or_else(|| roles.stroke_border.first_color().to_string()),
            border_width: if config.explicit_config_owns_path("themeVariables.strokeWidth") {
                css.stroke_width.clone()
            } else {
                theme.form.stroke_width.to_string()
            },
            radius: theme.form.radius,
            container_radius: theme.form.container_radius,
            accent_width: theme.form.stroke_width * 2.0,
            node_weight,
            source_gradient: css.use_neo_gradient
                && config.explicit_config_owns_path("themeVariables.useGradient"),
            event_fill: source("mainBkg")
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
