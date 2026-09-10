use proc_macro2::TokenStream;
use quote::quote;
use syn::{Expr, ExprLit, Lit, MetaNameValue, Token, parse::Parser, punctuated::Punctuated};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PipelineMode {
    /// Keep the Mermaid-compatible SVG unchanged for browser presentation.
    Parity,
    /// Retain native HTML labels and add SVG text fallbacks alongside them.
    ///
    /// Hosts that can render both representations must select or hide one to
    /// avoid displaying duplicate labels.
    Readable,
    /// Replace browser-only labels with fallbacks and finalize for resvg.
    ResvgSafe,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FailMode {
    Error,
    KeepSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceMode {
    Hide,
    Details,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScopeMode {
    Item,
    Tree,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SanitizeMode {
    Strict,
    Off,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ThemeMode {
    Rustdoc,
    Mermaid,
    Fixed(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub(crate) scope: ScopeMode,
    pub(crate) pipeline: PipelineMode,
    pub(crate) fail: FailMode,
    pub(crate) source: SourceMode,
    pub(crate) sanitize: SanitizeMode,
    pub(crate) theme: ThemeMode,
    pub(crate) background: String,
    pub(crate) id_prefix: Option<String>,
    pub(crate) inherit: bool,
    pub(crate) explicit: std::collections::HashSet<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scope: ScopeMode::Item,
            // Rustdoc is presented in a browser, where the native Mermaid
            // labels are supported and a second fallback can become visible.
            pipeline: PipelineMode::Parity,
            fail: FailMode::Error,
            source: SourceMode::Hide,
            sanitize: SanitizeMode::Strict,
            theme: ThemeMode::Rustdoc,
            background: "transparent".to_string(),
            id_prefix: None,
            inherit: true,
            explicit: std::collections::HashSet::new(),
        }
    }
}

impl Options {
    pub(crate) fn parse(args: TokenStream) -> Result<Self> {
        if args.is_empty() {
            return Ok(Self::default());
        }

        let parser = Punctuated::<MetaNameValue, Token![,]>::parse_terminated;
        let pairs = parser.parse2(args)?;
        let mut options = Self::default();

        let mut seen = std::collections::HashSet::new();
        for pair in pairs {
            let Some(ident) = pair.path.get_ident() else {
                return Err(Error::new(
                    "unsupported merman_rustdoc option path; expected scope, pipeline, fail, source, sanitize, theme, background, id_prefix, or inherit",
                ));
            };
            let key = ident.to_string();
            if !seen.insert(key.clone()) {
                return Err(Error::new(format!(
                    "duplicate merman_rustdoc option `{key}`"
                )));
            }
            let value = literal_string(&pair.value)?;
            match ident.to_string().as_str() {
                "scope" => options.scope = ScopeMode::parse(&value)?,
                "pipeline" => options.pipeline = PipelineMode::parse(&value)?,
                "fail" => options.fail = FailMode::parse(&value)?,
                "source" => options.source = SourceMode::parse(&value)?,
                "sanitize" => options.sanitize = SanitizeMode::parse(&value)?,
                "theme" => options.theme = ThemeMode::parse(&value)?,
                "inherit" => {
                    options.inherit = match value.as_str() {
                        "on" => true,
                        "off" => false,
                        _ => return Err(Error::new("merman_rustdoc inherit must be on or off")),
                    };
                }
                "background" => {
                    if value.trim().is_empty() {
                        return Err(Error::new("merman_rustdoc background must not be empty"));
                    }
                    options.background = value;
                }
                "id_prefix" => {
                    if value.is_empty()
                        || !value
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    {
                        return Err(Error::new(
                            "merman_rustdoc id_prefix must contain only ASCII letters, digits, hyphens, or underscores",
                        ));
                    }
                    options.id_prefix = Some(value);
                }
                other => {
                    return Err(Error::new(format!(
                        "unsupported merman_rustdoc option `{other}`; expected scope, pipeline, fail, source, sanitize, theme, background, id_prefix, or inherit"
                    )));
                }
            }
        }

        options.explicit = seen;
        Ok(options)
    }

    pub(crate) fn with_parent(&self, parent: &Self) -> Self {
        if !self.inherit {
            return self.clone();
        }
        let mut merged = parent.clone();
        merged.scope = self.scope;
        merged.inherit = self.inherit;
        merged.explicit = self.explicit.clone();
        if self.explicit.contains("pipeline") {
            merged.pipeline = self.pipeline;
        }
        if self.explicit.contains("fail") {
            merged.fail = self.fail;
        }
        if self.explicit.contains("source") {
            merged.source = self.source;
        }
        if self.explicit.contains("sanitize") {
            merged.sanitize = self.sanitize;
        }
        if self.explicit.contains("theme") {
            merged.theme = self.theme;
        }
        if self.explicit.contains("background") {
            merged.background.clone_from(&self.background);
        }
        if self.explicit.contains("id_prefix") {
            merged.id_prefix.clone_from(&self.id_prefix);
        }
        merged
    }

    pub(crate) fn tokens(&self) -> TokenStream {
        let scope = match self.scope {
            ScopeMode::Item => "item",
            ScopeMode::Tree => "tree",
        };
        let pipeline = match self.pipeline {
            PipelineMode::Parity => "parity",
            PipelineMode::Readable => "readable",
            PipelineMode::ResvgSafe => "resvg-safe",
        };
        let fail = match self.fail {
            FailMode::Error => "error",
            FailMode::KeepSource => "keep-source",
        };
        let source = match self.source {
            SourceMode::Hide => "hide",
            SourceMode::Details => "details",
        };
        let sanitize = match self.sanitize {
            SanitizeMode::Strict => "strict",
            SanitizeMode::Off => "off",
        };
        let theme = match self.theme {
            ThemeMode::Rustdoc => "rustdoc",
            ThemeMode::Mermaid => "mermaid",
            ThemeMode::Fixed(theme) => theme,
        };
        let background = &self.background;
        let id_prefix = self
            .id_prefix
            .as_ref()
            .map(|id| quote! { id_prefix = #id, });
        quote! { scope = #scope, pipeline = #pipeline, fail = #fail, source = #source,
        sanitize = #sanitize, theme = #theme, background = #background, #id_prefix }
    }
}

impl ScopeMode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "item" => Ok(Self::Item),
            "tree" => Ok(Self::Tree),
            other => Err(Error::new(format!(
                "unsupported merman_rustdoc scope `{other}`; expected item or tree"
            ))),
        }
    }
}

impl PipelineMode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "parity" => Ok(Self::Parity),
            "readable" => Ok(Self::Readable),
            "resvg-safe" | "resvg_safe" => Ok(Self::ResvgSafe),
            other => Err(Error::new(format!(
                "unsupported merman_rustdoc pipeline `{other}`; expected parity, readable, or resvg-safe"
            ))),
        }
    }
}

impl FailMode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "error" => Ok(Self::Error),
            "keep-source" | "keep_source" => Ok(Self::KeepSource),
            other => Err(Error::new(format!(
                "unsupported merman_rustdoc fail mode `{other}`; expected error or keep-source"
            ))),
        }
    }
}

impl SourceMode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "hide" => Ok(Self::Hide),
            "details" => Ok(Self::Details),
            other => Err(Error::new(format!(
                "unsupported merman_rustdoc source mode `{other}`; expected hide or details"
            ))),
        }
    }
}

impl SanitizeMode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "strict" => Ok(Self::Strict),
            "off" => Ok(Self::Off),
            other => Err(Error::new(format!(
                "unsupported merman_rustdoc sanitize mode `{other}`; expected strict or off"
            ))),
        }
    }
}

impl ThemeMode {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "rustdoc" => return Ok(Self::Rustdoc),
            "mermaid" => return Ok(Self::Mermaid),
            _ => {}
        }
        if let Some(theme) = supported_mermaid_theme(value) {
            return Ok(Self::Fixed(theme));
        }
        Err(Error::new(format!(
            "unsupported merman_rustdoc theme `{value}`; expected rustdoc, mermaid, or one of: {}",
            merman::supported_themes().join(", ")
        )))
    }
}

fn supported_mermaid_theme(value: &str) -> Option<&'static str> {
    merman::supported_themes()
        .iter()
        .copied()
        .find(|theme| *theme == value)
}

fn literal_string(expr: &Expr) -> Result<String> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(value),
        ..
    }) = expr
    else {
        return Err(Error::new(
            "merman_rustdoc options must use string literals, for example pipeline = \"readable\"",
        ));
    };
    Ok(value.value())
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::*;

    #[test]
    fn parses_default_options() {
        let options = Options::parse(TokenStream::new()).unwrap();

        assert_eq!(options, Options::default());
        assert_eq!(options.pipeline, PipelineMode::Parity);
    }

    #[test]
    fn parses_all_supported_options() {
        let options = Options::parse(quote! {
            scope = "tree",
            pipeline = "resvg-safe",
            fail = "keep-source",
            source = "details",
            sanitize = "off",
            theme = "dark"
        })
        .unwrap();

        assert_eq!(options.scope, ScopeMode::Tree);
        assert_eq!(options.pipeline, PipelineMode::ResvgSafe);
        assert_eq!(options.fail, FailMode::KeepSource);
        assert_eq!(options.source, SourceMode::Details);
        assert_eq!(options.sanitize, SanitizeMode::Off);
        assert_eq!(options.theme, ThemeMode::Fixed("dark"));
    }

    #[test]
    fn rejects_unknown_options() {
        let err = Options::parse(quote! { layout = "elk" }).unwrap_err();

        assert!(
            err.to_string()
                .contains("unsupported merman_rustdoc option")
        );
    }

    #[test]
    fn rejects_non_string_values() {
        let err = Options::parse(quote! { pipeline = readable }).unwrap_err();

        assert!(err.to_string().contains("string literals"));
    }

    #[test]
    fn rejects_unknown_scope() {
        let err = Options::parse(quote! { scope = "module" }).unwrap_err();

        assert!(err.to_string().contains("expected item or tree"));
    }

    #[test]
    fn rejects_unknown_sanitize_mode() {
        let err = Options::parse(quote! { sanitize = "loose" }).unwrap_err();

        assert!(err.to_string().contains("expected strict or off"));
    }

    #[test]
    fn parses_rustdoc_and_mermaid_theme_modes() {
        let rustdoc = Options::parse(quote! { theme = "rustdoc" }).unwrap();
        let mermaid = Options::parse(quote! { theme = "mermaid" }).unwrap();

        assert_eq!(rustdoc.theme, ThemeMode::Rustdoc);
        assert_eq!(mermaid.theme, ThemeMode::Mermaid);
    }

    #[test]
    fn rejects_unknown_theme() {
        let err = Options::parse(quote! { theme = "source" }).unwrap_err();

        assert!(
            err.to_string()
                .contains("expected rustdoc, mermaid, or one of")
        );
    }
}

#[cfg(test)]
mod embedding_tests {
    use super::*;
    #[test]
    fn options_round_trip_without_losing_explicit_values() {
        let options = Options::parse(quote! { background = "#123456", id_prefix = "overview", scope = "tree", theme = "dark" }).unwrap();
        assert_eq!(
            Options::parse(options.tokens())
                .unwrap()
                .tokens()
                .to_string(),
            options.tokens().to_string()
        );
    }
    #[test]
    fn rejects_ambiguous_or_empty_embedding_options() {
        for tokens in [
            quote! { theme = "dark", theme = "forest" },
            quote! { background = " " },
            quote! { id_prefix = "bad id" },
            quote! { inherit = "sometimes" },
        ] {
            assert!(Options::parse(tokens).is_err());
        }
    }

    #[test]
    fn child_overrides_only_explicit_fields_without_inheriting_scope() {
        let parent = Options::parse(quote! {
            scope = "tree", pipeline = "readable", fail = "keep-source",
            source = "details", sanitize = "off", theme = "dark",
            background = "#123456", id_prefix = "parent"
        })
        .unwrap();
        let child = Options::parse(quote! { theme = "forest" }).unwrap();
        let merged = child.with_parent(&parent);
        assert_eq!(merged.scope, ScopeMode::Item);
        assert_eq!(merged.theme, ThemeMode::Fixed("forest"));
        assert_eq!(merged.pipeline, parent.pipeline);
        assert_eq!(merged.fail, parent.fail);
        assert_eq!(merged.source, parent.source);
        assert_eq!(merged.sanitize, parent.sanitize);
        assert_eq!(merged.background, parent.background);
        assert_eq!(merged.id_prefix, parent.id_prefix);

        let defaults = Options::parse(quote! {
            scope = "tree", pipeline = "parity", fail = "error", source = "hide",
            sanitize = "strict", theme = "rustdoc", background = "transparent", id_prefix = "child"
        })
        .unwrap();
        let merged = defaults.with_parent(&parent);
        assert_eq!(merged.tokens().to_string(), defaults.tokens().to_string());
    }

    #[test]
    fn disabling_inheritance_resets_unspecified_fields_including_id_prefix() {
        let parent = Options::parse(quote! {
            scope = "tree", pipeline = "readable", fail = "keep-source",
            source = "details", sanitize = "off", theme = "dark",
            background = "#123456", id_prefix = "parent"
        })
        .unwrap();
        let child = Options::parse(quote! { inherit = "off", theme = "forest" }).unwrap();
        let merged = child.with_parent(&parent);
        assert_eq!(merged, child);
        assert_eq!(merged.scope, ScopeMode::Item);
        assert_eq!(merged.pipeline, PipelineMode::Parity);
        assert_eq!(merged.fail, FailMode::Error);
        assert_eq!(merged.source, SourceMode::Hide);
        assert_eq!(merged.sanitize, SanitizeMode::Strict);
        assert_eq!(merged.background, "transparent");
        assert_eq!(merged.id_prefix, None);
    }
}
