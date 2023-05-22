use proc_macro2::Ident;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use syn::parse::Parse;
use syn::parse2;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;
use syn::LitStr;
use syn::Path;
use syn::Visibility;

use crate::struct_field::StructField;

/// Parses a struct from derive attribute,
///
/// Generates impl's for Load, Config, and Apply traits
///
pub(crate) struct StructData {
    /// Name of the struct,
    ///
    name: Ident,
    /// Visibility of struct,
    ///
    vis: Visibility,
    /// Parsed struct fields,
    ///
    fields: Vec<StructField>,
    /// Types to add to dispatch trait impl,
    ///
    compile: Vec<Path>,
}

impl Parse for StructData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let derive_input = DeriveInput::parse(input)?;

        let name = derive_input.ident;

        let mut compile: Vec<Path> = vec![];

        for attr in derive_input.attrs.iter() {
            if attr.path().is_ident("compile") {
                if let Some(args) = attr
                    .parse_args_with(
                        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                    )
                    .ok()
                {
                    compile = args.iter().cloned().collect();
                }

                if compile.is_empty() {
                    compile.push(attr.path().clone());
                }
            }
        }

        let fields = if let Data::Struct(data) = &derive_input.data {
            let named = parse2::<FieldsNamed>(data.fields.to_token_stream())?;
            named
                .named
                .iter()
                .filter_map(|n| parse2::<StructField>(n.to_token_stream()).ok())
                .filter(|f| !f.ignore)
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        Ok(Self {
            name,
            vis: derive_input.vis,
            fields,
            compile,
        })
    }
}

impl StructData {
    pub(crate) fn linker_ty_ident(&self) -> Ident {
        format_ident!("{}Linker", self.name)
    }

    pub(crate) fn extensions_enum(&self) -> TokenStream {
        let ty_ident = self.linker_ty_ident();

        // Extension components that link to this type
        let extensions = self.fields.iter().filter(|f| !f.ignore && f.ext).map(|f| {
            let i = f.extension_interpolation_variant(&self.name);
            quote_spanned! {f.span=>
                #i
            }
        });

        let roots = self.fields.iter().filter(|f|!f.ignore && f.root).map(|f| {
            let i = f.root_interpolation_variant();
            quote_spanned! {f.span=>
                #i
            }
        });

        let vis = &self.vis;
        let doc_comment = LitStr::new(format!("Enumeration of identifier patterns that link to {}", self.name.to_string()).as_str(), self.name.span());
        quote_spanned! {self.name.span()=>
            #[doc = #doc_comment]
            #[patterns]
            #vis enum #ty_ident {
                #( #roots ),*
                #( #extensions ),*
            }
        }
    }

    /// Returns token stream of impl for the Apply trait
    ///
    pub(crate) fn apply_trait(&self) -> TokenStream {
        let name = &self.name;

        let map = self.fields.iter().map(|f| f.apply_expr());
        let fields = quote! {
            #( #map ),*
        };

        quote! {
            impl reality::v2::Apply for #name {
                fn apply(&self, rule_name: impl AsRef<str>, property: &reality::v2::Property) -> reality::Result<reality::v2::Property> {
                    let rule_name = rule_name.as_ref();
                    match rule_name {
                        #fields
                        _ => {
                            Ok(property.clone())
                        }
                    }
                }
            }
        }.into()
    }

    pub(crate) fn visit_trait(&self) -> TokenStream {
        let name = &self.name;

        let visits = self
            .fields
            .iter()
            .filter(|f| !f.ignore && !f.ext)
            .map(|f| f.visit_expr());

        quote! {
            impl<'a> reality::v2::prelude::Visit for &'a #name {
                fn visit(&self, context: (), visitor: &mut impl reality::v2::Visitor) -> reality::Result<()> {
                    #( #visits )*
                    Ok(())
                }
            }
        }
    }

    /// Returns token stream of impl for the Config trait,
    ///
    pub(crate) fn config_trait(&self) -> TokenStream {
        let name = &self.name;

        let root_config_pattern_match = self
            .fields
            .iter()
            .filter(|f| f.root)
            .map(|f| {
                let name = f.root_name();
                let lit = LitStr::new(&name.to_string(), f.span);
                quote_spanned! {lit.span()=>
                    #lit => {
                        let ident = format!("{:#}", ident).replace(#lit, "").trim_matches('.').parse::<reality::Identifier>()?;
                        reality::v2::Config::config(&mut self.#name, &ident, property)?;
                    }
                }
            });

        let root_fields = self
            .fields
            .iter()
            .filter(|f| f.root)
            .map(|f| (f.root_name()));

        let ext_fields = self
            .fields
            .iter()
            .filter(|f| !f.root)
            .map(|f| f.config_apply_root_expr(root_fields.clone().collect()));

        let compile_trait = if !self.compile.is_empty() {
            self.compile_trait()
        } else {
            quote! {}
        };

        quote! {
            #[allow(non_snake_case)]
            impl reality::v2::Config for #name {
                fn config(&mut self, ident: &reality::Identifier, property: &reality::v2::Property) -> reality::Result<()> {
                    match ident.root().as_str() {
                        #( #root_config_pattern_match ),*
                        _ => {
                        }
                    }

                    match ident.subject().as_str() {
                        #( #ext_fields ),*
                        _ => {
                        }
                    }

                    Ok(())
                }
            }

            #compile_trait
        }
    }

    /// Returns token stream of generated Load trait,
    ///
    pub(crate) fn load_trait(&self) -> TokenStream {
        let ident = &self.name;
        let format_ident = format_ident!("{}Format", ident);
        let systemdata_ident = format_ident!("{}SystemData", ident);

        let types = self
            .reference_fields()
            .map(|f| f.join_tuple_storage_type_expr());
        let types = quote! {
            #( #types ),*
        };

        let idents = self.reference_fields().map(|f| &f.name);
        let idents = quote! {
            #( #idents ),*
        };

        let systemdata_body = self.reference_fields().map(|f| f.system_data_expr());
        let systemdata_body = quote! {
            #( #systemdata_body ),*
        };

        let systemdata_provider_body = self.reference_fields().map(|f| f.system_data_ref_expr());
        let systemdata_provider_body = quote! {
            #( #systemdata_provider_body ),*
        };

        let load_struct_expr = self
            .fields
            .iter()
            .find(|f| f.ty.is_ident("Entity") && f.name.is_ident("entity"))
            .map(|f| {
                let ident = f.name.get_ident().unwrap();
                quote_spanned! {ident.span()=>
                    Self { entity, #idents }
                }
            })
            .unwrap_or(quote! {
                Self { #idents }
            });

        let vis = &self.vis;

        quote! {
            use specs::prelude::*;

            #vis type #format_ident<'a> = ( #types );

            #[derive(specs::SystemData)]
            #vis struct #systemdata_ident<'a> {
                entities: specs::Entities<'a>,
                #systemdata_body
            }

            impl<'a> reality::state::Load for #ident<'a> {
                type Layout = #format_ident<'a>;

                fn load(entity: Entity, (#idents): <Self::Layout as specs::Join>::Type) -> Self {
                    #load_struct_expr
                }
            }

            impl<'a> reality::state::Provider<'a, #format_ident<'a>> for #systemdata_ident<'a> {
                fn provide(&'a self) -> #format_ident<'a> {
                    (
                        #systemdata_provider_body
                    )
                }
            }

            impl<'a> AsRef<specs::Entities<'a>> for #systemdata_ident<'a> {
                fn as_ref(&self) -> &specs::Entities<'a> {
                    &self.entities
                }
            }
        }
    }

    /// Returns an impl for the compile trait,
    ///
    pub fn compile_trait(&self) -> TokenStream {
        let name = &self.name;

        let bootstraps = self.compile.iter().filter_map(|c| {
            if let Some(ident) = c.get_ident() {
                let ty_name = ident.to_string().replace("Thunk", "");
                let ty = format_ident!("{}", ty_name);
                Some(quote! {
                    .transform(|s| <#name as #ty>::__bootstrap(s))
                })
            } else {
                None
            }
        });

        let map = self.compile.iter().filter_map(|c| {
            if let Some(ident) = c.get_ident() {
                let ty_name = ident.to_string().replace("Thunk", "");
                let name = format_ident!("thunk_{}", ty_name.to_lowercase());
                Some(quote! {
                    .map(|b|{
                        Ok(#name(b.clone()))
                    })
                })
            } else {
                None
            }
        });
        let compile_thunks = quote! {
            #( #map )*
        };

        quote! {
            impl reality::v2::Dispatch for #name {
                fn dispatch<'a>(
                    &self,
                    dispatch_ref: reality::v2::DispatchRef<'a, reality::v2::Properties>,
                ) -> reality::v2::DispatchResult<'a> {
                    dispatch_ref
                        #( #bootstraps ) *
                        .transmute::<#name>()
                        #compile_thunks
                        .map(|_| {
                            Ok(reality::v2::Instance::Ready)
                        })
                        .transmute::<Properties>()
                        .result()
                }
            }
        }
    }

    /// Generates the Runmd trait as well as other traits so that the concrete type can be linked w/ entities in storage,
    ///
    /// **Example**
    /// ```
    /// #[derive(Runmd, Debug, Clone, Component)]
    /// #[storage(specs::VecStorage)]
    /// #[compile(ThunkCall)]
    /// pub struct Println {
    ///     /// Input to the println component,
    ///     println: String,
    ///     /// Map of properties that can be used to format lines being printed,
    ///     #[config(ext=plugin.map)]
    ///     fmt: Vec<String>,
    ///     /// Lines to print to stderr,
    ///     #[config(ext=plugin.format)]
    ///     stderr: Vec<String>,
    ///     /// Lines to print to stdout,
    ///     #[config(ext=plugin.format)]
    ///     stdout: Vec<String>,
    ///     /// Plugin extension
    ///     #[ext]
    ///     plugin: Plugin,
    /// }
    /// ```
    ///
    /// Would generate,
    /// 
    /// ```
    /// impl reality::v2::Runmd for Println {
    ///     type Extensions = PrintlnExtensions;
    /// }
    /// impl reality::v2::Visitor for Println {
    ///     fn visit_property(&mut self, name: &str, property: &reality::v2::Property) {
    ///        match name {
    ///            "println" => {
    ///                 self.println.visit_property(name, property);
    ///             }
    ///            "fmt" => {
    ///                 self.fmt.visit_property(name, property);
    ///             }
    ///             "stderr" => {
    ///                 self.stderr.visit_property(name, property);
    ///             }
    ///             "stdout" => {
    ///                 self.stdout.visit_property(name, property);
    ///             }
    ///             _ => {}
    ///         }
    ///       self.plugin.visit_property(name, property);
    ///     }
    ///     fn visit_extension(&mut self, ident: &reality::Identifier) {
    ///         self.plugin.visit_extension(ident);
    ///     }
    /// }
    /// impl reality::v2::Dispatch for Println {
    ///     fn dispatch<'a>(
    ///         &self,
    ///         dispatch_ref: reality::v2::DispatchRef<'a, reality::v2::Properties>,
    ///     ) -> reality::v2::DispatchResult<'a> {
    ///         dispatch_ref
    ///             .transform(|s| <Println as Call>::__bootstrap(s))
    ///             .transmute::<Println>()
    ///             .map(|b| Ok(reality::v2::thunk_call(b.clone())))
    ///             .map(|_| Ok(reality::v2::Instance::Ready))
    ///             .transmute::<Properties>()
    ///             .result()
    ///     }
    /// }
    /// #[doc = r" Enumeration of extension patterns,"]
    /// #[doc = r" "]
    /// #[patterns]
    /// pub enum PrintlnExtensions {
    ///     #[interpolate("!#block#.#root#.plugin.println;")]
    ///     PluginRoot,
    ///     #[interpolate("!#block#.#root#.plugin.println.(config).(property);")]
    ///     PluginConfig,
    ///     #[interpolate("#block#.#root#.plugin.println.(?property).(?value);")]
    ///     Plugin,
    /// }
    /// impl<'a> reality::v2::prelude::Visit for &'a Println {
    ///     fn visit(&self, context: (), visitor: &mut impl reality::v2::Visitor) -> reality::Result<()> {
    ///         <reality::v2::prelude::Property as reality::v2::prelude::Visit<
    ///             reality::v2::prelude::Name<'a>,
    ///         >>::visit(
    ///             &reality::v2::prelude::Property::from(&self.println),
    ///             "println",
    ///             visitor,
    ///         )?;
    ///         <reality::v2::prelude::Property as reality::v2::prelude::Visit<
    ///             reality::v2::prelude::Name<'a>,
    ///         >>::visit(&self.plugin.map("fmt", &self.fmt), "fmt", visitor)?;
    ///         <reality::v2::prelude::Property as reality::v2::prelude::Visit<
    ///             reality::v2::prelude::Name<'a>,
    ///         >>::visit(
    ///             &self.plugin.format("stderr", &self.stderr),
    ///             "stderr",
    ///             visitor,
    ///         )?;
    ///         <reality::v2::prelude::Property as reality::v2::prelude::Visit<
    ///             reality::v2::prelude::Name<'a>,
    ///         >>::visit(
    ///             &self.plugin.format("stdout", &self.stdout),
    ///             "stdout",
    ///             visitor,
    ///         )?;
    ///         Ok(())
    ///     }
    /// }
    /// type PrintlnCompilerEvents<'a> = reality::v2::prelude::CompilerEvents<'a, Println>;
    /// impl<'linking> reality::v2::prelude::Visit<PrintlnCompilerEvents<'linking>> for PrintlnExtensions {
    ///     fn visit(
    ///         &self,
    ///         context: PrintlnCompilerEvents<'linking>,
    ///         visitor: &mut impl reality::v2::prelude::Visitor,
    ///     ) -> reality::Result<()> {
    ///         match context {
    ///             CompilerEvents::Config(properties) => match self {
    ///                 PrintlnExtensions::PluginRoot {} => {
    ///                     if properties.len() > 0 {
    ///                         visitor.visit_property(
    ///                             "plugin",
    ///                             &reality::v2::prelude::Property::Properties(properties.clone().into()),
    ///                         );
    ///                     } else {
    ///                         visitor.visit_property("plugin", &reality::v2::prelude::Property::Empty);
    ///                     }
    ///                     return Ok(());
    ///                 }
    ///                 PrintlnExtensions::PluginConfig { config, property } => {
    ///                     if properties.len() > 0 {
    ///                         visitor.visit_property(
    ///                             &format!("plugin.{}.{}", config, property),
    ///                             &reality::v2::prelude::Property::Properties(properties.clone().into()),
    ///                         );
    ///                     } else {
    ///                         visitor.visit_property(
    ///                             &format!("plugin.{}.{}", config, property),
    ///                             &reality::v2::prelude::Property::Empty,
    ///                         );
    ///                     }
    ///                     return Ok(());
    ///                 }
    ///                 _ => {}
    ///             },
    ///             CompilerEvents::Load(loading) => match self {
    ///                 PrintlnExtensions::Plugin {
    ///                     property: Some(property),
    ///                     value: None,
    ///                 } => {
    ///                     visitor.visit_symbol("println", None, property);
    ///                     <Println as reality::v2::prelude::Visit<&Plugin>>::visit(
    ///                         &loading,
    ///                         &loading.plugin,
    ///                         visitor,
    ///                     )?;
    ///                     return Ok(());
    ///                 }
    ///                 PrintlnExtensions::Plugin {
    ///                     property: Some(property),
    ///                     value: Some(value),
    ///                 } => {
    ///                     visitor.visit_symbol(property, None, value);
    ///                     <Println as reality::v2::prelude::Visit<&Plugin>>::visit(
    ///                         &loading,
    ///                         &loading.plugin,
    ///                         visitor,
    ///                     )?;
    ///                     return Ok(());
    ///                 }
    ///                 PrintlnExtensions::Plugin {
    ///                     property: None,
    ///                     value: None,
    ///                 } => {
    ///                     <Println as reality::v2::prelude::Visit<&Plugin>>::visit(
    ///                         &loading,
    ///                         &loading.plugin,
    ///                         visitor,
    ///                     )?;
    ///                     return Ok(());
    ///                 }
    ///                 _ => {}
    ///             },
    ///         }
    ///         Err(reality::Error::skip())
    ///     }
    /// }
    /// impl Visit<&Plugin> for Println {
    ///     fn visit(&self, context: &Plugin, visitor: &mut impl Visitor) -> Result<()> {
    ///         context.visit((), visitor)
    ///     }
    /// }
    /// #[doc = r" This module was generated in order to load instances from storage"]
    /// #[doc = r" "]
    /// #[allow(non_snake_case)]
    /// pub mod Println__states {
    ///     use super::Println;
    ///     use specs::Entity;
    ///     #[derive(reality::v2::prelude::Load)]
    ///     pub struct PrintlnInstance<'a> {
    ///         pub entity: Entity,
    ///         pub instance: &'a reality::v2::prelude::Instance,
    ///         pub ty: &'a Println,
    ///     }
    /// }
    /// ```
    pub fn runmd_trait(&self) -> TokenStream {
        let name = &self.name;
        let name_lit = LitStr::new(name.to_string().as_str(), name.span());

        let visitor_trait = self.visitor_trait();
        let compile_trait = self.compile_trait();
        let extensions_enum = self.extensions_enum();
        let extensions_enum_ident = self.linker_ty_ident();
        let visit_trait = self.visit_trait();
        let visit_extensions_trait = self.visit_extensions_trait();
        let runmd_instance_state = self.runmd_instance_state();

        
        let mod_name = &self.runmd_instance_state_mod_ident();
        let instance_name = &self.runmd_instance_ident();
        let instance_system_data_name = &format_ident!("{}SystemData", instance_name);
        let instance_provider_alias = &format_ident!("{}Provider", name);

        quote! {
            pub type #instance_provider_alias<'a> = <#name as reality::v2::Runmd>::InstanceSystemData<'a>;

            pub type #instance_name<'a> = <#name as reality::v2::Runmd>::Instance<'a>;

            impl reality::v2::Runmd for #name {
                type InstanceSystemData<'instance> = #mod_name::#instance_system_data_name<'instance>;
                
                type Instance<'instance> = #mod_name::#instance_name<'instance>;
                
                type Linker = #extensions_enum_ident;

                fn type_name() -> &'static str {
                    #name_lit
                }
            }

            #visitor_trait
            #compile_trait
            #extensions_enum

            #visit_trait
            #visit_extensions_trait

            #runmd_instance_state
        }
    }

    /// Returns an impl for the visitor trait,
    ///
    pub fn visitor_trait(&self) -> TokenStream {
        let name = &self.name;

        let visit_property = self
            .fields
            .iter()
            .filter(|f| !f.ignore && !f.block && !f.root && !f.ext)
            .map(|f| f.visitor_expr());

        let visit_block = self.visit_block();
        let visit_root = self.visit_root();
        let visit_ext = self.visit_extension();

        let visit_ext_property = self
            .fields
            .iter()
            .filter(|f| !f.ignore && (f.block || f.root || f.ext))
            .filter_map(|f| f.name.get_ident())
            .map(|n| {
                quote_spanned! {n.span()=>
                    self.#n.visit_property(name, property);
                }
            });

        quote! {
            impl reality::v2::Visitor for #name {
                fn visit_property(&mut self, name: &str, property: &reality::v2::Property) {
                    match name {
                        #( #visit_property ),*
                        _ => {
                        }
                    }

                    #( #visit_ext_property )*
                }

                #visit_block

                #visit_ext

                #visit_root
            }
        }
    }

    fn visit_extension(&self) -> TokenStream {
        let visit_ext = self
            .fields
            .iter()
            .filter(|f| !f.ignore && !f.block && !f.root && f.ext)
            .map(|f| f.visitor_expr())
            .collect::<Vec<_>>();

        if !visit_ext.is_empty() {
            let visit_ext = visit_ext.iter();
            quote! {
                fn visit_extension(&mut self, ident: &reality::Identifier) {
                    #( #visit_ext )*
                }
            }
        } else {
            quote! {}
        }
    }

    fn visit_root(&self) -> TokenStream {
        let visit_root = self
            .fields
            .iter()
            .filter(|f| !f.ignore && !f.block && f.root && !f.ext)
            .map(|f| f.visitor_expr())
            .collect::<Vec<_>>();

        if !visit_root.is_empty() {
            let visit_root = visit_root.iter();
            quote! {
                fn visit_root(&mut self, root: &reality::v2::Root) {
                    #( #visit_root )*
                }
            }
        } else {
            quote! {}
        }
    }

    fn visit_block(&self) -> TokenStream {
        let visit_block = self
            .fields
            .iter()
            .filter(|f| !f.ignore && f.block && !f.root && !f.ext)
            .map(|f| f.visitor_expr())
            .collect::<Vec<_>>();

        if !visit_block.is_empty() {
            let visit_block = visit_block.iter();
            quote! {
                fn visit_block(&mut self, root: &reality::v2::Block) {
                    #( #visit_block )*
                }
            }
        } else {
            quote! {}
        }
    }

    fn reference_fields(&self) -> impl Iterator<Item = &StructField> {
        self.fields.iter().filter(|f| f.reference)
    }

    fn runmd_instance_state_mod_ident(&self) -> Ident {
        let name = &self.name;
        format_ident!("{}__states", name)
    }

    fn runmd_instance_ident(&self) -> Ident {
        let name = &self.name;
        format_ident!("{}Instance", name)
    }

    fn runmd_instance_state(&self) -> TokenStream {
        let name = &self.name;
        let mod_name = self.runmd_instance_state_mod_ident();
        let instance_name = self.runmd_instance_ident();

        quote_spanned! {name.span()=>
            /// This module was generated in order to load instances from storage
            ///
            #[allow(non_snake_case)]
            pub mod #mod_name {
                use super::#name;
                use specs::Entity;

                #[derive(reality::v2::prelude::Load)]
                pub struct #instance_name<'a> {
                    pub entity: Entity,
                    pub instance: &'a reality::v2::prelude::Instance,
                    pub ty: &'a #name,
                }
            }
        }
    }

    fn visit_extensions_trait(&self) -> TokenStream {
        let name = &self.name;
        let type_alias_ident = format_ident!("{}CompilerEvents", self.name);
        let linker_ident = format_ident!("{}Linker", self.name);

        let config_fields = self.fields.iter().filter(|f| !f.ignore && f.ext).map(|f| {
            let match_arms = f.visit_config_extensions(&self.name);
            quote_spanned! {f.span=>
                #match_arms
            }
        });

        let load_fields = self.fields.iter().filter(|f| !f.ignore && f.ext).map(|f| {
            let match_arms = f.visit_load_extensions(&self.name);
            quote_spanned! {f.span=>
                #match_arms
            }
        });

        let visit_fields = self
            .fields
            .iter()
            .filter(|f| !f.ignore && f.ext)
            .map(|f| f.visit_trait(&self.name));

        quote! {
            type #type_alias_ident<'a> = reality::v2::prelude::CompilerEvents<'a, #name>;

            impl<'linking> reality::v2::prelude::Visit<#type_alias_ident<'linking>> for #linker_ident {
                fn visit(&self, context: #type_alias_ident<'linking>, visitor: &mut impl reality::v2::prelude::Visitor) -> reality::Result<()> {
                    match context {
                        CompilerEvents::Config(properties) => match self {
                            #( #config_fields )*
                            _ => {

                            }
                        },
                        CompilerEvents::Load(loading) => match self {
                            #( #load_fields )*
                            _ => {

                            }
                        },
                    }

                    Err(reality::Error::skip())
                }
            }

            #( #visit_fields )*
        }
    }
}
