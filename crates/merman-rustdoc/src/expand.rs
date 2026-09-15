use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, Expr, ExprLit, Fields, ForeignItem, ImplItem, Item, Lit, LitStr, Meta, TraitItem,
    parse_quote,
};

use crate::error::{Error, Result};
use crate::options::{Options, ScopeMode};
use syn::parse::{Parse, ParseStream};
use syn::{Token, bracketed, parenthesized, spanned::Spanned};

pub(crate) fn expand(
    input: TokenStream,
    options: &Options,
    namespace: &str,
    helper: &syn::Path,
) -> Result<TokenStream> {
    let mut item = syn::parse2::<Item>(input)?;
    validate_scope(&item, options.scope)?;
    let recurse = options.scope == ScopeMode::Tree;
    let mut parent = None;
    visit_item(&mut item, recurse, &mut |attributes| {
        if parent.is_none() {
            for attribute in attributes {
                if let Some(document) = deferred_doc(attribute, helper)? {
                    parent = Some(document.options);
                    break;
                }
            }
        }
        Ok(())
    })?;
    let options = parent.map_or_else(|| options.clone(), |parent| options.with_parent(&parent));

    let mut context = Expansion {
        options: &options,
        namespace,
        helper,
        next_document: 0,
    };
    visit_item(&mut item, recurse, &mut |attributes| {
        rewrite_attrs(attributes, &mut context)
    })?;

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

type AttributeVisitor<'a> = dyn FnMut(&mut Vec<Attribute>) -> Result<()> + 'a;

fn visit_item(item: &mut Item, recurse: bool, visitor: &mut AttributeVisitor<'_>) -> Result<()> {
    visitor(item_attrs_mut(item))?;
    if recurse {
        visit_item_children(item, visitor)?;
    }
    Ok(())
}

fn visit_item_children(item: &mut Item, visitor: &mut AttributeVisitor<'_>) -> Result<()> {
    match item {
        Item::Enum(item) => {
            for variant in &mut item.variants {
                visitor(&mut variant.attrs)?;
                visit_fields(&mut variant.fields, visitor)?;
            }
        }
        Item::ForeignMod(item) => {
            for item in &mut item.items {
                visit_foreign_item(item, visitor)?;
            }
        }
        Item::Impl(item) => {
            for item in &mut item.items {
                visit_impl_item(item, visitor)?;
            }
        }
        Item::Mod(item) => {
            if let Some((_brace, items)) = &mut item.content {
                for item in items {
                    visit_item(item, true, visitor)?;
                }
            }
        }
        Item::Struct(item) => visit_fields(&mut item.fields, visitor)?,
        Item::Trait(item) => {
            for item in &mut item.items {
                visit_trait_item(item, visitor)?;
            }
        }
        Item::Union(item) => {
            for field in &mut item.fields.named {
                visitor(&mut field.attrs)?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn visit_fields(fields: &mut Fields, visitor: &mut AttributeVisitor<'_>) -> Result<()> {
    match fields {
        Fields::Named(fields) => {
            for field in &mut fields.named {
                visitor(&mut field.attrs)?;
            }
        }
        Fields::Unnamed(fields) => {
            for field in &mut fields.unnamed {
                visitor(&mut field.attrs)?;
            }
        }
        Fields::Unit => {}
    }
    Ok(())
}

fn visit_impl_item(item: &mut ImplItem, visitor: &mut AttributeVisitor<'_>) -> Result<()> {
    match item {
        ImplItem::Const(item) => visitor(&mut item.attrs),
        ImplItem::Fn(item) => visitor(&mut item.attrs),
        ImplItem::Macro(item) => visitor(&mut item.attrs),
        ImplItem::Type(item) => visitor(&mut item.attrs),
        ImplItem::Verbatim(_) => Ok(()),
        _ => Ok(()),
    }
}

fn visit_trait_item(item: &mut TraitItem, visitor: &mut AttributeVisitor<'_>) -> Result<()> {
    match item {
        TraitItem::Const(item) => visitor(&mut item.attrs),
        TraitItem::Fn(item) => visitor(&mut item.attrs),
        TraitItem::Macro(item) => visitor(&mut item.attrs),
        TraitItem::Type(item) => visitor(&mut item.attrs),
        TraitItem::Verbatim(_) => Ok(()),
        _ => Ok(()),
    }
}

fn visit_foreign_item(item: &mut ForeignItem, visitor: &mut AttributeVisitor<'_>) -> Result<()> {
    match item {
        ForeignItem::Fn(item) => visitor(&mut item.attrs),
        ForeignItem::Macro(item) => visitor(&mut item.attrs),
        ForeignItem::Static(item) => visitor(&mut item.attrs),
        ForeignItem::Type(item) => visitor(&mut item.attrs),
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

struct Expansion<'a> {
    options: &'a Options,
    namespace: &'a str,
    helper: &'a syn::Path,
    next_document: usize,
}

/// The internal protocol keeps original literals so nested attributes can override options.
pub(crate) struct DeferredDoc {
    pub(crate) options: Options,
    pub(crate) namespace: LitStr,
    pub(crate) indentation: usize,
    pub(crate) documents: Vec<LitStr>,
}

impl Parse for DeferredDoc {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let options_input;
        parenthesized!(options_input in input);
        let options =
            Options::parse(options_input.parse()?).map_err(|err| input.error(err.to_string()))?;
        input.parse::<Token![,]>()?;
        let namespace = input.parse()?;
        input.parse::<Token![,]>()?;
        let indentation = input.parse::<syn::LitInt>()?.base10_parse()?;
        input.parse::<Token![,]>()?;
        let documents_input;
        bracketed!(documents_input in input);
        let documents = documents_input
            .parse_terminated(|input| input.parse::<LitStr>(), Token![,])?
            .into_iter()
            .collect();
        Ok(Self {
            options,
            namespace,
            indentation,
            documents,
        })
    }
}

fn rewrite_attrs(attrs: &mut Vec<Attribute>, context: &mut Expansion<'_>) -> Result<()> {
    let input = std::mem::take(attrs);
    let mut all_documents = Vec::new();
    let mut dynamic = false;
    for attr in &input {
        if let Some(value) = doc_attr_value(attr) {
            let literal = LitStr::new(&value, attr.span());
            all_documents.push(LitStr::new(
                &crate::doc::prepare_literal(&literal),
                attr.span(),
            ));
        } else if let Some(deferred) = deferred_doc(attr, context.helper)? {
            all_documents.extend(deferred.documents);
        } else if attr.path().is_ident("doc") {
            dynamic = true;
        }
    }
    // Dynamic documentation is left to rustdoc; do not infer indentation from an isolated group.
    let indentation = if dynamic {
        0
    } else {
        crate::doc::common_indentation(&all_documents)
    };
    let mut output = Vec::with_capacity(input.len());
    let mut documents = Vec::new();
    let mut insert_at = None;
    let mut style = syn::AttrStyle::Outer;
    for attr in input {
        if let Some(value) = doc_attr_value(&attr) {
            if insert_at.is_none() {
                insert_at = Some(output.len());
                style = attr.style;
            }
            let literal = LitStr::new(&value, attr.span());
            documents.push(LitStr::new(
                &crate::doc::prepare_literal(&literal),
                attr.span(),
            ));
        } else if let Some(deferred) = deferred_doc(&attr, context.helper)? {
            if insert_at.is_none() {
                insert_at = Some(output.len());
                style = attr.style;
            }
            documents.extend(deferred.documents);
        } else {
            // Dynamic doc expressions retain their position and are not evaluated by this adapter.
            if attr.path().is_ident("doc") {
                flush_documents(
                    &mut documents,
                    &mut insert_at,
                    &style,
                    &mut output,
                    context,
                    indentation,
                );
            }
            output.push(attr);
        }
    }
    flush_documents(
        &mut documents,
        &mut insert_at,
        &style,
        &mut output,
        context,
        indentation,
    );
    *attrs = output;
    Ok(())
}

fn deferred_doc(attr: &Attribute, helper: &syn::Path) -> Result<Option<DeferredDoc>> {
    if !attr.path().is_ident("doc") {
        return Ok(None);
    }
    let Meta::NameValue(value) = &attr.meta else {
        return Ok(None);
    };
    let Expr::Macro(expr) = &value.value else {
        return Ok(None);
    };
    let expected: syn::Path = parse_quote!(#helper::__render_doc);
    if expr
        .mac
        .path
        .segments
        .iter()
        .map(|s| &s.ident)
        .ne(expected.segments.iter().map(|s| &s.ident))
    {
        return Ok(None);
    }
    Ok(Some(syn::parse2(expr.mac.tokens.clone())?))
}

fn flush_documents(
    documents: &mut Vec<LitStr>,
    insert_at: &mut Option<usize>,
    style: &syn::AttrStyle,
    output: &mut Vec<Attribute>,
    context: &mut Expansion<'_>,
    indentation: usize,
) {
    let Some(index) = insert_at.take() else {
        return;
    };
    let namespace = format!("{}-{}", context.namespace, context.next_document);
    context.next_document += 1;
    let options = context.options.tokens();
    let helper = context.helper;
    // Rustdoc trims an outer newline from each multiline doc value. Keep standalone
    // blank attributes at group boundaries so dynamic neighbors retain their separation.
    let values = std::mem::take(documents);
    let first = values
        .iter()
        .position(|value| !value.value().is_empty())
        .unwrap_or(values.len());
    let end = values
        .iter()
        .rposition(|value| !value.value().is_empty())
        .map_or(first, |index| index + 1);
    let middle = &values[first..end];
    let mut replacement: Vec<Attribute> = Vec::new();
    for value in &values[..first] {
        replacement.push(parse_quote! { #[doc = #value] });
    }
    if !middle.is_empty() {
        replacement.push(parse_quote! { #[doc = #helper::__render_doc!((#options), #namespace, #indentation, [#(#middle),*])] });
    }
    for value in &values[end..] {
        replacement.push(parse_quote! { #[doc = #value] });
    }
    for mut attr in replacement.into_iter().rev() {
        attr.style = *style;
        output.insert(index, attr);
    }
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

        let err = expand(
            quote! { pub mod external; },
            &options,
            "test",
            &syn::parse_quote!(::merman_rustdoc),
        )
        .unwrap_err();

        assert!(err.to_string().contains("requires an inline module"));
    }
}
