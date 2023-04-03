use std::ops::Deref;
use std::str::FromStr;

use proc_macro2::{TokenStream, Ident};
use quote::{quote, quote_spanned, format_ident};
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::token::Enum;
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Fields, GenericParam, Generics, Index, parse2, Type, Token,
};

///
/// 
#[proc_macro_derive(Config)]
pub fn derive_config(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);


    todo!()
}

/// Derives Load trait implementation,
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
/// The generated code would look like, 
/// 
/// ```
/// pub type LoadFormat<'a> = (
///     &'a ReadStorage<'a, Idenfier>,
/// )
/// 
/// impl<'a> Load for A<'a> {
///     type Layout = LoadFormat<'a>;
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
    /*
    - Need to map fields
     */
    // Derive Format
    // &'a #Type

    let ident = input.ident;
    let format_ident = format_ident!("{}Format", ident);

    let types = load_format(&input.data);
    let idents = load_idents(&input.data);
    let format = quote! {
        pub type #format_ident<'a> = ( #types );

        impl<'a> Load for #ident<'a> {
            type Layout = #format_ident<'a>;

            fn load((#idents): <Self::Layout as specs::Join>::Type) -> Self {
                Self { #idents }
            }
        }
    };

    TokenStream::from(format)
}

fn load_format(data: &Data) -> TokenStream {
    match &data {
        Data::Struct(_data) => {
            match &_data.fields {
                Fields::Named(named) => {
                    let map = named.named.iter().map(|n| {
                        match &n.ty {
                            syn::Type::Reference(reference) => {
                                let elem = &reference.elem;

                                if let Some(_) = reference.mutability {
                                    quote!{
                                        &'a mut WriteStorage <'a, #elem >
                                    }
                                } else {
                                    quote!{
                                        &'a ReadStorage <'a, #elem >
                                    }
                                }
                            },
                            _ => {
                                unimplemented!()
                            },
                        }
                    });

                    quote! {
                        #( #map ),*
                    }
                },
                _ => unimplemented!()
            }
        },
        _ => unimplemented!(),
    }
}

fn load_idents(data: &Data) -> TokenStream {
    match &data {
        Data::Struct(_data) => {
            match &_data.fields {
                Fields::Named(named) => {
                    let map = named.named.iter().filter_map(|n| {
                        n.ident.as_ref()
                    }).map(|i| {
                        quote!( #i )
                    });

                    quote! {
                        #( #map ),*
                    }
                },
                _ => unimplemented!()
            }
        },
        _ => unimplemented!(),
    }
}

#[test]
fn test_load_format() {
    let ts = proc_macro2::TokenStream::from_str(r#"
struct Test {
    identifier: &'a Identifier,
    properties: &'a mut Properties,
}
"#).unwrap();

    let ts = _derive_load(ts);

    assert_eq!("pub type TestFormat < 'a > = (& 'a ReadStorage < 'a , Identifier > , & 'a mut WriteStorage < 'a , Properties >) ; impl < 'a > Load for Test < 'a > { type Layout = TestFormat < 'a > ; fn load ((identifier , properties) : < Self :: Layout as specs :: Join > :: Type) -> Self { Self { identifier , properties } } }", ts.to_string().as_str());
}

/// Derives Provider<'a, T> implementations for all valid Storage types,
/// 
/// Example Generated:
/// 
/// ```
/// #[derive(Provider)]
/// struct Data<'a> {
///     identifier: ReadStorage<'a, Identifier>,
///     ...
/// }
/// 
/// impl<'a> Provider<'a, ReadStorage<'a, Identifier>> for Data<'a> {
///     fn provide(&'a self) -> ReadStorage<'a, Identifier> {
///         self.identifier
///     }
/// }
/// 
/// impl<'a> Provider<'a, MaybeJoin<&'a ReadStorage<'a, Identifier>>> for Data<'a> {
///     fn provide(&'a self) -> MaybeJoin<&'a ReadStorage<'a, Identifier>> {
///         self.identifier.maybe()
///     }
/// }
/// ```
/// 
#[proc_macro_derive(Provider)]
pub fn derive_provider(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    todo!()
}

/// Parses a list of types that implement Load as part of deriving Provider,
/// 
/// load(TypeA, TypeB, TypeC)
/// 
/// Usage:
/// 
/// ```norun
/// #[derive(Load)]
/// struct Object<'a> {
/// ...
/// }
/// 
/// #[derive(Provider, SystemData)]
/// #[load(Object)]
/// struct Compiled<'a> {
/// ...
/// }
/// ```
/// 
struct LoadArgs {
    /// Types that must implement Load
    /// 
    types: Vec<syn::Type>,
}

impl Parse for LoadArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        todo!()
    }
}

/// Derives a provide implementation for types that implement Load,
/// 
/// Example Generated: 
/// ```
/// #[derive(Load)]
/// struct A<'a> {
///   identifier: &'a Identifier
///   ...
/// }
/// 
/// #[derive(Provider)]
/// #[load(A)]
/// struct Alphabet<'a> {
///   identifier: ReadStorage<'a, Identifier>,
///   ...
/// }
/// 
/// impl<'a> Provider<'a, AFormat<'a>> for Alphabet<'a> {
///     fn provide(&'a self) -> AFormat<'a> {
///         (
///             <Self as Provider<'a, ReadStorage<Identifier<'a>>>>::provide(self),
///         )
///     } 
/// }
/// ```
/// 
/// 
#[proc_macro_attribute]
pub fn load(args: proc_macro::TokenStream, _: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let args = parse_macro_input!(args as LoadArgs);



    todo!()
}