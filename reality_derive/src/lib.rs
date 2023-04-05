use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use syn::parse2;
use syn::parse_macro_input;
use syn::Data;
use syn::DeriveInput;
use syn::Fields;

mod struct_data;
use struct_data::StructData;

mod struct_field;
pub(crate) use struct_field::StructField;

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

                    struct_field.join_tuple_storage_type_expr()
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

                    struct_field.system_data_expr()
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

                    struct_field.system_data_ref_expr()
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

/// Derives config trait,
///
/// ```
/// #[derive(Config)]
/// struct Plugin {
///     name: String,
/// }
///
/// ```
///
/// Generates code like this,
///
/// ```
/// impl Config for Plugin {
///     fn config(&mut self, ident: &Identifier, property: &Property) {
///         match ident.subject().as_ref() {
///             "name" => {
///                 self.name = property.into();
///             }
///             _ => {}
///         }
///     }
/// }
/// ```
///
/// Also includes attributes to provide additional config,
///
/// ```
/// #[derive(Config)]
/// struct Plugin {
///     #[config(config_name)]
///     name: String,
/// }
///
/// fn config_name(ident: &Identifier, property: &Property) -> String {
///     ...
/// }
/// ```
///
/// The generated code will look like this,
///
/// ```
/// impl Config for Plugin {
///     fn config(&mut self, ident: &Identifier, property: &Property) {
///         match ident.subject().as_ref() {
///             "name" => {
///                 self.name = config_name(ident, property);
///             }
///             _ => {}
///         }
///     }
/// }
/// ```
///
#[proc_macro_derive(Config, attributes(config, root, apply))]
pub fn derive_config(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let struct_data = parse_macro_input!(input as StructData);

    struct_data.config_trait().into()
}

/// Derives Apply trait,
/// 
#[proc_macro_derive(Apply)]
pub fn derive_apply(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let struct_data = parse_macro_input!(input as StructData);

    struct_data.apply_trait().into()
}

#[allow(unused_imports)]
mod tests {
    use crate::StructData;
    use crate::_derive_load;
    use proc_macro2::Ident;
    use proc_macro2::Span;
    use proc_macro2::TokenStream;
    use quote::format_ident;
    use quote::quote;
    use quote::ToTokens;
    use syn::ext::IdentExt;
    use syn::parse::Parse;
    use syn::parse2;
    use syn::token::Mut;
    use syn::Attribute;
    use syn::Data;
    use syn::DeriveInput;
    use syn::Fields;
    use syn::Lifetime;
    use syn::LitStr;
    use syn::Token;
    use syn::Visibility;

    #[test]
    fn test_derive_apply() {
        let ts = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
            r#"
    struct Test {
        #[apply]
        name: NameConfig,
        rule: (),
        #[apply]
        call: CallConfig,
    }
    "#,
        )
        .unwrap();

        let input = parse2::<StructData>(ts).unwrap();

        println!("{:#}", input.apply_trait());
    }

    #[test]
    fn test_derive_config() {
        let ts = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
            r#"
    struct Test {
        name: String,
        rules: Vec<String>,
        #[root]
        plugin: Plugin,
        #[root]
        event: Event,
    }
    "#,
        )
        .unwrap();

        let input = parse2::<StructData>(ts).unwrap();

        println!("{:#}", input.config_trait());
    }

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

        assert_eq!("use specs :: prelude :: * ; pub type TestFormat < 'a > = (& 'a specs :: ReadStorage < 'a , Identifier > , & 'a specs :: ReadStorage < 'a , Properties > , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , Block >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , Root >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkCall >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkBuild >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkUpdate >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkListen >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkCompile >>) ; # [derive (specs :: SystemData)] pub struct TestSystemData < 'a > { entities : specs :: Entities < 'a > , identifier_storage : specs :: ReadStorage < 'a , Identifier > , properties_storage : specs :: ReadStorage < 'a , Properties > , block_storage : specs :: ReadStorage < 'a , Block > , root_storage : specs :: ReadStorage < 'a , Root > , call_storage : specs :: ReadStorage < 'a , ThunkCall > , build_storage : specs :: ReadStorage < 'a , ThunkBuild > , update_storage : specs :: ReadStorage < 'a , ThunkUpdate > , listen_storage : specs :: ReadStorage < 'a , ThunkListen > , compile_storage : specs :: ReadStorage < 'a , ThunkCompile > } impl < 'a > reality :: state :: Load for Test < 'a > { type Layout = TestFormat < 'a > ; fn load ((identifier , properties , block , root , call , build , update , listen , compile) : < Self :: Layout as specs :: Join > :: Type) -> Self { Self { identifier , properties , block , root , call , build , update , listen , compile } } } impl < 'a > reality :: state :: Provider < 'a , TestFormat < 'a >> for TestSystemData < 'a > { fn provide (& 'a self) -> TestFormat < 'a > { (& self . identifier_storage , & self . properties_storage , self . block_storage . maybe () , self . root_storage . maybe () , self . call_storage . maybe () , self . build_storage . maybe () , self . update_storage . maybe () , self . listen_storage . maybe () , self . compile_storage . maybe ()) } } impl < 'a > AsRef < specs :: Entities < 'a >> for TestSystemData < 'a > { fn as_ref (& self) -> & specs :: Entities < 'a > { & self . entities } }", ts.to_string().as_str());

        // let test_ident = LitStr::new("test", Span::call_site());
        // let tokens = quote! {
        //     [#test_ident]
        // };

        // println!("{}", tokens);
    }
}
