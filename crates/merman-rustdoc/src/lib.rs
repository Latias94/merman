#![forbid(unsafe_code)]

//! Render Mermaid diagrams in rustdoc as inline SVG.
//!
//! `merman-rustdoc` is a proc-macro integration for crates that want Mermaid diagrams in API docs
//! without loading Mermaid JavaScript in the browser. The [`macro@merman`] attribute reads Mermaid
//! code fences and `include_mmd!` lines from item documentation, renders them with Merman during
//! `cargo doc`, and writes the resulting SVG back into the generated rustdoc page.
//!
//! # Install
//!
//! This checkout is ahead of published `0.8.0-alpha.6`. The dependency examples install alpha.6
//! for the basic workflow. New `background`, `id_prefix`, and `inherit` options, Markdown container
//! support, and the refactored behavior described here require this source checkout until the
//! next workspace release; they are not available in alpha.6. This checkout also makes math
//! opt-in. The quick-start recipe explicitly selects the smaller feature set because alpha.6
//! still enables math by default.
//!
//! Keep the renderer out of ordinary builds with an optional documentation dependency:
//!
//! ```toml
//! [dependencies]
//! merman-rustdoc = { version = "=0.8.0-alpha.6", default-features = false, features = ["svg", "layout-cytoscape"], optional = true }
//!
//! [features]
//! doc-diagrams = ["dep:merman-rustdoc"]
//!
//! [package.metadata.docs.rs]
//! features = ["doc-diagrams"]
//! ```
//!
//! Build locally with `cargo doc --features doc-diagrams`; docs.rs enables the feature through
//! the metadata above. Leave `doc-diagrams` out of your default features. `cfg_attr` controls macro
//! expansion, not Cargo dependency selection: enabling `doc-diagrams`, including through
//! `--all-features`, compiles the renderer even during ordinary builds.
//!
//! For no renderer dependency in either ordinary builds or documentation builds, generate and
//! commit fragments with `merman-cli rustdoc build/check`, then consume them through standard
//! `#[doc = include_str!("...")]`. This also supports crate-level documentation.
//!
//! This checkout defaults to `svg` and `layout-cytoscape`, without math, the optional EPL-2.0
//! ELK implementation, or system clock, time-zone, random, or timing adapters. When upgrading
//! from alpha.6, add `math` to the dependency's features for math labels, or select `complete-svg`
//! to retain the previous SVG, Cytoscape, and math combination. The separate `merman` facade
//! keeps its `complete-svg` default. Use `complete-svg-elk` only when the artifact intentionally
//! carries the ELK closure and its notices. For base SVG only, use:
//!
//! ```toml
//! [dependencies]
//! merman-rustdoc = { version = "=0.8.0-alpha.6", default-features = false, features = ["svg"], optional = true }
//! ```
//!
//! Keep the documentation feature and docs.rs metadata above. Add `layout-cytoscape`, `layout-elk`,
//! or `math` only when the diagrams need those capabilities.
//!
//! # Quickstart
//!
//! Put the attribute on any item whose docs contain a Mermaid fence:
//!
//! ````rust
//! #[cfg_attr(all(doc, feature = "doc-diagrams"), merman_rustdoc::merman)]
//! /// Rendered by rustdoc as inline SVG:
//! ///
//! /// ```mermaid
//! /// flowchart TD
//! ///   A[Start] --> B[Done]
//! /// ```
//! pub fn example() {}
//! ````
//!
//! # Include Mermaid files
//!
//! Large diagrams can live in separate `.mmd` files. Paths are resolved relative to the consuming
//! crate's `CARGO_MANIFEST_DIR`.
//!
//! ```rust
//! #[cfg_attr(all(doc, feature = "doc-diagrams"), merman_rustdoc::merman)]
//! /// Crate architecture.
//! ///
//! /// include_mmd!("docs/diagrams/architecture.mmd")
//! pub fn architecture() {}
//! ```
//!
//! Use JSON-compatible quoted include paths; Rust raw-string paths are not supported. The macro
//! reads includes during expansion but does not automatically track them for stable Cargo builds.
//! To regenerate docs when only diagram files change, add this line to the `main` function in
//! the consuming crate's `build.rs`:
//!
//! ```rust
//! println!("cargo::rerun-if-changed=docs/diagrams");
//! ```
//!
//! No build dependency is needed. Watch an existing dedicated directory, not just existing files,
//! so additions, modifications, deletions, and restoration of missing `fail = "keep-source"`
//! includes all invalidate the build. Then run `cargo doc` with the usual documentation features.
//! If a build script already exists, add this instruction to it. The first `rerun-if-changed`
//! instruction narrows Cargo's default package-wide tracking; explicitly retain the script's
//! other file inputs when it previously relied on that default.
//!
//! Watched and included paths are relative to the consuming crate's manifest directory. An
//! include under `../shared/docs/diagrams` needs that directory watched by the consumer's script.
//! Package diagram files with the crate, accounting for any `[package] include` allowlist, and
//! check `cargo package --list`. Paths outside the package are local workspace inputs, not
//! portable published resources; copy diagrams into the package or use CLI-generated fragments.
//!
//! # Embedding defaults
//!
//! Embedded SVGs have a transparent canvas by default. Set `background = "white"`
//! for the previous behavior, or use another CSS color independently of the Mermaid theme.
//! IDs are automatically isolated per invocation, document, diagram, and theme.
//! `id_prefix = "overview"` adds a namespace; it does not replace the unique suffix.
//! Duplicate options are rejected.
//!
//! `scope = "tree"` defers rendering until Rust processes conditional compilation.
//! Disabled functions, fields, and variants do not read includes or render diagrams.
//! A child macro inherits parent tree rendering options and overrides explicitly supplied fields.
//! `scope` is not inherited: its default remains `item`. Use `scope = "tree"` on the child to
//! propagate its effective rendering options to its descendants. `inherit = "off"` starts from
//! defaults, clearing any inherited `id_prefix`, before applying the child's explicit options.
//! Imported and renamed attributes follow the same inheritance rules. On an item with no own
//! documentation, `scope = "item"` is a no-op and does not save options for a later tree attribute.
//! An inactive `cfg_attr` keeps the parent options. Cargo dependency renaming is supported.
//!
//! HTML comments, indented code, and non-Mermaid fences are preserved. Diagram fences and
//! standalone include directives can appear in lists, blockquotes, footnotes, and nested
//! combinations. Retain explicit container indentation and blockquote markers. Include directives
//! on lazy continuation lines without those prefixes report an error; add the required prefix or
//! use `fail = "keep-source"`. Dynamic `#[doc = include_str!(...)]` remains outside this macro.
//!
//! # Options
//!
//! The attribute accepts string options:
//!
//! ```rust
//! #[cfg_attr(
//!     all(doc, feature = "doc-diagrams"),
//!     merman_rustdoc::merman(
//!         scope = "item",
//!         pipeline = "parity",
//!         fail = "error",
//!         source = "hide",
//!         sanitize = "strict",
//!         theme = "rustdoc"
//!     )
//! )]
//! /// ```mermaid
//! /// flowchart TD
//! ///   A --> B
//! /// ```
//! pub fn configured() {}
//! ```
//!
//! | Option | Values | Default | Meaning |
//! | --- | --- | --- | --- |
//! | `scope` | `item`, `tree` | `item` | Controls whether only the annotated item or the inline item tree is rewritten. |
//! | `inherit` | `on`, `off` | `on` | Inherits parent tree rendering options, or starts from defaults before local overrides. |
//! | `pipeline` | `parity`, `readable`, `resvg-safe` | `parity` | Selects the SVG output pipeline. |
//! | `fail` | `error`, `keep-source` | `error` | Controls what happens when rendering or file includes fail. |
//! | `source` | `hide`, `details` | `hide` | Adds a collapsed Mermaid source block under the SVG when set to `details`. |
//! | `sanitize` | `strict`, `off` | `strict` | Checks rendered SVG for script elements, event attributes, and unsafe resource references. |
//! | `background` | CSS color | `transparent` | Sets the embedded SVG canvas background independently of the theme. |
//! | `id_prefix` | ASCII letters, digits, `-`, `_` | automatic | Adds an optional namespace to automatically isolated diagram IDs. |
//! | `theme` | `rustdoc`, `mermaid`, or a supported Mermaid theme name | `rustdoc` | Controls whether diagrams follow rustdoc light/dark themes, use Mermaid source config, or use a fixed Mermaid theme. |
//!
//! `parity` is the default because rustdoc pages target browsers, which render Mermaid's native
//! `<foreignObject>` labels directly. `readable` deliberately adds SVG `<text>` fallbacks alongside
//! those labels and can display both representations in consumers that support each one.
//! `resvg-safe` removes the native labels and retains the SVG text fallback for compatible
//! rasterizers.
//!
//! Use `scope = "tree"` to process docs on children inside an inline module, trait, impl block,
//! struct fields, and enum variants:
//!
//! ````rust
//! #[cfg_attr(
//!     all(doc, feature = "doc-diagrams"),
//!     merman_rustdoc::merman(scope = "tree")
//! )]
//! pub mod api {
//!     /// ```mermaid
//!     /// flowchart TD
//!     ///   Child --> Docs
//!     /// ```
//!     pub fn child() {}
//! }
//! ````
//!
//! For example, with `source = "details"` on a parent tree, a child annotation containing only
//! `theme = "dark"` keeps the source details and changes the theme. Setting
//! `inherit = "off", theme = "dark"` on that child restores `source = "hide"` and the other
//! rendering defaults before selecting the dark theme.
//!
//! # Scope
//!
//! Supported today:
//!
//! - Mermaid fences using backticks or tildes.
//! - `include_mmd!("path/to/file.mmd")` lines outside other Markdown code fences.
//! - Diagrams within lists, blockquotes, and footnotes using explicit container prefixes.
//! - Item docs on functions, modules, structs, traits, and impl blocks.
//! - Recursive inline item docs with `scope = "tree"`.
//! - Multiple diagrams on the same item.
//! - Footnotes and normal Markdown around diagrams.
//! - Re-exported item docs when the upstream item was rendered first.
//!
//! Not supported today:
//!
//! - Crate-level inner docs using `//!`.
//! - Rewriting Markdown loaded through `#[doc = include_str!("...")]`.
//! - Rustdoc intra-doc symbol links inside rendered Mermaid SVG text.
//! - Recursive processing for external `mod name;` files.
//! - Running Mermaid JavaScript in the browser.
//! - Fetching Mermaid source or assets from remote URLs.
//!
//! # Crate-level docs
//!
//! `merman-rustdoc` rewrites item-level outer docs. It does not rewrite crate-level inner docs
//! written with `//!`.
//!
//! Put crate-level diagrams on a public module or item instead:
//!
//! ````rust
//! #[cfg_attr(all(doc, feature = "doc-diagrams"), merman_rustdoc::merman)]
//! /// Crate architecture.
//! ///
//! /// ```mermaid
//! /// flowchart TD
//! ///   Crate --> Module
//! /// ```
//! pub mod architecture {}
//! ````
//!
//! # External docs, links, and themes
//!
//! `merman-rustdoc` does not evaluate or rewrite Markdown loaded through
//! `#[doc = include_str!("...")]`. Use `include_mmd!("path.mmd")` for Mermaid files instead.
//!
//! Mermaid source is rendered to SVG before rustdoc resolves intra-doc links. Text inside the SVG
//! does not participate in rustdoc link resolution, so labels such as `[Type](crate::Type)` are
//! treated as Mermaid text or Mermaid links, not rustdoc symbol links.
//!
//! By default, `merman-rustdoc` follows rustdoc's light/dark theme setting. It renders light and
//! dark SVG variants during `cargo doc` and uses rustdoc's page theme state to show the matching
//! variant.
//! The switch is CSS-only: both variants are embedded in the generated HTML, and the browser does
//! not load Mermaid JavaScript to render or recolor diagrams.
//!
//! Use `theme = "mermaid"` for a single SVG controlled by Mermaid source config. Use
//! `theme = "dark"` or another supported Mermaid theme to choose one fixed build-time theme.
//! Source-level Mermaid config, such as an `%%init%%` directive, is still passed to Merman with the
//! rest of the diagram and overrides the rustdoc-level theme default. Whether a specific theme
//! directive works depends on Merman's renderer support for that diagram and config.

extern crate proc_macro;

#[cfg(feature = "svg")]
mod doc;
#[cfg(feature = "svg")]
mod error;
#[cfg(feature = "svg")]
mod expand;
#[cfg(feature = "svg")]
mod options;
#[cfg(feature = "svg")]
mod render;

use proc_macro::TokenStream;
#[cfg(feature = "svg")]
use proc_macro2::TokenStream as TokenStream2;
#[cfg(feature = "svg")]
use quote::quote;
#[cfg(feature = "svg")]
use syn::LitStr;

/// Render Mermaid code fences in rustdoc comments as inline SVG.
///
/// Use the optional `doc-diagrams` dependency setup in the crate documentation so normal builds
/// neither compile the renderer nor expand diagrams while that feature is disabled:
///
/// ````rust
/// #[cfg_attr(all(doc, feature = "doc-diagrams"), merman_rustdoc::merman)]
/// /// ```mermaid
/// /// flowchart TD
/// ///   A --> B
/// /// ```
/// pub fn example() {}
/// ````
#[proc_macro_attribute]
#[cfg(feature = "svg")]
pub fn merman(args: TokenStream, input: TokenStream) -> TokenStream {
    let namespace = invocation_namespace(&input);
    let input: TokenStream2 = input.into();
    let args: TokenStream2 = args.into();

    let options = match options::Options::parse(args) {
        Ok(options) => options,
        Err(err) => return compile_error_with_input(input, &err.to_string()),
    };

    let helper_name = match proc_macro_crate::crate_name("merman-rustdoc") {
        Ok(proc_macro_crate::FoundCrate::Name(name)) => name,
        _ => "merman_rustdoc".to_string(),
    };
    let helper_ident = syn::Ident::new(&helper_name, proc_macro2::Span::call_site());
    let helper = syn::parse_quote!(::#helper_ident);
    match expand::expand(input.clone(), &options, &namespace, &helper) {
        Ok(output) => output.into(),
        Err(err) => compile_error_with_input(input, &err.to_string()),
    }
}

/// Report a missing renderer capability instead of compiling an accidental partial macro.
#[cfg(not(feature = "svg"))]
#[proc_macro_attribute]
pub fn merman(_args: TokenStream, input: TokenStream) -> TokenStream {
    let mut output = "compile_error!(\"merman-rustdoc requires the `svg` feature; enable it on the dependency\");"
        .parse::<TokenStream>()
        .expect("static compile_error token stream");
    output.extend(input);
    output
}

#[cfg(feature = "svg")]
fn compile_error_with_input(input: TokenStream2, message: &str) -> TokenStream {
    let message = LitStr::new(message, proc_macro2::Span::call_site());
    quote! {
        compile_error!(#message);
        #input
    }
    .into()
}

/// Internal delayed documentation expansion. Use the `merman` attribute instead.
#[doc(hidden)]
#[cfg(feature = "svg")]
#[proc_macro]
pub fn __render_doc(input: TokenStream) -> TokenStream {
    let result = syn::parse::<expand::DeferredDoc>(input)
        .map_err(error::Error::from)
        .and_then(|deferred| {
            let fragments = deferred
                .documents
                .iter()
                .map(LitStr::value)
                .collect::<Vec<_>>();
            let document = doc::normalize_document(&fragments, deferred.indentation);
            let namespace = deferred.namespace.value();
            let marker = format!("{namespace}-__merman_callsite__");
            let mut rendered = doc::rewrite_document(&document, &deferred.options, &marker)?;
            if rendered.starts_with('\n') && rendered.ends_with('\n') {
                rendered.push('\n');
            }
            Ok((rendered, namespace, marker))
        });
    match result {
        Ok((document, namespace, marker)) => {
            // Built-in location macros resolve the outer declarative-macro invocation, unlike
            // procedural Span::call_site(), which can point into its definition.
            let mut parts = document.split(&marker);
            let first = parts.next().unwrap_or_default();
            let rest =
                parts.map(|part| quote! { #namespace, "-L", line!(), "-C", column!(), #part, });
            quote!(concat!(#first, #(#rest)*)).into()
        }
        Err(err) => {
            let message = LitStr::new(&err.to_string(), proc_macro2::Span::call_site());
            quote!(compile_error!(#message)).into()
        }
    }
}

#[cfg(feature = "svg")]
fn invocation_namespace(input: &TokenStream) -> String {
    let span = proc_macro::Span::call_site();
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").map(std::path::PathBuf::from);
    let file = span
        .local_file()
        .and_then(|file| {
            manifest.as_ref().and_then(|base| {
                file.strip_prefix(base)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
            })
        })
        .unwrap_or_else(|| {
            let displayed = span.file();
            let path = std::path::Path::new(&displayed);
            if path.is_absolute() {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                displayed
            }
        });
    let file = file.replace('\\', "/");
    let package = format!(
        "{}@{}",
        std::env::var("CARGO_PKG_NAME").unwrap_or_default(),
        std::env::var("CARGO_PKG_VERSION").unwrap_or_default()
    );
    // Item tokens distinguish separate macro_rules expansions at one source location.
    let identity = format!("{package}:{file}:{}:{}:{input}", span.line(), span.column());
    format!(
        "merman-rustdoc-{:016x}",
        render::stable_hash(identity.as_bytes())
    )
}
