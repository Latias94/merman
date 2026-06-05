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
pub(crate) struct Options {
    pub(crate) scope: ScopeMode,
    pub(crate) pipeline: PipelineMode,
    pub(crate) fail: FailMode,
    pub(crate) source: SourceMode,
    pub(crate) sanitize: SanitizeMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scope: ScopeMode::Item,
            pipeline: PipelineMode::Readable,
            fail: FailMode::Error,
            source: SourceMode::Hide,
            sanitize: SanitizeMode::Strict,
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
                    "unsupported merman_rustdoc option path; expected scope, pipeline, fail, source, or sanitize",
                ));
            };
            let value = literal_string(&pair.value)?;
            match ident.to_string().as_str() {
                "scope" => options.scope = ScopeMode::parse(&value)?,
                "pipeline" => options.pipeline = PipelineMode::parse(&value)?,
                "fail" => options.fail = FailMode::parse(&value)?,
                "source" => options.source = SourceMode::parse(&value)?,
                "sanitize" => options.sanitize = SanitizeMode::parse(&value)?,
                other => {
                    return Err(Error::new(format!(
                        "unsupported merman_rustdoc option `{other}`; expected scope, pipeline, fail, source, or sanitize"
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
            sanitize = "off"
        })
        .unwrap();

        assert_eq!(options.scope, ScopeMode::Tree);
        assert_eq!(options.pipeline, PipelineMode::ResvgSafe);
        assert_eq!(options.fail, FailMode::KeepSource);
        assert_eq!(options.source, SourceMode::Details);
        assert_eq!(options.sanitize, SanitizeMode::Off);
    }

    #[test]
    fn rejects_unknown_options() {
        let err = Options::parse(quote! { theme = "dark" }).unwrap_err();

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
}
