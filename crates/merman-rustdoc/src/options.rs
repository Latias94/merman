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
pub(crate) struct Options {
    pub(crate) pipeline: PipelineMode,
    pub(crate) fail: FailMode,
    pub(crate) source: SourceMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            pipeline: PipelineMode::Readable,
            fail: FailMode::Error,
            source: SourceMode::Hide,
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
                    "unsupported merman_rustdoc option path; expected pipeline, fail, or source",
                ));
            };
            let value = literal_string(&pair.value)?;
            match ident.to_string().as_str() {
                "pipeline" => options.pipeline = PipelineMode::parse(&value)?,
                "fail" => options.fail = FailMode::parse(&value)?,
                "source" => options.source = SourceMode::parse(&value)?,
                other => {
                    return Err(Error::new(format!(
                        "unsupported merman_rustdoc option `{other}`; expected pipeline, fail, or source"
                    )));
                }
            }
        }

        Ok(options)
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
            pipeline = "resvg-safe",
            fail = "keep-source",
            source = "details"
        })
        .unwrap();

        assert_eq!(options.pipeline, PipelineMode::ResvgSafe);
        assert_eq!(options.fail, FailMode::KeepSource);
        assert_eq!(options.source, SourceMode::Details);
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
}
