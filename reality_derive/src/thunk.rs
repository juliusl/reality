use std::ops::Deref;

use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse2;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::FnArg;
use syn::Generics;
use syn::Ident;
use syn::ItemTrait;
use syn::PatType;
use syn::ReturnType;
use syn::Type;
use syn::Visibility;
use syn::WhereClause;

/// Struct containing parts of a trait definition,
///
pub(crate) struct ThunkMacro {
    /// Name of the trait,
    ///
    name: Ident,
    /// Attributes used w/this attribute,
    ///
    attributes: Vec<Attribute>,
    /// Visibility modifier,
    ///
    visibility: Visibility,
    /// Thunk trait expressions,
    ///
    exprs: Vec<ThunkTraitFn>,
    /// Other expressions defined in the trait,
    ///
    other_exprs: Vec<syn::TraitItem>,
    _generics: Generics,
}

impl ThunkMacro {
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
        let bootstrap_fn = self.bootstrap_fn();

        quote_spanned! {vis.span()=>
            #attributes
            #vis trait #name
            where
                Self: Send + Sync,
            {
                #exprs

                #( #other_exprs )*

                #bootstrap_fn
            }

            #arc_impl_expr

            #thunk_trait_type_expr

            #thunk_trait_impl_expr

            #dispatch_exprs
        }
    }

    /// Generates trait impl for Thunk<Arc<dyn T>>,
    ///
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

    /// Generates trait impl for Arc<dyn T>,
    ///
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

    /// Generates statements to bootstrap dispatch extensions,
    ///
    pub(crate) fn bootstrap_fn(&self) -> TokenStream {
        let bootstrap = self
            .exprs
            .iter()
            .filter(|e| !e.skip && !e.default)
            .map(|e| {
                let dispatch_thunk_struct_alias = format_ident!("dispatch_{}", e.name);
                quote! {
                    .map(|_| Ok(#dispatch_thunk_struct_alias {}))
                }
            });

        quote! {
            fn __bootstrap<'a>(dispatch_ref: DispatchRef<'a, Properties>) -> DispatchRef<'a, Properties>
            where
            Self: Sized
            {
                dispatch_ref
                #( #bootstrap )*
            }
        }
    }

    /// Generates a dispatch expr for trait fn,
    ///
    pub(crate) fn impl_dispatch_exprs(&self) -> TokenStream {
        let name = &self.name;
        let thunk_type_name = format_ident!("Thunk{}", name);
        let dispatch_thunk_struct_name = format_ident!("DispatchThunk{}", name);

        let struct_def = quote! {
            /// Pointer-struct for dispatching thunk,
            ///
            #[derive(specs::Component, Clone, Debug)]
            #[storage(specs::DenseVecStorage)]
            pub struct #dispatch_thunk_struct_name<const SLOT: usize = 0>;
        };

        let mut dispatch_exprs = vec![];
        let mut using = vec![];

        for (idx, e) in self
            .exprs
            .iter()
            .filter(|e| !e.skip && !e.default)
            .enumerate()
        {
            let stmt = e.dispatch_stmt(&thunk_type_name);
            let idx_lit = syn::LitInt::new(format!("{}", idx).as_str(), e.name.span());
            let ext_fn_name = &e.name;
            let dispatch_thunk_struct_alias = format_ident!("dispatch_{}", e.name);
            let dispatch_thunk_trait_ext_name = format_ident!("Dispatch{}Ext", e.name);

            using.push(quote! {
                .map(|_| Ok(#dispatch_thunk_struct_alias {}))
            });

            let ext_def = quote_spanned! {e.name.span()=>
                /// Type-alias for dispatch thunk pointer type,
                ///
                #[allow(non_camel_case_types)]
                pub type #dispatch_thunk_struct_alias = #dispatch_thunk_struct_name<#idx_lit>;
            };

            if e.is_async {
                let body = dispatch_ref_stmts::async_dispatch_seq(
                    &thunk_type_name,
                    Some(stmt).into_iter(),
                );

                let expr = quote_spanned! {e.name.span()=>
                    #ext_def

                    #[async_trait]
                    pub trait #dispatch_thunk_trait_ext_name<'a> {
                        /// Extension function,
                        ///
                        async fn #ext_fn_name(self) -> reality::v2::DispatchResult<'a>;
                    }

                    #[async_trait]
                    impl<'a> #dispatch_thunk_trait_ext_name<'a> for reality::v2::DispatchRef<'a, reality::v2::Properties, true> {
                        async fn #ext_fn_name(self) -> reality::v2::DispatchResult<'a> {
                            self.exec_slot::<#idx_lit, #dispatch_thunk_struct_alias>().await
                        }
                    }

                    #[async_trait]
                    impl reality::v2::AsyncDispatch<#idx_lit> for #dispatch_thunk_struct_name<#idx_lit> {
                        async fn async_dispatch<'a, 'b>(
                            &'a self,
                            dispatch_ref: DispatchRef<'b, Properties>,
                        ) -> DispatchResult<'b> {
                            dispatch_ref
                            #body
                        }
                    }
                };
                dispatch_exprs.push(expr);
            } else {
                let body =
                    dispatch_ref_stmts::dispatch_seq(&thunk_type_name, Some(stmt).into_iter());

                let expr = quote_spanned! {e.name.span()=>
                    #ext_def

                    pub trait #dispatch_thunk_trait_ext_name<'a> {
                        /// Extension function,
                        ///
                        fn #ext_fn_name(self) -> reality::v2::DispatchResult<'a>;
                    }

                    impl<'a> #dispatch_thunk_trait_ext_name<'a> for reality::v2::DispatchRef<'a, reality::v2::Properties> {
                        fn #ext_fn_name(self) -> reality::v2::DispatchResult<'a> {
                            self.exec_slot::<#idx_lit, #dispatch_thunk_struct_alias>()
                        }
                    }

                    impl reality::v2::Dispatch<#idx_lit> for #dispatch_thunk_struct_name<#idx_lit> {
                        fn dispatch<'a>(&self, dispatch_ref: reality::v2::DispatchRef<'a, reality::v2::Properties>) -> reality::v2::DispatchResult<'a> {
                            dispatch_ref
                            #body
                        }
                    }
                };
                dispatch_exprs.push(expr);
            }
        }

        quote! {
            #struct_def

            #( #dispatch_exprs )*
        }
    }
}

impl Parse for ThunkMacro {
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
                }
                _ => {}
            }

            let expr = parse2::<ThunkTraitFn>(trait_fn.to_token_stream())?;
            exprs.push(expr);
        }

        Ok(Self {
            attributes,
            visibility,
            _generics: generics,
            name,
            exprs,
            other_exprs,
        })
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub(crate) struct ThunkTraitFn {
    /// Fn name,
    ///
    name: Ident,
    /// Attributes define on this trait fn,
    ///
    attributes: Vec<Attribute>,
    /// Original parsed ReturnType
    ///
    return_type: ReturnType,
    /// Output type, inner-type in Result<T>,
    ///
    output_type: Type,
    /// Optional "with" fn argument,
    ///
    with_type: Option<PatType>,
    /// Where clause
    ///
    where_clause: Option<WhereClause>,
    /// Function arguments,
    ///
    args: Vec<syn::FnArg>,
    /// Original TraitItemFn
    ///
    traitfn: syn::TraitItemFn,
    /// True if the trait fn has a mutable receiver,
    ///
    mutable_recv: bool,
    /// True if the trait fn has a mutable with,
    ///
    mutable_with: bool,
    /// True if function has a default impl,
    ///
    default: bool,
    /// True if function is async,
    ///
    is_async: bool,
    /// Value of the #[skip] attribute,
    ///
    skip: bool,
    /// The position of a fn arg LazyUpdate,
    ///
    lazy_update_pos: Option<usize>,
    /// The position of a fn arg LazyBuilder,
    ///
    lazy_builder_pos: Option<usize>,
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
            let fields = self.args.iter().map(|f| {
                quote_spanned! {f.span()=>
                    #f
                }
            });
            let fields = quote! {
                #( #fields ),*
            };

            let input = self
                .args
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
            let fields = self.args.iter().map(|f| {
                quote_spanned! {f.span()=>
                    #f
                }
            });
            let fields = quote! {
                #( #fields ),*
            };

            let input = self
                .args
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
                        std::ops::Deref::deref(self).#name(#input).await
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
        dbg!(self.mutable_recv);
        dbg!(self.mutable_with);

        let closure = match &self {
            ThunkTraitFn {
                name,
                with_type: None,
                mutable_recv,
                is_async: false,
                default: false,
                lazy_builder_pos: Some(_),
                ..
            } => dispatch_ref_stmts::recv_build_closure(*mutable_recv, recv_type, name),
            ThunkTraitFn {
                name,
                with_type: None,
                mutable_recv,
                is_async: false,
                default: false,
                lazy_update_pos: Some(_),
                ..
            } => dispatch_ref_stmts::recv_update_closure(*mutable_recv, recv_type, name),
            ThunkTraitFn {
                name,
                with_type: Some(with_ty),
                mutable_recv,
                is_async: false,
                default: false,
                lazy_builder_pos: Some(_),
                ..
            } => dispatch_ref_stmts::recv_build_with_closure(
                *mutable_recv,
                recv_type,
                &with_ty.ty,
                name,
            ),
            ThunkTraitFn {
                name,
                with_type: Some(with_ty),
                mutable_recv,
                is_async: false,
                default: false,
                lazy_update_pos: Some(_),
                ..
            } => dispatch_ref_stmts::recv_update_with_closure(
                *mutable_recv,
                recv_type,
                &with_ty.ty,
                name,
            ),
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
            } => dispatch_ref_stmts::recv_with_async_closure(recv_type, with_ty.ty.deref(), name),
            ThunkTraitFn {
                name,
                with_type: Some(with_ty),
                mutable_with,
                output_type,
                is_async: false,
                default: false,
                ..
            } => {
                let closure = dispatch_ref_stmts::recv_with_closure(
                    *mutable_with,
                    recv_type,
                    with_ty.ty.deref(),
                    name,
                );

                return match output_type {
                    Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                        dispatch_ref_stmts::write_with_stmt(&closure, &with_ty.ty)
                    }
                    _ => dispatch_ref_stmts::map_with_stmt(&closure, output_type, false),
                };
            },
            _ => {
                quote! {}
            }
        };

        match &self {
            ThunkTraitFn {
                with_type: Some(w),
                mutable_with: true,
                lazy_update_pos: Some(_),
                ..
            } => dispatch_ref_stmts::dispatch_mut_with_stmt(&closure, Some(recv_type), &w.ty),
            ThunkTraitFn {
                with_type: None,
                mutable_recv: true,
                lazy_update_pos: Some(_),
                ..
            } => dispatch_ref_stmts::dispatch_mut_stmt(&closure),
            ThunkTraitFn {
                with_type: None,
                mutable_recv: false,
                lazy_update_pos: Some(_),
                ..
            } => dispatch_ref_stmts::dispatch_stmt(&closure),
            ThunkTraitFn {
                with_type: Some(w),
                mutable_recv: true,
                lazy_builder_pos: Some(_),
                ..
            } => dispatch_ref_stmts::fork_into_with_mut(&closure, &w.ty),
            ThunkTraitFn {
                with_type: Some(w),
                mutable_recv: false,
                lazy_builder_pos: Some(_),
                ..
            } => dispatch_ref_stmts::fork_into_with(&closure, &w.ty),
            ThunkTraitFn {
                with_type: None,
                mutable_recv: false,
                lazy_builder_pos: Some(_),
                ..
            } => dispatch_ref_stmts::fork_into_stmt(&closure, &recv_type),
            ThunkTraitFn {
                output_type,
                with_type: None,
                mutable_recv: true,
                mutable_with: false,
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
                mutable_with: false,
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
                with_type: Some(with_ty),
                mutable_recv: false,
                mutable_with: true,
                is_async,
                ..
            } => match output_type {
                Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                    dispatch_ref_stmts::write_with_stmt(&closure, &with_ty.ty)
                }
                _ => dispatch_ref_stmts::map_with_stmt(&closure, output_type, *is_async),
            },
            ThunkTraitFn {
                output_type,
                with_type: Some(..),
                mutable_recv: false,
                mutable_with: false,
                is_async,
                ..
            } => match output_type {
                Type::Tuple(tuplety) if tuplety.elems.is_empty() => {
                    dispatch_ref_stmts::read_with_stmt(&closure)
                }
                _ => dispatch_ref_stmts::map_with_stmt(&closure, output_type, *is_async),
            },
            _ => {
                quote! {}
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

        if !skip && fields.len() > 3 {
            return Err(
                input.error("Currently, only a max of 3 inputs for thunk trait fn's are supported")
            );
        }

        match fields.first() {
            Some(FnArg::Receiver(r)) => {
                assert!(r.reference.is_some(), "Must be either &self or &mut self");
                mutable_recv = r.mutability.is_some();
            }
            _ if !default && !skip => {
                return Err(input.error(format!(
                "Trait fn's must have a receiver type, i.e. `fn (self)` skip: {} default: {}, {}",
                skip,
                default,
                traitfn.to_token_stream()
            )))
            }
            _ => {}
        }

        let mut other_fields = fields
            .iter()
            .skip(1)
            .take(2)
            .filter_map(|f| match f {
                FnArg::Typed(with_fn_arg) => Some(with_fn_arg),
                _ => None,
            })
            .cloned();

        let mut lazy_update_pos = None;
        let mut lazy_builder_pos = None;
        let mut with_mut = false;
        let mut with_type = match other_fields.next() {
            Some(pat) => match pat.ty.deref() {
                Type::Path(p) => {
                    if p.path.is_ident("LazyUpdate") {
                        lazy_update_pos = Some(0);
                        None
                    } else if p.path.is_ident("LazyBuilder") {
                        lazy_builder_pos = Some(0);
                        None
                    } else {
                        dbg!(pat.ty.to_token_stream());
                        match pat.ty.deref() {
                            Type::Reference(r) if r.mutability.is_some() => {
                                with_mut = true;
                            }
                            _ => {}
                        }
                        Some(pat)
                    }
                }
                _ => Some(pat),
            },
            None => None,
        };

        if let Some(other_type) = other_fields.next() {
            match other_type.ty.deref() {
                Type::Path(p) => {
                    if p.path.is_ident("LazyUpdate") {
                        lazy_update_pos = Some(0);
                    } else if p.path.is_ident("LazyBuilder") {
                        lazy_builder_pos = Some(0);
                    } else if with_type.is_none() {
                        match other_type.ty.deref() {
                            Type::Reference(r) if r.mutability.is_some() => {
                                with_mut = true;
                            }
                            _ => {}
                        }
                        with_type = Some(other_type);
                    }
                }
                _ => {}
            }
        }

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

        return Ok(Self {
            name,
            return_type: return_type.clone(),
            mutable_recv,
            mutable_with: with_mut,
            with_type,
            output_type,
            attributes: vec![],
            where_clause,
            args: fields,
            traitfn,
            default,
            is_async,
            skip,
            lazy_builder_pos,
            lazy_update_pos,
        });
    }
}

/// This module is for generating dispatch sequences for thunk types,
///
#[allow(dead_code)]
#[allow(unused_imports)]
pub mod dispatch_ref_stmts {
    use std::ops::Deref;

    use proc_macro2::Ident;
    use proc_macro2::TokenStream;
    use quote::format_ident;
    use quote::quote;
    use quote::quote_spanned;
    use quote::ToTokens;
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

    /// Generates a closure for a `.fork_into_mut_with` statement,
    ///
    pub fn recv_build_with_closure(
        is_mut: bool,
        recv: &Ident,
        with: &Type,
        fn_ident: &Ident,
    ) -> TokenStream {
        let reference_ty = if is_mut {
            quote! { &mut }
        } else {
            quote! { & }
        };

        quote_spanned! {fn_ident.span()=>
            |recv: #reference_ty #recv, with: #with, lazy: LazyBuilder| {
                recv.#fn_ident(with, lazy)
            }
        }
    }

    /// Generates a closure for a `.fork_into` statement,
    ///
    pub fn recv_build_closure(is_mut: bool, recv: &Ident, fn_ident: &Ident) -> TokenStream {
        let reference_ty = if is_mut {
            quote! { &mut }
        } else {
            quote! { & }
        };

        quote_spanned! {fn_ident.span()=>
            |recv: #reference_ty #recv, lazy: LazyBuilder| {
                recv.#fn_ident(lazy)
            }
        }
    }

    /// Generates a closure for a `dispatch_with` statement,
    ///
    pub fn recv_update_with_closure(
        is_mut: bool,
        recv: &Ident,
        with: &Type,
        fn_ident: &Ident,
    ) -> TokenStream {
        let reference_ty = if is_mut {
            quote! { &mut }
        } else {
            quote! { & }
        };

        quote_spanned! {fn_ident.span()=>
            |recv: #reference_ty #recv, with: #with, lazy: LazyUpdate| {
                recv.#fn_ident(with, lazy)
            }
        }
    }

    ///  Generates a closure for a `dispatch` statement,
    ///
    pub fn recv_update_closure(is_mut: bool, recv: &Ident, fn_ident: &Ident) -> TokenStream {
        let reference_ty = if is_mut {
            quote! { &mut }
        } else {
            quote! { & }
        };

        quote_spanned! {fn_ident.span()=>
            |recv: #reference_ty #recv, lazy: LazyUpdate| {
                recv.#fn_ident(lazy)
            }
        }
    }

    pub fn recv_with_closure(
        is_mut: bool,
        with: &Ident,
        recv: &Type,
        fn_ident: &Ident,
    ) -> TokenStream {
        let reference_ty = if is_mut {
            quote! { &mut }
        } else {
            quote! { & }
        };

        quote_spanned! {fn_ident.span()=>
            |recv: #recv, with: #reference_ty #with| {
                with.#fn_ident(recv)
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
            |recv: &#recv, with: #with| {
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
        let out_ty = out_ty
            .to_token_stream()
            .to_string()
            .trim_start_matches("&")
            .trim_start_matches("mut")
            .trim_start()
            .to_string();
        let out_ty = format_ident!("{}", out_ty);

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
        let out_ty = out_ty
            .to_token_stream()
            .to_string()
            .trim_start_matches("&")
            .trim_start_matches("mut")
            .trim_start()
            .to_string();
        let out_ty = format_ident!("{}", out_ty);

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
    pub fn write_with_stmt(closure: &TokenStream, with_ty: &Type) -> TokenStream {
        match with_ty {
            Type::Reference(r) if r.mutability.is_some() => {
                let with_ty = &r.elem;
                quote! {
                .transmute::<#with_ty>()
                .write_with(#closure)
                }
            }
            _ => quote! {
                .write_with(#closure)
            },
        }
    }

    pub fn dispatch_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .dispatch(#closure)?
        }
    }

    pub fn dispatch_mut_stmt(closure: &TokenStream) -> TokenStream {
        quote! {
            .dispatch_mut(#closure)?
        }
    }

    pub fn dispatch_mut_with_stmt(
        closure: &TokenStream,
        mut_ty: Option<&Ident>,
        with_ty: &Type,
    ) -> TokenStream {
        let with_ty = with_ty
            .to_token_stream()
            .to_string()
            .trim_start_matches("&")
            .trim_start_matches("mut")
            .trim_start()
            .to_string();
        let with_ty = format_ident!("{}", with_ty);

        let mut_ty = mut_ty
            .map(|t| {
                quote_spanned! {t.span()=>
                    .transmute::<#t>()
                }
            })
            .into_iter();

        quote! {
            #( #mut_ty )*
            .dispatch_mut_with::<#with_ty>(#closure)?
        }
    }

    pub fn fork_into_stmt(closure: &TokenStream, into_ty: &Ident) -> TokenStream {
        quote! {
            .fork_into::<#into_ty>(#closure)?
        }
    }

    pub fn fork_into_with(closure: &TokenStream, with_ty: &Type) -> TokenStream {
        let with_ty = with_ty
            .to_token_stream()
            .to_string()
            .trim_start_matches("&")
            .trim_start_matches("mut")
            .trim_start()
            .to_string();
        let with_ty = format_ident!("{}", with_ty);
        quote! {
            .fork_into_with::<#with_ty>(#closure)?
        }
    }

    pub fn fork_into_with_mut(closure: &TokenStream, with_ty: &Type) -> TokenStream {
        let with_ty = with_ty
            .to_token_stream()
            .to_string()
            .trim_start_matches("&")
            .trim_start_matches("mut")
            .trim_start()
            .to_string();
        let with_ty = format_ident!("{}", with_ty);
        quote! {
            .fork_into_with_mut::<#with_ty>(#closure)?
        }
    }
}

mod tests {
    #[test]
    fn test() {}
}
