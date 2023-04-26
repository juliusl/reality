use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote_spanned;
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::parse::Parse;
use syn::parse2;
use syn::token::Mut;
use syn::Attribute;
use syn::Generics;
use syn::Lifetime;
use syn::LitStr;
use syn::Path;
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
#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct StructField {
    pub(crate) span: Span,
    /// Name of the field,
    ///
    pub(crate) name: Path,
    /// Name of the type,
    ///
    pub(crate) ty: Path,
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
    /// True if this field has a #[block] attribute,
    ///
    pub(crate) block: bool,
    /// True if this field has a #[root] attribute,
    ///
    pub(crate) root: bool,
    /// True if this field has a #[ext] attribute,
    ///
    pub(crate) ext: bool,
    /// Sets the first doc comment from in the struct
    ///
    pub(crate) doc: Option<LitStr>,
}

impl StructField {
    /// Returns a match expression for visitor trait,
    ///
    pub(crate) fn visitor_expr(&self) -> TokenStream {
        let name_lit = self.name_str_literal();
        let name = &self.name;
        if let Some(ident) = name.get_ident() {
            // The type of visitor expression that will be generated,
            match self {
                Self { block: true, .. } => {
                    quote_spanned! {ident.span()=>
                        self.#name.visit_block(block);
                    }
                }
                Self { root: true, .. } => {
                    quote_spanned! {ident.span()=>
                        self.#name.visit_root(root);
                    }
                }
                Self { ext: true, .. } => {
                    quote_spanned! {ident.span()=>
                        self.#name.visit_extension(ident);
                    }
                }
                _ => {
                    quote_spanned! {ident.span()=>
                        #name_lit => {
                            self.#name.visit_property(name, property);
                        }
                    }
                }
            }
        } else {
            quote::quote! {}
        }
    }

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
        let name = name.get_ident().unwrap();
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
        let name = name.get_ident().unwrap();
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

    pub(crate) fn apply_expr(&self) -> TokenStream {
        let name = &self.name;
        let name_lit = self.name_str_literal();

        quote_spanned! {self.span=>
            #name_lit => {
                return self.#name.apply(#name_lit, property);
            }
        }
    }

    pub(crate) fn config_apply_root_expr(&self, roots: Vec<Ident>) -> TokenStream {
        let name = &self.name;
        let name_lit = self.name_str_literal();

        let root_apply = roots.iter().map(|r| {
            quote_spanned! {r.span()=>
                let ext = ident.pos(1)?;
                let property = self.#r.apply(&ext, &property)?;
            }
        });

        quote_spanned! {self.span=>
            #name_lit => {
                // Apply all roots
                #( #root_apply )*
                let ident = format!("{:#}", ident).replace("plugin", "").trim_matches('.').parse::<reality::Identifier>()?;
                reality::v2::Config::config(&mut self.#name, &ident, &property)?;
                return Ok(());
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn runmd_root_expr(&self) -> TokenStream {
        let ty = &self.ty;
        let ty = ty.get_ident().unwrap();
        let runmd = if let Some(runmd_doc) = self.doc.as_ref() {
            let lit_str = format!("+ {} .symbol # {}", ty, runmd_doc.value());
            LitStr::new(&lit_str, Span::call_site())
        } else {
            let lit_str = format!("+ {} .symbol", ty);
            LitStr::new(&lit_str, Span::call_site())
        };

        quote_spanned! {self.span=>
            .parse_line(#runmd)?
        }
    }

    pub(crate) fn root_name(&self) -> Ident {
        assert!(self.root);
        let root_ident = &self.name;
        let root_ident = root_ident.get_ident().unwrap();
        format_ident!("{}", root_ident.to_string().to_lowercase())
    }

    pub(crate) fn name_str_literal(&self) -> LitStr {
        let name = &self.name;
        let name = name.get_ident().unwrap();
        LitStr::new(&name.to_string(), name.span())
    }

    pub(crate) fn root_ext_input_pattern_lit_str(&self, ext: &Ident) -> LitStr {
        let name = &self.name;
        let name = name.get_ident().unwrap();

        let format = format!(
            "{}.{}.(?input)",
            name.to_string().to_lowercase(),
            ext.to_string().to_lowercase()
        );
        LitStr::new(&format, Span::call_site())
    }

    pub(crate) fn extension_interpolation_variant(&self, subject: &Ident) -> TokenStream {
        if let Some(ident) = self.ty.get_ident() {
            let root_pattern = format!(
                r##"!#block#.#root#.{}.{};"##,
                ident.to_string().to_lowercase(),
                subject.to_string().to_lowercase()
            );
            let root_ident = format_ident!("{}Root", ident);

            // let root_config_pattern = format!(
            //     r##"!#block#.#root#.{}.(config);"##,
            //     ident.to_string().to_lowercase()
            // );
            // let root_config_ident = format_ident!("{}RootConfig", ident);

            let config_pattern = format!(
                r##"!#block#.#root#.{}.{}.(config).(property);"##,
                ident.to_string().to_lowercase(),
                subject.to_string().to_lowercase()
            );
            let config_ident = format_ident!("{}Config", ident);

            let pattern = format!(
                r##"#block#.#root#.{}.{}.(?property).(?value);"##,
                ident.to_string().to_lowercase(),
                subject.to_string().to_lowercase()
            );
            let pattern = LitStr::new(pattern.as_str(), Span::call_site());
            quote_spanned! {self.span=>
                #[interpolate(#root_pattern)]
                #root_ident,
                // #[interpolate(#root_config_pattern)]
                // #root_config_ident,
                #[interpolate(#config_pattern)]
                #config_ident,
                #[interpolate(#pattern)]
                #ident
            }
        } else {
            quote_spanned! {self.span=>
            }
        }
    }
}

impl Parse for StructField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse attributes
        let attributes = Attribute::parse_outer(input)?;
        let mut config_attr = None::<Ident>;
        let mut doc = None::<LitStr>;
        let mut block = false;
        let mut root = false;
        let mut ext = false;
        let span = input.span();

        for attribute in attributes {
            if attribute.path().is_ident("config") {
                let ident: Ident = attribute.parse_args()?;
                config_attr = Some(ident);
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

            if attribute.path().is_ident("root") {
                root = true;
            }

            if attribute.path().is_ident("ext") {
                ext = true;
            }

            if attribute.path().is_ident("block") {
                block = true;
            }
        }

        // Parse any visibility modifiers
        Visibility::parse(input)?;

        // Name of this struct field
        let name = input.parse::<Path>()?;
        input.parse::<Token![:]>()?;

        // Type is a reference type
        if input.peek(Token![&]) {
            input.parse::<Token![&]>()?;
            input.parse::<Lifetime>()?;

            let mutable = input.peek(Mut);
            if mutable {
                input.parse::<Mut>()?;
            }

            let ty = input.parse::<Path>()?;
            Ok(Self {
                span,
                name,
                ty,
                reference: true,
                mutable,
                option: false,
                ignore: false,
                block,
                root,
                ext,
                config: config_attr,
                doc,
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

                let ty = input.parse::<Path>()?;
                input.parse::<Token![>]>()?;
                Ok(Self {
                    span,
                    name,
                    ty,
                    reference: false,
                    mutable,
                    option: true,
                    ignore: false,
                    block,
                    root,
                    ext,
                    config: config_attr,
                    doc,
                })
            } else {
                let ty = parse2::<Path>(ident.to_token_stream())?;
                input.parse::<Generics>()?;

                Ok(Self {
                    span,
                    name,
                    ty,
                    reference: false,
                    mutable: false,
                    option: false,
                    ignore: false,
                    block,
                    root,
                    ext,
                    config: config_attr,
                    doc,
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
                block,
                root,
                ext,
                config: config_attr,
                doc,
            })
        }
    }
}
