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

    /// Generates a dispatch expr for trait fn,
    /// 
    pub(crate) fn dispatch_expr(&self) -> TokenStream {
        let name = &self.type_path;
        let ident = name.get_ident().expect("should have an ident");
        let thunk_type_name = format_ident!("Thunk{}", ident);
        
        let stmts = self.exprs.iter()
            .filter(|e| !e.is_async && !e.skip)
            .map(|e| e.dispatch_stmt(&self.type_path))
            .take(1);
        
        dispatch_ref_stmts::dispatch_seq(&thunk_type_name, stmts)
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
                    .map(|s| parse2::<ThunkTraitFn>(s.to_token_stream()))
                {
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
    pub(crate) fn dispatch_stmt(&self, recv_type_path: &Path) -> TokenStream {
        let closure = match &self {
            ThunkTraitFn { name, with_type: None, is_async: true, default: false, .. } => {
                dispatch_ref_stmts::recv_async_closure(recv_type_path, name)
            }
            ThunkTraitFn { name, with_type: None, is_async: false, default: false, .. } => {
                dispatch_ref_stmts::recv_closure(recv_type_path, name)
            }
            ThunkTraitFn { name, with_type: Some(with_ty), is_async: true, default: false, .. } => {
                dispatch_ref_stmts::recv_with_async_closure(recv_type_path, with_ty.ty.deref(), name)
            }
            ThunkTraitFn { name, with_type: Some(with_ty), mutable_recv: false, is_async: false, default: false, .. } => {
                dispatch_ref_stmts::recv_with_closure(recv_type_path, with_ty.ty.deref(), name)
                
            }
            _ => {
                quote!{}
            }
        };

        match &self {
            ThunkTraitFn { output_type, with_type: None, mutable_recv: true, .. } => {
                match output_type {
                    Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                        dispatch_ref_stmts::write_stmt(&closure)
                    },
                    _ => {
                        dispatch_ref_stmts::map_stmt(&closure)
                    },
                }
            }
            ThunkTraitFn { output_type, with_type: None, mutable_recv: false, .. } => {
                match output_type {
                    Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                        dispatch_ref_stmts::read_stmt(&closure)
                    },
                    _ => {
                        dispatch_ref_stmts::map_stmt(&closure)
                    },
                }
            }
            ThunkTraitFn { output_type, with_type: Some(..), mutable_recv: true, .. } => {
                match output_type {
                    Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                        dispatch_ref_stmts::write_with_stmt(&closure)
                    },
                    _ => {
                        dispatch_ref_stmts::map_with_stmt(&closure)
                    },
                }
            }
            ThunkTraitFn { output_type, with_type: Some(..), mutable_recv: false, .. } => {
                match output_type {
                    Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                        dispatch_ref_stmts::read_with_stmt(&closure)
                    },
                    _ => {
                        dispatch_ref_stmts::map_with_stmt(&closure)
                    },
                }
            }
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
                assert!(r.reference.is_some(), "must be either &self or &mut self");
                mutable_recv = r.mutability.is_some();
            }
            _ if !default && !skip => {
                return Err(input.error("Trait fn's must have a receiver type, i.e. `fn (self)`"))
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
            ReturnType::Default if !skip => {
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
                                Err(input.error("Must be in the format of reality::Result<T>"))
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

    pub fn recv_with_closure(recv: &Path, with: &Type, fn_ident: &Ident) -> TokenStream {
        quote_spanned! {fn_ident.span()=>
            |recv: #recv, with: #with| {
                recv.#fn_ident(with)
            }
        }
    }

    pub fn recv_closure(recv: &Path, fn_ident: &Ident) -> TokenStream {
        quote_spanned! {fn_ident.span()=>
            |recv: #recv| {
                recv.#fn_ident()
            }
        }
    }

    pub fn recv_with_async_closure(recv: &Path, with: &Type, fn_ident: &Ident) -> TokenStream {
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

    pub fn recv_async_closure(recv: &Path, fn_ident: &Ident) -> TokenStream {
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
    fn test() {

    }
}