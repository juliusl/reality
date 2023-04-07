use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::Path;
use syn::Token;

/// Contains arguments for the apply_framework!(..) macro,
/// 
pub struct ApplyFrameworkMacro {
    /// Span of where this is being called,
    ///
    span: Span,
    /// Name of the variable that is a Compiler
    ///
    compiler_name: Path,
    /// Types to apply runmd w/
    ///
    types: Vec<Path>,
}

impl ApplyFrameworkMacro {
    /// Generates code like,
    /// 
    /// ```
    /// {
    ///     reality::v2::framework::configure(compiler.as_mut())?;
    ///     Process::runmd(&mut compiler)?;
    /// }
    /// ```
    /// 
    pub(crate) fn apply_framework_expr(&self) -> TokenStream {
        if let Some(compiler_name) = self.compiler_name.get_ident() {
            let configure_expr = quote_spanned! {self.compiler_name.span()=>
                reality::v2::framework::configure(#compiler_name.as_mut());
            };

            let types_map = self.types.iter().map(|p| {
                quote_spanned! {p.span()=>
                    #p::new().runmd(&mut #compiler_name)?;
                }
            });
            let types = quote! {
                #( #types_map )*
            };

            quote_spanned! {self.span=>
                {
                    #configure_expr
                    #types
                }
            }
        } else {
            TokenStream::new()
        }
    }
}

impl Parse for ApplyFrameworkMacro {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse::<Path>()?;
        input.parse::<Token![,]>()?;

        let types =
            syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated(input)?
                .iter()
                .cloned()
                .collect();
        
        Ok(Self {
            compiler_name: name.clone(),
            span: input.span(),
            types,
        })
    }
}
