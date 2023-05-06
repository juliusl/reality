use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote_spanned;
use syn::ExprLit;
use syn::Lit;
use syn::parse_macro_input;
use syn::Item;
use syn::ItemEnum;
use syn::LitStr;
mod struct_data;
use struct_data::StructData;
mod apply_framework;
mod struct_field;
use apply_framework::ApplyFrameworkMacro;
mod thunk;
use syn::spanned::Spanned;
use thunk::ThunkMacro;

mod enum_data;
use enum_data::InterpolationExpr;

/// Add's namespace alias for macros to be used inside of the reality crate,
///
#[proc_macro]
pub fn internal_use(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    quote::quote! {
        mod reality {
            pub mod v2 {
                pub use crate::v2::*;
            }
        }
    }
    .into()
}

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
    let struct_data = parse_macro_input!(input as StructData);

    struct_data.load_trait().into()
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
#[proc_macro_derive(Config, attributes(config, root, ext, compile))]
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

/// Derives Runmd trait,
///
/// Note: In order to derive this trait, the type must also derive `Component and Clone`.
///
#[proc_macro_derive(Runmd, attributes(compile, config, block, root, ext))]
pub fn derive_runmd(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let struct_data = parse_macro_input!(input as StructData);

    struct_data.runmd_trait().into()
}

/// Applies config_queue of all framework components to all builds, generating an action buffer for each entity in the queue.
///
#[proc_macro]
pub fn apply_framework(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let apply_framework = parse_macro_input!(input as ApplyFrameworkMacro);

    apply_framework.apply_framework_expr().into()
}

/// Generates support code for a thunk type,
///
/// Example,
///
/// ```
/// thunk! MyTrait {
///     fn my_trait(&self) -> () {
///         ()
///     }
/// }
/// ```
///
/// Generates code like,
///
/// ```
/// pub trait MyTrait {
///     fn my_trait(&self) -> () {
///         ()
///     }
/// }
///
/// pub type ThunkMyTrait = Thunk<Arc<dyn MyTrait>>;
///
/// pub thunk_mytrait(mytrait: impl MyTrait + 'static) -> ThunkMyTrait {
///     ...
/// }
///
/// impl<T: MyTrait + Send + Sync> MyTrait for Thunk<T> {
///     fn my_trait(&self) -> () {
///         &self.my_trait()
///     }
/// }
/// ```
///
#[proc_macro_attribute]
pub fn thunk(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let thunk_macro = parse_macro_input!(input as ThunkMacro);

    thunk_macro.trait_impl().into()
}

/// Generates structs for enum fields that use an #[interpolate(..)] attribute,
///
#[proc_macro_attribute]
pub fn patterns(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item_enum = parse_macro_input!(input as ItemEnum);
    let name = &item_enum.ident;
    let generics = &item_enum.generics;
    let vis = &item_enum.vis;

    let interpolations = item_enum.variants.iter().filter_map(|variant| {
        variant
            .attrs
            .iter()
            .find(|a| a.path().is_ident("interpolate"))
            .map(|a| (a, variant.clone()))
    });

    let variants = interpolations.clone().filter_map(|(attr, variant)| {
        if let Some(expr) = attr.parse_args::<LitStr>().ok() {
            let name = variant.ident.clone();
            let expr = InterpolationExpr { name, expr }.signature_struct();
            Some(quote_spanned! {variant.span()=>
                #expr
            })
        } else {
            None
        }
    });

    let variant_matches = interpolations.clone().filter_map(|(attr, variant)| {
        if let Some(expr) = attr.parse_args::<LitStr>().ok() {
            let expr = InterpolationExpr {
                name: variant.ident.clone(),
                expr,
            }
            .impl_expr(name.clone(), false);
            Some(quote_spanned! {variant.span()=>
                #expr
            })
        } else {
            None
        }
    });
    let variant_matches = quote::quote! {
        #( #variant_matches )*
    };

    quote::quote! {
        #[derive(Debug, Clone)]
        #vis enum #name #generics {
            #( #variants ),*
        }

        impl #generics reality::v2::GetMatches for #name #generics {
            fn get_match(ident: &reality::v2::prelude::Identifier) -> Vec<#name #generics> {
                let mut matches = vec![];

                #variant_matches

                matches
            }
        }
    }
    .into()
}

/// Parses doc comments and generates a compile fn,
/// 
#[proc_macro_attribute]
pub fn parse_docs(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = parse_macro_input!(input as Item);

    let compile_fn = parse_runmd_from_docs(&item);

    quote::quote! {
        #item

        #compile_fn
    }.into()
}

/// Parses doc comments into a fn for 
/// 
fn parse_runmd_from_docs(item: &Item) -> TokenStream {
    let (attrs, name, vis) = match &item {
        Item::Type(i) => (&i.attrs, &i.ident, &i.vis),
        Item::Trait(i) => (&i.attrs, &i.ident, &i.vis),
        Item::Struct(i) => (&i.attrs, &i.ident, &i.vis),
        Item::Static(i) => (&i.attrs, &i.ident, &i.vis),
        Item::Const(i) => (&i.attrs, &i.ident, &i.vis),
        Item::Enum(i) => (&i.attrs, &i.ident, &i.vis),
        Item::Fn(i) => (&i.attrs, &i.sig.ident, &i.vis),
        _ => todo!(),
    };

    let mut lines = vec![];
    
    for line in attrs
        .iter()
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(nv) => Some(nv),
            _ => None,
        })
        .filter(|m| m.path.is_ident("doc"))
        .filter_map(|m| match &m.value {
            syn::Expr::Lit(ExprLit { lit: Lit::Str(line), ..}) => {
                Some(line.value())
            }
            _ => None
        })
    {
        lines.push(line);
    }

    let lines = lines.iter().map(|l| {
        let lit = LitStr::new(l, Span::call_site());
        quote::quote! {
            .parse_line(#lit)?
        }
    });

    let fn_ident = format_ident!("compile_runmd_{}", name.to_string().to_lowercase());
    quote_spanned! {name.span()=>
        /// Generated from documentation,
        /// 
        #vis fn #fn_ident(compiler: &mut reality::v2::prelude::Compiler) -> Result<Entity> {
            use reality::v2::prelude::Parser;

            let _ = Parser::new()
                #( #lines )*
                .parse("", compiler)?;

            compiler.compile()
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use crate::parse_runmd_from_docs;
    use crate::thunk::ThunkMacro;
    use crate::thunk::ThunkTraitFn;
    use crate::StructData;
    use proc_macro2::Ident;
    use proc_macro2::Span;
    use proc_macro2::TokenStream;
    use quote::format_ident;
    use quote::quote;
    use quote::ToTokens;
    use syn::ext::IdentExt;
    use syn::parse::Parse;
    use syn::parse2;
    use syn::punctuated::Punctuated;
    use syn::token::Mut;
    use syn::Attribute;
    use syn::Data;
    use syn::DeriveInput;
    use syn::Expr;
    use syn::Fields;
    use syn::Item;
    use syn::Lifetime;
    use syn::LitStr;
    use syn::Path;
    use syn::Token;
    use syn::Visibility;

    #[test]
    fn test_include_docs_attr() {
        let ts = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
            r#"
            /// Doc comment
            /// 
            /// ```runmd
            /// + .symbol Example
            /// ```
            /// 
            pub fn main() {
            }
    "#,
        )
        .unwrap();

        let item = parse2::<Item>(ts).unwrap();

        parse_runmd_from_docs(&item);
    }

    #[test]
    fn test_compile_thunk_derive_expr() {
        let ts = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
            r#"
#[compile(Test(|e| e.testa))]
struct Test {
}       
"#,
        )
        .unwrap();

        let _input = parse2::<DeriveInput>(ts).unwrap();

        // for a in input.attrs.iter() {
        //     a.parse_nested_meta(|nested| {

        //         let expr: Expr = nested.input.parse().unwrap();
        //         if let Expr::Paren(expr) = &expr {
        //             if let Expr::Closure(expr) = *expr.expr {

        //             }
        //         } else {
        //             Err(nested.error("Expecting a closure like this `|e| e.fn_name`"))
        //         }
        //     }).unwrap();
        // }
    }

    #[test]
    fn test_thunk_macro() {
        let ts = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
            r#"
            /// Doc comment
            pub trait MyTrait 
            where
                Self: Clone
            {
                /// test
                fn test(&self) -> Result<Properties>;

                fn test2() -> String {
                    String::new()
                }

                #[skip]
                fn my_trait() -> Result<Test>;
            }
    "#,
        )
        .unwrap();

        let input = parse2::<ThunkMacro>(ts).unwrap();

        println!("{:#}", input.trait_impl());
        println!("{:#}", input.impl_dispatch_exprs());
    }

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
    #[compile(ThunkCompile, ThunkCall)]
    struct Test {
        /// This is a name
        name: String,
        /// These are rules
        rules: Vec<String>,
        /// RUST_LOG env variable,
        #[config(rename = "RUST_LOG", ext = plugin.list)]
        rust_log: String,
        /// This is a plugin
        #[ext]
        plugin: Plugin,
        /// This is an event
        #[root]
        event: Event,
    }
    "#,
        )
        .unwrap();

        let input = parse2::<StructData>(ts).unwrap();
        // println!("{:#}", input.config_trait());
        // println!("{:#}", input.runmd_trait());
        // println!("{:#}", input.extensions_enum());
        // println!("{:#}", input.visit_trait());
        println!("{:#}", input.runmd_trait());
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

        let input = parse2::<StructData>(ts).unwrap();

        let ts = input.load_trait();

        assert_eq!("use specs :: prelude :: * ; pub type TestFormat < 'a > = (& 'a specs :: ReadStorage < 'a , Identifier > , & 'a specs :: ReadStorage < 'a , Properties > , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , Block >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , Root >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkCall >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkBuild >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkUpdate >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkListen >> , specs :: join :: MaybeJoin < & 'a specs :: ReadStorage < 'a , ThunkCompile >>) ; # [derive (specs :: SystemData)] pub struct TestSystemData < 'a > { entities : specs :: Entities < 'a > , identifier_storage : specs :: ReadStorage < 'a , Identifier > , properties_storage : specs :: ReadStorage < 'a , Properties > , block_storage : specs :: ReadStorage < 'a , Block > , root_storage : specs :: ReadStorage < 'a , Root > , call_storage : specs :: ReadStorage < 'a , ThunkCall > , build_storage : specs :: ReadStorage < 'a , ThunkBuild > , update_storage : specs :: ReadStorage < 'a , ThunkUpdate > , listen_storage : specs :: ReadStorage < 'a , ThunkListen > , compile_storage : specs :: ReadStorage < 'a , ThunkCompile > } impl < 'a > reality :: state :: Load for Test < 'a > { type Layout = TestFormat < 'a > ; fn load ((identifier , properties , block , root , call , build , update , listen , compile) : < Self :: Layout as specs :: Join > :: Type) -> Self { Self { identifier , properties , block , root , call , build , update , listen , compile } } } impl < 'a > reality :: state :: Provider < 'a , TestFormat < 'a >> for TestSystemData < 'a > { fn provide (& 'a self) -> TestFormat < 'a > { (& self . identifier_storage , & self . properties_storage , self . block_storage . maybe () , self . root_storage . maybe () , self . call_storage . maybe () , self . build_storage . maybe () , self . update_storage . maybe () , self . listen_storage . maybe () , self . compile_storage . maybe ()) } } impl < 'a > AsRef < specs :: Entities < 'a >> for TestSystemData < 'a > { fn as_ref (& self) -> & specs :: Entities < 'a > { & self . entities } }", ts.to_string().as_str());
    }
}
