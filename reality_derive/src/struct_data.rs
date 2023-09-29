use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use quote::quote_spanned;
use syn::GenericParam;
use syn::Generics;
use syn::LitStr;
use syn::Path;
use syn::Token;
use syn::parse::Parse;
use syn::parse2;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;

use crate::struct_field::StructField;

/// Parses a struct from derive attribute,
/// 
/// ``` norun
/// #[derive(AttributeType)]
/// #[reality(rename="")]
/// struct Test {
///        
/// }
/// ```
pub(crate) struct StructData {
    /// Span of the struct being derived,
    /// 
    span: Span,
    /// Name of the struct,
    ///
    name: Ident,
    /// Generics
    /// 
    generics: Generics,
    /// Parsed struct fields,
    ///
    fields: Vec<StructField>,
    /// Reality attribute, rename option
    /// 
    reality_rename: Option<LitStr>,
    /// Reality attribute, resource-label,
    /// 
    reality_resource_label: Option<LitStr>,
}

impl Parse for StructData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let derive_input = DeriveInput::parse(input)?;

        let name = derive_input.ident;

        let mut reality_rename = None;
        let mut reality_resource_label = None;

        for attr in derive_input.attrs.iter() {
            if attr.path().is_ident("reality") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        meta.input.parse::<Token![=]>()?;
                        reality_rename = meta.input.parse::<LitStr>().ok();
                    }

                    if meta.path.is_ident("resource_label") {
                        meta.input.parse::<Token![=]>()?;
                        reality_resource_label = meta.input.parse::<LitStr>().ok();
                    }

                    Ok(())
                })?;

                // if compile.is_empty() {
                //     compile.push(attr.path().clone());
                // }
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

        if let Some(lifetime) = derive_input.generics.lifetimes().find(|l| l.lifetime.ident != format_ident!("static")) {
            Err(input.error(format!("Struct must be `'static`, therefore may not contain any fields w/ generic lifetimes. Please remove `'{}`", lifetime.lifetime.ident.to_string())))
        } else {
            Ok(Self {
                span: input.span(),
                name,
                generics: derive_input.generics,
                fields,
                reality_rename,
                reality_resource_label,
            })
        }
    }
}

impl StructData {
    /// Returns token stream of generated AttributeType trait
    /// 
    pub(crate) fn attribute_type_trait(mut self) -> TokenStream {
        let ident = &self.name;
        let original = self.generics.clone();
        let (original_impl_generics, ty_generics, _) = original.split_for_impl();
        let ty_generics = ty_generics.clone();
        self.generics.params.push(
            parse2::<GenericParam>(
                quote!(S: StorageTarget<Namespace = Complex> + Send + Sync + 'static)).expect("should be able to tokenize")
            );

        let (impl_generics, _, where_clause) = &self.generics.split_for_impl();
        let trait_ident = self.reality_rename.unwrap_or(LitStr::new(ident.to_string().to_lowercase().as_str(), self.span));
        let fields = self.fields.clone();
        let fields = fields.iter().enumerate().map(|(offset, f)| {
            let ty = &f.ty;
            if let Some(attribute_type) = f.attribute_type.as_ref() {
                quote_spanned! {f.span=>
                    parser.add_parseable_attribute_type_field::<#offset, Self, #attribute_type>();
                }
            } else {
                quote_spanned! {f.span=>
                    parser.add_parseable_field::<#offset, Self, #ty>();
                }
            }
        });

        let resource_key = self.reality_resource_label.clone().map_or_else(|| quote!(None), |l| {
            quote! {
                Some(ResourceKey::with_label(#l))
            }
        });

        //  Implementation for fields parsers,
        // 
        let fields_on_parse_impl = self.fields.iter().enumerate().map(|(offset, f)| {
            let name = &f.name;
            let field_ident = f.field_name_lit_str();
            let ty = &f.ty;

            // Callback to use
            let callback = f.parse_callback.as_ref().map_or_else(|| {
                if let Ok(path) = parse2::<Path>(ty.to_token_stream()) {

                    let idents = path.segments.iter().fold(String::new(), |mut acc, v| {
                        acc.push_str(&v.ident.to_string());
                        acc.push(':');
                        acc
                    });

                    if vec!["crate:Tag:", "reality:Tag:", "Tag:"].into_iter().find(|s| idents == *s).is_some() {
                        quote!(
                            self.#name = value;

                            if let Some(tag) = tag {
                                self.#name.set_tag(tag);
                            }
                        )
                    } else {
                        quote!(
                            self.#name = value;
                        )
                    }
                } else {
                    quote!(
                        self.#name = value;
                    )
                }

            }, |c| {
                quote! {
                    #c(self, value, tag);
                }
            });

            quote_spanned! {f.span=>
                impl #original_impl_generics OnParseField<#offset, #ty> for #ident #ty_generics #where_clause {
                    fn field_name() -> &'static str {
                        #field_ident
                    }

                    fn owner_resource_key() -> Option<ResourceKey<Self>> {
                        #resource_key
                    }
                
                    #[allow(unused_variables)]
                    fn on_parse(&mut self, value: #ty, tag: Option<&String>) {
                        #callback
                    }
                }
            }
        });

        quote_spanned! {self.span=> 
            impl #impl_generics AttributeType<S> for #ident #ty_generics #where_clause {
                fn ident() -> &'static str {
                    #trait_ident
                }

                fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
                    let mut enable = false;
                    {
                    // Storage target must be enabled,
                    if let Some(storage) = parser.storage() {
                        // Initialize attribute type,
                        if let Ok(init) = content.as_ref().parse::<Self>() {
                            storage.lazy_put_resource(init, #resource_key);
                            enable = true;
                        }
                    }
                    }

                    if enable {
                        #(#fields)*
                    }
                }
            }

            #(#fields_on_parse_impl)*
        }
    }
}

#[test]
fn test_parse_struct_data() {
    use quote::ToTokens;
    
    let stream = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
        r#"
struct Test {
    #[reality(rename = "Name")]
    name: String,
}
"#,
    )
    .unwrap();

    let mut data = syn::parse2::<StructData>(stream).unwrap();

    let field = data.fields.remove(0);
    assert_eq!(false, field.ignore);
    assert_eq!(Some("\"Name\"".to_string()), field.rename.map(|r| r.to_token_stream().to_string()));
    assert_eq!("name", field.name.to_string().as_str());
    assert_eq!("String", field.ty.to_token_stream().to_string().as_str());
}