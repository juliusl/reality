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
    /// Location of this field,
    ///
    pub span: Span,

    pub offset: usize,
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
            .unwrap_or(&self.ty)
    }

    /// Renders the callback to use w/ ParseField trait,
    ///
    pub fn render_field_parse_callback(&self) -> TokenStream {
        let name = &self.name;
        let ty = &self.ty;

        fn handle_tagged(ty: &Type, mut on_tagged: impl FnMut() -> TokenStream, mut on_nottagged: impl FnMut() -> TokenStream) -> TokenStream {
            if let Ok(path) = parse2::<Path>(ty.to_token_stream()) {
                let idents = path.segments.iter().fold(String::new(), |mut acc, v| {
                    acc.push_str(&v.ident.to_string());
                    acc.push(':');
                    acc
                });
    
                if vec!["crate:Tagged:", "reality:Tagged:", "Tagged:"]
                    .into_iter()
                    .find(|s| idents == *s)
                    .is_some()
                {
                    on_tagged()
                } else {
                    on_nottagged()
                }
            } else {
                quote::quote! { }
            }
        }
        
        let mut callback = handle_tagged(ty, || {
            quote_spanned!(self.span=>
                self.#name = value;

                if let Some(tag) = _tag {
                    self.#name.set_tag(tag);
                }
            )
            }, || {
            quote_spanned!(self.span=>
                self.#name = value;
            )
        });

        if let Some(cb) = self.parse_callback.as_ref() {
            callback = quote_spanned! {self.span=>
                #cb(self, value, _tag);
            };
        } else if let Some(_) = self.map_of.as_ref() {
            callback = quote_spanned! {self.span=>
                if let Some(tag) = _tag {
                    self.#name.insert(tag.to_string(), value.into());
                }
            };
        } else if let Some(_) = self.vec_of.as_ref() {
            callback = handle_tagged(ty, || {
                quote_spanned!(self.span=>
                    self.#name.push(value.into());

                    if let (Some(tag), Some(last)) = (_tag, self.#name.last_mut()) {
                        last.set_tag(tag);
                    }
                )
            }, || {
                quote_spanned! {self.span=>
                    self.#name.push(value.into());
                }
            });
        } else if let Some(_) = self.option_of.as_ref() {
            callback = handle_tagged(ty, || {
                quote_spanned!(self.span=>
                    self.#name = Some(value.into());

                    if let (Some(tag), Some(last)) = (_tag, self.#name.as_mut()) {
                        last.set_tag(tag);
                    }
                )
            }, || {
                quote_spanned! {self.span=>
                    self.#name = Some(value.into());
                }
            });
        } else if let Some(ty) = self.vecdeq_of.as_ref() {
           callback= handle_tagged(ty, || {
                quote_spanned!(self.span=>
                    self.#name.push_back(value.into());

                    if let (Some(tag), Some(last)) = (_tag, self.#name.back_mut()) {
                        last.set_tag(tag);
                    }
                )
            }, || {
                quote_spanned! {self.span=>
                    self.#name.push_back(value.into());
                }
            });
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
        let mut derive_fromstr = false;
        let span = input.span();

        let visibility = input.parse::<Visibility>().ok();

        // Name of this struct field
        let name = input.parse::<Ident>()?;
        input.parse::<Token![:]>()?;

        let ty = input.parse::<Type>()?;

        for attribute in attributes {
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
                        if let Ok(_) = meta.input.parse::<Token![=]>() {
                            attribute_type = meta.input.parse::<Path>().ok();
                        } else {
                            attribute_type = parse2::<Path>(ty.to_token_stream()).ok();
                        }
                    }

                    if meta.path.is_ident("map_of") {
                        if callback.is_some() || vec_of.is_some() || option_of.is_some() {
                            return Err(syn::Error::new(meta.input.span(), "Can only have one of either, `parse`, `map_of`, of `list_of`"))
                        }

                        if let Ok(_) = meta.input.parse::<Token![=]>() {
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


                        if let Ok(_) = meta.input.parse::<Token![=]>() {
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


                        if let Ok(_) = meta.input.parse::<Token![=]>() {
                            vecdeq_of = meta.input.parse::<syn::Type>().ok();
                        } else {
                            return Err(syn::Error::new(
                                meta.input.span(),
                                "Expecting a type for the value of the Vec",
                            ));
                        }
                    }

                    if meta.path.is_ident("option_of") {
                        if callback.is_some() || map_of.is_some() || vec_of.is_some() {
                            return Err(syn::Error::new(meta.input.span(), "Can only have one of either, `parse`, `option_of`, `map_of`, of `vec_of`"))
                        }

                        if let Ok(_) = meta.input.parse::<Token![=]>() {
                            option_of = meta.input.parse::<syn::Type>().ok();
                        } else {
                            return Err(syn::Error::new(
                                meta.input.span(),
                                "Expecting a type for the value of the Vec",
                            ));
                        }
                    }

                    if meta.path.is_ident("derive_fromstr") {
                        derive_fromstr = true;
                    }

                    Ok(())
                })?;
            }
        }

        Ok(Self {
            rename,
            derive_fromstr,
            vec_of,
            vecdeq_of,
            map_of,
            option_of,
            parse_callback: callback,
            attribute_type,
            span,
            ignore,
            visibility,
            name,
            ty,
            offset: 0,
        })
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
