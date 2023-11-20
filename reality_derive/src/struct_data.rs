use std::collections::HashMap;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;
use quote::quote_spanned;
use syn::GenericParam;
use syn::Generics;
use syn::ImplGenerics;
use syn::LitStr;
use syn::Path;
use syn::Token;
use syn::TypeGenerics;
use syn::WhereClause;
use syn::parse::Parse;
use syn::parse2;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;

use crate::struct_field::StructField;

/// Parses a struct from derive attribute,
/// 
/// ``` norun
/// #[derive(AttributeType)]
/// #[reality(rename="")]
/// struct Test {
///        
/// }
/// ```
#[derive(Clone)]
pub(crate) struct StructData {
    /// Span of the struct being derived,
    /// 
    span: Span,
    /// Name of the struct,
    ///
    name: Ident,
    /// Generics
    /// 
    generics: Generics,
    /// Parsed struct fields,
    ///
    fields: Vec<StructField>,
    plugin: bool,
    ext: bool,
    /// Reality attribute, rename option
    /// 
    reality_rename: Option<LitStr>,
    /// Reality attribute, on_load fn path,
    /// 
    reality_on_load: Option<Path>,
    /// Reality attribute, on_unload fn path,
    /// 
    reality_on_unload: Option<Path>,
    /// Reality attribute, on_completed fn path,
    /// 
    reality_on_completed: Option<Path>,
    /// Group name,
    /// 
    group: Option<LitStr>,
    /// CallAsync fn,
    /// 
    call: Option<Ident>,
}

impl Parse for StructData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let derive_input = DeriveInput::parse(input)?;

        let name = derive_input.ident;

        let mut reality_rename = None;
        let mut reality_on_load = None;
        let mut reality_on_unload = None;
        let mut reality_on_completed = None;
        let mut group = None;
        let mut plugin = false;
        let mut ext = false;
        let mut enum_flags = false;
        let mut call = None;

        for attr in derive_input.attrs.iter() {
            if attr.path().is_ident("reality") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("call") {
                        meta.input.parse::<Token![=]>()?;
                        call = meta.input.parse::<Ident>().ok();
                    }

                    if meta.path.is_ident("rename") {
                        meta.input.parse::<Token![=]>()?;
                        reality_rename = meta.input.parse::<LitStr>().ok();
                    }

                    if meta.path.is_ident("group") {
                        meta.input.parse::<Token![=]>()?;
                        group = meta.input.parse::<LitStr>().ok();
                    }
                    
                    if meta.path.is_ident("load") {
                        meta.input.parse::<Token![=]>()?;
                        reality_on_load = meta.input.parse::<Path>().ok();
                    }

                    if meta.path.is_ident("unload") {
                        meta.input.parse::<Token![=]>()?;
                        reality_on_unload = meta.input.parse::<Path>().ok();
                    }

                    if meta.path.is_ident("completed") {
                        meta.input.parse::<Token![=]>()?;
                        reality_on_completed = meta.input.parse::<Path>().ok();
                    }

                    if meta.path.is_ident("plugin") {
                        plugin = true;
                    }

                    if meta.path.is_ident("ext") {
                        ext = true;
                    }

                    if meta.path.is_ident("enum_flags") {
                        enum_flags = true;
                    }
                    Ok(())
                })?;
            }
        }

        let fields = match &derive_input.data {
            Data::Struct(data) if data.semi_token.is_none() => {
                let named = parse2::<FieldsNamed>(data.fields.to_token_stream())?;
                named
                    .named
                    .iter()
                    .filter_map(|n| parse2::<StructField>(n.to_token_stream()).ok())
                    .filter(|f| !f.ignore)
                    .enumerate()
                    .map(|(idx, mut f)| {
                        f.offset = idx;
                        f
                    })
                    .collect::<Vec<_>>()
            }
            Data::Enum(data) => { 
                let mut variants = vec![];
                let fields = data.variants.clone()
                    .into_pairs()
                    .map(|pair| {
                        let variant = pair.into_value();
                        variants.push(variant.clone());
                        variant
                    })
                    .flat_map(|v| {
                        let variant_name = v.ident.clone();
                        v.fields.iter()
                            .filter_map(|n| parse2::<StructField>(n.to_token_stream()).ok())
                            .map(|mut f| { 
                                f.variant = Some((variant_name.clone(), name.clone()));
                                f
                            })
                            .collect::<Vec<_>>()
                    })
                    .enumerate()
                    .map(|(idx, mut f)| {
                        f.offset = idx;
                        f
                    })
                    .collect();
                    fields
            }
            _ => vec![],
        };

        if let Some(lifetime) = derive_input.generics.lifetimes().find(|l| l.lifetime.ident != format_ident!("static")) {
            Err(input.error(format!("Struct must be `'static`, therefore may not contain any fields w/ generic lifetimes. Please remove `'{}`", lifetime.lifetime.ident)))
        } else {
            Ok(Self {
                span: input.span(),
                name,
                group,
                generics: derive_input.generics,
                fields,
                reality_rename,
                reality_on_load,
                reality_on_unload,
                reality_on_completed,
                plugin,
                ext,
                call,
            })
        }
    }
}

impl StructData {
    /// Implements visit traits,
    /// 
    pub(crate) fn visit_trait(&self, impl_generics: &ImplGenerics<'_>, ty_generics: &TypeGenerics<'_>, where_clause: &Option<&WhereClause>) -> TokenStream {
        let fields = self.fields.iter().fold(HashMap::new(), |mut acc, f| {
            if !acc.contains_key(&f.ty) {
                acc.insert(f.ty.clone(), vec![f.clone()]);
            } else if let Some(list) = acc.get_mut(&f.ty) {
                list.push(f.clone());
            }

            acc
        });

        let visit_impls = fields.iter().map(|(ty, fields)| {
            let owner = &self.name;

            let _fields = fields.iter().map(|f| {
                let ty = f.field_ty();
                let offset = &f.offset;
                quote_spanned!(f.span=> 
                    <Self as OnParseField<#offset, #ty>>::get_field(self)
                )
            });

            let _fields_mut = fields.iter().filter_map(|f| {
                let ty = f.field_ty();
                let offset = &f.offset;
                let name_lit = f.field_name_lit_str();
                let name = &f.name;
                if f.variant.is_some() {
                    None
                } else {
                    Some(quote_spanned!(f.span=>
                        FieldMut { owner: std::any::type_name::<#ty>(), name: #name_lit, offset: #offset, value: &mut self.#name }
                    ))
                }
            });

            let _set_field_cases = fields.iter().map(|f| {
                let ty = f.field_ty();
                let offset = &f.offset;
                let field_name_lit = f.field_name_lit_str();
                quote_spanned!(f.span=>
                    (#field_name_lit, #offset) => { *<Self as OnParseField<#offset, #ty>>::get_field_mut(self).value = field.value; true }
                )
            });
 
            let _fromstr_derive = {
                fields.iter().find(|f| f.derive_fromstr).map(|f| {
                    let name = &f.name;
    
                    quote_spanned!(self.span=>
                      impl #impl_generics std::str::FromStr for #owner #ty_generics #where_clause {
                          type Err = anyhow::Error;
    
                          fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                              let mut _s = Self::default();
                              _s.#name = s.parse()?;
                              Ok(_s)
                          }
                        }
                      )
                }).unwrap_or(quote!())
            };

            quote_spanned!(self.span=>
                #_fromstr_derive

                impl #impl_generics Visit<#ty> for #owner #ty_generics #where_clause {
                    fn visit<'a>(&'a self) -> Vec<Field<'a, #ty>> {
                        vec![
                            #(#_fields),*
                        ]
                    }
                }

                impl #impl_generics VisitMut<#ty> for #owner #ty_generics #where_clause {
                    fn visit_mut<'a: 'b, 'b>(&'a mut self) -> Vec<FieldMut<'b, #ty>> {
                        vec![
                            #(#_fields_mut),*
                        ]
                    }
                }

                impl #impl_generics SetField<#ty> for #owner #ty_generics #where_clause {
                    fn set_field(&mut self, field: FieldOwned<#ty>) -> bool {
                        if field.owner.as_str() != std::any::type_name::<Self>() {
                            return false;
                        }

                        match (field.name.as_str(), field.offset) {
                            #(#_set_field_cases),*
                            _ => {
                                false
                            }
                        }
                    }
                }
            )
        });

        quote_spanned!(self.span=> 
            #(#visit_impls)*
        )
    }

    /// Returns token stream of generated AttributeType trait
    /// 
    pub(crate) fn attribute_type_trait(mut self) -> TokenStream {
        let ident = &self.name;
        let original = self.generics.clone();
        let (original_impl_generics, ty_generics, _) = original.split_for_impl();
        let ty_generics = ty_generics.clone();
        self.generics.params.push(
            parse2::<GenericParam>(
                quote!(S: StorageTarget + Send + Sync + 'static)).expect("should be able to tokenize")
            );

        let (impl_generics, _, where_clause) = &self.generics.split_for_impl();
        
        let visit_impl = self.visit_trait(&original_impl_generics, &ty_generics, where_clause);
        
        let symbol = self.attr_symbol(ident);
        let fields = self.fields.clone();
        let fields = fields.iter().enumerate().map(|(offset, f)| {
            let ty = &f.field_ty();
            if let Some(attribute_type) = f.attribute_type.as_ref() {
                quote_spanned! {f.span=>
                    parser.add_parseable_attribute_type_field::<#offset, Self, #attribute_type>();
                }
            } else if f.ext {
                quote_spanned! {f.span=>
                    parser.add_parseable_extension_type_field::<#offset, Self, #ty>();
                }
            } else {
                let comment = LitStr::new(format!("Parsing field `{}`", f.name).as_str(), Span::call_site());
                quote_spanned! {f.span=>
                    let _ = #comment;
                    parser.add_parseable_field::<#offset, Self, #ty>();
                }
            }
        });

        //  Implementation for fields parsers,
        // 
        let fields_on_parse_impl = self.fields.iter().map(|f| {
            let field_ident = f.field_name_lit_str();
            let ty = f.field_ty();
            let absolute_ty = &f.ty;
            let offset = &f.offset;

            // Callback to use
            let callback = f.render_field_parse_callback();
            let get_fn = f.render_get_fn();
            let get_mut_fn = f.render_get_mut_fn();

            let mut_value = f.set_of.as_ref().or(f.map_of.as_ref()).map(|_| quote!(mut));

            quote_spanned! {f.span=>
                impl #original_impl_generics OnParseField<#offset, #ty> for #ident #ty_generics #where_clause {
                    type ProjectedType = #absolute_ty;

                    fn field_name() -> &'static str {
                        #field_ident
                    }
                
                    #[allow(unused_variables)]
                    fn on_parse(&mut self, #mut_value value: #ty, _tag: Option<&String>) -> ResourceKey<Property> {
                        let mut hasher = ResourceKeyHashBuilder::new_default_hasher();
                        hasher.hash(_tag);
                        hasher.hash(#offset);
                        hasher.hash(#field_ident);
                        hasher.hash(std::any::type_name::<#ty>());
                        hasher.hash(std::any::type_name::<Self>());

                        #callback
                    }

                    #[inline]
                    fn get(&self) -> &Self::ProjectedType {
                        #get_fn
                    }

                    #[inline]
                    fn get_mut(&mut self) -> &mut Self::ProjectedType {
                        #get_mut_fn
                    }
                }
            }
        });

        quote_spanned! {self.span=> 
            impl #impl_generics AttributeType<S> for #ident #ty_generics #where_clause {
                fn symbol() -> &'static str {
                    #symbol
                }

                fn parse(parser: &mut AttributeParser<S>, content: impl AsRef<str>) {
                    let mut enable = parser.parse_attribute::<Self>(content.as_ref());

                    if enable.is_ok() {
                        #(#fields)*
                    }
                }
            }

            #(#fields_on_parse_impl)*

            #visit_impl
        }
    }

    /// Get the attribute ty symbol,
    /// 
    fn attr_symbol(&self, ident: &Ident) -> TokenStream {
        let group = self.group.as_ref().map(|g| quote_spanned!(self.span=> #g)).unwrap_or(quote_spanned!(self.span=> std::env!("CARGO_PKG_NAME")));
        let name = self.reality_rename.clone().unwrap_or(
            LitStr::new(ident.to_string().to_lowercase().as_str(), self.span)
        );
        
        quote_spanned!(self.span=>
            concat!(#group, '.', #name)
        )
    }

     /// Returns token stream of generated AttributeType trait
    /// 
    pub(crate) fn object_type_trait(self) -> TokenStream {
        let name = self.name.clone();
        let original = self.generics.clone();
        let (_impl_generics, ty_generics, _) = original.split_for_impl();
        let ty_generics = ty_generics.clone();
        let mut generics = self.generics.clone();
        generics.params.push(
            parse2::<GenericParam>(
                quote!(Storage: StorageTarget + Send + Sync + 'static)).expect("should be able to tokenize")
            );

        let (impl_generics, _, where_clause) = &generics.split_for_impl();

        let on_load = self.reality_on_load.clone().map(|p| quote!(#p(storage).await;)).unwrap_or_default();
        let on_unload = self.reality_on_unload.clone().map(|p| quote!(#p(storage).await;)).unwrap_or_default();
        let on_completed = self.reality_on_completed.clone().map(|p| quote!(#p(storage))).unwrap_or(quote!(None));

        let ext = self.fields.iter().filter(|f| f.ext).map(|f| {
            let ty = f.field_ty();
            quote_spanned!(f.span=>
                _parser.with_object_type::<Thunk<#ty>>();
            )
        });

        let plugins = self.fields.iter().filter(|f| f.plugin).map(|f| {
            let ty = f.field_ty();
            quote_spanned!(f.span=>
                #ty::register(host); 
            )
        });

        let to_frame = self.fields.iter().filter(|f| !f.ignore && !f.not_wire).map(|f| {
            let offset = f.offset; 
            let ty = f.field_ty();
            let pty = &f.ty;
            let _name = &f.name;
            quote_spanned!(f.span=>
                {
                    let mut packet = <Self as OnParseField<#offset, #ty>>::into_packet(self.#_name.clone());
                    packet.owner_name = std::any::type_name::<#name #ty_generics>().to_string();
                    packet.field_name = <Self as OnParseField<#offset, #ty>>::field_name().to_string();
                    packet.attribute_hash = key.map(|k| k.data);
                    packet.into_wire::<#pty>()
                }
            )
        });

        let synchronizable = self.fields.iter()
            .filter(|f| !f.ignore && f.is_decorated)
            .map(|f| {
                let name = &f.name;

                if f.vec_of.is_some() {
                    quote_spanned!(f.span=>
                        for m in self.#name.iter_mut() {
                            m.sync(context);
                        }
                    )
                } else if f.map_of.is_some() {
                    quote_spanned!(f.span=>
                        for (_, m) in self.#name.iter_mut() {
                            m.sync(context);
                        }
                    )
                } else if f.set_of.is_some() {
                    let __sync_temp = format_ident!("__sync_temp_{}", name);
                    quote_spanned!(f.span=>
                        let mut #__sync_temp = vec![];
                        while let Some(mut m) = self.#name.pop_first() {
                            m.sync(context);
                            #__sync_temp.push(m);
                        }
                        for m in #__sync_temp {
                            self.#name.insert(m);
                        }
                    )
                } else if f.option_of.is_some() {
                    quote_spanned!(f.span=>
                        if let Some(f) = self.#name.as_mut() {
                            f.sync(context);
                        }
                    )
                }else if f.vecdeq_of.is_some() {
                    quote_spanned!(f.span=>
                        for m in self.#name.iter_mut() {
                            m.sync(context);
                        }
                    )
                } else {
                    quote_spanned!(f.span=>
                        self.#name.sync(context);
                    )
                }
            });
        
        let plugin = if self.plugin {
            quote!(
            impl #_impl_generics Plugin for #name #ty_generics #where_clause  {
                #[allow(unused_variables)]
                fn sync(&mut self, context: &ThunkContext) {
                    #(#synchronizable)*
                }
            }

            impl #_impl_generics #name #ty_generics #where_clause  {
                pub fn register(mut host: &mut impl RegisterWith) {
                  #(#plugins)*
                  host.register_with(|_parser| {
                    #(#ext)*
                  }); 
                }
            }
            )
        } else if self.ext {
            quote!(
                impl #_impl_generics #name #ty_generics #where_clause  {
                    pub fn register(mut host: &mut impl RegisterWith) {
                        #(#plugins)*
                        host.register_with(|_parser| {
                            #(#ext)*
                        });
                    }
                }
            )
        } else {
            quote!()
        };

        let call = self.call.as_ref().map(|ref c| {
            quote_spanned!(c.span()=>
                #[async_trait]
                impl #_impl_generics CallAsync for #name #ty_generics #where_clause {
                    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
                        #c(context).await
                    }
                }
            )
        });

        let unit_from_str = if self.fields.is_empty() {
            quote_spanned!(name.span()=>
                impl #_impl_generics FromStr for #name #ty_generics #where_clause {
                    type Err = anyhow::Error;

                    fn from_str(_: &str) -> std::result::Result<Self, Self::Err> {
                        Ok(Self)
                    }
                }
            )
        } else {
            quote!()
        };

        let object_type_trait = quote_spanned!(self.span=>
            #[async_trait]
            impl #impl_generics BlockObject<Storage> for #name #ty_generics #where_clause {
                async fn on_load(storage: AsyncStorageTarget<Storage::Namespace>) {
                    #on_load
                }

                async fn on_unload(storage: AsyncStorageTarget<Storage::Namespace>) {
                    #on_unload
                }

                fn on_completed(storage: AsyncStorageTarget<Storage::Namespace>) -> Option<AsyncStorageTarget<Storage::Namespace>> {
                    #on_completed
                }
            }

            impl #_impl_generics ToFrame for #name #ty_generics #where_clause {
                fn to_frame(&self, key: Option<ResourceKey<Attribute>>) -> Frame {
                    Frame { 
                        recv: self.receiver_packet(key),
                        fields: vec![
                            #(#to_frame),*
                        ]
                    }
                }
            }

            #plugin
            #call
            #unit_from_str
        );


        let mut attribute_type = self.clone().attribute_type_trait();
        attribute_type.extend(object_type_trait);
        attribute_type.extend(self.object_ty_api());
        attribute_type
    }


    fn object_ty_api(self) -> TokenStream {
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let name = &self.name;
        let fields = self.fields.iter().filter(|f| !f.ignore && !f.not_wire).map(|f| {
            let ty = &f.ty;
            let name = f.field_name_lit_str();
            let offset = f.offset;

            quote_spanned!(f.span=>
                (#offset, #name) => {
                    if let Some(value) = value.into_box::<#ty>() {
                        <Self as SetField<#ty>>::set_field(self, FieldOwned { owner, name, offset, value: *value })
                    } else {
                        tracing::error!("Could not read value for {}.{}", stringify!(#ty), #name);
                        false
                    }
                }
            )
        });

        quote_spanned!(self.span=>
            impl #impl_generics SetField<FieldPacket> for #name #ty_generics #where_clause {
                fn set_field(&mut self, field: FieldOwned<FieldPacket>) -> bool {
                    let FieldOwned { owner, name, offset, value } = field;
        
                    match (offset, value.field_name.as_str()) {
                        #(#fields)*
                        _ => false
                    }
                }
            }
        )
    }
}

#[test]
fn test_parse_struct_data() {
    use quote::ToTokens;
    
    let stream = <proc_macro2::TokenStream as std::str::FromStr>::from_str(
        r#"
struct Test {
    #[reality(rename = "Name")]
    name: String,
}
"#,
    )
    .unwrap();

    let mut data = syn::parse2::<StructData>(stream).unwrap();

    let field = data.fields.remove(0);
    assert_eq!(false, field.ignore);
    assert_eq!(Some("\"Name\"".to_string()), field.rename.map(|r| r.to_token_stream().to_string()));
    assert_eq!("name", field.name.to_string().as_str());
    assert_eq!("String", field.ty.to_token_stream().to_string().as_str());
}
