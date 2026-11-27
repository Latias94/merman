use merman_render_core::config::ConfigLookup;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct SequenceActorPalette {
    enabled: bool,
    borders: Vec<PaletteEntry>,
    fills: Vec<PaletteEntry>,
    activation_default_fill: Option<String>,
}

#[derive(Debug)]
enum PaletteEntry {
    Null,
    Token(String),
    Other,
}

impl SequenceActorPalette {
    pub fn resolve(config: &(impl ConfigLookup + ?Sized)) -> Self {
        let theme = config.get("themeVariables");
        let palette = |key: &str| {
            theme
                .and_then(|value| value.get(key))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|entry| match entry {
                    Value::Null => PaletteEntry::Null,
                    Value::String(value) => PaletteEntry::Token(value.clone()),
                    _ => PaletteEntry::Other,
                })
                .collect()
        };
        let theme_name = config.get("theme").and_then(Value::as_str);
        Self {
            enabled: matches!(theme_name, Some("redux-color" | "redux-dark-color")),
            borders: palette("borderColorArray"),
            fills: palette("bkgColorArray"),
            activation_default_fill: theme
                .and_then(|value| value.get("mainBkg"))
                .and_then(Value::as_str)
                .map(str::to_owned),
        }
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn actor_colors(&self, index: usize) -> (Option<&str>, Option<&str>) {
        (
            palette_token(&self.borders, index),
            palette_token(&self.fills, index),
        )
    }

    pub(super) fn activation_colors(&self, index: usize) -> (Option<&str>, Option<&str>) {
        if !self.enabled {
            return (None, None);
        }
        let fill = match palette_entry(&self.fills, index) {
            None | Some(PaletteEntry::Null) => self.activation_default_fill.as_deref(),
            Some(PaletteEntry::Token(value)) => Some(value.as_str()),
            Some(PaletteEntry::Other) => None,
        };
        (palette_token(&self.borders, index), fill)
    }
}

fn palette_entry(palette: &[PaletteEntry], index: usize) -> Option<&PaletteEntry> {
    (!palette.is_empty()).then(|| &palette[index % palette.len()])
}

fn palette_token(palette: &[PaletteEntry], index: usize) -> Option<&str> {
    match palette_entry(palette, index) {
        Some(PaletteEntry::Token(value)) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::SequenceActorPalette;
    use serde_json::json;

    #[test]
    fn raw_palette_preserves_slots_and_activation_null_fallback() {
        let palette = SequenceActorPalette::resolve(&json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": [null, "red", 7],
                "bkgColorArray": [null, "blue", 7],
                "mainBkg": "white"
            }
        }));
        assert_eq!(palette.actor_colors(0), (None, None));
        assert_eq!(palette.activation_colors(0), (None, Some("white")));
        assert_eq!(palette.actor_colors(1), (Some("red"), Some("blue")));
        assert_eq!(palette.activation_colors(2), (None, None));
        assert_eq!(palette.activation_colors(3), (None, Some("white")));

        let missing = SequenceActorPalette::resolve(&json!({
            "theme": "redux-dark-color", "themeVariables": {"mainBkg": "white"}
        }));
        assert_eq!(missing.activation_colors(42), (None, Some("white")));
        let ordinary = SequenceActorPalette::resolve(&json!({
            "theme": "redux", "themeVariables": {"mainBkg": "white"}
        }));
        assert!(!ordinary.enabled());
        assert_eq!(ordinary.activation_colors(42), (None, None));
    }
}
