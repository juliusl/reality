use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse2;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;
use syn::LitStr;
use syn::Path;

use crate::struct_field::StructField;

/// Parses a struct from derive attribute,
///
/// Generates impl's for Load, Config, and Apply traits
///
pub(crate) struct StructData {
    /// Name of the struct,
    ///
    name: Ident,
    /// Parsed struct fields,
    ///
    fields: Vec<StructField>,
    /// Types to add to dispatch trait impl,
    ///
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

        let root_config_pattern_match = self
            .fields
            .iter()
            .filter(|f| f.root)
            .map(|f| {
                let name = f.root_name();
                let lit = LitStr::new(&name.to_string(), f.span);
                quote_spanned! {lit.span()=>
                    #lit => {
                        let ident = format!("{:#}", ident).replace(#lit, "").trim_matches('.').parse::<reality::Identifier>()?;
                        reality::v2::Config::config(&mut self.#name, &ident, property)?;
                        return Ok(());
                    }
                }
            });

        let root_fields = self
            .fields
            .iter()
            .filter(|f| f.root)
            .map(|f| (f.root_name()));

        let ext_fields = self
            .fields
            .iter()
            .filter(|f| !f.root)
            .map(|f| f.config_apply_root_expr(root_fields.clone().collect()));

        let compile_trait = if !self.compile.is_empty() {
            self.compile_trait()
        } else {
            quote! {}
        };

        quote! {
            #[allow(non_snake_case)]
            impl reality::v2::Config for #name {
                fn config(&mut self, ident: &reality::Identifier, property: &reality::v2::Property) -> Result<(), reality::Error> {
                    match ident.root().as_str() {
                        #( #root_config_pattern_match ),*
                        _ => {
                        }
                    }

                    match ident.subject().as_str() {
                        #( #ext_fields ),*
                        _ => {
                        }
                    }

                    Ok(())
                }
            }

            #compile_trait
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
            } else if let Some(ident) = c.get_ident().filter(|i| i.to_string().starts_with("Thunk"))
            {
                let name = ident.to_string().replace("Thunk", "");
                let name = format_ident!("thunk_{}", name.to_lowercase());
                Some(quote! {
                    .map(|b|{
                        Ok(#name(b.clone()))
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
            impl reality::v2::Dispatch for #name {
                fn dispatch<'a>(
                    &self,
                    dispatch_ref: reality::v2::DispatchRef<'a, reality::v2::Properties>,
                ) -> Result<reality::v2::DispatchRef<'a, reality::v2::Properties>, Error> {
                    let clone = self.clone();
                    dispatch_ref
                        .transmute::<ActionBuffer>()
                        .map_into(move |b| {
                            let mut clone = clone;
                            b.config(&mut clone)?;
                            Ok(clone)
                        })
                        .result()?
                        #additional_compile
                        .transmute::<Properties>()
                        .result()
                }
            }
        }
    }

    pub fn runmd_trait(&self) -> TokenStream {
        let name = &self.name;

        // Mapping compile
        let compile_map = self.root_fields().map(|f| {
            let pattern = f.root_ext_input_pattern_lit_str(name);
            quote_spanned! {f.span=>
                if let Some(log) = compiler.last_build_log() {
                    for (_, _, entity) in log.search_index(#pattern) {
                        let dispatch_ref = DispatchRef::<Properties>::new(*entity, compiler);

                        reality::v2::Dispatch::dispatch(self, dispatch_ref)?;
                    }
                }
            }
        });
        let compile_map = quote! {
            #( #compile_map )*
        };

        quote! {
            impl reality::v2::Runmd for #name {
                fn runmd(&self, compiler: &mut reality::v2::Compiler) -> Result<(), reality::Error> {
                    #compile_map

                    Ok(())
                }
            }
        }
    }

    fn root_fields(&self) -> impl Iterator<Item = &StructField> {
        self.fields.iter().filter(|f| f.root)
    }

    fn reference_fields(&self) -> impl Iterator<Item = &StructField> {
        self.fields.iter().filter(|f| f.reference)
    }
}
