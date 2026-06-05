use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Expr, ExprLit, Fields, ForeignItem, ImplItem, Item, Lit, LitStr, Meta, TraitItem,
    parse_quote,
};

use crate::doc::rewrite_doc_lines;
use crate::error::{Error, Result};
use crate::options::{Options, ScopeMode};

pub(crate) fn expand(input: TokenStream, options: Options) -> Result<TokenStream> {
    let mut item = syn::parse2::<Item>(input)?;
    validate_scope(&item, options.scope)?;

    let mut next_diagram = 0;
    rewrite_item(
        &mut item,
        options,
        &mut next_diagram,
        options.scope == ScopeMode::Tree,
    )?;

    Ok(quote! { #item })
}

fn validate_scope(item: &Item, scope: ScopeMode) -> Result<()> {
    if scope == ScopeMode::Tree && matches!(item, Item::Mod(module) if module.content.is_none()) {
        return Err(Error::new(
            "merman_rustdoc scope = \"tree\" requires an inline module; external `mod name;` items cannot be inspected by the proc macro",
        ));
    }
    Ok(())
}

fn rewrite_item(
    item: &mut Item,
    options: Options,
    next_diagram: &mut usize,
    recurse: bool,
) -> Result<()> {
    rewrite_attrs(item_attrs_mut(item), options, next_diagram)?;
    if recurse {
        rewrite_item_children(item, options, next_diagram)?;
    }
    Ok(())
}

fn rewrite_item_children(
    item: &mut Item,
    options: Options,
    next_diagram: &mut usize,
) -> Result<()> {
    match item {
        Item::Enum(item) => {
            for variant in &mut item.variants {
                rewrite_attrs(&mut variant.attrs, options, next_diagram)?;
                rewrite_fields(&mut variant.fields, options, next_diagram)?;
            }
        }
        Item::ForeignMod(item) => {
            for item in &mut item.items {
                rewrite_foreign_item(item, options, next_diagram)?;
            }
        }
        Item::Impl(item) => {
            for item in &mut item.items {
                rewrite_impl_item(item, options, next_diagram)?;
            }
        }
        Item::Mod(item) => {
            if let Some((_brace, items)) = &mut item.content {
                for item in items {
                    rewrite_item(item, options, next_diagram, true)?;
                }
            }
        }
        Item::Struct(item) => rewrite_fields(&mut item.fields, options, next_diagram)?,
        Item::Trait(item) => {
            for item in &mut item.items {
                rewrite_trait_item(item, options, next_diagram)?;
            }
        }
        Item::Union(item) => {
            for field in &mut item.fields.named {
                rewrite_attrs(&mut field.attrs, options, next_diagram)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn rewrite_fields(fields: &mut Fields, options: Options, next_diagram: &mut usize) -> Result<()> {
    match fields {
        Fields::Named(fields) => {
            for field in &mut fields.named {
                rewrite_attrs(&mut field.attrs, options, next_diagram)?;
            }
        }
        Fields::Unnamed(fields) => {
            for field in &mut fields.unnamed {
                rewrite_attrs(&mut field.attrs, options, next_diagram)?;
            }
        }
        Fields::Unit => {}
    }
    Ok(())
}

fn rewrite_impl_item(
    item: &mut ImplItem,
    options: Options,
    next_diagram: &mut usize,
) -> Result<()> {
    match item {
        ImplItem::Const(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        ImplItem::Fn(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        ImplItem::Macro(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        ImplItem::Type(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        ImplItem::Verbatim(_) => Ok(()),
        _ => Ok(()),
    }
}

fn rewrite_trait_item(
    item: &mut TraitItem,
    options: Options,
    next_diagram: &mut usize,
) -> Result<()> {
    match item {
        TraitItem::Const(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        TraitItem::Fn(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        TraitItem::Macro(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        TraitItem::Type(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        TraitItem::Verbatim(_) => Ok(()),
        _ => Ok(()),
    }
}

fn rewrite_foreign_item(
    item: &mut ForeignItem,
    options: Options,
    next_diagram: &mut usize,
) -> Result<()> {
    match item {
        ForeignItem::Fn(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        ForeignItem::Macro(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        ForeignItem::Static(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        ForeignItem::Type(item) => rewrite_attrs(&mut item.attrs, options, next_diagram),
        ForeignItem::Verbatim(_) => Ok(()),
        _ => Ok(()),
    }
}

fn item_attrs_mut(item: &mut Item) -> &mut Vec<Attribute> {
    match item {
        Item::Const(item) => &mut item.attrs,
        Item::Enum(item) => &mut item.attrs,
        Item::ExternCrate(item) => &mut item.attrs,
        Item::Fn(item) => &mut item.attrs,
        Item::ForeignMod(item) => &mut item.attrs,
        Item::Impl(item) => &mut item.attrs,
        Item::Macro(item) => &mut item.attrs,
        Item::Mod(item) => &mut item.attrs,
        Item::Static(item) => &mut item.attrs,
        Item::Struct(item) => &mut item.attrs,
        Item::Trait(item) => &mut item.attrs,
        Item::TraitAlias(item) => &mut item.attrs,
        Item::Type(item) => &mut item.attrs,
        Item::Union(item) => &mut item.attrs,
        Item::Use(item) => &mut item.attrs,
        Item::Verbatim(_) => unreachable!("attribute macros are not invoked on verbatim items"),
        _ => unreachable!("unsupported syn item variant"),
    }
}

fn rewrite_attrs(
    attrs: &mut Vec<Attribute>,
    options: Options,
    next_diagram: &mut usize,
) -> Result<()> {
    let input = std::mem::take(attrs);
    let mut output = Vec::with_capacity(input.len());
    let mut doc_lines = Vec::new();

    for attr in input {
        if let Some(line) = doc_attr_value(&attr) {
            doc_lines.push(line);
            continue;
        }

        flush_doc_lines(&mut doc_lines, next_diagram, options, &mut output)?;
        output.push(attr);
    }

    flush_doc_lines(&mut doc_lines, next_diagram, options, &mut output)?;
    *attrs = output;
    Ok(())
}

fn flush_doc_lines(
    doc_lines: &mut Vec<String>,
    next_diagram: &mut usize,
    options: Options,
    out_attrs: &mut Vec<Attribute>,
) -> Result<()> {
    if doc_lines.is_empty() {
        return Ok(());
    }

    for line in rewrite_doc_lines(doc_lines, next_diagram, options)? {
        let line = LitStr::new(&line, Span::call_site());
        out_attrs.push(parse_quote! { #[doc = #line] });
    }
    doc_lines.clear();
    Ok(())
}

fn doc_attr_value(attr: &Attribute) -> Option<String> {
    if !attr.path().is_ident("doc") {
        return None;
    }

    let Meta::NameValue(name_value) = &attr.meta else {
        return None;
    };
    let Expr::Lit(ExprLit {
        lit: Lit::Str(value),
        ..
    }) = &name_value.value
    else {
        return None;
    };

    Some(value.value())
}

#[cfg(test)]
mod tests {
    use quote::quote;

    use super::*;

    #[test]
    fn tree_scope_rejects_external_modules() {
        let options = Options {
            scope: ScopeMode::Tree,
            ..Options::default()
        };

        let err = expand(quote! { pub mod external; }, options).unwrap_err();

        assert!(err.to_string().contains("requires an inline module"));
    }
}
