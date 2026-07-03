use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{
    LitStr, Path, Result, Token,
    parse::{Parse, ParseStream, Parser},
    parse_quote,
};

mod kw {
    syn::custom_keyword!(args);
    syn::custom_keyword!(reexported_as);
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

struct ReexportedAs {
    crate_name: Path,
}

impl Parse for ReexportedAs {
    fn parse(input: ParseStream) -> Result<Self> {
        let _ = input.parse::<kw::reexported_as>()?;
        let _ = input.parse::<Token![=]>()?;
        let crate_str: LitStr = input.parse()?;
        let crate_name = crate_str.parse()?;
        Ok(Self { crate_name })
    }
}

#[derive(Debug, Default)]
struct InstrumentAttr {
    args: Option<LitStr>,
    crate_name: Option<Path>,
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
            } else if lookahead.peek(kw::reexported_as) {
                if ret.crate_name.is_some() {
                    return Err(input.error("expected only a single `reexported_as` argument"));
                }
                let crate_name = input.parse::<ReexportedAs>()?.crate_name;
                ret.crate_name = Some(crate_name);
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
        let attr = syn::parse::<InstrumentAttr>(attr).unwrap();
        let args = attr.args.map(|args| args.value()).unwrap_or_default();
        let crate_name: Path = attr
            .crate_name
            .unwrap_or_else(|| parse_quote!(::chrometracer));

        *item.block = parse_quote! {{
            use #crate_name as __chrometracer;
            let start = __chrometracer::current(|tracer| tracer.map(|t| t.start));

            if let Some(start) = start {
                let span = __chrometracer::Span {
                    name: stringify!(#name).into(),
                    args: #args.into(),
                    tid: None,
                    from: start.elapsed(),
                    is_async: #is_async,
                };
                #original
            } else {
                #original
            }
        }};
    } else {
        unreachable!()
    }

    input.into_token_stream().into()
}
