use std::ops::Deref;

use proc_macro2::TokenStream;
use quote::__private::ext::RepToTokensExt;
use quote::format_ident;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse2;
use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::Expr;
use syn::FnArg;
use syn::Generics;
use syn::Ident;
use syn::Path;
use syn::ReturnType;
use syn::Token;
use syn::Type;
use syn::TypeParam;
use syn::TypeTuple;
use syn::Visibility;
use syn::WhereClause;

pub(crate) struct ThunkMacroArguments {
    attributes: Vec<Attribute>,
    visibility: Visibility,
    generics: Generics,
    type_path: Path,
    exprs: Vec<ThunkTraitFn>,
}

impl ThunkMacroArguments {
    /// Generates thunk method impl for a trait definition,
    ///
    pub(crate) fn trait_impl(&self) -> TokenStream {
        let attributes = self.attributes.iter().map(|a| {
            quote_spanned! {a.span()=>
                #a
            }
        });
        let attributes = quote! {
            #( #attributes )*
        };
        let vis = &self.visibility;
        let name = &self.type_path;
        let exprs = self.exprs.iter().map(|e| e.trait_fn());
        let exprs = quote! {
            #( #exprs )*
        };

        let thunk_trait_type_expr = self.thunk_trait_convert_fn_expr();
        let thunk_trait_impl_expr = self.thunk_trait_impl_expr();
        let arc_impl_expr = self.arc_impl_expr();

        quote_spanned! {vis.span()=>
            #attributes
            #vis trait #name
            where
                Self: Send + Sync,
            { #exprs }

            #arc_impl_expr

            #thunk_trait_type_expr

            #thunk_trait_impl_expr
        }
    }

    pub(crate) fn thunk_trait_impl_expr(&self) -> TokenStream {
        let name = &self.type_path;
        let exprs = self
            .exprs
            .iter()
            .filter(|f| !f.default)
            .map(|e| e.thunk_impl_fn());
        let exprs = quote! {
            #( #exprs )*
        };

        let async_trait = if self.exprs.iter().any(|e| e.is_async) {
            quote! {#[async_trait]}
        } else {
            quote! {}
        };

        quote! {
            #async_trait
            impl<T: #name + Send + Sync> #name for reality::v2::Thunk<T>
            {
                #exprs
            }
        }
    }

    pub(crate) fn arc_impl_expr(&self) -> TokenStream {
        let name = &self.type_path;
        let exprs = self
            .exprs
            .iter()
            .filter(|f| !f.default)
            .map(|e| e.arc_impl_fn());
        let exprs = quote! {
            #( #exprs )*
        };

        let async_trait = if self.exprs.iter().any(|e| e.is_async) {
            quote! {#[async_trait]}
        } else {
            quote! {}
        };

        quote! {
            #async_trait
            impl #name for std::sync::Arc<dyn #name> {
                #exprs
            }
        }
    }

    /// Generates a fn to convert an implementing type into a Thunk component,
    ///
    pub(crate) fn thunk_trait_convert_fn_expr(&self) -> TokenStream {
        let name = &self.type_path;
        let ident = name.get_ident().expect("should have an ident");
        let thunk_type_name = format_ident!("Thunk{}", ident);
        let vis = &self.visibility;

        let convert_fn_name = format_ident!("thunk_{}", ident.to_string().to_lowercase());

        quote! {
            #vis type #thunk_type_name = reality::v2::Thunk<std::sync::Arc<dyn #name>>;

            #vis fn #convert_fn_name(t: impl #name + 'static) -> #thunk_type_name {
                reality::v2::Thunk {
                    thunk: std::sync::Arc::new(t)
                }
            }
        }
    }
}

impl Parse for ThunkMacroArguments {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attributes = Attribute::parse_outer(input)?;
        let visibility = input.parse::<Visibility>()?;
        input.parse::<Token![trait]>()?;
        let type_path = input.parse::<Path>()?;
        let generics = input.parse::<Generics>().ok().unwrap_or_default();
        let expr = input.parse::<Expr>()?;
        let mut exprs = vec![];

        match expr {
            Expr::Block(block) => {
                for expr in block
                    .block
                    .stmts
                    .iter()
                    .map(|s| parse2::<ThunkTraitFn>(s.to_token_stream())) {
                    let expr = expr.map_err(|e| input.error(e))?;
                    exprs.push(expr);
                }
            }
            _ => {}
        }

        Ok(Self {
            attributes,
            visibility,
            generics,
            type_path,
            exprs,
        })
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct ThunkTraitFn {
    name: Ident,
    attributes: Vec<Attribute>,
    return_type: ReturnType,
    fields: Vec<syn::FnArg>,
    traitfn: syn::TraitItemFn,
    default: bool,
    is_async: bool,
    skip: bool,
}

impl ThunkTraitFn {
    pub(crate) fn trait_fn(&self) -> TokenStream {
        let trait_fn = &self.traitfn;
        let attrs = self.attributes.iter().map(|a| {
            quote_spanned! {a.span()=>
                #a
            }
        });
        let attrs = quote! {
            #( #attrs )*
        };
        quote! {
            #attrs
            #trait_fn
        }
    }

    pub(crate) fn thunk_impl_fn(&self) -> TokenStream {
        if !self.default {
            let name = &self.name;
            let fields = self.fields.iter().map(|f| {
                quote_spanned! {f.span()=>
                    #f
                }
            });
            let fields = quote! {
                #( #fields ),*
            };

            let input = self
                .fields
                .iter()
                .filter_map(|f| match f {
                    FnArg::Receiver(_) => None,
                    FnArg::Typed(typed) => Some(typed.pat.clone()),
                })
                .map(|f| {
                    quote_spanned! {f.span()=>
                        #f
                    }
                });
            let input = quote! {
                #( #input ),*
            };

            let return_type = &self.return_type;

            if self.is_async {
                quote! {
                    async fn #name(#fields) #return_type {
                        self.thunk.#name(#input).await
                    }
                }
            } else {
                quote! {
                    fn #name(#fields) #return_type {
                        self.thunk.#name(#input)
                    }
                }
            }
        } else {
            quote! {}
        }
    }

    pub(crate) fn arc_impl_fn(&self) -> TokenStream {
        if !self.default {
            let name = &self.name;
            let fields = self.fields.iter().map(|f| {
                quote_spanned! {f.span()=>
                    #f
                }
            });
            let fields = quote! {
                #( #fields ),*
            };

            let input = self
                .fields
                .iter()
                .filter_map(|f| match f {
                    FnArg::Receiver(_) => None,
                    FnArg::Typed(typed) => Some(typed.pat.clone()),
                })
                .map(|f| {
                    quote_spanned! {f.span()=>
                        #f
                    }
                });
            let input = quote! {
                #( #input ),*
            };

            let return_type = &self.return_type;

            if self.is_async {
                quote! {
                    async fn #name(#fields) #return_type {
                        self.deref().#name(#input).await
                    }
                }
            } else {
                quote! {
                    fn #name(#fields) #return_type {
                        std::ops::Deref::deref(self).#name(#input)
                    }
                }
            }
        } else {
            quote! {}
        }
    }
}

impl Parse for ThunkTraitFn {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let traitfn = input.parse::<syn::TraitItemFn>()?;

        let mut skip = false;
        for attr in traitfn.attrs.iter() {
            if attr.path().is_ident("skip") {
                skip = true;
            }
        }

        let name = traitfn.sig.ident.clone();
        let is_async = traitfn.sig.asyncness.is_some();
        let fields: Vec<FnArg> = traitfn.sig.inputs.iter().cloned().collect();
        let return_type = &traitfn.sig.output;
        let default = traitfn.default.is_some();
        let where_clause = &traitfn.sig.generics.where_clause;

        if !skip && fields.len() > 2 {
            return Err(input.error( "Currently, only 2 inputs for thunk trait fn's are supported"));
        }

        match fields.first() {
            Some(FnArg::Receiver(r)) => {
                assert!(r.reference.is_some(), "must be either &self or &mut self");
            }
            _ if !default && !skip => {
                return Err(input.error(
                    "Trait fn's must have a receiver type, i.e. `fn (self)`",
                ))
            }
            _ => {}
        }

        match fields.iter().skip(1).take(1).next() {
            Some(f) => {
                
            },
            None => {
                
            },
        }

        match &return_type {
            ReturnType::Default if !skip => {
                return Err(input.error(
                    "Must return a reality::Result<T> from a thunk trait fn",
                ));
            }
            ReturnType::Type(_, ty) => {
                match ty.deref() {
                    Type::Path(p) => {
                        let segments = &p.path.segments;

                    },
                    _ => return Err(input.error("Return type must be a variant of reality::Result<T>")),
                }
            }
            _ => {

            }
        }

        return Ok(Self {
            name,
            return_type: return_type.clone(),
            attributes: vec![],
            fields,
            traitfn,
            default,
            is_async,
            skip,
        });
    }
}

/// Parses a return type for a thunk trait fn that match one of the following forms,
///
/// -  `reality::Result<()>`
/// -  `reality::Result<Entity>`
/// -  `reality::Result<T> where T: Component + Send + Sync`
/// -  `reality::Result<T> where T: <Self::Layout as Join>::Type`
///
struct ThunkTraitFnReturnType {
    /// Inner Type, i.e.
    ///
    ///  ```
    /// Result<T>
    ///         ^
    /// ```
    ty: Type,
    ///
    ///
    where_clause: Option<WhereClause>,
}

impl ThunkTraitFnReturnType {
    /// Generates dispatch expression to prepare a return type,
    ///
    pub fn dispatch_output_expr(&self) -> TokenStream {
        quote_spanned! {self.ty.span()=>
            .transmute::<Properties>()
            .result()
        }
    }

    /// Generates an expression for handling an async dispatch,
    ///
    pub fn async_dispatch_output_expr(&self) -> TokenStream {
        quote_spanned! {self.ty.span()=>
            .await
            .disable_async()
            .transmute::<Properties>()
            .result()
        }
    }
}

impl Parse for ThunkTraitFnReturnType {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let p = input.parse::<ReturnType>()?;
        match p {
            ReturnType::Type(_, ty) => match *ty {
                Type::Path(p) => {
                    let p = &p.path;
                    for r in p.segments.iter() {
                        match &r.arguments {
                            syn::PathArguments::None => {
                                if r.ident.to_string() != "reality" {
                                    return Err(syn::Error::new(
                                        input.span(),
                                        format!("Must have generic parameter"),
                                    ));
                                } else {
                                    continue;
                                }
                            }
                            syn::PathArguments::AngleBracketed(args) => {
                                if let Some(ty) = args
                                    .args
                                    .iter()
                                    .filter_map(|p| match p {
                                        syn::GenericArgument::Type(t) => Some(t),
                                        _ => None,
                                    })
                                    .cloned()
                                    .next()
                                {
                                    return Ok(Self {
                                        ty,
                                        where_clause: input.parse::<syn::WhereClause>().ok(),
                                    });
                                } else {
                                    return Err(syn::Error::new(
                                            input.span(),
                                            "Return type must include generic parameter ex: `-> reality::Result<T>`",
                                        ));
                                }
                            }
                            syn::PathArguments::Parenthesized(_) => {
                                return Err(syn::Error::new(
                                    input.span(),
                                    "Return type must be a variant of reality::Result<T>",
                                ))
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        Err(syn::Error::new(
            input.span(),
            "Return type must be a variant of reality::Result<T>",
        ))
    }
}

/// This module is for generating dispatch sequences for thunk types,
///
#[allow(dead_code)]
pub mod dispatch_ref_stmts {
    use proc_macro2::Ident;
    use proc_macro2::TokenStream;
    use quote::quote;
    use quote::quote_spanned;
    use syn::Type;

    /// Returns an async dispatch sequence for stmts,
    ///
    pub fn async_dispatch_seq(
        thunk_type: &Ident,
        stmts: impl Iterator<Item = TokenStream>,
    ) -> TokenStream {
        let begin_async = quote_spanned! {thunk_type.span()=>
            .enable_async()
            .transmute::<#thunk_type>()
        };

        let stmts = stmts.map(|s| {
            quote! {
                #s
                .await
            }
        });

        let async_body = quote! {
            #( #stmts )*
            .disable_async()
            .transmute::<Properties>()
            .result()
        };

        quote_spanned! {thunk_type.span()=>
            #begin_async
            #async_body
        }
    }

    /// Generates code for a synchronous dispatch seq,
    ///
    pub fn dispatch_seq(
        thunk_type: &Ident,
        stmts: impl Iterator<Item = TokenStream>,
    ) -> TokenStream {
        let begin = quote_spanned! {thunk_type.span()=>
            .transmute::<#thunk_type>()
        };

        let stmts = stmts.map(|s| {
            quote! {
                #s
            }
        });

        let body = quote! {
            #( #stmts )*
            .transmute::<Properties>()
            .result()
        };

        quote_spanned! {thunk_type.span()=>
            #begin
            #body
        }
    }

    pub fn recv_with_closure(recv: &Type, with: &Type, fn_ident: &Ident) -> TokenStream {
        quote_spanned! {fn_ident.span()=>
            |recv: #recv, with: #with| {
                recv.#fn_ident(with)
            }
        }
    }

    pub fn recv_closure(recv: &Type, fn_ident: &Ident) -> TokenStream {
        quote_spanned! {fn_ident.span()=>
            |recv: #recv| {
                recv.#fn_ident()
            }
        }
    }

    pub fn recv_with_async_closure(recv: &Type, with: &Type, fn_ident: &Ident) -> TokenStream {
        quote_spanned! {fn_ident.span()=>
            |recv: #recv, with: #with| {
                let recv = recv.clone();
                let with = with.clone();
                async move {
                    recv.#fn_ident(with).await
                }
            }
        }
    }

    pub fn recv_async_closure(recv: &Type, fn_ident: &Ident) -> TokenStream {
        quote_spanned! {fn_ident.span()=>
            |recv: #recv| {
                let recv = recv.clone();
                async move {
                    recv.#fn_ident().await
                }
            }
        }
    }

    /// Generates a `.transmute::<Type>` statement,
    ///
    pub fn transmute_stmt(ty: &Ident) -> TokenStream {
        quote_spanned!(ty.span()=>{
            transmute::<#ty>()
        })
    }

    /// Generates a `map` statement,
    ///
    pub fn map_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .map(#closure)
        }
    }

    /// Generates a `map_into` statement,
    ///
    pub fn map_into_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .map_into(#closure)
        }
    }

    /// Generates a `map_into_with` statement,
    ///
    pub fn map_into_with_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .map_into_with(#closure)
        }
    }

    /// Generates a `map_with` statement,
    ///
    pub fn map_with_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .map_with(#closure)
        }
    }
}

#[allow(unused_imports)]
mod tests {
    use proc_macro2::TokenStream;
    use quote::ToTokens;
    use syn::{parse2, Path, PredicateType};

    use super::ThunkTraitFnReturnType;

    #[test]
    fn test_thunk_trait_fn_return_type() -> Result<(), Box<dyn std::error::Error>> {
        let ts = "-> reality::Result<T> where T: Component".parse::<TokenStream>()?;
        let ts = parse2::<ThunkTraitFnReturnType>(ts)?;
        // dbg!(ts.dispatch_output_expr());

        match ts.where_clause {
            Some(w) => {
                dbg!(w.to_token_stream());
                for p in w.predicates.iter() {
                    match p {
                        syn::WherePredicate::Lifetime(_) => continue,
                        syn::WherePredicate::Type(ty) => {
                            dbg!(ty.to_token_stream());
                            dbg!(ty.bounded_ty.to_token_stream());
                            if ty.bounded_ty.to_token_stream().to_string()
                                == ts.ty.to_token_stream().to_string()
                            {
                                assert!(ty.bounds.iter().any(|b| match b {
                                    syn::TypeParamBound::Trait(t) => {
                                        t.path.is_ident("Component")
                                            || t.path.is_ident("specs::Component")
                                    }
                                    _ => false,
                                }));

                                break;
                            } else {
                                continue;
                            }
                        }
                        _ => {}
                    }
                }
            }
            None => assert!(false),
        }

        Ok(())
    }
}
