use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, parse_quote, Data, DeriveInput, Fields};

#[proc_macro_derive(ConfigEmit)]
pub fn derive_config_emit(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let mut generics = input.generics;

    for type_param in generics.type_params_mut() {
        type_param
            .bounds
            .push(parse_quote!(::rinasys_config_schema::ConfigEmit));
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => panic!("ConfigEmit only supports structs with named fields"),
        },
        _ => panic!("ConfigEmit only supports structs"),
    };

    let field_idents = fields
        .iter()
        .map(|field| field.ident.as_ref().unwrap())
        .collect::<Vec<_>>();
    let token_vars = field_idents
        .iter()
        .map(|field| format_ident!("{field}_tokens"))
        .collect::<Vec<_>>();
    let field_names = field_idents
        .iter()
        .map(|field| field.to_string())
        .collect::<Vec<_>>();

    quote! {
        #[cfg(feature = "codegen")]
        impl #impl_generics ::rinasys_config_schema::ConfigEmit for #ident #ty_generics #where_clause {
            fn emit_config(
                &self,
                context: &::rinasys_config_schema::EmitContext,
            ) -> ::proc_macro2::TokenStream {
                #(
                    let #token_vars = ::rinasys_config_schema::ConfigEmit::emit_config(
                        &self.#field_idents,
                        context,
                    );
                )*

                ::quote::quote! {
                    ::rinasys_config_schema::#ident {
                        #(
                            #field_idents: ##token_vars,
                        )*
                    }
                }
            }

            fn emit_cfgs(
                &self,
                context: &::rinasys_config_schema::EmitContext,
                path: &mut ::std::vec::Vec<&'static str>,
            ) {
                #(
                    path.push(#field_names);
                    ::rinasys_config_schema::ConfigEmit::emit_cfgs(
                        &self.#field_idents,
                        context,
                        path,
                    );
                    path.pop();
                )*
            }

            fn emit_rerun_if_changed(
                &self,
                context: &::rinasys_config_schema::EmitContext,
            ) {
                #(
                    ::rinasys_config_schema::ConfigEmit::emit_rerun_if_changed(
                        &self.#field_idents,
                        context,
                    );
                )*
            }
        }
    }
    .into()
}
