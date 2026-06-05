use proc_macro2::TokenStream;
use syn::{Expr, ExprLit, Lit, MetaNameValue, Token, parse::Parser, punctuated::Punctuated};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PipelineMode {
    Parity,
    Readable,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub(crate) scope: ScopeMode,
    pub(crate) pipeline: PipelineMode,
    pub(crate) fail: FailMode,
    pub(crate) source: SourceMode,
    pub(crate) sanitize: SanitizeMode,
    pub(crate) theme: ThemeMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scope: ScopeMode::Item,
            pipeline: PipelineMode::Readable,
            fail: FailMode::Error,
            source: SourceMode::Hide,
            sanitize: SanitizeMode::Strict,
            theme: ThemeMode::Rustdoc,
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

        for pair in pairs {
            let Some(ident) = pair.path.get_ident() else {
                return Err(Error::new(
                    "unsupported merman_rustdoc option path; expected scope, pipeline, fail, source, sanitize, or theme",
                ));
            };
            let value = literal_string(&pair.value)?;
            match ident.to_string().as_str() {
                "scope" => options.scope = ScopeMode::parse(&value)?,
                "pipeline" => options.pipeline = PipelineMode::parse(&value)?,
                "fail" => options.fail = FailMode::parse(&value)?,
                "source" => options.source = SourceMode::parse(&value)?,
                "sanitize" => options.sanitize = SanitizeMode::parse(&value)?,
                "theme" => options.theme = ThemeMode::parse(&value)?,
                other => {
                    return Err(Error::new(format!(
                        "unsupported merman_rustdoc option `{other}`; expected scope, pipeline, fail, source, sanitize, or theme"
                    )));
                }
            }
        }

        Ok(options)
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
        assert_eq!(
            Options::parse(TokenStream::new()).unwrap(),
            Options::default()
        );
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
