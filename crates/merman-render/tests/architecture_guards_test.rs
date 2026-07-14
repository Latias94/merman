use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{FnArg, ImplItemFn, ItemFn, Pat, Signature, Type};

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
