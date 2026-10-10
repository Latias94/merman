use std::num::NonZeroUsize;

use merman::{
    resources::ResourceProfile,
    svg::{RenderResourcePolicy, ResourceLimitId},
};
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
    pub(crate) resource_profile: ResourceProfile,
    pub(crate) max_layout_work_units: Option<NonZeroUsize>,
    pub(crate) background: String,
    pub(crate) id_prefix: Option<String>,
    pub(crate) inherit: bool,
    pub(crate) crate_path: Option<String>,
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
            // Documentation is an offline native build, like the CLI's trusted local workflow.
            resource_profile: ResourceProfile::TrustedNative,
            max_layout_work_units: None,
            background: "transparent".to_string(),
            id_prefix: None,
            inherit: true,
            crate_path: None,
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
                    "unsupported merman_rustdoc option path; expected scope, pipeline, fail, source, sanitize, theme, resource_profile, max_layout_work_units, background, id_prefix, crate_path, or inherit",
                ));
            };
            let key = ident.to_string();
            if !seen.insert(key.clone()) {
                return Err(Error::new(format!(
                    "duplicate merman_rustdoc option `{key}`"
                )));
            }
            if key == "max_layout_work_units" {
                options.max_layout_work_units = Some(layout_work_limit(&pair.value)?);
                continue;
            }
            let value = literal_string(&pair.value)?;
            match key.as_str() {
                "crate_path" => {
                    syn::parse_str::<syn::Path>(&value)?;
                    options.crate_path = Some(value);
                }
                "scope" => options.scope = ScopeMode::parse(&value)?,
                "pipeline" => options.pipeline = PipelineMode::parse(&value)?,
                "fail" => options.fail = FailMode::parse(&value)?,
                "source" => options.source = SourceMode::parse(&value)?,
                "sanitize" => options.sanitize = SanitizeMode::parse(&value)?,
                "theme" => options.theme = ThemeMode::parse(&value)?,
                "resource_profile" => {
                    options.resource_profile = value.parse().map_err(Error::new)?;
                }
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
                        "unsupported merman_rustdoc option `{other}`; expected scope, pipeline, fail, source, sanitize, theme, resource_profile, max_layout_work_units, background, id_prefix, crate_path, or inherit"
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
        // Helper resolution belongs to this macro invocation, not inherited render policy.
        merged.crate_path.clone_from(&self.crate_path);
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
        if self.explicit.contains("resource_profile") {
            merged.resource_profile = self.resource_profile;
        }
        if self.explicit.contains("max_layout_work_units") {
            merged.max_layout_work_units = self.max_layout_work_units;
        }
        if self.explicit.contains("background") {
            merged.background.clone_from(&self.background);
        }
        if self.explicit.contains("id_prefix") {
            merged.id_prefix.clone_from(&self.id_prefix);
        }
        merged
    }

    pub(crate) fn resource_policy(&self) -> Result<RenderResourcePolicy> {
        let policy = RenderResourcePolicy::for_profile(self.resource_profile);
        match self.max_layout_work_units {
            Some(limit) => policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit.get())
                .map_err(|error| Error::new(error.to_string())),
            None => Ok(policy),
        }
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
        let crate_path = self
            .crate_path
            .as_ref()
            .map(|path| quote! { crate_path = #path, });
        let resource_profile = self.resource_profile.id();
        let max_layout_work_units = self.max_layout_work_units.map(|limit| {
            let value = limit.get();
            quote! { max_layout_work_units = #value, }
        });
        let background = &self.background;
        let id_prefix = self
            .id_prefix
            .as_ref()
            .map(|id| quote! { id_prefix = #id, });
        quote! { scope = #scope, pipeline = #pipeline, fail = #fail, source = #source,
        sanitize = #sanitize, theme = #theme, resource_profile = #resource_profile,
        #max_layout_work_units background = #background, #id_prefix #crate_path }
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

fn layout_work_limit(expr: &Expr) -> Result<NonZeroUsize> {
    let invalid = || {
        Error::new(
            "merman_rustdoc max_layout_work_units must be a positive integer literal that fits usize",
        )
    };
    let Expr::Lit(ExprLit {
        lit: Lit::Int(value),
        ..
    }) = expr
    else {
        return Err(invalid());
    };
    value.base10_parse::<NonZeroUsize>().map_err(|_| invalid())
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
        let options = Options::parse(quote! { background = "#123456", id_prefix = "overview", scope = "tree", theme = "dark", crate_path = "::facade" }).unwrap();
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
            quote! { crate_path = "not a path" },
            quote! { crate_path = "" },
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

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn parses_resource_profiles_and_positive_layout_limits() {
        for profile in ResourceProfile::ALL {
            let expected = RenderResourcePolicy::for_profile(profile)
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 2_000_000)
                .unwrap();
            let profile = profile.id();
            let options = Options::parse(quote! {
                resource_profile = #profile, max_layout_work_units = 2_000_000
            })
            .unwrap();
            assert_eq!(options.resource_policy().unwrap(), expected);
            assert_eq!(
                Options::parse(options.tokens())
                    .unwrap()
                    .resource_policy()
                    .unwrap(),
                expected
            );
        }
        let maximum = usize::MAX;
        let options = Options::parse(quote! { max_layout_work_units = #maximum }).unwrap();
        assert_eq!(
            options
                .resource_policy()
                .unwrap()
                .value(ResourceLimitId::MaxLayoutWorkUnits),
            Some(maximum)
        );
    }

    #[test]
    fn omitted_layout_limit_preserves_profile_defaults() {
        assert_eq!(
            Options::default().resource_policy().unwrap(),
            RenderResourcePolicy::trusted_native()
        );
        for profile in ResourceProfile::ALL {
            let name = profile.id();
            let options = Options::parse(quote! { resource_profile = #name }).unwrap();
            assert_eq!(options.max_layout_work_units, None);
            assert_eq!(
                options.resource_policy().unwrap(),
                RenderResourcePolicy::for_profile(profile)
            );
            assert_eq!(
                Options::parse(options.tokens())
                    .unwrap()
                    .max_layout_work_units,
                None
            );
        }
    }

    #[test]
    fn resource_option_order_does_not_change_effective_options() {
        let profile_first = Options::parse(quote! {
            resource_profile = "constrained", max_layout_work_units = 2_000_000
        })
        .unwrap();
        let limit_first = Options::parse(quote! {
            max_layout_work_units = 2_000_000, resource_profile = "constrained"
        })
        .unwrap();
        assert_eq!(profile_first, limit_first);
    }

    #[test]
    fn rejects_invalid_or_duplicate_resource_options() {
        for (tokens, expected) in [
            (
                quote! { resource_profile = "unknown" },
                "unsupported resource profile",
            ),
            (
                quote! { resource_profile = "interactive", resource_profile = "constrained" },
                "duplicate merman_rustdoc option `resource_profile`",
            ),
            (
                quote! { max_layout_work_units = 1, max_layout_work_units = 2 },
                "duplicate merman_rustdoc option `max_layout_work_units`",
            ),
            (
                quote! { max_layout_work_units = 0 },
                "positive integer literal",
            ),
            (
                quote! { max_layout_work_units = -1 },
                "positive integer literal",
            ),
            (
                quote! { max_layout_work_units = "2000000" },
                "positive integer literal",
            ),
            (
                quote! { max_layout_work_units = 1 + 2 },
                "positive integer literal",
            ),
            (
                quote! { max_layout_work_units = 340282366920938463463374607431768211455 },
                "positive integer literal",
            ),
        ] {
            let error = Options::parse(tokens).unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn resource_options_inherit_and_override_independently() {
        let parent = Options::parse(quote! {
            scope = "tree", resource_profile = "trusted-native", max_layout_work_units = 2_000_000
        })
        .unwrap();
        for (child, expected) in [
            (
                quote! {},
                quote! { resource_profile = "trusted-native", max_layout_work_units = 2_000_000 },
            ),
            (
                quote! { resource_profile = "constrained" },
                quote! { resource_profile = "constrained", max_layout_work_units = 2_000_000 },
            ),
            (
                quote! { max_layout_work_units = 3_000_000 },
                quote! { resource_profile = "trusted-native", max_layout_work_units = 3_000_000 },
            ),
        ] {
            let merged = Options::parse(child).unwrap().with_parent(&parent);
            let expected = Options::parse(expected).unwrap();
            assert_eq!(
                merged.resource_policy().unwrap(),
                expected.resource_policy().unwrap()
            );
            assert_eq!(
                Options::parse(merged.tokens())
                    .unwrap()
                    .resource_policy()
                    .unwrap(),
                expected.resource_policy().unwrap()
            );
        }
    }

    #[test]
    fn disabling_inheritance_resets_resource_options() {
        let parent = Options::parse(quote! {
            resource_profile = "trusted-native", max_layout_work_units = 2_000_000
        })
        .unwrap();
        for child in [
            quote! { inherit = "off" },
            quote! { inherit = "off", resource_profile = "constrained" },
            quote! { inherit = "off", max_layout_work_units = 3_000_000 },
        ] {
            let child = Options::parse(child).unwrap();
            let merged = child.with_parent(&parent);
            assert_eq!(merged, child);
            assert_eq!(
                merged.resource_policy().unwrap(),
                child.resource_policy().unwrap()
            );
        }
        let defaults = Options::parse(quote! { inherit = "off" })
            .unwrap()
            .with_parent(&parent);
        assert_eq!(
            defaults.resource_policy().unwrap(),
            RenderResourcePolicy::trusted_native()
        );
    }
}
