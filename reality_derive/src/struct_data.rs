use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse2;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;
use syn::LitStr;

use crate::struct_field::StructField;

pub(crate) struct StructData {
    name: Ident,
    fields: Vec<StructField>,
    root_ident: Option<StructField>,
}

impl Parse for StructData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let derive_input = DeriveInput::parse(input)?;

        let name = derive_input.ident;

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

        let root_ident = fields.iter().find(|f| f.root).cloned();

        Ok(Self {
            name,
            fields,
            root_ident,
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
        let map = self.fields.iter().map(|f| f.config_assign_property_expr());
        let fields = quote! {
            #( #map ),*
        };

        if let Some(root_ident) = self.root_ident.as_ref() {
            let interpolate_lit = LitStr::new(
                &format!("{}.{}.(ext).(prop)", root_ident.name, name),
                Span::call_site(),
            );

            let root_name = &root_ident.name;

            // TODO: fix .clone()
            quote! {
                impl reality::v2::Config for #name {
                    fn config(&mut self, ident: &reality::Identifier, property: &reality::v2::Property) -> Result<(), reality::Error> {
                        let property = if let Some(map) = ident.interpolate(#interpolate_lit) {
                            let ext = &map["ext"];

                            self.#root_name.apply(ext, property)?
                        } else {
                            property.clone()
                        };

                        let property = &property;

                        match ident.subject().as_str() {
                            #fields
                            _ => {}
                        }

                        Ok(())
                    }
                }
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
            }
        }
    }
}