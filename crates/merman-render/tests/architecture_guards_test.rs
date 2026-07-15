use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, FnArg, ImplItemFn, ItemFn, LitStr, Pat, Signature, Type};

const FORBIDDEN_CENTRAL_SYMBOLS: &[&str] = &[
    "LayoutDiagram",
    "LayoutedDiagram",
    "layout_parsed",
    "layout_json_by_type",
    "render_layouted_svg",
    "render_layout_svg_parts",
];

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    fn visit(path: &Path, sources: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(path).expect("read Rust source directory") {
            let entry = entry.expect("read Rust source entry");
            let path = entry.path();
            if path.is_dir() {
                visit(&path, sources);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                sources.push(path);
            }
        }
    }

    let mut sources = Vec::new();
    visit(root, &mut sources);
    sources.sort();
    sources
}

fn is_json_value_reference(ty: &Type) -> bool {
    match ty {
        Type::Reference(reference) => is_json_value_type(&reference.elem),
        Type::Group(group) => is_json_value_reference(&group.elem),
        Type::Paren(paren) => is_json_value_reference(&paren.elem),
        _ => false,
    }
}

fn is_json_value_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Value"),
        Type::Group(group) => is_json_value_type(&group.elem),
        Type::Paren(paren) => is_json_value_type(&paren.elem),
        _ => false,
    }
}

fn json_model_parameter(signature: &Signature) -> Option<&str> {
    signature.inputs.iter().find_map(|argument| {
        let FnArg::Typed(argument) = argument else {
            return None;
        };
        let Pat::Ident(name) = argument.pat.as_ref() else {
            return None;
        };
        matches!(name.ident.to_string().as_str(), "semantic" | "model")
            .then(|| is_json_value_reference(&argument.ty))
            .filter(|is_json| *is_json)
            .map(|_| {
                if name.ident == "semantic" {
                    "semantic"
                } else {
                    "model"
                }
            })
    })
}

struct ArchitectureGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

impl ArchitectureGuard<'_> {
    fn inspect_signature(&mut self, signature: &Signature) {
        let name = signature.ident.to_string();
        let Some(parameter) = json_model_parameter(signature) else {
            return;
        };

        if name.starts_with("layout_") {
            self.violations.push(format!(
                "{}: `{name}` accepts raw JSON `{parameter}` input",
                self.relative_path
            ));
        }
        if name.starts_with("render_") && name.contains("svg") {
            self.violations.push(format!(
                "{}: `{name}` accepts a raw JSON semantic model",
                self.relative_path
            ));
        }
    }
}

impl<'ast> Visit<'ast> for ArchitectureGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        self.inspect_signature(&function.sig);
        visit::visit_item_fn(self, function);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast ImplItemFn) {
        self.inspect_signature(&function.sig);
        visit::visit_impl_item_fn(self, function);
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if FORBIDDEN_CENTRAL_SYMBOLS.contains(&ident.to_string().as_str()) {
            self.violations.push(format!(
                "{}: legacy central render symbol `{ident}` remains",
                self.relative_path
            ));
        }
    }
}

#[test]
fn built_in_rendering_has_no_json_input_or_central_dispatch_fallback() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for path in rust_sources(&source_root) {
        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let syntax = syn::parse_file(&source).expect("parse Rust source");
        let relative = path
            .strip_prefix(&source_root)
            .unwrap_or(&path)
            .to_string_lossy();
        ArchitectureGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "built-in rendering must stay typed from family semantics through SVG:\n{}",
        violations.join("\n")
    );
}

struct CanonicalFunctionNameGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

impl<'ast> Visit<'ast> for CanonicalFunctionNameGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if is_test_only(&function.attrs) {
            return;
        }

        let name = function.sig.ident.to_string();
        let is_canonical_entry = ["layout_", "render_", "try_render_", "build_", "debug_"]
            .iter()
            .any(|prefix| name.starts_with(prefix));
        let has_version_segment = name.split('_').any(|segment| segment == "v2");
        if is_canonical_entry && has_version_segment {
            self.violations.push(format!(
                "{}: canonical family function `{name}` carries a transition version",
                self.relative_path
            ));
        }

        visit::visit_item_fn(self, function);
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }
}

#[test]
fn canonical_family_functions_are_versionless() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for path in rust_sources(&source_root) {
        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let syntax = syn::parse_file(&source).expect("parse Rust source");
        let relative = path
            .strip_prefix(&source_root)
            .unwrap_or(&path)
            .to_string_lossy();
        CanonicalFunctionNameGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "canonical family functions must not expose transition version names:\n{}",
        violations.join("\n")
    );
}

struct RootOwnershipGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

fn is_test_only(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("test")
            || (attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Meta>()
                    .is_ok_and(|meta| cfg_requires_test(&meta)))
    })
}

fn cfg_requires_test(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) if list.path.is_ident("all") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|items| items.iter().any(cfg_requires_test)),
        syn::Meta::List(list) if list.path.is_ident("any") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|items| !items.is_empty() && items.iter().all(cfg_requires_test)),
        syn::Meta::List(_) | syn::Meta::NameValue(_) => false,
    }
}

#[test]
fn test_only_cfg_detection_respects_boolean_structure() {
    fn classified_as_test_only(attribute: &str) -> bool {
        let function: ItemFn = syn::parse_str(&format!("{attribute}\nfn guarded() {{}}"))
            .expect("parse attributed function");
        is_test_only(&function.attrs)
    }

    assert!(classified_as_test_only("#[test]"));
    assert!(classified_as_test_only("#[cfg(test)]"));
    assert!(classified_as_test_only("#[cfg(all(test, unix))]"));
    assert!(!classified_as_test_only("#[cfg(not(test))]"));
    assert!(!classified_as_test_only("#[cfg(any(test, unix))]"));
}

impl<'ast> Visit<'ast> for RootOwnershipGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !is_test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        let name = ident.to_string();
        let forbidden = matches!(
            name.as_str(),
            "push_svg_root_open"
                | "push_svg_root_open_with_viewport_plan"
                | "SvgRootAttrs"
                | "SvgRootWidth"
                | "apply_root_viewport_override"
                | "build_root_viewport_plan"
                | "fmt_max_width_px"
                | "fmt_max_width_px_into"
                | "VIEWBOX_PLACEHOLDER"
                | "MAX_WIDTH_PLACEHOLDER"
        ) || (name.starts_with("lookup_")
            && name.ends_with("_root_viewport_override"));
        if forbidden {
            self.violations.push(format!(
                "{}: family bypasses Root Viewport ownership through `{name}`",
                self.relative_path
            ));
        }
    }

    fn visit_lit_str(&mut self, literal: &'ast LitStr) {
        let value = literal.value();
        if value.contains("<svg id=")
            || value.contains("__MERMAN_ROOT_")
            || value.contains("__MERMAID_VIEWBOX__")
            || value.contains("__MERMAID_MAX_WIDTH__")
        {
            self.violations.push(format!(
                "{}: family contains a direct root SVG emitter or viewport placeholder",
                self.relative_path
            ));
        }
    }
}

#[test]
fn every_family_delegates_root_viewport_policy_and_emission() {
    let parity_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/svg/parity");
    let root_module = parity_root.join("root_svg.rs");
    let mut violations = Vec::new();

    for path in rust_sources(&parity_root) {
        if path == root_module {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let syntax = syn::parse_file(&source).expect("parse Rust source");
        let relative = path
            .strip_prefix(&parity_root)
            .unwrap_or(&path)
            .to_string_lossy();
        RootOwnershipGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "built-in families must delegate root sizing, overrides, and emission to root_svg:\n{}",
        violations.join("\n")
    );
}

struct RootPolicySelectionGuard<'a> {
    relative_path: &'a str,
    violations: &'a mut Vec<String>,
}

fn calls_computed_root_policy(function: &Expr) -> bool {
    let Expr::Path(path) = function else {
        return false;
    };
    let mut segments = path.path.segments.iter().rev();
    segments
        .next()
        .is_some_and(|segment| segment.ident == "computed")
        && segments
            .next()
            .is_some_and(|segment| segment.ident == "RootViewportContext")
}

impl<'ast> Visit<'ast> for RootPolicySelectionGuard<'_> {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if !is_test_only(&function.attrs) {
            visit::visit_item_fn(self, function);
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if !is_test_only(&module.attrs) {
            visit::visit_item_mod(self, module);
        }
    }

    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        if calls_computed_root_policy(&call.func) {
            self.violations.push(format!(
                "{}: production family selects ComputedOnly instead of the operation root policy",
                self.relative_path
            ));
        }
        visit::visit_expr_call(self, call);
    }
}

#[test]
fn production_families_use_the_operation_root_viewport_policy() {
    let parity_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/svg/parity");
    let root_module = parity_root.join("root_svg.rs");
    let mut violations = Vec::new();

    for path in rust_sources(&parity_root) {
        if path == root_module {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("read Rust source");
        let syntax = syn::parse_file(&source).expect("parse Rust source");
        let relative = path
            .strip_prefix(&parity_root)
            .unwrap_or(&path)
            .to_string_lossy();
        RootPolicySelectionGuard {
            relative_path: &relative,
            violations: &mut violations,
        }
        .visit_file(&syntax);
    }

    assert!(
        violations.is_empty(),
        "production families must consume the operation-selected root viewport policy:\n{}",
        violations.join("\n")
    );
}
