use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream, Parser},
    parse_quote, LitStr, Result, Token,
};

mod kw {
    syn::custom_keyword!(args);
}

struct Args {
    value: LitStr,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> Result<Self> {
        let _ = input.parse::<kw::args>()?;
        let _ = input.parse::<Token![=]>()?;
        let value = input.parse()?;
        Ok(Self { value })
    }
}

#[derive(Debug, Default)]
struct InstrumentAttr {
    args: Option<LitStr>,
}

impl Parse for InstrumentAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut ret = Self::default();
        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::args) {
                if ret.args.is_some() {
                    return Err(input.error("expected only a single `args` argument"));
                }
                let args = input.parse::<Args>()?.value;
                ret.args = Some(args);
            } else {
                let _ = input.parse::<proc_macro2::TokenTree>();
            }
        }
        Ok(ret)
    }
}

#[proc_macro_attribute]
pub fn instrument(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = syn::Item::parse.parse(item).unwrap();

    if let syn::Item::Fn(ref mut item) = input {
        let original = &item.block;
        let name = &item.sig.ident;
        let is_async = item.sig.asyncness.is_some();
        let args = syn::parse::<InstrumentAttr>(attr)
            .unwrap()
            .args
            .map(|args| args.value())
            .unwrap_or(String::new());

        item.block = Box::new(parse_quote! {{
            let start = chrometracer::current(|tracer| tracer.map(|t| t.start));

            if let Some(start) = start {
                let span = chrometracer::Span {
                    name: stringify!(#name).into(),
                    args: stringify!(#args).into(),
                    tid: None,
                    from: start.elapsed(),
                    is_async: #is_async,
                };
                #original
            } else {
                #original
            }
        }});
    } else {
        unreachable!()
    }

    input.into_token_stream().into()
}
