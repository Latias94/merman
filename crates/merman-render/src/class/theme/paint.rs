use merman_model::presentation::PresentationConfig;

#[derive(Debug)]
pub(super) struct ClassPaintBinding {
    css: String,
    authored: bool,
}

impl ClassPaintBinding {
    pub(super) fn from_config(config: &PresentationConfig, key: &str, fallback: &str) -> Self {
        let token = crate::config::config_string(config.as_value(), &["themeVariables", key]);
        Self {
            authored: token.is_some()
                && config.explicit_config_owns_path(&format!("themeVariables.{key}")),
            css: token.unwrap_or_else(|| fallback.to_owned()),
        }
    }

    pub(super) fn apply_native(&mut self, value: String) {
        if !self.authored {
            self.css = value;
        }
    }

    pub(super) fn css(&self) -> &str {
        &self.css
    }
}
