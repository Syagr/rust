use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::{Parse, ParseStream}, parse_macro_input, punctuated::Punctuated, Expr, Token};

/// Procedural macro `btreemap_proc!{ key => value, ... }`
#[proc_macro]
pub fn btreemap_proc(input: TokenStream) -> TokenStream {
    // Parse input as a comma-separated list of entries: key => value
    struct Entry {
        key: Expr,
        _fat: Token![=>],
        value: Expr,
    }

    impl Parse for Entry {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            Ok(Entry {
                key: input.parse()?,
                _fat: input.parse()?,
                value: input.parse()?,
            })
        }
    }

    let entries: Punctuated<Entry, Token![,]> =
        parse_macro_input!(input with Punctuated::<Entry, Token![,]>::parse_terminated);

    let inserts = entries.iter().map(|e| {
        let key = &e.key;
        let value = &e.value;
        quote! { map.insert(#key, #value); }
    });

    TokenStream::from(quote! {
        {
            let mut map = ::std::collections::BTreeMap::new();
            #(#inserts)*
            map
        }
    })
}
