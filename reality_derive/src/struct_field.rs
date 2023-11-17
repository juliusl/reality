use std::vec;

use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote_spanned;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse2;
use syn::Attribute;
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
    /// Visibility modifier,
    ///
    pub visibility: Option<Visibility>,
    /// Name of the field,
    ///
    pub name: Ident,
    /// Name of the type,
    ///
    pub ty: Type,
    /// Name to use for the field,
    ///
    pub rename: Option<LitStr>,
    /// If set, will ignore this field
    ///
    pub ignore: bool,
    /// If set, will use this field to derive
    /// FromStr for this type,
    ///
    pub derive_fromstr: bool,
    /// Attribute Type,
    ///
    pub attribute_type: Option<Path>,
    /// Parse callback,
    ///
    pub parse_callback: Option<Path>,
    /// Parse as a vec_of Type,
    ///
    pub vec_of: Option<Type>,
    /// Parses as a vecdeq_of Type,
    ///
    pub vecdeq_of: Option<Type>,
    /// Parse as a map_of (String, Type)
    ///
    pub map_of: Option<Type>,
    /// Parse as an option_of Type,
    ///
    pub option_of: Option<Type>,
    /// Parse as an set_of Type,
    /// 
    pub set_of: Option<Type>,
    /// True if this field should be enabled as an ext,
    ///
    pub ext: bool,
    /// True if this field should be enabled as a plugin collection,
    ///
    pub plugin: bool,
    /// Location of this field,
    ///
    pub span: Span,
    /// Allow field to be handled as wire-data,
    ///
    pub wire: bool,
    pub is_decorated: bool,
    pub offset: usize,
    pub variant: Option<(Ident, Ident)>,
    /// TODO: Enable aliased struct fields,
    /// 
    __aliased: Vec<StructField>,
}

impl StructField {
    pub fn field_name_lit_str(&self) -> LitStr {
        self.rename.clone().unwrap_or(LitStr::new(
            self.name.to_string().as_str(),
            Span::call_site(),
        ))
    }

    /// Returns the field type to use,
    ///
    pub fn field_ty(&self) -> &Type {
        self.vec_of
            .as_ref()
            .or(self.vecdeq_of.as_ref())
            .or(self.map_of.as_ref())
            .or(self.option_of.as_ref())
            .or(self.set_of.as_ref())
            .unwrap_or(&self.ty)
    }

    pub fn render_get_fn(&self) -> TokenStream {
        let name = &self.name;
        if let Some((variant, enum_ty)) = self.variant.as_ref() {
            quote_spanned! {self.span=>
                if let #enum_ty::#variant { #name, .. } = self {
                    #name
                } else {
                    unreachable!()
                }
            }
        } else {
            quote_spanned!{self.span=>
                &self.#name
            }
        }
    }

    pub fn render_get_mut_fn(&self) -> TokenStream {
        let name = &self.name;
        if let Some((variant, enum_ty)) = self.variant.as_ref() {
            quote_spanned! {self.span=>
                if let #enum_ty::#variant { #name, .. } = self {
                   #name
                } else {
                    unreachable!()
                }
            }
        } else {
            quote_spanned!{self.span=>
                &mut self.#name
            }
        }
    }

    /// Renders the callback to use w/ ParseField trait,
    ///
    pub fn render_field_parse_callback(&self) -> TokenStream {
        let name = &self.name;
        let ty = &self.field_ty();

        fn handle_tagged(
            ty: &Type,
            mut on_tagged: impl FnMut() -> TokenStream,
            mut on_nottagged: impl FnMut() -> TokenStream,
        ) -> TokenStream {
            if let Ok(path) = parse2::<Path>(ty.to_token_stream()) {
                let idents = path.segments.iter().fold(String::new(), |mut acc, v| {
                    acc.push_str(&v.ident.to_string());
                    acc.push(':');
                    acc
                });

                if vec!["crate:Decorated:", "reality:Decorated:", "Decorated:"]
                    .into_iter()
                    .any(|s| idents == *s)
                {
                    on_tagged()
                } else {
                    on_nottagged()
                }
            } else {
                quote::quote! {}
            }
        }

        if let Some((variant, enum_ty)) = self.variant.as_ref() {
            let mut callback = handle_tagged(
                ty,
                || {
                    quote_spanned!(self.span=>
                        if let #enum_ty::#variant { #name, .. } = self {
                            *#name = value;

                            if let Some(tag) = _tag {
                                #name.set_tag(tag);
                            }
                        }
                        let key = hasher.finish();
                        #name.set_property(key);
                        key
                    )
                },
                || {
                    quote_spanned!(self.span=>
                        if let #enum_ty::#variant { #name, .. } = self {
                            *#name = value;
                        }
                        hasher.finish()
                    )
                },
            );

            if let Some(cb) = self.parse_callback.as_ref() {
                callback = quote_spanned! {self.span=>
                    #cb(self.#name, value, _tag);
                    hasher.finish()
                };
            } else if self.map_of.as_ref().is_some() {
                callback = handle_tagged(
                    ty,
                    || quote_spanned! {self.span=>
                        let key = hasher.finish();
                        if let #enum_ty::#variant { #name, .. } = self {
                            if let Some(tag) = _tag {
                                value.set_tag(tag);
                                value.set_property(key);
                                #name.insert(tag.to_string(), value);
                            }
                        }
                        key
                }, || quote_spanned!{self.span=> 
                    let key = hasher.finish();
                    if let #enum_ty::#variant { #name, .. } = self {
                        if let Some(tag) = _tag {
                            #name.insert(tag.to_string(), value);
                        }
                    }
                    key
                })
            } else if let Some(ty) = self.vec_of.as_ref() {
                callback = handle_tagged(
                    ty,
                    || {
                        quote_spanned!(self.span=>
                            if let #enum_ty::#variant { #name, .. } = self {
                                hasher.hash(#name.len());
                                let key = hasher.finish();
                                #name.push(value);

                                if let (Some(tag), Some(last)) = (_tag, #name.last_mut()) {
                                    last.set_tag(tag);
                                    last.set_property(key);
                                }
                                key
                            } else {
                                hasher.finish()
                            }
                        )
                    },
                    || {
                        quote_spanned! {self.span=>
                            if let #enum_ty::#variant { #name, .. } = self {
                                hasher.hash(#name.len());
                                #name.push(value);
                            }
                            hasher.finish()
                        }
                    },
                );
            } else if let Some(ty) = self.option_of.as_ref() {
                callback = handle_tagged(
                    ty,
                    || {
                        quote_spanned!(self.span=>
                            let key = hasher.finish();
                            if let #enum_ty::#variant { #name, .. } = self {
                                *#name = Some(value);

                                if let (Some(tag), Some(last)) = (_tag, #name.as_mut()) {
                                    last.set_tag(tag);
                                    last.set_property(key);
                                }
                            }
                            key
                        )
                    },
                    || {
                        quote_spanned! {self.span=>
                            if let #enum_ty::#variant { #name, .. } = self {
                                *#name = Some(value);
                            }
                            hasher.finish()
                        }
                    },
                );
            } else if let Some(ty) = self.vecdeq_of.as_ref() {
                callback = handle_tagged(
                    ty,
                    || {
                        quote_spanned!(self.span=>
                            if let #enum_ty::#variant { #name, .. } = self {
                                hasher.hash(#name.len());
                                let key = hasher.finish();
                                #name.push_back(value);
    
                                if let (Some(tag), Some(last)) = (_tag, #name.back_mut()) {
                                    last.set_tag(tag);
                                    last.set_property(key);
                                }
                                key
                            } else { 
                                hasher.finish()
                            }
                        )
                    },
                    || {
                        quote_spanned! {self.span=>
                            if let #enum_ty::#variant { #name, .. } = self {
                                hasher.hash(#name.len());
                                #name.push_back(value);
                            }
                            hasher.finish()
                        }
                    },
                );
            } else if let Some(ty) = self.set_of.as_ref() {
                callback = handle_tagged(
                    ty,
                    || {
                        quote_spanned!(self.span=>
                            if let #enum_ty::#variant { #name, .. } = self {
                                hasher.hash(#name.len());
                                let key = hasher.finish();
                                if let Some(tag) = _tag {
                                    value.set_tag(tag);
                                    value.set_property(key);
                                }
                                #name.insert(value);
                                key
                            } else {
                                hasher.finish()
                            }
                        )
                    },
                    || {
                        quote_spanned! {self.span=>
                            if let #enum_ty::#variant { #name, .. } = self {
                                hasher.hash(#name.len());
                                #name.insert(value);
                            }
                            hasher.finish()
                        }
                    },
                );
            }

            return callback;
        }

        let mut callback = handle_tagged(
            ty,
            || {
                quote_spanned!(self.span=>
                    self.#name = value;

                    let key = hasher.finish();
                    if let Some(tag) = _tag {
                        self.#name.set_tag(tag);
                        self.#name.set_property(key);
                    }
                    key
                )
            },
            || {
                quote_spanned!(self.span=>
                    self.#name = value;
                    hasher.finish()
                )
            },
        );

        if let Some(cb) = self.parse_callback.as_ref() {
            callback = quote_spanned! {self.span=>
                #cb(self, value, _tag);
                hasher.finish()
            };
        } else if self.map_of.as_ref().is_some() {
            callback = handle_tagged(
                ty, 
                || quote_spanned! {self.span=>
                    let key = hasher.finish();
                    if let Some(tag) = _tag {
                        value.set_tag(tag);
                        value.set_property(key);
                        self.#name.insert(tag.to_string(), value);
                    }
                    key
                }, 
                ||  quote_spanned! (self.span=>
                    let key = hasher.finish();
                    if let Some(tag) = _tag {
                        self.#name.insert(tag.to_string(), value);
                    }
                    key
                ));
        } else if let Some(ty) = self.vec_of.as_ref() {
            callback = handle_tagged(
                ty,
                || {
                    quote_spanned!(self.span=>
                        hasher.hash(self.#name.len());
                        self.#name.push(value);

                        let key = hasher.finish();
                        if let (Some(tag), Some(last)) = (_tag, self.#name.last_mut()) {
                            last.set_tag(tag);
                            last.set_property(key);
                        }
                        key
                    )
                },
                || {
                    quote_spanned! {self.span=>
                        hasher.hash(self.#name.len());
                        self.#name.push(value);
                        hasher.finish()
                    }
                },
            );
        } else if let Some(ty) = self.option_of.as_ref() {
            callback = handle_tagged(
                ty,
                || {
                    quote_spanned!(self.span=>
                        self.#name = Some(value);
                        let key = hasher.finish();

                        if let (Some(tag), Some(last)) = (_tag, self.#name.as_mut()) {
                            last.set_tag(tag);
                            last.set_property(key);
                        }
                        key
                    )
                },
                || {
                    quote_spanned! {self.span=>
                        self.#name = Some(value);
                        hasher.finish()
                    }
                },
            );
        } else if let Some(ty) = self.vecdeq_of.as_ref() {
            callback = handle_tagged(
                ty,
                || {
                    quote_spanned!(self.span=>
                        hasher.hash(self.#name.len());
                        self.#name.push_back(value);
                        let key = hasher.finish();

                        if let (Some(tag), Some(last)) = (_tag, self.#name.back_mut()) {
                            last.set_tag(tag);
                            last.set_property(key);
                        }

                        key
                    )
                },
                || {
                    quote_spanned! {self.span=>
                        hasher.hash(self.#name.len());
                        self.#name.push_back(value);
                        hasher.finish()
                    }
                },
            );
        } else if let Some(ty) = self.set_of.as_ref() {
            callback = handle_tagged(
                ty,
                || {
                    quote_spanned!(self.span=>
                        if let Some(tag) = _tag {
                            value.set_tag(tag);
                        }
                        hasher.hash(self.#name.len());
                        let key = hasher.finish();
                        value.set_property(key);
                        self.#name.insert(value);
                        key
                    )
                },
                || {
                    quote_spanned! {self.span=>
                        hasher.hash(self.#name.len());
                        self.#name.insert(value);
                        hasher.finish()
                    }
                },
            );
        }

        callback
    }
}

impl Parse for StructField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse attributes
        let attributes = Attribute::parse_outer(input)?;
        let mut rename = None::<LitStr>;
        let mut ignore = false;
        let mut callback = None;
        let mut attribute_type = None;
        let mut map_of = None;
        let mut vec_of = None;
        let mut vecdeq_of = None;
        let mut option_of = None;
        let mut set_of = None;
        let mut derive_fromstr = false;
        let mut ext = false;
        let mut plugin = false;
        let mut wire = false;
        let span = input.span();

        let visibility = input.parse::<Visibility>().ok();

        // Name of this struct field
        let name = input.parse::<Ident>()?;
        input.parse::<Token![:]>()?;

        let ty = input.parse::<Type>()?;

        for attribute in attributes {
            if attribute.path().is_ident("skip") {
                ignore = true;
            }

            // #[reality(ignore, rename = "SOME_NAME")]
            if attribute.path().is_ident("reality") {
                attribute.parse_nested_meta(|meta| {
                    if meta.path.is_ident("ignore") {
                        ignore = true;
                    }

                    if meta.path.is_ident("rename") {
                        meta.input.parse::<Token![=]>()?;
                        let _r = meta.input.parse::<LitStr>()?;
                        rename = Some(_r);
                    }

                    if meta.path.is_ident("parse") {
                        meta.input.parse::<Token![=]>()?;
                        callback = meta.input.parse::<Path>().ok();
                    }

                    if meta.path.is_ident("attribute_type") {
                        if meta.input.parse::<Token![=]>().is_ok() {
                            attribute_type = meta.input.parse::<Path>().ok();
                        } else {
                            attribute_type = parse2::<Path>(ty.to_token_stream()).ok();
                        }
                    }

                    if meta.path.is_ident("map_of") {
                        if callback.is_some() || vec_of.is_some() || option_of.is_some() {
                            return Err(syn::Error::new(meta.input.span(), "Can only have one of either, `parse`, `map_of`, of `list_of`"))
                        }

                        if meta.input.parse::<Token![=]>().is_ok() {
                            map_of = meta.input.parse::<syn::Type>().ok();
                        } else {
                            return Err(syn::Error::new(
                                meta.input.span(),
                                "Expecting a type for the value of the map",
                            ));
                        }
                    }

                    if meta.path.is_ident("vec_of") {
                        if callback.is_some() || map_of.is_some() || option_of.is_some() {
                            return Err(syn::Error::new(meta.input.span(), "Can only have one of either, `parse`, `map_of`, of `vec_of`"))
                        }


                        if meta.input.parse::<Token![=]>().is_ok() {
                            vec_of = meta.input.parse::<syn::Type>().ok();
                        } else {
                            return Err(syn::Error::new(
                                meta.input.span(),
                                "Expecting a type for the value of the Vec",
                            ));
                        }
                    }

                    if meta.path.is_ident("vecdeq_of") {
                        if callback.is_some() || map_of.is_some() || option_of.is_some() {
                            return Err(syn::Error::new(meta.input.span(), "Can only have one of either, `parse`, `map_of`, of `vec_of`"))
                        }


                        if meta.input.parse::<Token![=]>().is_ok() {
                            vecdeq_of = meta.input.parse::<syn::Type>().ok();
                        } else {
                            return Err(syn::Error::new(
                                meta.input.span(),
                                "Expecting a type for the value of the Vec",
                            ));
                        }
                    }

                    if meta.path.is_ident("option_of") {
                        if callback.is_some() || map_of.is_some() || vec_of.is_some() || vecdeq_of.is_some() || set_of.is_some() {
                            return Err(syn::Error::new(meta.input.span(), "Can only have one of either, `parse`, `option_of`, `map_of`, of `vec_of`"))
                        }

                        if meta.input.parse::<Token![=]>().is_ok() {
                            option_of = meta.input.parse::<syn::Type>().ok();
                        } else {
                            return Err(syn::Error::new(
                                meta.input.span(),
                                "Expecting a type for the value of the Vec",
                            ));
                        }
                    }

                    if meta.path.is_ident("set_of") {
                        if callback.is_some() || map_of.is_some() || vec_of.is_some() || vecdeq_of.is_some() {
                            return Err(syn::Error::new(meta.input.span(), "Can only have one of either, `parse`, `option_of`, `map_of`, of `vec_of`"))
                        }

                        if meta.input.parse::<Token![=]>().is_ok() {
                            set_of = meta.input.parse::<syn::Type>().ok();
                        } else {
                            return Err(syn::Error::new(
                                meta.input.span(),
                                "Expecting a type for the value of the Vec",
                            ));
                        }
                    }

                    if meta.path.is_ident("ext") {
                        ext = true;
                    }
                    
                    if meta.path.is_ident("plugin") {
                        plugin = true;
                    }
                    
                    if meta.path.is_ident("wire") {
                        wire = true;
                    }

                    if meta.path.is_ident("derive_fromstr") {
                        derive_fromstr = true;
                    }

                    Ok(())
                })?;
            }
        }

        let mut field = Self {
            rename,
            derive_fromstr,
            vec_of,
            vecdeq_of,
            map_of,
            option_of,
            set_of,
            parse_callback: callback,
            attribute_type,
            ext,
            plugin,
            wire,
            span,
            ignore,
            visibility,
            name,
            ty,
            is_decorated: false,
            variant: None,
            offset: 0,
            __aliased: vec![],
        };

        let ty = field.field_ty();
        field.is_decorated = if let Ok(path) = parse2::<Path>(ty.to_token_stream()) {
            let idents = path.segments.iter().fold(String::new(), |mut acc, v| {
                acc.push_str(&v.ident.to_string());
                acc.push(':');
                acc
            });

            vec!["crate:Decorated:", "reality:Decorated:", "Decorated:"]
                .into_iter()
                .any(|s| idents == *s)
        } else {
            false
        };

        Ok(field)
    }
}

#[test]
fn test_struct_field_parsing() {
    use quote::ToTokens;

    let stream = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
        r#"
#[reality(ignore, rename = "Name")]
name: String
"#,
    )
    .unwrap();

    let field = syn::parse2::<StructField>(stream).unwrap();

    assert_eq!(true, field.ignore);
    assert_eq!(
        Some("\"Name\"".to_string()),
        field.rename.map(|r| r.to_token_stream().to_string())
    );
    assert_eq!("name", field.name.to_string().as_str());
    assert_eq!("String", field.ty.to_token_stream().to_string().as_str());
}
