use std::ops::Deref;

use proc_macro2::TokenStream;
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
    pub(crate) fn trait_expr(&self) -> TokenStream {
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

        let thunk_trait_type_expr = self.thunk_trait_type_expr();
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

    pub(crate) fn thunk_trait_type_expr(&self) -> TokenStream {
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
                exprs = block
                    .block
                    .stmts
                    .iter()
                    .filter_map(|s| parse2::<ThunkTraitFn>(s.to_token_stream()).ok())
                    .collect();
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

    fn async_dispatch(&self, trait_ident: &Ident) -> TokenStream {
        let fn_ident = &self.name;
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
        let input = quote_spanned! {self.name.span()=>
            #( #input ),*
        };
        let thunk_type = format_ident!("Thunk{}", trait_ident);

        match &self.return_type {
            ReturnType::Default => {
                quote_spanned! {trait_ident.span()=>
                    compile_error!("Must return at least a reality::Result<()>");
                };
            }
            ReturnType::Type(_, ty) => match ty.deref() {
                syn::Type::Verbatim(v) => {
                    let v = v.to_token_stream();
                    if let Ok(retty) = parse2::<ThunkTraitFnReturnType>(v) {}
                }
                _ => {}
            },
        }

        quote! {
        #[async_trait]
        impl<T: #trait_ident + Send + Sync> reality::v2::AsyncDispatch for Arc<T> {
            async fn async_dispatch<'a, 'b>(
                &'a self,
                build_ref: reality::v2::DispatchRef<'b, reality::v2::Properties>,
            ) -> reality::v2::DispatchResult<'b> {
                build_ref
                    .enable_async()
                    .transmute::<#thunk_type>()
                    .map_into::<reality::v2::Properties, _>(|r| {
                        let r = r.clone();
                        async move { r.#fn_ident(#input).await }
                    })
                    .await
                    .disable_async()
                    .result()
            }
        }
        }
    }
}

impl Parse for ThunkTraitFn {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        println!("{}", input.to_string());

        let traitfn = input.parse::<syn::TraitItemFn>()?;
        let name = traitfn.sig.ident.clone();
        let is_async = traitfn.sig.asyncness.is_some();
        let fields: Vec<FnArg> = traitfn.sig.inputs.iter().cloned().collect();
        let return_type = traitfn.sig.output.clone();
        let default = traitfn.default.is_some();

        match fields.first() {
            Some(FnArg::Receiver(r)) => {
                assert!(r.mutability.is_none(), "must be an immutable reference");
                assert!(r.reference.is_some(), "must be an immutable reference");
            }
            _ if !default => {
                panic!("First argument must be a receiver type")
            }
            _ => {}
        }

        return Ok(Self {
            name,
            return_type,
            attributes: vec![],
            fields,
            traitfn,
            default,
            is_async,
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

/// Generates DispatchRef statements,
/// 
pub mod dispatch_ref_stmts {
    use proc_macro2::TokenStream;
    use proc_macro2::Ident;
    use quote::quote_spanned;

    /// Generates a `.transmute::<Type>` statement,
    ///
    pub fn transmute_stmt(ty: &Ident) -> TokenStream {
        quote_spanned!(ty.span()=>{
            transmute::<#ty>()
        })
    }

    /// 
    /// 
    pub fn map_into_async(thunk_type: &Ident, fn_ident: &Ident, fn_input: TokenStream) -> TokenStream {
        let begin_async = quote_spanned! {thunk_type.span()=>
            .enable_async()
            .transmute::<#thunk_type>()
        };

        let async_body = quote_spanned! {fn_ident.span()=>
            .map_into::<reality::v2::Properties, _>(|r| {
                let r = r.clone();
                async move { r.#fn_ident(#fn_input).await }
            })
            .await
            .disable_async()
            .transmute::<Properties>()
            .result()
        };

        quote_spanned! {thunk_type.span()=>
            #begin_async
            #async_body
        }
    }

    ///
    /// 
    pub fn async_map_into_stmt(ty: &Ident, fn_ident: &Ident) -> TokenStream {
        let fn_stmt = quote_spanned! {fn_ident.span()=>
            let r = r.clone();
            async move { r.#fn_ident().await }
        };

        quote_spanned! {ty.span()=>
            .map_into::<#ty, _>(|r| {
                #fn_stmt
            })
        }
    }

    ///
    /// 
    pub fn map_into_stmt(ty: &Ident, fn_ident: &Ident) -> TokenStream {
        let fn_stmt = quote_spanned! {fn_ident.span()=>
            let r = r.clone();
            r.#fn_ident()
        };

        quote_spanned! {ty.span()=>
            .map_into::<#ty>(|r| {
                #fn_stmt
            })
        }
    }

    ///
    /// 
    pub fn map_into_with_stmt(ty: &Ident, fn_ident: &Ident) -> TokenStream {
        let fn_stmt = quote_spanned! {fn_ident.span()=>
            let r = r.clone();
            r.#fn_ident(w)
        };

        quote_spanned! {ty.span()=>
            .map_into_with::<#ty>(|r, w| {
                #fn_stmt
            })
        }
    }

    ///
    /// 
    pub fn map_with_stmt(ty: &Ident, fn_ident: &Ident) -> TokenStream {
        let fn_stmt = quote_spanned! {fn_ident.span()=>
            let r = r.clone();
            r.#fn_ident(w)
        };

        quote_spanned! {ty.span()=>
            .map_with::<#ty>(|r, w| {
                #fn_stmt
            })
        }
    }

    /// <Self::Layout as Join>::Type
    /// 
    pub fn join_map(ty: &Ident) -> TokenStream {
        quote_spanned! {ty.span()=>
            
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
