use std::ops::Deref;

use proc_macro2::TokenStream;
use quote::__private::ext::RepToTokensExt;
use quote::format_ident;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use syn::ItemTrait;
use syn::parse::Parse;
use syn::parse2;
use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::Expr;
use syn::FnArg;
use syn::Generics;
use syn::Ident;
use syn::PatType;
use syn::Path;
use syn::ReturnType;
use syn::Token;
use syn::Type;
use syn::TypeParam;
use syn::TypeTuple;
use syn::Visibility;
use syn::WhereClause;

pub(crate) struct ThunkMacroArguments {
    name: Ident,
    generics: Generics,
    attributes: Vec<Attribute>,
    visibility: Visibility,
    exprs: Vec<ThunkTraitFn>,
    other_exprs: Vec<syn::TraitItem>,
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
        let name = &self.name;
        let exprs = self.exprs.iter().map(|e| e.trait_fn());
        let dispatch_exprs = self.impl_dispatch_exprs();
        let exprs = quote! {
            #( #exprs )*
        };

        let other_exprs = self.other_exprs.iter();

        let thunk_trait_type_expr = self.thunk_trait_convert_fn_expr();
        let thunk_trait_impl_expr = self.thunk_trait_impl_expr();
        let arc_impl_expr = self.arc_impl_expr();

        quote_spanned! {vis.span()=>
            #attributes
            #vis trait #name
            where
                Self: Send + Sync,
            { 
                #exprs 
                
                #( #other_exprs )*
            }

            #arc_impl_expr

            #thunk_trait_type_expr

            #thunk_trait_impl_expr
        }
    }

    pub(crate) fn thunk_trait_impl_expr(&self) -> TokenStream {
        let name = &self.name;
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
        let name = &self.name;
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
        let name = &self.name;
        let thunk_type_name = format_ident!("Thunk{}", name);
        let vis = &self.visibility;

        let convert_fn_name = format_ident!("thunk_{}", name.to_string().to_lowercase());

        quote! {
            #vis type #thunk_type_name = reality::v2::Thunk<std::sync::Arc<dyn #name>>;

            #vis fn #convert_fn_name(t: impl #name + 'static) -> #thunk_type_name {
                reality::v2::Thunk {
                    thunk: std::sync::Arc::new(t)
                }
            }
        }
    }

    /// Generates a dispatch expr for trait fn,
    ///
    pub(crate) fn impl_dispatch_exprs(&self) -> TokenStream {
        let name = &self.name;
        let thunk_type_name = format_ident!("Thunk{}", name);

        let dispatch_exprs = self
            .exprs
            .iter()
            .filter(|e| !e.skip && !e.default)
            .map(|e| {
                let fn_name = &e.name;
                let fn_name = format_ident!("thunk_{}_dispatch_{}", name.to_string().to_lowercase(), fn_name);
                let stmt = e.dispatch_stmt(&thunk_type_name);

                if e.is_async {
                    let body = dispatch_ref_stmts::async_dispatch_seq(&thunk_type_name, Some(stmt).into_iter());
                    quote! {
                        pub(super) async fn #fn_name<'a>(dispatch_ref: reality::v2::DispatchRef<'a, Properties>) -> reality::v2::DispatchResult<'a> {
                            dispatch_ref
                            #body
                        }
                    }
                } else {
                    let body = dispatch_ref_stmts::dispatch_seq(&thunk_type_name, Some(stmt).into_iter());
                    quote! {
                        pub(super) fn #fn_name<'a>(dispatch_ref: reality::v2::DispatchRef<'a, Properties>) -> reality::v2::DispatchResult<'a> {
                            dispatch_ref
                            #body
                        }
                    }
                }
            });

        quote! {
            #( #dispatch_exprs )*
        }
    }
}

impl Parse for ThunkMacroArguments {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let attributes = Attribute::parse_outer(input)?;

        let item_trait = input.parse::<ItemTrait>()?;
        let visibility = item_trait.vis;
        let name = item_trait.ident;
        let generics = item_trait.generics;

        let mut other_exprs = vec![];
        let mut exprs = vec![];
        for trait_fn in item_trait.items.iter() {
            match &trait_fn {
                syn::TraitItem::Fn(ty) => {
                    if ty.default.is_some() {
                        other_exprs.push(trait_fn.clone());
                        continue;
                    } else if ty.attrs.iter().any(|a| a.path().is_ident("skip")) {
                        other_exprs.push(trait_fn.clone());
                        continue;
                    }
                },
                _ => {
                },
            }

            let expr = parse2::<ThunkTraitFn>(trait_fn.to_token_stream())?;
            exprs.push(expr);
        }

        Ok(Self {
            attributes,
            visibility,
            generics,
            name,
            exprs,
            other_exprs,
        })
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct ThunkTraitFn {
    name: Ident,
    attributes: Vec<Attribute>,
    return_type: ReturnType,
    output_type: Type,
    with_type: Option<PatType>,
    where_clause: Option<WhereClause>,
    fields: Vec<syn::FnArg>,
    traitfn: syn::TraitItemFn,
    mutable_recv: bool,
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
            let where_clause = &self.where_clause;

            if self.is_async {
                quote! {
                    async fn #name(#fields) #return_type #where_clause {
                        self.thunk.#name(#input).await
                    }
                }
            } else {
                quote! {
                    fn #name(#fields) #return_type #where_clause {
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

    /// Generates a dispatch expr for a thunk'ed fn,
    ///
    pub(crate) fn dispatch_stmt(&self, recv_type: &Ident) -> TokenStream {
        let closure = match &self {
            ThunkTraitFn {
                name,
                with_type: None,
                is_async: true,
                default: false,
                ..
            } => dispatch_ref_stmts::recv_async_closure(recv_type, name),
            ThunkTraitFn {
                name,
                with_type: None,
                is_async: false,
                default: false,
                mutable_recv,
                ..
            } => dispatch_ref_stmts::recv_closure(*mutable_recv, recv_type, name),
            ThunkTraitFn {
                name,
                with_type: Some(with_ty),
                is_async: true,
                default: false,
                ..
            } => dispatch_ref_stmts::recv_with_async_closure(
                recv_type,
                with_ty.ty.deref(),
                name,
            ),
            ThunkTraitFn {
                name,
                with_type: Some(with_ty),
                mutable_recv,
                is_async: false,
                default: false,
                ..
            } => dispatch_ref_stmts::recv_with_closure(*mutable_recv, recv_type, with_ty.ty.deref(), name),
            _ => {
                quote! {}
            }
        };

        match &self {
            ThunkTraitFn {
                output_type,
                with_type: None,
                mutable_recv: true,
                is_async,
                ..
            } => match output_type {
                Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                    dispatch_ref_stmts::write_stmt(&closure)
                }
                _ => dispatch_ref_stmts::map_stmt(&closure, output_type, *is_async),
            },
            ThunkTraitFn {
                output_type,
                with_type: None,
                mutable_recv: false,
                is_async,
                ..
            } => match output_type {
                Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                    dispatch_ref_stmts::read_stmt(&closure)
                }
                _ => dispatch_ref_stmts::map_stmt(&closure, output_type, *is_async),
            },
            ThunkTraitFn {
                output_type,
                with_type: Some(..),
                mutable_recv: true,
                is_async,
                ..
            } => match output_type {
                Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                    dispatch_ref_stmts::write_with_stmt(&closure)
                }
                _ => dispatch_ref_stmts::map_with_stmt(&closure, output_type, *is_async),
            },
            ThunkTraitFn {
                output_type,
                with_type: Some(..),
                mutable_recv: false,
                is_async,
                ..
            } => match output_type {
                Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                    dispatch_ref_stmts::read_with_stmt(&closure)
                }
                _ => dispatch_ref_stmts::map_with_stmt(&closure, output_type, *is_async),
            },
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
        let where_clause = traitfn.sig.generics.where_clause.clone();
        let mut mutable_recv = false;

        if !skip && fields.len() > 2 {
            return Err(input.error("Currently, only 2 inputs for thunk trait fn's are supported"));
        }

        match fields.first() {
            Some(FnArg::Receiver(r)) => {
                assert!(r.reference.is_some(), "Must be either &self or &mut self");
                mutable_recv = r.mutability.is_some();
            }
            _ if !default && !skip => {
                return Err(input.error(format!("Trait fn's must have a receiver type, i.e. `fn (self)` skip: {} default: {}, {}", skip, default, traitfn.to_token_stream())))
            }
            _ => {}
        }

        let with_type = fields
            .iter()
            .skip(1)
            .take(2)
            .filter_map(|f| match f {
                FnArg::Typed(with_fn_arg) => Some(with_fn_arg),
                _ => None,
            })
            .cloned()
            .next();

        let output_type = match &return_type {
            ReturnType::Type(_, ty) if default || skip => ty.deref().clone(),
            ReturnType::Default if !skip && !default => {
                return Err(input.error("Must return a reality::Result<T> from a thunk trait fn"));
            }
            ReturnType::Type(_, ty) => match ty.deref() {
                Type::Path(p) => {
                    match p
                        .path
                        .segments
                        .iter()
                        .filter(|s| s.ident.to_string().as_str() != "reality")
                        .map(|s| match &s.arguments {
                            syn::PathArguments::None
                                if s.ident.to_string().as_str() != "reality" =>
                            {
                                Err(input.error(format!(
                                    "Must be in the format of reality::Result<T>, found: {}",
                                    s.ident
                                )))
                            }
                            syn::PathArguments::AngleBracketed(args)
                                if s.ident.to_string().starts_with("Result")
                                    && args.args.len() == 1 =>
                            {
                                match &args.args.first() {
                                    Some(arg) => match arg {
                                        syn::GenericArgument::Type(ty) => Ok(ty),
                                        _ => Err(input.error(
                                            "Expecting a type for the return type of Result",
                                        )),
                                    },
                                    _ => Err(input.error("Missing a type in result")),
                                }
                            }
                            _ => Err(input.error(format!(
                                "Must be in the format of reality::Result<T>, found: {}",
                                s.ident
                            ))),
                        })
                        .next()
                    {
                        Some(ty) => ty?.clone(),
                        None => {
                            return Err(
                                input.error("Return type must be a variant of reality::Result<T>")
                            );
                        }
                    }
                }
                _ => return Err(input.error("Return type must be a variant of reality::Result<T>")),
            },
            _ => {
                return Err(input.error("Return type must be a variant of reality::Result<T>"));
            }
        };

        // TODO: Handle LazyUpdate/LazyBuilder

        return Ok(Self {
            name,
            return_type: return_type.clone(),
            mutable_recv,
            with_type,
            output_type,
            attributes: vec![],
            where_clause,
            fields,
            traitfn,
            default,
            is_async,
            skip,
        });
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
    use syn::Path;
    use syn::Type;

    /// Returns an async dispatch sequence for stmts,
    ///
    pub fn async_dispatch_seq(
        thunk_type: &Ident,
        stmts: impl Iterator<Item = TokenStream>,
    ) -> TokenStream {
        let begin_async = quote_spanned! {thunk_type.span()=>
            .transmute::<#thunk_type>()
            .enable_async()
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

    pub fn recv_with_closure(is_mut: bool, recv: &Ident, with: &Type, fn_ident: &Ident) -> TokenStream {
        let reference_ty = if is_mut {
            quote! { &mut }
        } else {
            quote! { & }
        };

        quote_spanned! {fn_ident.span()=>
            |recv: #reference_ty #recv, with: &#with| {
                recv.#fn_ident(with)
            }
        }
    }

    pub fn recv_closure(is_mut: bool, recv: &Ident, fn_ident: &Ident) -> TokenStream {
        let reference_ty = if is_mut {
            quote! { &mut }
        } else {
            quote! { & }
        };

        quote_spanned! {fn_ident.span()=>
            |recv: #reference_ty #recv| {
                recv.#fn_ident()
            }
        }
    }

    pub fn recv_with_async_closure(recv: &Ident, with: &Type, fn_ident: &Ident) -> TokenStream {
        quote_spanned! {fn_ident.span()=>
            |recv: &#recv, with: &#with| {
                let recv = recv.clone();
                let with = with.clone();
                async move {
                    recv.#fn_ident(with).await
                }
            }
        }
    }

    pub fn recv_async_closure(recv: &Ident, fn_ident: &Ident) -> TokenStream {
        quote_spanned! {fn_ident.span()=>
            |recv: &#recv| {
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
    pub fn map_stmt(closure: &TokenStream, out_ty: &Type, is_async: bool) -> TokenStream {
        if is_async {
            quote! {
                .map::<#out_ty, _>(#closure)
            }
        } else {
            quote! {
                .map::<#out_ty>(#closure)
            }
        }
    }

    /// Generates a `map_into` statement,
    ///
    pub fn map_into_stmt(closure: &TokenStream, out_ty: &Type, is_async: bool) -> TokenStream {
        if is_async {
            quote! {
                .map_into::<#out_ty, _>(#closure)
            }
        } else {
            quote! {
                .map_into::<#out_ty>(#closure)
            }
        }
    }

    /// Generates a `map_into_with` statement,
    ///
    pub fn map_into_with_stmt(closure: &TokenStream, out_ty: &Type, is_async: bool) -> TokenStream {
        if is_async {
            quote! {
                .map_into_with::<#out_ty, _>(#closure)
            }
        } else {
            quote! {
                .map_into_with::<#out_ty>(#closure)
            }
        }
    }

    /// Generates a `map_with` statement,
    ///
    pub fn map_with_stmt(closure: &TokenStream, out_ty: &Type, is_async: bool) -> TokenStream {
        if is_async {
            quote! {
                .map_with::<#out_ty, _>(#closure)
            }
        } else {
            quote! {
                .map_with::<#out_ty>(#closure)
            }
        }
    }

    /// Generates a `read` statement,
    ///
    pub fn read_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .read(#closure)
        }
    }

    /// Generates a `write` statement,
    ///
    pub fn write_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .write(#closure)
        }
    }

    /// Generates a `read_with` statement,
    ///
    pub fn read_with_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .read_with(#closure)
        }
    }

    /// Generates a `write_with` statement,
    ///
    pub fn write_with_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .write_with(#closure)
        }
    }
}

mod tests {
    #[test]
    fn test() {}
}
