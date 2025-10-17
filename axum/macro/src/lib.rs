use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{ItemFn, parse_macro_input, spanned::Spanned};

#[proc_macro_attribute]
pub fn attr_macro_http_server(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    if input.sig.ident != "main" {
        return quote_spanned! { input.sig.ident.span()=>
            compile_error!("only `fn main` can be used for #[wstd_axum::http_server]");
        }
        .into();
    }

    if !input.sig.inputs.is_empty() {
        return quote_spanned! { input.sig.inputs.span()=>
            compile_error!("arguments to main are not supported");
        }
        .into();
    }
    let (async_, call) = if input.sig.asyncness.is_some() {
        (quote!(async), quote!(__make_service().await))
    } else {
        (quote!(), quote!(__make_service()))
    };
    let attrs = input.attrs;
    let output = input.sig.output;
    let block = input.block;
    quote! {
        #[::wstd::http_server]
        pub async fn main(
            __request: ::wstd::http::Request<::wstd::http::Body>
        ) -> ::wstd::http::error::Result<::wstd::http::Response<::wstd::http::Body>> {

            #(#attrs)*
            #async_ fn __make_service() #output {
                #block
            }

            let __service = #call;

            ::wstd_axum::serve(__request, __service).await
        }
    }
    .into()
}
