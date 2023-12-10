use proc_macro2::{Ident, TokenStream};
use quote::quote_spanned;
use syn::{parse::Parse, spanned::Spanned, DeriveInput, LitStr, Token, Variant};

/// Struct for storing configuration when deriving RealityEnum,
///
pub struct EnumData {
    /// Input from the derive macro,
    ///
    input: DeriveInput,
    /// String to use as the prefix when generating idents,
    ///
    rename_prefix: Option<LitStr>,
    /// Variants parsed from this enum data,
    ///
    variants: Vec<VariantStruct>,
}

pub struct VariantStruct {
    variant: Variant,
    /// Will generate a struct for this variant,
    ///
    /// **Note** Must provide a value for call attribute,
    ///
    plugin: bool,
    /// Suffix to use which identifies this variant,
    ///
    rename_suffix: Option<LitStr>,
    /// If plugin is set, this fn will be mapped to the CallAsync trait impl of
    /// the generated type,
    ///
    call: Option<Ident>,
}

impl VariantStruct {
    /// Creates a new variant struct from variant,
    ///
    pub fn new(variant: &Variant) -> syn::Result<Self> {
        let mut variant_data = VariantStruct {
            variant: variant.clone(),
            plugin: false,
            rename_suffix: None,
            call: None,
        };

        if variant.fields.iter().any(|f| f.colon_token.is_some()) {
            for attr in variant.attrs.iter() {
                if attr.meta.path().is_ident("reality") {
                    attr.parse_nested_meta(|meta| {
                        if meta.path.is_ident("plugin") {
                            variant_data.plugin = true;
                        }

                        if meta.path.is_ident("rename_suffix") {
                            meta.input.parse::<Token![=]>()?;
                            variant_data.rename_suffix = meta.input.parse()?;
                        }

                        if meta.path.is_ident("call") {
                            meta.input.parse::<Token![=]>()?;
                            variant_data.call = meta.input.parse()?;
                        }

                        Ok(())
                    })?;
                }
            }
        }

        Ok(variant_data)
    }

    pub fn render_plugin_struct(&self) -> TokenStream {
        let name = &self.variant.ident;
        let mut attrs = vec![];

        let fields = self.variant.fields.iter();

        if self.call.is_some() {
            attrs.push(quote_spanned!(name.span()=>
                #[derive(Reality, Default, Clone)]
            ));
        }

        attrs.extend_from_slice(
            &self
                .variant
                .attrs
                .clone()
                .iter()
                .map(|f| quote_spanned!(f.span()=> #f))
                .collect::<Vec<_>>(),
        );

        quote_spanned!(self.variant.span()=>
            #(#attrs)*
            pub struct #name {
                #(#fields),*
            }
        )
    }
}

impl Parse for EnumData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let input = DeriveInput::parse(input)?;

        let mut enum_data = Self {
            input,
            rename_prefix: None,
            variants: vec![],
        };

        for attr in enum_data.input.attrs.iter() {
            if attr.meta.path().is_ident("reality") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename_prefix") {
                        meta.input.parse::<Token![=]>()?;
                        enum_data.rename_prefix = meta.input.parse()?;
                    }
                    Ok(())
                })?;
            }
        }

        if let syn::Data::Enum(ref _enum_data) = enum_data.input.data {
            enum_data.variants =
                _enum_data
                    .variants
                    .clone()
                    .into_pairs()
                    .try_fold(vec![], |mut acc, v| {
                        let val = VariantStruct::new(v.value())?;
                        acc.push(val);
                        Ok::<_, syn::Error>(acc)
                    })?;
        }

        Ok(enum_data)
    }
}

impl EnumData {
    /// Renders enum data code,
    ///  
    pub fn render(&self) -> TokenStream {
        let from_str_impl = self.render_from_str();
        let register = self.render_register();

        let plugins = self
            .variants
            .iter()
            .filter(|v| v.plugin)
            .map(|v| v.render_plugin_struct());

        quote_spanned! {self.input.ident.span()=>
            #from_str_impl
            #register

            #(#plugins)*
        }
    }

    /// Renders a register fn,
    ///
    pub fn render_register(&self) -> TokenStream {
        let name = &self.input.ident;

        let (_, ty_generics, where_clause) = self.input.generics.split_for_impl();

        let variants = self.variants.iter().filter(|v| v.plugin).map(|v| {
            let ty = &v.variant.ident;
            quote_spanned!(v.variant.span()=>
                _parser.with_object_type::<Thunk<#ty>>();
            )
        });

        quote_spanned! {name.span()=>
            impl #name #ty_generics #where_clause  {
                pub fn register(host: &mut impl RegisterWith) {
                  host.register_with(|_parser| {
                    #(#variants)*
                  });
                }
            }
        }
    }

    /// Renders a from_str implementation,
    ///
    pub fn render_from_str(&self) -> TokenStream {
        let name = &self.input.ident;
        let (impl_generics, ty_generics, where_clause) = &self.input.generics.split_for_impl();

        if let syn::Data::Enum(enum_data) = &self.input.data {
            let variants = enum_data
                .variants
                .clone()
                .into_pairs()
                .map(|p| p.into_value());

            let cases = variants.map(|v| {
                let variant_lit_str =
                    syn::LitStr::new(&v.ident.to_string().to_lowercase(), v.ident.span());
                let variant = &v.ident;
                let body = if v.fields.iter().any(|f| f.colon_token.is_none()) {
                    let fields = v.fields.iter().map(|f| {
                        let ty = &f.ty;
                        quote_spanned!(f.ident.span()=>
                            #ty::default()
                        )
                    });

                    quote_spanned!(variant.span()=>
                        (#(#fields),*)
                    )
                } else if v.fields.is_empty() {
                    quote::quote!()
                } else {
                    let fields = v.fields.iter().map(|f| {
                        let name = &f.ident;
                        let ty = &f.ty;
                        quote_spanned!(f.ident.span()=>
                            #name: #ty::default(),
                        )
                    });

                    quote_spanned!(variant.span()=>
                        { #(#fields),* }
                    )
                };

                quote_spanned! {name.span()=>
                    #variant_lit_str => {
                        Ok(#name::#variant #body)
                    }
                }
            });

            quote_spanned!(self.input.ident.span()=>
                impl #impl_generics FromStr for #name #ty_generics #where_clause {
                    type Err = anyhow::Error;

                    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                        match s {
                            #(#cases)*
                            _ => {
                                Err(anyhow::anyhow!("Unrecognized type, {s}"))
                            }
                        }
                    }
                }
            )
        } else {
            quote::quote!()
        }
    }
}
