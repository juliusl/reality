use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::ToTokens;
use quote::quote_spanned;
use syn::GenericParam;
use syn::Generics;
use syn::LitStr;
use syn::parse::Parse;
use syn::parse2;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;

use crate::struct_field::StructField;

/// Parses a struct from derive attribute,
///
/// Generates impl's for Load, Config, and Apply traits
///
pub(crate) struct StructData {
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
}

impl Parse for StructData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let derive_input = DeriveInput::parse(input)?;

        let name = derive_input.ident;

        // for attr in derive_input.attrs.iter() {
        //     if attr.path().is_ident("compile") {
        //         if let Some(args) = attr
        //             .parse_args_with(
        //                 syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
        //             )
        //             .ok()
        //         {
        //             compile = args.iter().cloned().collect();
        //         }

        //         if compile.is_empty() {
        //             compile.push(attr.path().clone());
        //         }
        //     }
        // }

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
            span: input.span(),
            name,
            generics: derive_input.generics,
            fields,
        })
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
        self.generics.params.push(parse2::<GenericParam>(quote!(S: StorageTarget<Namespace = Complex> + StorageTargetCallbackProvider + Send + Sync + 'static)).unwrap());

        let (impl_generics, _, where_clause) = &self.generics.split_for_impl();
        let trait_ident = LitStr::new(ident.to_string().to_lowercase().as_str(), Span::call_site());
        let fields = self.fields.clone();
        let fields = fields.iter().enumerate().map(|(offset, f)| {
            let ty = &f.ty;
            quote_spanned! {f.span=>
                parser.add_parseable::<#offset, Self, #ty>();
            }
        });

        //  Implementation for fields parsers,
        // 
        let fields_on_parse_impl = self.fields.iter().enumerate().map(|(offset, f)| {
            let name = &f.name;
            let trait_ident = LitStr::new(f.name.to_string().as_str(), Span::call_site());
            let ty = &f.ty;
            quote_spanned! {f.span=>
                impl #original_impl_generics OnParseField<#offset, #ty> for #ident #ty_generics #where_clause {
                    fn field_name() -> &'static str {
                        #trait_ident
                    }
                
                    fn on_parse(&mut self, value: #ty) {
                        self.#name = value;
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
                    #(#fields)*
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