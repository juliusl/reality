use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use quote::quote_spanned;
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
    /// Attribute Type,
    ///
    pub attribute_type: Option<Path>,
    /// Parse callback,
    ///
    pub parse_callback: Option<Path>,
    /// Parse as a vec_of Type,
    ///
    pub vec_of: Option<Type>,
    /// Parse as a map_of (Key, Value)
    ///
    pub map_of: Option<Type>,
    /// Location of this field,
    ///
    pub span: Span,
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
        self.vec_of.as_ref().or(self.map_of.as_ref()).unwrap_or(&self.ty)
    }

    /// Renders the callback to use w/ ParseField trait,
    ///
    pub fn render_field_parse_callback(&self) -> TokenStream {
        let name = &self.name;
        let ty = &self.ty;

        let mut callback = quote_spanned!(self.span=>
            self.#name = value;
        );

        if let Ok(path) = parse2::<Path>(ty.to_token_stream()) {
            let idents = path.segments.iter().fold(String::new(), |mut acc, v| {
                acc.push_str(&v.ident.to_string());
                acc.push(':');
                acc
            });

            if vec!["crate:Tag:", "reality:Tag:", "Tag:"]
                .into_iter()
                .find(|s| idents == *s)
                .is_some()
            {
                callback = quote_spanned!(self.span=>
                    self.#name = value;

                    if let Some(tag) = _tag {
                        self.#name.set_tag(tag);
                    }
                )
            }
        }
        
        if let Some(cb) = self.parse_callback.as_ref() {
            callback = quote_spanned! {self.span=>
                #cb(self, value, _tag);
            }
        } else if let Some(_) = self.map_of.as_ref() {
            callback = quote_spanned! {self.span=>
                if let Some(tag) = _tag {
                    self.#name.insert(tag.to_string(), value);
                }
            }
        } else if let Some(_) = self.vec_of.as_ref() {
            callback = quote_spanned! {self.span=>
                self.#name.push(value);
            }
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
                        if callback.is_some() || vec_of.is_some() {
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
                        if callback.is_some() || map_of.is_some() {
                            return Err(syn::Error::new(meta.input.span(), "Can only have one of either, `parse`, `map_of`, of `list_of`"))
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

                    Ok(())
                })?;
            }
        }

        //     if let Some((Ok(ident), Ok(generics))) = syn::parse::Parser::parse2(|input: &ParseBuffer| {
        //         let ident = input.parse::<Ident>();
        //         let generics = input.parse::<Generics>();

        //         if ident.is_ok() && generics.is_ok() {
        //             Ok((ident, generics))
        //         } else {
        //             Err(syn::Error::new(Span::call_site(), "noop"))
        //         }
        //    }, ty.to_token_stream()).ok() {
        //         if ident.to_string().as_str() == "Vec" {

        //         }
        //    }

        Ok(Self {
            rename,
            vec_of,
            map_of,
            parse_callback: callback,
            attribute_type,
            span,
            ignore,
            visibility,
            name,
            ty,
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
