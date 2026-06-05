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
//! Use a normal dependency for the simplest setup:
//!
//! ```toml
//! [dependencies]
//! merman-rustdoc = "0.7"
//! ```
//!
//! This works for local `cargo doc` and for docs.rs because the examples below use
//! `cfg_attr(doc, ...)`. The macro only expands during rustdoc builds, but Cargo will still compile
//! the dependency during ordinary builds.
//!
//! If you want ordinary builds to avoid compiling `merman-rustdoc`, make it optional behind a
//! documentation feature:
//!
//! ```toml
//! [dependencies]
//! merman-rustdoc = { version = "0.7", optional = true }
//!
//! [features]
//! doc-diagrams = ["dep:merman-rustdoc"]
//!
//! [package.metadata.docs.rs]
//! features = ["doc-diagrams"]
//! ```
//!
//! With this optional setup, build docs locally with:
//!
//! ```sh
//! cargo doc --features doc-diagrams
//! ```
//!
//! # Quickstart
//!
//! Put the attribute on any item whose docs contain a Mermaid fence:
//!
//! ````rust
//! #[cfg_attr(doc, merman_rustdoc::merman)]
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
//! #[cfg_attr(doc, merman_rustdoc::merman)]
//! /// Crate architecture.
//! ///
//! /// include_mmd!("docs/architecture.mmd")
//! pub fn architecture() {}
//! ```
//!
//! # Options
//!
//! The attribute accepts string options:
//!
//! ```rust
//! #[cfg_attr(
//!     doc,
//!     merman_rustdoc::merman(
//!         scope = "item",
//!         pipeline = "readable",
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
//! | `pipeline` | `readable`, `parity`, `resvg-safe` | `readable` | Selects the SVG output pipeline. |
//! | `fail` | `error`, `keep-source` | `error` | Controls what happens when rendering or file includes fail. |
//! | `source` | `hide`, `details` | `hide` | Adds a collapsed Mermaid source block under the SVG when set to `details`. |
//! | `sanitize` | `strict`, `off` | `strict` | Checks rendered SVG for script elements, event attributes, and unsafe resource references. |
//! | `theme` | `rustdoc`, `mermaid`, or a supported Mermaid theme name | `rustdoc` | Controls whether diagrams follow rustdoc light/dark themes, use Mermaid source config, or use a fixed Mermaid theme. |
//!
//! Use `scope = "tree"` to process docs on children inside an inline module, trait, impl block,
//! struct fields, and enum variants:
//!
//! ````rust
//! #[cfg_attr(
//!     doc,
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
//! # Scope
//!
//! Supported today:
//!
//! - Mermaid fences using backticks or tildes.
//! - `include_mmd!("path/to/file.mmd")` lines outside other Markdown code fences.
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
//! #[cfg_attr(doc, merman_rustdoc::merman)]
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

mod doc;
mod error;
mod expand;
mod html;
mod options;
mod render;
mod svg;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::LitStr;

/// Render Mermaid code fences in rustdoc comments as inline SVG.
///
/// Use this with `cfg_attr` so normal builds do not need to expand diagrams:
///
/// ````rust
/// #[cfg_attr(doc, merman_rustdoc::merman)]
/// /// ```mermaid
/// /// flowchart TD
/// ///   A --> B
/// /// ```
/// pub fn example() {}
/// ````
#[proc_macro_attribute]
pub fn merman(args: TokenStream, input: TokenStream) -> TokenStream {
    let input: TokenStream2 = input.into();
    let args: TokenStream2 = args.into();

    let options = match options::Options::parse(args) {
        Ok(options) => options,
        Err(err) => return compile_error_with_input(input, &err.to_string()),
    };

    match expand::expand(input.clone(), options) {
        Ok(output) => output.into(),
        Err(err) => compile_error_with_input(input, &err.to_string()),
    }
}

fn compile_error_with_input(input: TokenStream2, message: &str) -> TokenStream {
    let message = LitStr::new(message, proc_macro2::Span::call_site());
    quote! {
        compile_error!(#message);
        #input
    }
    .into()
}
