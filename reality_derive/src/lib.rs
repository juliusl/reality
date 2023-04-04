use proc_macro2::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use quote::ToTokens;
use quote::format_ident;
use syn::ext::IdentExt;
use syn::parse::Parse;
use syn::token::Mut;
use syn::Data;
use syn::Visibility;
use syn::Token;
use syn::Lifetime;
use syn::Fields;
use syn::DeriveInput;
use syn::Attribute;
use syn::parse2;

/// Derives Load trait implementation as well as system data impl,
///
/// Given a struct such as,
///
/// ```
/// #[derive(Load)]
/// struct A<'a> {
///     identifier: &'a Identifier
/// }
/// ```
///
/// The generated code would look like this,
///
/// ```
/// pub type AFormat<'a> = (
///     &'a ReadStorage<'a, Identifier>,
/// )
///
/// #[derive(SystemData)]
/// pub type ASystemData<'a> {
///     identifier_storage: ReadStorage<'a, Identifier>,
/// }
///
/// impl<'a> Provider<'a, AFormat<'a>> for ASystemData<'a> {
///     fn provide(&'a self) -> TestFormat<'a> {
///         (
///             &self.identifier_storage
///         )
///     }
/// }
///
/// impl<'a> Load for A<'a> {
///     type Layout = AFormat<'a>;
///
///     fn load(identifier: <Self::Layout as Join>::Type) -> Self {
///         Self {
///             identifier
///         }
///     }
/// }
/// ```
///
#[proc_macro_derive(Load)]
pub fn derive_load(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let load_format = _derive_load(input.into());

    proc_macro::TokenStream::from(load_format)
}

fn _derive_load(input: TokenStream) -> TokenStream {
    let input = syn::parse2::<DeriveInput>(input).unwrap();

    let ident = input.ident;
    let format_ident = format_ident!("{}Format", ident);
    let systemdata_ident = format_ident!("{}SystemData", ident);

    let types = load_format(&input.data);
    let idents = load_idents(&input.data);
    let systemdata_body = load_systemdata(&input.data);
    let systemdata_provider_body = load_systemdata_provider(&input.data);

    let format = quote! {
        use specs::prelude::*;

        pub type #format_ident<'a> = ( #types );

        #[derive(specs::SystemData)]
        pub struct #systemdata_ident<'a> {
            entities: specs::Entities<'a>,
            #systemdata_body
        }

        impl<'a> reality::state::Load for #ident<'a> {
            type Layout = #format_ident<'a>;

            fn load((#idents): <Self::Layout as specs::Join>::Type) -> Self {
                Self { #idents }
            }
        }

        impl<'a> reality::state::Provider<'a, #format_ident<'a>> for #systemdata_ident<'a> {
            fn provide(&'a self) -> #format_ident<'a> {
                (
                    #systemdata_provider_body
                )
            }
        }

        impl<'a> AsRef<specs::Entities<'a>> for #systemdata_ident<'a> {
            fn as_ref(&self) -> &specs::Entities<'a> {
                &self.entities
            }
        }
    };

    TokenStream::from(format)
}

/// Returns the body of the layout type alias,
///
fn load_format(data: &Data) -> TokenStream {
    match &data {
        Data::Struct(_data) => match &_data.fields {
            Fields::Named(named) => {
                let map = named.named.iter().map(|n| {
                    let struct_field =
                        parse2::<StructField>(n.to_token_stream()).expect("should parse");

                    match &struct_field {
                        StructField {
                            ty,
                            reference,
                            mutable,
                            ..
                        } if *reference => {
                            if *mutable {
                                quote! {
                                    &'a mut specs::WriteStorage<'a, #ty>
                                }
                            } else {
                                quote! {
                                    &'a specs::ReadStorage<'a, #ty>
                                }
                            }
                        }
                        StructField {
                            ty,
                            mutable,
                            option,
                            ..
                        } if *option => {
                            if *mutable {
                                quote! {
                                    specs::join::MaybeJoin<&'a mut WriteStorage<'a, #ty>>
                                }
                            } else {
                                quote! {
                                    specs::join::MaybeJoin<&'a ReadStorage<'a, #ty>>
                                }
                            }
                        }
                        _ => {
                            unimplemented!()
                        }
                    }
                });

                quote! {
                    #( #map ),*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

/// Returns a comma seperated identifiers,
///
fn load_idents(data: &Data) -> TokenStream {
    match &data {
        Data::Struct(_data) => match &_data.fields {
            Fields::Named(named) => {
                let map = named
                    .named
                    .iter()
                    .filter_map(|n| n.ident.as_ref())
                    .map(|i| quote!( #i ));

                quote! {
                    #( #map ),*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

fn load_systemdata(data: &Data) -> TokenStream {
    match &data {
        Data::Struct(_data) => match &_data.fields {
            Fields::Named(named) => {
                let map = named.named.iter().map(|n| {
                    let struct_field =
                        parse2::<StructField>(n.to_token_stream()).expect("should parse");

                    match &struct_field {
                        StructField {
                            name: ident, ty, mutable, ..
                        } => {
                            let ident = format_ident!("{}_storage", ident);

                            if *mutable {
                                quote! {
                                    #ident: mut specs::WriteStorage<'a, #ty>
                                }
                            } else {
                                quote! {
                                    #ident: specs::ReadStorage<'a, #ty>
                                }
                            }
                        }
                    }
                });

                quote! {
                    #( #map ),*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

fn load_systemdata_provider(data: &Data) -> TokenStream {
    match &data {
        Data::Struct(_data) => match &_data.fields {
            Fields::Named(named) => {
                let map = named.named.iter().map(|n| {
                    let struct_field =
                        parse2::<StructField>(n.to_token_stream()).expect("should parse");

                    match &struct_field {
                        StructField {
                            name: ident,
                            reference,
                            mutable,
                            ..
                        } if *reference => {
                            let ident = format_ident!("{}_storage", ident);
                            if *mutable {
                                quote! {
                                    &mut self.#ident
                                }
                            } else {
                                quote! {
                                    &self.#ident
                                }
                            }
                        }
                        StructField { name: ident, option, .. } if *option => {
                            let ident = format_ident!("{}_storage", ident);
                            quote! {
                                self.#ident.maybe()
                            }
                        }
                        _ => {
                            unimplemented!()
                        }
                    }
                });

                quote! {
                    #( #map ),*
                }
            }
            _ => unimplemented!(),
        },
        _ => unimplemented!(),
    }
}

/// Parses a struct field such as,
///
/// #visibility #ident: #reference #lifetime #mutability #type,
///
struct StructField {
    /// Name of the field,
    /// 
    name: Ident,
    /// Name of the type,
    /// 
    ty: Ident,
    /// True if reference type
    /// 
    reference: bool,
    /// True if mutable
    /// 
    mutable: bool,
    /// True if Option<T> type,
    /// 
    option: bool,
}

impl Parse for StructField {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse any doc comments
        Attribute::parse_outer(input)?;
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
                })
            } else {
                unimplemented!("Only Option<&'a T>, &'a T, and &'a mut T are supported")
            }
        } else {
            unimplemented!("Only Option<&'a T>, &'a T, and &'a mut T are supported")
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use proc_macro2::TokenStream;
    use proc_macro2::Span;
    use proc_macro2::Ident;
    use quote::quote;
    use quote::ToTokens;
    use quote::format_ident;
    use syn::ext::IdentExt;
    use syn::parse::Parse;
    use syn::token::Mut;
    use syn::Data;
    use syn::Visibility;
    use syn::Token;
    use syn::LitStr;
    use syn::Lifetime;
    use syn::Fields;
    use syn::DeriveInput;
    use syn::Attribute;
    use syn::parse2;
    use crate::_derive_load;

    #[test]
    fn test_derive_load() {
        let ts = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
            r#"
    struct Test {
        /// Object's identifier,
        ///
        identifier: &'a Identifier,
        /// Block properties,
        ///
        properties: &'a Properties,
        /// Compiled source block,
        ///
        block: Option<&'a Block>,
        /// Compiled root,
        ///
        root: Option<&'a Root>,
        /// Thunk'ed call fn,
        ///
        call: Option<&'a ThunkCall>,
        /// Thunk'ed build fn,
        ///
        build: Option<&'a ThunkBuild>,
        /// Thunk'ed update fn,
        ///
        update: Option<&'a ThunkUpdate>,
        /// Thunk'ed listen fn,
        /// 
        listen: Option<&'a ThunkListen>,
        /// Thunk'ed compile fn,
        /// 
        compile: Option<&'a ThunkCompile>,
    }
    "#,
        )
        .unwrap();
    
        let ts = _derive_load(ts);
    
        assert_eq!("pub type TestFormat < 'a > = (& 'a specs :: ReadStorage < 'a , Identifier > , & 'a specs :: ReadStorage < 'a , Properties > , specs :: join :: MaybeJoin < & 'a ReadStorage < 'a , Block >> , specs :: join :: MaybeJoin < & 'a ReadStorage < 'a , Root >> , specs :: join :: MaybeJoin < & 'a ReadStorage < 'a , ThunkCall >> , specs :: join :: MaybeJoin < & 'a ReadStorage < 'a , ThunkBuild >> , specs :: join :: MaybeJoin < & 'a ReadStorage < 'a , ThunkUpdate >> , specs :: join :: MaybeJoin < & 'a ReadStorage < 'a , ThunkListen >> , specs :: join :: MaybeJoin < & 'a ReadStorage < 'a , ThunkCompile >>) ; # [derive (specs :: SystemData)] pub struct TestSystemData < 'a > { entities : Entities < 'a > , identifier_storage : specs :: ReadStorage < 'a , Identifier > , properties_storage : specs :: ReadStorage < 'a , Properties > , block_storage : specs :: ReadStorage < 'a , Block > , root_storage : specs :: ReadStorage < 'a , Root > , call_storage : specs :: ReadStorage < 'a , ThunkCall > , build_storage : specs :: ReadStorage < 'a , ThunkBuild > , update_storage : specs :: ReadStorage < 'a , ThunkUpdate > , listen_storage : specs :: ReadStorage < 'a , ThunkListen > , compile_storage : specs :: ReadStorage < 'a , ThunkCompile > } impl < 'a > Load for Test < 'a > { type Layout = TestFormat < 'a > ; fn load ((identifier , properties , block , root , call , build , update , listen , compile) : < Self :: Layout as specs :: Join > :: Type) -> Self { Self { identifier , properties , block , root , call , build , update , listen , compile } } } impl < 'a > Provider < 'a , TestFormat < 'a >> for TestSystemData < 'a > { fn provide (& 'a self) -> TestFormat < 'a > { (& self . identifier_storage , & self . properties_storage , self . block_storage . maybe () , self . root_storage . maybe () , self . call_storage . maybe () , self . build_storage . maybe () , self . update_storage . maybe () , self . listen_storage . maybe () , self . compile_storage . maybe ()) } } impl < 'a > AsRef < Entities < 'a >> for TestSystemData < 'a > { fn as_ref (& self) -> & Entities < 'a > { & self . entities } }", ts.to_string().as_str());
    
        let test_ident = LitStr::new("test", Span::call_site());
        let tokens = quote! {
            [#test_ident]
        };
    
        println!("{}", tokens);
    }
    
}
