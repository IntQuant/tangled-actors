use heck::ToUpperCamelCase;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Error, FnArg, Ident, ItemImpl, Pat, ReceiverKind, parse_macro_input};

struct PerMessageCtx<'a> {
    /// Name of source function.
    fn_name: &'a Ident,
    /// Name of a message variant. Same as [`PerMessageCtx::fn_name`] but in CamelCase.
    variant_name: Ident,
    /// Arguments of a function/message. Does not include self receiver.
    inputs: Vec<&'a FnArg>,
    /// Same as [`PerMessageCtx::inputs`], but only includes type names.
    inputs_types: Vec<&'a Box<Pat>>,
    /// Return type of source function.
    return_type_name: proc_macro2::TokenStream,
}

impl PerMessageCtx<'_> {
    fn from_impl_item_fn(impl_item_fn: &syn::ImplItemFn) -> Result<PerMessageCtx<'_>, TokenStream> {
        let sig = &impl_item_fn.sig;
        let fn_name = &sig.ident;
        let variant_name = Ident::new(
            &sig.ident.to_string().to_upper_camel_case(),
            sig.ident.span(),
        );
        let receiver = &sig
            .inputs
            .iter()
            .find(|item| matches!(item, syn::FnArg::Receiver(..)));
        match receiver {
            Some(FnArg::Receiver(rec)) => {
                if matches!(rec.kind, ReceiverKind::Value) {
                    return Err(Error::new_spanned(
                        rec,
                        "can't receive function type by value. Use &self or &mut self instead",
                    )
                    .into_compile_error()
                    .into());
                }
            }
            Some(FnArg::Typed(rec)) => {
                return Err(Error::new_spanned(
                    rec,
                    "typed receivers are not supported. Use &self or &mut self instead",
                )
                .into_compile_error()
                .into());
            }
            None => {
                return Err(Error::new_spanned(sig, "function needs to have a receiver")
                    .into_compile_error()
                    .into());
            }
        }
        let inputs = sig
            .inputs
            .iter()
            .filter(|item| !matches!(item, syn::FnArg::Receiver(..)))
            .collect::<Vec<_>>();
        let inputs_types = inputs
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
        Ok(PerMessageCtx {
            variant_name,
            inputs,
            return_type_name,
            fn_name,
            inputs_types,
        })
    }
}

struct ActorMakerState {
    /// Identifier of generated actor messages enum.
    messages_name: Ident,
    /// Identifier of generated "actor link" type, which wraps generic `ActorLink<A>`
    /// and contains helper names that emit actor messages.
    link_helper_name: Ident,
    /// Calls that are added to the generated "actor link" type.
    helper_calls: Vec<proc_macro2::TokenStream>,
    message_variants: Vec<proc_macro2::TokenStream>,
    /// Contains match arms that call original actor function based on message variant.
    dispatches: Vec<proc_macro2::TokenStream>,
}

impl ActorMakerState {
    /// Generate message variant that will be used in per actor Messages struct.
    fn add_message_variant(&mut self, ctx: &PerMessageCtx) {
        let name = &ctx.variant_name;
        let inputs = &ctx.inputs;
        let return_type_name = &ctx.return_type_name;

        self.message_variants.push(quote! {
            #name {#(#inputs,)* __tangled_actor_return: ::tangled_actors::ReturnChannelSender<#return_type_name>}
        });
    }

    fn add_dispatch(&mut self, ctx: &PerMessageCtx, is_async: bool) {
        let call_inputs = &ctx.inputs_types;
        let variant_name = &ctx.variant_name;
        let fn_name = ctx.fn_name;
        let maybe_await = is_async.then_some(quote! {.await});
        self.dispatches.push(quote! {
            Self::Message::#variant_name {#(#call_inputs,),* __tangled_actor_return} => {
                let res = self.#fn_name(#(#call_inputs),*)#maybe_await;
                let _ = __tangled_actor_return.send(res);
            }
        });
    }

    fn add_helper_call(&mut self, ctx: &PerMessageCtx, impl_item_fn: &syn::ImplItemFn) {
        let PerMessageCtx {
            variant_name,
            inputs,
            return_type_name,
            fn_name,
            inputs_types: call_inputs,
        } = ctx;
        let messages_name = &self.messages_name;
        let visibility = &impl_item_fn.vis;
        let doc_attrs = impl_item_fn
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("doc"));
        self.helper_calls.push(
            quote! {
                #(#doc_attrs)*
                #visibility fn #fn_name(&self, #(#inputs),*) -> ::tangled_actors::RpcFut<#return_type_name> {
                    let (__actor_sender, __actor_receiver) = ::tangled_actors::oneshot_channel();
                    let msg =#messages_name::#variant_name {#(#call_inputs,)* __tangled_actor_return: __actor_sender};
                    let send_res = self.0.send(msg).map_err(|_| ::tangled_actors::ActorClosed).map(|()| __actor_receiver);
                    ::tangled_actors::RpcFut::new(send_res)
                }
            }
        )
    }
}

#[proc_macro_attribute]
pub fn make_actor(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // let messages_name = parse_macro_input!(attr as Ident);
    let orig_item_impl = parse_macro_input!(item as ItemImpl);

    let orig_name = match &*orig_item_impl.self_ty {
        syn::Type::Path(type_path) => &type_path.path.segments.last().unwrap().ident,
        _ => todo!(),
    };

    let messages_name = format_ident!("{}__TangledActorsMessages", orig_name);
    let link_helper_name = format_ident!("{}Link", orig_name);

    assert!(
        orig_item_impl.trait_.is_none(),
        "Macro doesn't support trait impl for now"
    );

    // TODO: Add support for taking in other message types from traits
    let mut state = ActorMakerState {
        messages_name,
        link_helper_name,
        message_variants: vec![],
        dispatches: vec![],
        helper_calls: vec![],
    };
    let mut any_async_handlers = false;

    for item in &orig_item_impl.items {
        match item {
            syn::ImplItem::Fn(impl_item_fn) => {
                let ctx = match PerMessageCtx::from_impl_item_fn(impl_item_fn) {
                    Ok(value) => value,
                    Err(value) => return value,
                };

                state.add_message_variant(&ctx);
                state.add_dispatch(&ctx, impl_item_fn.sig.asyncness.is_some());
                state.add_helper_call(&ctx, impl_item_fn);

                if impl_item_fn.sig.asyncness.is_some() {
                    any_async_handlers = true;
                }
            }
            _ => {}
        }
    }

    let orig_impl_type = &orig_item_impl.self_ty;

    let ActorMakerState {
        message_variants,
        dispatches,
        helper_calls,
        messages_name,
        link_helper_name,
    } = state;

    let sync_message_handler = (!any_async_handlers).then_some(quote! {
        impl ::tangled_actors::ActorSync for #orig_impl_type {
            fn process_message_sync(&mut self, message: Self::Message) {
                match message {
                    #(#dispatches),*
                }
            }
        }
    });

    TokenStream::from(quote!(
        // Messages enum
        #[expect(non_camel_case_types)]
        #[doc("Internal message enum, not supposed to be used")]
        pub enum #messages_name {
            #(#message_variants),*
        }
        // Message handler
        impl ::tangled_actors::Actor for #orig_impl_type {
            type Message = #messages_name;
            type Link = #link_helper_name;
            async fn process_message(&mut self, message: Self::Message) {
                match message {
                    #(#dispatches),*
                }
            }
        }
        #sync_message_handler
        // Helper
        #[derive(Clone)]
        pub struct #link_helper_name(::tangled_actors::ActorLink<#orig_impl_type>);

        impl From<::tangled_actors::ActorLink<#orig_impl_type>> for #link_helper_name {
            fn from(link: ::tangled_actors::ActorLink<#orig_impl_type>) -> Self {
                Self(link)
            }
        }

        impl #link_helper_name {
            #(#helper_calls)*
        }

        // Original impl
        #orig_item_impl
    ))
}
