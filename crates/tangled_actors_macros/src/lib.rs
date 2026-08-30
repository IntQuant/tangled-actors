use heck::ToUpperCamelCase;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ItemImpl, parse_macro_input};

#[proc_macro_attribute]
pub fn make_actor(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // let messages_name = parse_macro_input!(attr as Ident);
    let orig_item_impl = parse_macro_input!(item as ItemImpl);

    let orig_name = match &*orig_item_impl.self_ty {
        syn::Type::Path(type_path) => &type_path.path.segments.last().unwrap().ident,
        _ => todo!(),
    };

    let messages_name = format_ident!("{}__TangledActorsMessages", orig_name);
    let helper_name = format_ident!("{}Link", orig_name);

    assert!(
        orig_item_impl.trait_.is_none(),
        "Macro doesn't support trait impl for now"
    );

    // Add support for taking in other message types from traits
    let mut messages = vec![];
    let mut calls = vec![];
    let mut helper_calls = vec![];

    for item in &orig_item_impl.items {
        match item {
            syn::ImplItem::Fn(impl_item_fn) => {
                let sig = &impl_item_fn.sig;
                let fn_name = &sig.ident;
                let name = Ident::new(
                    &sig.ident.to_string().to_upper_camel_case(),
                    sig.ident.span(),
                );
                let receiver = &sig
                    .inputs
                    .iter()
                    .find(|item| matches!(item, syn::FnArg::Receiver(..)));
                match receiver {
                    Some(_) => {
                        // TODO: only allow &self and &mut self in receivers
                    }
                    None => panic!("function needs to have a receiver"),
                }

                let inputs = &sig
                    .inputs
                    .iter()
                    .filter(|item| !matches!(item, syn::FnArg::Receiver(..)))
                    .collect::<Vec<_>>();

                let call_inputs = inputs
                    .iter()
                    .map(|arg| match arg {
                        syn::FnArg::Receiver(_receiver) => unreachable!(),
                        syn::FnArg::Typed(pat_type) => &pat_type.pat,
                    })
                    .collect::<Vec<_>>();

                let return_type_name = match &sig.output {
                    syn::ReturnType::Default => quote! {()},
                    syn::ReturnType::Type(_, return_type_name) => quote! {#return_type_name},
                };

                let comma = if inputs.is_empty() {
                    quote! {}
                } else {
                    quote! { , }
                };

                messages.push(quote! {
                    #name {#(#inputs),* #comma __tangled_actor_return: ::tangled_actors::ReturnChannelSender<#return_type_name>}
                });
                let maybe_await = sig.asyncness.map(|_| quote! {.await});
                calls.push(quote! {
                    Self::Message::#name {#(#call_inputs),* #comma __tangled_actor_return} => {
                        let res = self.#fn_name(#(#call_inputs),*)#maybe_await;
                        let _ = __tangled_actor_return.send(res);
                    }
                });
                let visibility = &impl_item_fn.vis;
                let doc_attrs = impl_item_fn
                    .attrs
                    .iter()
                    .filter(|attr| attr.path().is_ident("doc"));
                helper_calls.push(
                    quote! {
                        #(#doc_attrs)*
                        #visibility fn #fn_name(&self, #(#inputs),*) -> impl Future<Output=Result<#return_type_name, ::tangled_actors::ActorClosed>> {
                            let (__actor_sender, __actor_receiver) = ::tangled_actors::oneshot_channel();
                            let msg =#messages_name::#name {#(#call_inputs),* #comma __tangled_actor_return: __actor_sender};
                            let send_res = self.0.send(msg);
                            async {send_res?; __actor_receiver.await.map_err(|_| ::tangled_actors::ActorClosed)}
                        }
                    }
                )
            }
            _ => {}
        }
    }

    let orig_impl_type = &orig_item_impl.self_ty;

    TokenStream::from(quote!(
        // Messages enum
        #[expect(non_camel_case_types)]
        #[doc("Internal message enum, not supposed to be used")]
        pub enum #messages_name {
            #(#messages),*
        }
        // Message handler
        impl ::tangled_actors::Actor for #orig_impl_type {
            type Message = #messages_name;
            type Link = #helper_name;
            async fn process_message(&mut self, message: Self::Message) {
                match message {
                    #(#calls),*
                }
            }
        }
        // Helper
        #[derive(Clone)]
        pub struct #helper_name(::tangled_actors::ActorLink<#orig_impl_type>);

        impl From<::tangled_actors::ActorLink<#orig_impl_type>> for #helper_name {
            fn from(link: ::tangled_actors::ActorLink<#orig_impl_type>) -> Self {
                Self(link)
            }
        }

        impl #helper_name {
            #(#helper_calls)*
        }

        // Original impl
        #orig_item_impl
    ))
}
