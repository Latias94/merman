use std::borrow::Cow;

use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::{Color, DataPalette, FamilyTheme};

use super::JourneyMouthKind;

#[derive(Debug)]
pub(crate) struct JourneyNativeStyle {
    palette: DataPalette,
    pub(super) section_fills_owned: bool,
    pub(super) actor_colors_owned: bool,
    fill_types: Vec<Option<String>>,
    actor_colors: Vec<Option<String>>,
    face_override: Option<String>,
    scores: [Color; 3],
    pub(crate) section_colors_owned: bool,
    pub(crate) text: String,
    pub(crate) connector: String,
    pub(crate) arrow: String,
    pub(crate) border: String,
    pub(crate) border_width: f64,
    pub(crate) radius: f64,
    pub(crate) container_radius: f64,
    pub(crate) label_weight: u16,
    pub(crate) title_weight: u16,
    task_fill: String,
}

impl JourneyNativeStyle {
    pub(super) fn new(theme: &FamilyTheme, config: &PresentationConfig) -> Self {
        let roles = &theme.roles;
        let source = |path: &str| {
            config
                .get_str(path)
                .filter(|_| config.explicit_config_owns_path(path))
                .map(str::to_owned)
        };
        let text = source("themeVariables.textColor");
        let connector = text
            .clone()
            .or_else(|| source("themeVariables.lineColor"))
            .unwrap_or_else(|| roles.stroke_guide.first_color().to_string());
        let has_theme_variables = config.explicit_config_owns_path("themeVariables");
        let source_colors = |prefix: &str, count| {
            let mut colors = Vec::new();
            if has_theme_variables {
                for index in 0..count {
                    if let Some(color) = source(&format!("{prefix}{index}")) {
                        colors.resize_with(index, || None);
                        colors.push(Some(color));
                    }
                }
            }
            colors
        };
        Self {
            palette: theme.data.clone(),
            section_fills_owned: config.explicit_config_owns_path("journey.sectionFills"),
            actor_colors_owned: config.explicit_config_owns_path("journey.actorColours"),
            section_colors_owned: config.explicit_config_owns_path("journey.sectionColours"),
            fill_types: source_colors("themeVariables.fillType", 8),
            actor_colors: source_colors("themeVariables.actor", 6),
            face_override: source("themeVariables.faceColor"),
            scores: [roles.status_danger, roles.status_warn, roles.status_success],
            text: text.unwrap_or_else(|| roles.text_primary.to_string()),
            arrow: source("themeVariables.arrowheadColor").unwrap_or_else(|| connector.clone()),
            connector,
            border: source("themeVariables.nodeBorder")
                .unwrap_or_else(|| roles.stroke_border.first_color().to_string()),
            border_width: theme.form.stroke_width,
            radius: theme.form.radius,
            container_radius: theme.form.container_radius,
            label_weight: theme.typography.label_weight,
            title_weight: theme.typography.title_weight,
            task_fill: source("themeVariables.mainBkg")
                .unwrap_or_else(|| roles.surface_node.first_color().to_string()),
        }
    }

    pub(super) fn actor_fill(&self, index: usize, source: &str) -> String {
        self.actor_colors
            .get(index)
            .and_then(Option::as_deref)
            .or(self.actor_colors_owned.then_some(source))
            .map_or_else(|| self.palette.ink(index).to_string(), str::to_owned)
    }

    pub(super) fn section_fill(&self, ordinal: usize, slot: usize, source: &str) -> String {
        self.section_override(slot, source, true)
            .map_or_else(|| self.palette.fill(ordinal).to_string(), str::to_owned)
    }

    pub(super) fn task_fill(&self, slot: usize, source: &str, has_section: bool) -> String {
        self.section_override(slot, source, has_section)
            .unwrap_or(&self.task_fill)
            .to_owned()
    }

    fn section_override<'a>(
        &'a self,
        slot: usize,
        source: &'a str,
        has_section: bool,
    ) -> Option<&'a str> {
        self.fill_types
            .get(slot)
            .and_then(Option::as_deref)
            .or((has_section && self.section_fills_owned).then_some(source))
    }

    pub(crate) fn section_ink(&self, ordinal: usize, slot: usize) -> Cow<'_, str> {
        if self.section_fills_owned || self.fill_types.get(slot).is_some_and(Option::is_some) {
            Cow::Borrowed(&self.text)
        } else {
            Cow::Owned(self.palette.fill_ink(ordinal).to_string())
        }
    }

    pub(crate) fn face_paint(&self, mouth: &JourneyMouthKind) -> (Cow<'_, str>, &'static str) {
        if let Some(fill) = self.face_override.as_deref() {
            return (Cow::Borrowed(fill), "#666");
        }
        let color = self.scores[match mouth {
            JourneyMouthKind::Sad => 0,
            JourneyMouthKind::Ambivalent => 1,
            JourneyMouthKind::Smile => 2,
        }];
        let ink =
            if Color::rgb(0, 0, 0).contrast(color) >= Color::rgb(255, 255, 255).contrast(color) {
                "#000"
            } else {
                "#FFF"
            };
        (Cow::Owned(color.to_string()), ink)
    }
}
