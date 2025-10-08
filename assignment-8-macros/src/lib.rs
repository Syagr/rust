use proc_macro::TokenStream;
use quote::quote;
use syn::Expr;

/// Procedural macro `btreemap_proc!{ key => value, ... }`
#[proc_macro]
pub fn btreemap_proc(input: TokenStream) -> TokenStream {
    use proc_macro2::TokenTree as TT2;

    // convert to proc_macro2 token stream to preserve spans
    let ts2: proc_macro2::TokenStream = input.into();
    let mut iter = ts2.into_iter().peekable();
    let mut pairs: Vec<(proc_macro2::TokenStream, proc_macro2::TokenStream)> = Vec::new();

    while let Some(token) = iter.next() {
        // skip stray commas
        if let TT2::Punct(p) = &token {
            if p.as_char() == ',' {
                continue;
            }
        }

        // collect key tokenstream until we hit '=>'
        let mut key_ts2 = proc_macro2::TokenStream::new();
        key_ts2.extend(std::iter::once(token.clone()));
        while let Some(peek) = iter.peek() {
            if let TT2::Punct(p) = peek {
                if p.as_char() == '=' {
                    // consume '=' and the following '>' if present
                    let _ = iter.next();
                    if let Some(TT2::Punct(_)) = iter.peek() {
                        // expect '>' after '='; consume if present
                        let _ = iter.next();
                    }
                    break;
                }
            }
            key_ts2.extend(std::iter::once(iter.next().unwrap()));
        }

        // collect value until top-level comma
        let mut value_ts2 = proc_macro2::TokenStream::new();
        let mut depth: i32 = 0;
        while let Some(peek) = iter.peek() {
            match peek {
                TT2::Punct(p) if p.as_char() == ',' && depth == 0 => {
                    let _ = iter.next();
                    break;
                }
                TT2::Group(_g) => {
                    // include the whole group token
                    value_ts2.extend(std::iter::once(iter.next().unwrap()));
                }
                TT2::Punct(p) if p.as_char() == '(' || p.as_char() == '[' || p.as_char() == '<' => {
                    depth += 1;
                    value_ts2.extend(std::iter::once(iter.next().unwrap()));
                }
                TT2::Punct(p) if p.as_char() == ')' || p.as_char() == ']' || p.as_char() == '>' => {
                    depth -= 1;
                    value_ts2.extend(std::iter::once(iter.next().unwrap()));
                }
                _ => value_ts2.extend(std::iter::once(iter.next().unwrap())),
            }
        }

        pairs.push((key_ts2, value_ts2));
    }

    let mut inits = Vec::new();
    for (k_ts, v_ts) in pairs {
        let k_ts2 = k_ts;
        let v_ts2 = v_ts;

        let k_expr: Expr = match syn::parse2(k_ts2.clone()) {
            Ok(e) => e,
            Err(err) => {
                let compile_err = err.to_compile_error();
                return TokenStream::from(quote! { #compile_err });
            }
        };

        let v_expr: Expr = match syn::parse2(v_ts2.clone()) {
            Ok(e) => e,
            Err(err) => {
                let compile_err = err.to_compile_error();
                return TokenStream::from(quote! { #compile_err });
            }
        };

        inits.push(quote! { map.insert(#k_expr, #v_expr); });
    }

    let expanded = quote! {
        {
            let mut map = ::std::collections::BTreeMap::new();
            #(#inits)*
            map
        }
    };

    TokenStream::from(expanded)
}
