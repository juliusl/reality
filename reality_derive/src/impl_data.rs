use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use quote::quote_spanned;
use syn::Generics;
use syn::parse::Parse;
use syn::parse2;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;
use syn::Path;
use syn::spanned::Spanned;

pub struct ImplData {
    path: Path,
    generics: Generics,
    dispatches: Vec<ImplFn>,
}

impl ImplData {
    /// Follows dispatch instructions,
    /// 
    pub(crate) fn dispatch_impl(&self) -> TokenStream {
        let generics = &self.generics;
        let path = &self.path;
        quote! {
            impl #generics Dispatch for #path #generics {
                pub fn dispatch(&self, dispatch_ref: DispatchRef<Properties, Error>) -> Result<DispatchRef<Properties, Error>, Error> {
                    dispatch_ref
                }
            }
        }
    }
}

impl Parse for ImplData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        todo!()
    }
}

/// Struct for a fn impl,
/// 
/// Built-in supported signatures, 
/// 
/// ```
/// fn #tags_#root_#ext(&self) -> Result<(), reality::Error>
/// 
/// fn #tags_#root_#ext(&self) -> Result<#Return, reality::Error>
/// 
/// fn #tags_#root_#ext(&self, with: &#With) -> Result<Self, reality::Error>
/// 
/// fn #tags_#root_#ext(&mut self) -> Result<(), reality::Error>
/// 
/// fn #tags_#root_#ext(&mut self, with: &#With) -> Result<(), reality::Error>
/// 
/// fn #tags_#root_#ext(&self, with: &mut With) -> Result<(), reality::Error>
/// 
/// fn #tags_#root_#ext(&self, lazy_builder: LazyBuilder) -> Result<Entity, reality::Error>
/// 
/// fn #tags_#root_#ext(&self, with: &#With, lazy_builder: LazyBuilder) -> Result<specs::Entity, reality::Error>
/// 
/// fn #tags_#root_#ext(&self, with: &#With, listen: Properties, lazy_builder: LazyBuilder) -> Result<specs::Entity, reality::Error>
/// 
/// ```
/// 
pub struct ImplFn {
    span: Span,
    name: Ident,
    args: Vec<syn::FnArg>,
}

impl ImplFn {
    /// Generates a closure like this,
    /// 
    /// ```
    /// |recv, #fn_args| {
    ///     recv.#fn_name(#fn_args)
    /// }
    /// ```
    /// 
    pub(crate) fn invoke_expr(&self) -> TokenStream {
        let fn_name = &self.name;
        let fn_args = self.args.iter().filter_map(|a| match a {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(typed) => Some(typed.pat.clone()),
        }).map(|a| {
            quote_spanned!{a.span()=> 
                #a
            }
        });

        let fn_args = quote! {
            #( #fn_args ),*
        };

        quote! {
            |recv, #fn_args| {
                recv.#fn_name(#fn_args)
            }
        }
    }

    pub(crate) fn dispatch_read_expr(&self) -> TokenStream {
        let expr = self.invoke_expr();
        quote_spanned!{self.span=>
            .read(#expr)
        }
    }

    pub(crate) fn dispatch_write_expr(&self) -> TokenStream {
        let expr = self.invoke_expr();
        quote_spanned!{self.span=>
            .write(#expr)
        }
    }

    pub(crate) fn dispatch_read_with_expr(&self) -> TokenStream {
        let expr = self.invoke_expr();
        quote_spanned!{self.span=>
            .read_with(#expr)
        }
    }

    pub(crate) fn dispatch_write_with_expr(&self) -> TokenStream {
        let expr = self.invoke_expr();
        quote_spanned!{self.span=>
            .write_with(#expr)
        }
    }

    pub(crate) fn dispatch_root_signature_expr(&self) -> TokenStream {
        quote! {
            ("usage", "plugin", "Println", "stderr") => {
                Err(Error::not_implemented())
            },
        }
    }

    pub(crate) fn dispatch_root_variant_signature_expr(&self) -> TokenStream {
        quote! {
            ("usage", "plugin", "Println", "stderr") => {
                Err(Error::not_implemented())
            },
        }
    }

    pub(crate) fn dispatch_root_config_signature_expr(&self) -> TokenStream {
        quote! {
            ("usage", "plugin", "Println", "stderr") => {
                Err(Error::not_implemented())
            },
        }
    }

    pub(crate) fn dispatch_root_variant_config_signature_expr(&self) -> TokenStream {
        quote! {
            ("usage", "plugin", "Println", "stderr") => {
                Err(Error::not_implemented())
            },
        }
    }
}