use proc_macro2::TokenStream;
use quote::format_ident;
use quote::ToTokens;
use quote::quote_spanned;
use quote::quote;
use syn::Visibility;
use syn::Token;
use syn::ReturnType;
use syn::Path;
use syn::Ident;
use syn::FnArg;
use syn::Expr;
use syn::Attribute;
use syn::spanned::Spanned;
use syn::parse2;
use syn::parse::Parse;

pub(crate) struct ThunkMacroArguments {
    attributes: Vec<Attribute>,
    visibility: Visibility,
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
        let exprs = self.exprs.iter().filter(|f| !f.default).map(|e| e.thunk_impl_fn());
        let exprs = quote! {
            #( #exprs )*
        };

        quote! {
            impl<T: #name + Send + Sync> #name for reality::v2::Thunk<T>
            { 
                #exprs 
            }
        }
    }

    pub(crate) fn arc_impl_expr(&self) -> TokenStream {
        let name = &self.type_path;
        let exprs = self.exprs.iter().filter(|f| !f.default).map(|e| e.arc_impl_fn());
        let exprs = quote! {
            #( #exprs )*
        };

        quote! {
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
        let expr = input.parse::<Expr>()?;
        let mut exprs = vec![];

        match expr {
            Expr::Block(block) => {
                exprs = block
                    .block
                    .stmts
                    .iter()
                    .filter_map(|s| {
                        parse2::<ThunkTraitFn>(s.to_token_stream()).ok()
                    })
                    .collect();
            }
            _ => {}
        }

        Ok(Self {
            attributes,
            visibility,
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
        let attrs = self.attributes.iter().map(|a| quote_spanned! {a.span()=>
            #a
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
                quote_spanned!{f.span()=>
                    #f
                }
            });
            let fields = quote! {
                #( #fields ),*
            };

            let input = self.fields.iter().filter_map(|f| {
                match f {
                    FnArg::Receiver(_) => None,
                    FnArg::Typed(typed) => {
                        Some(typed.pat.clone())
                    },
                }
            }).map(|f| {
                quote_spanned!{f.span()=>
                    #f
                }
            });
            let input = quote! {
                #( #input ),*
            };

            let return_type = &self.return_type;

            quote! {
                fn #name(#fields) #return_type {
                    self.thunk.#name(#input)
                }
            }
        } else {
            quote! { }
        }
    }

    pub(crate) fn arc_impl_fn(&self) -> TokenStream {
        if !self.default {
            let name = &self.name;
            let fields = self.fields.iter().map(|f| {
                quote_spanned!{f.span()=>
                    #f
                }
            });
            let fields = quote! {
                #( #fields ),*
            };

            let input = self.fields.iter().filter_map(|f| {
                match f {
                    FnArg::Receiver(_) => None,
                    FnArg::Typed(typed) => {
                        Some(typed.pat.clone())
                    },
                }
            }).map(|f| {
                quote_spanned!{f.span()=>
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
            quote! { }
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
            },
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
