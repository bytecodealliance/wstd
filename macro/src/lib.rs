use proc_macro::TokenStream;
use quote::{quote, quote_spanned};
use syn::{ItemFn, parse_macro_input, spanned::Spanned};

#[proc_macro_attribute]
pub fn attr_macro_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    if input.sig.asyncness.is_none() {
        return quote_spanned! { input.sig.fn_token.span()=>
            compile_error!("fn must be `async fn`");
        }
        .into();
    }

    if input.sig.ident != "main" {
        return quote_spanned! { input.sig.ident.span()=>
            compile_error!("only `async fn main` can be used for #[wstd::main]");
        }
        .into();
    }

    if !input.sig.inputs.is_empty() {
        return quote_spanned! { input.sig.inputs.span()=>
            compile_error!("arguments to main are not supported");
        }
        .into();
    }
    let attrs = input.attrs;
    let output = input.sig.output;
    let block = input.block;
    quote! {
        pub fn main() #output {

            #(#attrs)*
            async fn __run() #output {
                #block
            }

            ::wstd::runtime::block_on(async {
                __run().await
            })
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn attr_macro_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    if input.sig.asyncness.is_none() {
        return quote_spanned! { input.sig.fn_token.span()=>
            compile_error!("fn must be `async fn`");
        }
        .into();
    }

    let name = input.sig.ident;

    if !input.sig.inputs.is_empty() {
        return quote_spanned! { input.sig.inputs.span()=>
            compile_error!("arguments to main are not supported");
        }
        .into();
    }
    let attrs = input.attrs;
    let output = input.sig.output;
    let block = input.block;
    quote! {
        #(#attrs)*
        #[::core::prelude::v1::test]
        pub fn #name() #output {

            async fn __run() #output {
                #block
            }

            ::wstd::runtime::block_on(async {
                __run().await
            })
        }
    }
    .into()
}

/// Enables a HTTP server main function, for creating [HTTP servers].
///
/// [HTTP servers]: https://docs.rs/wstd/latest/wstd/http/server/index.html
///
/// # Examples
///
/// ```ignore
/// #[wstd::http_server]
/// async fn main(request: Request<Body>) -> Result<Response<Body>> {
///     Ok(Response::new("Hello!\n".into()))
/// }
/// ```
#[proc_macro_attribute]
pub fn attr_macro_http_server(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let (run_async, run_await) = if input.sig.asyncness.is_some() {
        (quote!(async), quote!(.await))
    } else {
        (quote!(), quote!())
    };

    let output = &input.sig.output;
    let inputs = &input.sig.inputs;
    let name = &input.sig.ident;
    let body = &input.block;
    let attrs = &input.attrs;
    let vis = &input.vis;

    if name != "main" {
        return quote_spanned! { input.sig.ident.span()=>
            compile_error!("only `async fn main` can be used for #[wstd::http_server]");
        }
        .into();
    }

    quote! {
        struct TheServer;

        impl ::wstd::__internal::wasip2::exports::http::incoming_handler::Guest for TheServer {
            fn handle(
                request: ::wstd::__internal::wasip2::http::types::IncomingRequest,
                response_out: ::wstd::__internal::wasip2::http::types::ResponseOutparam
            ) {
                #(#attrs)*
                #vis #run_async fn __run(#inputs) #output {
                    #body
                }

                let responder = ::wstd::http::server::Responder::new(response_out);
                ::wstd::runtime::block_on(async move {
                    match ::wstd::http::request::try_from_incoming(request) {
                        Ok(request) => match __run(request) #run_await {
                            Ok(response) => { responder.respond(response).await.unwrap() },
                            Err(err) => responder.fail(err),
                        }
                        Err(err) => responder.fail(err),
                    }
                })
            }
        }

        ::wstd::__internal::wasip2::http::proxy::export!(TheServer with_types_in ::wstd::__internal::wasip2);

        // Provide an actual function named `main`.
        //
        // WASI HTTP server components don't use a traditional `main` function.
        // They export a function named `handle` which takes a `Request`
        // argument, and which may be called multiple times on the same
        // instance. To let users write a familiar `fn main` in a file
        // named src/main.rs, we provide this `wstd::http_server` macro, which
        // transforms the user's `fn main` into the appropriate `handle`
        // function.
        //
        // However, when the top-level file is named src/main.rs, rustc
        // requires there to be a function named `main` somewhere in it. This
        // requirement can be disabled using `#![no_main]`, however we can't
        // use that automatically because macros can't contain inner
        // attributes, and we don't want to require users to add `#![no_main]`
        // in their own code.
        //
        // So, we include a definition of a function named `main` here, which
        // isn't intended to ever be called, and exists just to satify the
        // requirement for a `main` function.
        //
        // Users could use `#![no_main]` if they want to. Or, they could name
        // their top-level file src/lib.rs and add
        // ```toml
        // [lib]
        // crate-type = ["cdylib"]
        // ```
        // to their Cargo.toml. With either of these, this "main" function will
        // be ignored as dead code.
        fn main() {
            unreachable!("HTTP server components should be run with `handle` rather than `run`")
        }
    }
    .into()
}
