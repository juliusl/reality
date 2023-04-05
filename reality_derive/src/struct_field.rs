use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use syn::ext::IdentExt;
use syn::parse::Parse;
use syn::token::Mut;
use syn::Attribute;
use syn::Generics;
use syn::Lifetime;
use syn::LitStr;
use syn::Token;
use syn::Type;
use syn::Visibility;

/// Parses a struct field such as,
///
/// #visibility #ident: #reference #lifetime #mutability #type,
///
/// Also attributes such as,
///
/// - ignore
/// - config(handler)
///
#[derive(Clone)]
pub(crate) struct StructField {
    /// Name of the field,
    ///
    pub(crate) name: Ident,
    /// Name of the type,
    ///
    pub(crate) ty: Ident,
    /// Ident of the config attribute,
    ///
    pub(crate) config: Option<Ident>,
    /// True if reference type
    ///
    pub(crate) reference: bool,
    /// True if mutable
    ///
    pub(crate) mutable: bool,
    /// True if Option<T> type,
    ///
    pub(crate) option: bool,
    /// True if this field should be ignored,
    ///
    pub(crate) ignore: bool,
    /// True if this field has a #[root] attribute,
    ///
    pub(crate) root: bool,
    /// True if this field has a #[apply] attribute,
    ///
    pub(crate) apply: bool,
}

impl StructField {
    pub(crate) fn join_tuple_storage_type_expr(&self) -> TokenStream {
        let ty = &self.ty;
        if self.mutable && !self.option {
            quote! {
                &'a mut specs::WriteStorage<'a, #ty>
            }
        } else if self.mutable && self.option {
            quote! {
                specs::join::MaybeJoin<&'a mut specs::WriteStorage<'a, #ty>>
            }
        } else if !self.mutable && self.option {
            quote! {
                specs::join::MaybeJoin<&'a specs::ReadStorage<'a, #ty>>
            }
        } else {
            quote! {
                &'a specs::ReadStorage<'a, #ty>
            }
        }
    }

    pub(crate) fn system_data_expr(&self) -> TokenStream {
        let name = &self.name;
        let name = format_ident!("{}_storage", name);
        let ty = &self.ty;
        if self.mutable {
            quote! {
                #name: specs::WriteStorage<'a, #ty>
            }
        } else {
            quote! {
                #name: specs::ReadStorage<'a, #ty>
            }
        }
    }

    pub(crate) fn system_data_ref_expr(&self) -> TokenStream {
        let name = &self.name;
        let name = format_ident!("{}_storage", name);
        if self.mutable {
            quote! {
                &mut self.#name
            }
        } else if self.option {
            quote! {
                self.#name.maybe()
            }
        } else {
            quote! {
                &self.#name
            }
        }
    }

    pub(crate) fn config_assign_property_expr(&self) -> TokenStream {
        let name = &self.name;
        let name_lit = self.name_str_literal();

        if let Some(config_attr) = self.config.as_ref() {
            quote! {
                #name_lit => {
                    self.#name = #config_attr(&self, ident, property)?;
                }
            }
        } else if self.apply {
            let rule_name = LitStr::new("", Span::call_site());
            let apply_expr = self.config_apply_expr(&rule_name);
            quote! {
                #name_lit => {
                    #apply_expr

                    reality::v2::Config::config(&mut self.#name, ident, &property)?;
                }
            }
        } else {
            quote! {
                #name_lit => {
                    reality::v2::Config::config(&mut self.#name, ident, property)?;
                }
            }
        }
    }

    pub(crate) fn config_apply_expr(&self, rule_name: &LitStr) -> TokenStream {
        let name = &self.name;
        assert!(self.apply);

        quote! {
            let property = self.#name.apply(#rule_name, property)?;
        }
    }

    pub(crate) fn apply_expr(&self) -> TokenStream {
        let name = &self.name;
        let name_lit = self.name_str_literal();

        quote! {
            #name_lit => {
                return self.#name.apply("", property);
            }
        }
    }

    pub(crate) fn name_str_literal(&self) -> LitStr {
        LitStr::new(&self.name.to_string(), Span::call_site())
    }
}

impl Parse for StructField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse attributes
        let attributes = Attribute::parse_outer(input)?;
        let mut config_attr = None::<Ident>;
        let mut root = false;
        let mut apply = false;

        for attribute in attributes {
            if attribute.path().is_ident("config") {
                let ident: Ident = attribute.parse_args()?;
                config_attr = Some(ident);
            }

            if attribute.path().is_ident("root") {
                root = true;
            }

            if attribute.path().is_ident("apply") {
                apply = true;
            }
        }

        // Parse any visibility modifiers
        Visibility::parse(input)?;

        // Name of this struct field
        let name = input.parse::<Ident>()?;
        input.parse::<Token![:]>()?;

        // Type is a reference type
        if input.peek(Token![&]) {
            input.parse::<Token![&]>()?;
            input.parse::<Lifetime>()?;

            let mutable = input.peek(Mut);
            if mutable {
                input.parse::<Mut>()?;
            }

            let ty = input.parse::<Ident>()?;
            Ok(Self {
                name,
                ty,
                reference: true,
                mutable,
                option: false,
                ignore: false,
                root,
                apply,
                config: config_attr,
            })
        } else if input.peek(Ident::peek_any) {
            let ident = input.parse::<Ident>()?;
            if ident.to_string() == "Option" {
                input.parse::<Token![<]>()?;
                input.parse::<Token![&]>()?;
                input.parse::<Lifetime>()?;

                let mutable = input.peek(Mut);
                if mutable {
                    input.parse::<Mut>()?;
                }

                let ty = input.parse::<Ident>()?;
                input.parse::<Token![>]>()?;
                Ok(Self {
                    name,
                    ty,
                    reference: false,
                    mutable,
                    option: true,
                    ignore: false,
                    root,
                    apply,
                    config: config_attr,
                })
            } else {
                let ty = ident;
                input.parse::<Generics>()?;

                Ok(Self {
                    name,
                    ty,
                    reference: false,
                    mutable: false,
                    option: false,
                    ignore: false,
                    root,
                    apply,
                    config: config_attr,
                })
            }
        } else {
            let ty = name.clone();
            input.parse::<Type>()?;
            Ok(Self {
                name,
                ty,
                reference: false,
                mutable: false,
                option: false,
                ignore: true,
                root,
                apply,
                config: config_attr,
            })
        }
    }
}