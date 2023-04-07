use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::ToTokens;
use quote::format_ident;
use quote::quote_spanned;
use syn::ext::IdentExt;
use syn::parse::Parse;
use syn::parse2;
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
    pub(crate) span: Span,
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
    /// Sets the first doc comment from in the struct
    /// 
    pub(crate) doc: Option<LitStr>,
}

impl StructField {
    pub(crate) fn join_tuple_storage_type_expr(&self) -> TokenStream {
        let ty = &self.ty;
        if self.mutable && !self.option {
            quote_spanned! {self.span=>
                &'a mut specs::WriteStorage<'a, #ty>
            }
        } else if self.mutable && self.option {
            quote_spanned! {self.span=>
                specs::join::MaybeJoin<&'a mut specs::WriteStorage<'a, #ty>>
            }
        } else if !self.mutable && self.option {
            quote_spanned! {self.span=>
                specs::join::MaybeJoin<&'a specs::ReadStorage<'a, #ty>>
            }
        } else {
            quote_spanned! {self.span=>
                &'a specs::ReadStorage<'a, #ty>
            }
        }
    }

    pub(crate) fn system_data_expr(&self) -> TokenStream {
        let name = &self.name;
        let name = format_ident!("{}_storage", name);
        let ty = &self.ty;
        if self.mutable {
            quote_spanned! {self.span=>
                #name: specs::WriteStorage<'a, #ty>
            }
        } else {
            quote_spanned! {self.span=>
                #name: specs::ReadStorage<'a, #ty>
            }
        }
    }

    pub(crate) fn system_data_ref_expr(&self) -> TokenStream {
        let name = &self.name;
        let name = format_ident!("{}_storage", name);
        if self.mutable {
            quote_spanned! {self.span=>
                &mut self.#name
            }
        } else if self.option {
            quote_spanned! {self.span=>
                self.#name.maybe()
            }
        } else {
            quote_spanned! {self.span=>
                &self.#name
            }
        }
    }

    pub(crate) fn config_assign_property_expr(&self) -> TokenStream {
        let name = &self.name;
        let name_lit = self.name_str_literal();

        if let Some(config_attr) = self.config.as_ref() {
            quote_spanned! {self.span=>
                #name_lit => {
                    self.#name = #config_attr(&self, ident, property)?;
                }
            }
        } else {
            quote_spanned! {self.span=>
                #name_lit => {
                    if let Some(properties) = property.as_properties().and_then(|props| props[#name_lit].as_properties()) {
                        for (name, prop) in properties.iter_properties() {
                            let ident = properties.owner().branch(name)?;
                            reality::v2::Config::config(&mut self.#name, &ident, prop)?;
                        }
                    } else {
                        reality::v2::Config::config(&mut self.#name, ident, property)?;

                        if let Some(properties) = property.as_properties() {
                            for (name, prop) in properties.iter_properties().filter(|(name, _)| name.as_str() != #name_lit) {
                                let ident = properties.owner().branch(name)?;
                                self.config(&ident, prop)?;
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn apply_expr(&self) -> TokenStream {
        let name = &self.name;
        let name_lit = self.name_str_literal();

        quote_spanned! {self.span=>
            #name_lit => {
                return self.#name.apply(#name_lit, property);
            }
        }
    }

    pub(crate) fn config_root_expr(&self, ty: &Ident) -> TokenStream {
        assert!(self.root);

        let name = &self.name;

        let property_name = self.root_property_name_ident();
        
        let interpolate_lit = LitStr::new(
            &format!("#root#.{}.{}.(ext).(prop)", name, ty),
            Span::call_site(),
        );

        // let interpolate_format_lit = LitStr::new(
        //     &format!("{}.{}.{{ext}}.{{prop}}", name, ty),
        //     Span::call_site(),
        // );
        
        quote_spanned! {self.span=>
            let #property_name = if let Some(map) = ident.interpolate(#interpolate_lit) {
                let ext = &map["ext"];
                let ext_ident = ident.branch(ext)?;
                self.#name.config(&ext_ident, property)?;
                self.#name.apply(ext, property)?
            } else {
                reality::v2::Property::Empty
            };
        }
    }

    pub(crate) fn runmd_root_expr(&self) -> TokenStream {
        let runmd = if let Some(runmd_doc) = self.doc.as_ref() {
            let lit_str = format!("+ {} .symbol # {}", self.ty, runmd_doc.value());
            LitStr::new(&lit_str, Span::call_site())
        } else {
            let lit_str = format!("+ {} .symbol", self.ty);
            LitStr::new(&lit_str, Span::call_site())
        };

        quote_spanned! {self.span=>
            .parse_line(#runmd)?
        }
    }

    pub(crate) fn root_property_name_ident(&self) -> Ident {
        assert!(self.root);

        let root_ident = &self.ty;
        format_ident!("property_{}", root_ident)
    }

    pub(crate) fn name_str_literal(&self) -> LitStr {
        LitStr::new(&self.name.to_string(), Span::call_site())
    }

    pub(crate) fn root_ext_input_pattern_lit_str(&self, ext: &Ident) -> LitStr {
        let format = format!("{}.{}.(?input)", &self.name.to_string().to_lowercase(), ext.to_string().to_lowercase());
        LitStr::new(&format, Span::call_site())
    }
}

impl Parse for StructField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse attributes
        let attributes = Attribute::parse_outer(input)?;
        let mut config_attr = None::<Ident>;
        let mut doc = None::<LitStr>;
        let mut root = false;
        let span = input.span();

        for attribute in attributes {
            if attribute.path().is_ident("config") {
                let ident: Ident = attribute.parse_args()?;
                config_attr = Some(ident);
            }

            if attribute.path().is_ident("root") {
                root = true;
            }

            if attribute.path().is_ident("doc") {
                if doc.is_none() {
                    // doc = Some(attribute.parse_args()?);
                    let name_value = attribute.meta.require_name_value()?;
                    if name_value.path.is_ident("doc") {
                        let lit_str = parse2::<LitStr>(name_value.value.to_token_stream())?;
                        doc = Some(lit_str);
                    }
                }
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
                span,
                name,
                ty,
                reference: true,
                mutable,
                option: false,
                ignore: false,
                root,
                config: config_attr,
                doc
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
                    span,
                    name,
                    ty,
                    reference: false,
                    mutable,
                    option: true,
                    ignore: false,
                    root,
                    config: config_attr,
                    doc
                })
            } else {
                let ty = ident;
                input.parse::<Generics>()?;

                Ok(Self {
                    span,
                    name,
                    ty,
                    reference: false,
                    mutable: false,
                    option: false,
                    ignore: false,
                    root,
                    config: config_attr,
                    doc
                })
            }
        } else {
            let ty = name.clone();
            input.parse::<Type>()?;
            Ok(Self {
                span,
                name,
                ty,
                reference: false,
                mutable: false,
                option: false,
                ignore: true,
                root,
                config: config_attr,
                doc
            })
        }
    }
}