use merman::model::json::ordered::JsonOrderScope;

pub(super) use merman::model::json::ordered::{
    OrderedJson as OptionsValue, OrderedJsonRef as OptionsValueRef,
};

#[derive(Clone, Copy)]
enum OrderScope {
    Options,
    Analysis,
    SiteConfig,
    Layout,
    Nested,
}

impl JsonOrderScope for OrderScope {
    fn member(self, key: &str) -> Option<Self> {
        match (self, key) {
            (Self::Options | Self::Analysis, "analysis" | "merman") => Some(Self::Analysis),
            (Self::Options | Self::Analysis, "site_config") => Some(Self::SiteConfig),
            (Self::Options | Self::Analysis, "resources") => Some(Self::Nested),
            #[cfg(feature = "analysis")]
            (Self::Options | Self::Analysis, "lint") => Some(Self::Nested),
            (Self::Options, "layout") => Some(Self::Layout),
            (
                Self::Options,
                "parse" | "ascii" | "environment" | "svg" | "raster" | "jpeg" | "pdf",
            )
            | (Self::SiteConfig, "themeVariables" | "theme_variables")
            | (Self::Nested, _) => Some(Self::Nested),
            _ => None,
        }
    }

    fn array_item(self) -> Option<Self> {
        matches!(self, Self::Nested).then_some(Self::Nested)
    }
}

pub(super) fn decode_options(text: &str) -> Result<OptionsValue, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = OptionsValue::deserialize_with_scope(&mut deserializer, OrderScope::Options)?;
    deserializer.end()?;
    Ok(value)
}
