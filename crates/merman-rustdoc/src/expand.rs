use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Attribute, Expr, ExprLit, Lit, LitStr, Meta,
    parse::{Parse, ParseStream},
};

use crate::doc::rewrite_doc_lines;
use crate::error::Result;
use crate::options::Options;

struct MacroInput {
    attrs: Vec<Attribute>,
    rest: TokenStream,
}

impl Parse for MacroInput {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let rest = input.parse()?;
        Ok(Self { attrs, rest })
    }
}

pub(crate) fn expand(input: TokenStream, options: Options) -> Result<TokenStream> {
    let input = syn::parse2::<MacroInput>(input)?;
    let mut out_attrs = Vec::with_capacity(input.attrs.len());
    let mut doc_lines = Vec::new();
    let mut next_diagram = 0;

    for attr in input.attrs {
        if let Some(line) = doc_attr_value(&attr) {
            doc_lines.push(line);
            continue;
        }

        flush_doc_lines(&mut doc_lines, &mut next_diagram, options, &mut out_attrs)?;
        out_attrs.push(quote! { #attr });
    }

    flush_doc_lines(&mut doc_lines, &mut next_diagram, options, &mut out_attrs)?;

    let rest = input.rest;
    Ok(quote! {
        #(#out_attrs)*
        #rest
    })
}

fn flush_doc_lines(
    doc_lines: &mut Vec<String>,
    next_diagram: &mut usize,
    options: Options,
    out_attrs: &mut Vec<TokenStream>,
) -> Result<()> {
    if doc_lines.is_empty() {
        return Ok(());
    }

    for line in rewrite_doc_lines(doc_lines, next_diagram, options)? {
        let line = LitStr::new(&line, Span::call_site());
        out_attrs.push(quote! { #[doc = #line] });
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
