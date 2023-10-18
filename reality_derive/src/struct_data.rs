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
    /// Reality attribute, on_load fn path,
    /// 
    reality_on_load: Option<Path>,
    /// Reality attribute, on_unload fn path,
    /// 
    reality_on_unload: Option<Path>,
    /// Reality attribute, on_completed fn path,
    /// 
    reality_on_completed: Option<Path>,
}

impl Parse for StructData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let derive_input = DeriveInput::parse(input)?;

        let name = derive_input.ident;

        let mut reality_rename = None;
        let mut reality_on_load = None;
        let mut reality_on_unload = None;
        let mut reality_on_completed = None;

        for attr in derive_input.attrs.iter() {
            if attr.path().is_ident("reality") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        meta.input.parse::<Token![=]>()?;
                        reality_rename = meta.input.parse::<LitStr>().ok();
                    }
                    
                    if meta.path.is_ident("load") {
                        meta.input.parse::<Token![=]>()?;
                        reality_on_load = meta.input.parse::<Path>().ok();
                    }

                    if meta.path.is_ident("unload") {
                        meta.input.parse::<Token![=]>()?;
                        reality_on_unload = meta.input.parse::<Path>().ok();
                    }

                    if meta.path.is_ident("completed") {
                        meta.input.parse::<Token![=]>()?;
                        reality_on_completed = meta.input.parse::<Path>().ok();
                    }

                    Ok(())
                })?;
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
                reality_on_load,
                reality_on_unload,
                reality_on_completed,
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
                quote!(S: StorageTarget + Send + Sync + 'static)).expect("should be able to tokenize")
            );

        let (impl_generics, _, where_clause) = &self.generics.split_for_impl();
        let trait_ident = self.reality_rename.unwrap_or(LitStr::new(ident.to_string().to_lowercase().as_str(), self.span));
        let fields = self.fields.clone();
        let fields = fields.iter().enumerate().map(|(offset, f)| {
            let ty = &f.field_ty();
            if let Some(attribute_type) = f.attribute_type.as_ref() {
                quote_spanned! {f.span=>
                    parser.add_parseable_attribute_type_field::<#offset, Self, #attribute_type>();
                }
            } else {
                let comment = LitStr::new(format!("Parsing field `{}`", f.name.to_string()).as_str(), Span::call_site());
                quote_spanned! {f.span=>
                    let _ = #comment;
                    parser.add_parseable_field::<#offset, Self, #ty>();
                }
            }
        });

        //  Implementation for fields parsers,
        // 
        let fields_on_parse_impl = self.fields.iter().enumerate().map(|(offset, f)| {
            let field_ident = f.field_name_lit_str();
            let ty = f.field_ty();
            let absolute_ty = &f.ty;
            let name = &f.name;

            // Callback to use
            let callback = f.render_field_parse_callback();

            quote_spanned! {f.span=>
                impl #original_impl_generics OnParseField<#offset, #ty> for #ident #ty_generics #where_clause {
                    type ProjectedType = #absolute_ty;

                    fn field_name() -> &'static str {
                        #field_ident
                    }
                
                    #[allow(unused_variables)]
                    fn on_parse(&mut self, value: #ty, _tag: Option<&String>) {
                        #callback
                    }

                    #[inline]
                    fn get(&self) -> &Self::ProjectedType {
                        &self.#name
                    }

                    #[inline]
                    fn get_mut(&mut self) -> &mut Self::ProjectedType {
                        &mut self.#name
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
                    let mut enable = parser.parse_attribute::<Self>(content);

                    if enable.is_some() {
                        #(#fields)*
                    }
                }
            }

            #(#fields_on_parse_impl)*
        }
    }

     /// Returns token stream of generated AttributeType trait
    /// 
    pub(crate) fn object_type_trait(self) -> TokenStream {
        let name = self.name.clone();
        let original = self.generics.clone();
        let (_, ty_generics, _) = original.split_for_impl();
        let ty_generics = ty_generics.clone();
        let mut generics = self.generics.clone();
        generics.params.push(
            parse2::<GenericParam>(
                quote!(Storage: StorageTarget + Send + Sync + 'static)).expect("should be able to tokenize")
            );

        let (impl_generics, _, where_clause) = &generics.split_for_impl();

        let on_load = self.reality_on_load.clone().map(|p| quote!(#p(storage).await;)).unwrap_or(quote!());
        let on_unload = self.reality_on_unload.clone().map(|p| quote!(#p(storage).await;)).unwrap_or(quote!());
        let on_completed = self.reality_on_completed.clone().map(|p| quote!(#p(storage))).unwrap_or(quote!(None));

        let object_type_trait = quote_spanned!(self.span=>
            #[reality::runmd::async_trait]
            impl #impl_generics BlockObject<Storage> for #name #ty_generics #where_clause {
                async fn on_load(storage: AsyncStorageTarget<Storage::Namespace>) {
                    #on_load
                }

                async fn on_unload(storage: AsyncStorageTarget<Storage::Namespace>) {
                    #on_unload
                }

                fn on_completed(storage: AsyncStorageTarget<Storage::Namespace>) -> Option<AsyncStorageTarget<Storage::Namespace>> {
                    #on_completed
                }
            });


        let mut attribute_type = self.attribute_type_trait();
        attribute_type.extend(object_type_trait);
        attribute_type
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