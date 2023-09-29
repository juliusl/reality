use proc_macro2::Ident;
use proc_macro2::Span;
use quote::ToTokens;
use syn::Path;
use syn::parse::Parse;
use syn::Attribute;
use syn::LitStr;
use syn::Token;
use syn::Type;
use syn::Visibility;
use syn::parse2;

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
    /// Location of this field,
    /// 
    pub span: Span,
}

impl StructField {
    pub fn field_name_lit_str(&self) -> LitStr {
        self.rename.clone().unwrap_or(LitStr::new(self.name.to_string().as_str(), Span::call_site()))
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

                    Ok(())
                })?;
            }
        }

        Ok(Self {
            rename,
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
    assert_eq!(Some("\"Name\"".to_string()), field.rename.map(|r| r.to_token_stream().to_string()));
    assert_eq!("name", field.name.to_string().as_str());
    assert_eq!("String", field.ty.to_token_stream().to_string().as_str());
}
