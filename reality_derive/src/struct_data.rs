use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse2;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;
use syn::Path;

use crate::struct_field::StructField;

/// Parses a struct from derive attribute,
///
/// Generates impl's for Load, Config, and Apply traits
///
pub(crate) struct StructData {
    name: Ident,
    fields: Vec<StructField>,
    compile: Vec<Path>,
}

impl Parse for StructData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let derive_input = DeriveInput::parse(input)?;

        let name = derive_input.ident;

        let mut compile: Vec<Path> = vec![];

        for attr in derive_input.attrs.iter() {
            if attr.path().is_ident("compile") {
                if let Some(args) = attr
                    .parse_args_with(
                        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                    )
                    .ok()
                {
                    compile = args.iter().cloned().collect();
                }

                if compile.is_empty() {
                    compile.push(attr.path().clone());
                }
            }
        }

        let fields = if let Data::Struct(data) = &derive_input.data {
            let named = parse2::<FieldsNamed>(data.fields.to_token_stream())?;
            named
                .named
                .iter()
                .filter_map(|n| parse2::<StructField>(n.to_token_stream()).ok())
                .filter(|f| !f.ignore)
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        Ok(Self {
            name,
            fields,
            compile,
        })
    }
}

impl StructData {
    /// Returns token stream of impl for the Apply trait
    ///
    pub(crate) fn apply_trait(&self) -> TokenStream {
        let name = &self.name;

        let map = self.fields.iter().map(|f| f.apply_expr());
        let fields = quote! {
            #( #map ),*
        };

        quote! {
            impl reality::v2::Apply for #name {
                fn apply(&self, rule_name: impl AsRef<str>, property: &reality::v2::Property) -> Result<reality::v2::Property, reality::Error> {
                    let rule_name = rule_name.as_ref();
                    match rule_name {
                        #fields
                        _ => {
                            Ok(property.clone())
                        }
                    }
                }
            }
        }.into()
    }

    /// Returns token stream of impl for the Config trait,
    ///
    pub(crate) fn config_trait(&self) -> TokenStream {
        let name = &self.name;
        let map = self
            .fields
            .iter()
            .filter(|f| !f.root)
            .map(|f| f.config_assign_property_expr());
        let fields = quote! {
            #( #map ),*
        };

        let root_fields = self
            .fields
            .iter()
            .filter(|f| f.root)
            .map(|f| (f.root_property_name_ident(), f.config_root_expr(name)))
            .collect::<Vec<_>>();

        let root_apply = root_fields.iter().map(|(_, a)| a);
        let root_apply = quote! {
            #( #root_apply )*
        };

        let root_select = root_fields.iter().map(|(i, _)| {
            quote! {
                if #i != reality::v2::Property::Empty {
                    break &#i;
                }
            }
        });
        let root_select = quote! {
            #( #root_select )*
        };

        let compile_trait = if !self.compile.is_empty() {
            self.compile_trait()
        } else {
            quote! {}
        };

        if !root_fields.is_empty() {
            quote! {
                #[allow(non_snake_case)]
                impl reality::v2::Config for #name {
                    fn config(&mut self, ident: &reality::Identifier, property: &reality::v2::Property) -> Result<(), reality::Error> {
                        #root_apply

                        let property = loop {
                            #root_select

                            break property;
                        };

                        match ident.subject().as_str() {
                            #fields
                            _ => {}
                        }

                        Ok(())
                    }
                }

                #compile_trait
            }
        } else {
            quote! {
                impl reality::v2::Config for #name {
                    fn config(&mut self, ident: &reality::Identifier, property: &reality::v2::Property) -> Result<(), reality::Error> {
                        match ident.subject().as_str() {
                            #fields
                            _ => {}
                        }

                        Ok(())
                    }
                }

                #compile_trait
            }
        }
    }

    /// Returns token stream of generated Load trait,
    ///
    pub(crate) fn load_trait(&self) -> TokenStream {
        let ident = &self.name;
        let format_ident = format_ident!("{}Format", ident);
        let systemdata_ident = format_ident!("{}SystemData", ident);

        let types = self
            .reference_fields()
            .map(|f| f.join_tuple_storage_type_expr());
        let types = quote! {
            #( #types ),*
        };

        let idents = self.reference_fields().map(|f| &f.name);
        let idents = quote! {
            #( #idents ),*
        };

        let systemdata_body = self.reference_fields().map(|f| f.system_data_expr());
        let systemdata_body = quote! {
            #( #systemdata_body ),*
        };

        let systemdata_provider_body = self.reference_fields().map(|f| f.system_data_ref_expr());
        let systemdata_provider_body = quote! {
            #( #systemdata_provider_body ),*
        };

        quote! {
            use specs::prelude::*;

            pub type #format_ident<'a> = ( #types );

            #[derive(specs::SystemData)]
            pub struct #systemdata_ident<'a> {
                entities: specs::Entities<'a>,
                #systemdata_body
            }

            impl<'a> reality::state::Load for #ident<'a> {
                type Layout = #format_ident<'a>;

                fn load((#idents): <Self::Layout as specs::Join>::Type) -> Self {
                    Self { #idents }
                }
            }

            impl<'a> reality::state::Provider<'a, #format_ident<'a>> for #systemdata_ident<'a> {
                fn provide(&'a self) -> #format_ident<'a> {
                    (
                        #systemdata_provider_body
                    )
                }
            }

            impl<'a> AsRef<specs::Entities<'a>> for #systemdata_ident<'a> {
                fn as_ref(&self) -> &specs::Entities<'a> {
                    &self.entities
                }
            }
        }
    }

    /// Returns an impl for the compile trait,
    ///
    pub fn compile_trait(&self) -> TokenStream {
        let name = &self.name;

        let map = self.compile.iter().filter_map(|c| {
            if c.is_ident("ThunkCall") || c.is_ident("Call") {
                Some(quote! {
                    .map(|b| {
                        Ok(reality::v2::thunk_call(b.clone()))
                    })
                })
            } else if c.is_ident("ThunkCompile") || c.is_ident("Compile") {
                Some(quote! {
                    .map(|b| {
                        Ok(reality::v2::thunk_compile(b.clone()))
                    })
                })
            } else if c.is_ident("ThunkBuild") || c.is_ident("Build") {
                Some(quote! {
                    .map(|b| {
                        Ok(reality::v2::thunk_build(b.clone()))
                    })
                })
            } else if c.is_ident("ThunkUpdate") || c.is_ident("Update") {
                Some(quote! {
                    .map(|b| {
                        Ok(reality::v2::thunk_update(b.clone()))
                    })
                })
            } else if c.is_ident("ThunkListen") || c.is_ident("Listen") {
                Some(quote! {
                    .map(|b| {
                        Ok(reality::v2::thunk_listen(b.clone()))
                    })
                })
            } else {
                None
            }
        });
        let additional_compile = quote! {
            #( #map )*
        };

        quote! {
            impl reality::v2::Compile for #name {
                fn compile<'a>(
                    &self,
                    build_ref: reality::v2::BuildRef<'a, reality::v2::Properties>,
                ) -> Result<reality::v2::BuildRef<'a, reality::v2::Properties>, Error> {
                    build_ref
                        .transmute::<ActionBuffer>()
                        .map_into(|b| {
                            let mut default = Self::default();

                            b.config(&mut default)?;

                            Ok(default)
                        })
                        #additional_compile
                        .transmute::<Properties>()
                        .result()
                }
            }
        }
    }

    fn reference_fields(&self) -> impl Iterator<Item = &StructField> {
        self.fields.iter().filter(|f| f.reference)
    }
}
