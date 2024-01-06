use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use std::collections::HashMap;
use syn::parse::Parse;
use syn::parse2;
use syn::spanned::Spanned;
use syn::Data;
use syn::DeriveInput;
use syn::FieldsNamed;
use syn::Generics;
use syn::ImplGenerics;
use syn::LitStr;
use syn::Path;
use syn::Token;
use syn::Type;
use syn::TypeGenerics;
use syn::Visibility;
use syn::WhereClause;

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
    vis: Visibility,
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
    rename: Option<LitStr>,
    /// Reality attribute, on_load fn path,
    ///
    on_load: Option<Path>,
    /// Reality attribute, on_unload fn path,
    ///
    on_unload: Option<Path>,
    /// Reality attribute, on_completed fn path,
    ///
    on_completed: Option<Path>,
    /// Group name,
    ///
    group: Option<LitStr>,
    /// CallAsync fn,
    ///
    call: Option<Ident>,
    /// Replace thee
    ///
    replace: Option<Type>,
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
        let mut replace = None;

        for attr in derive_input.attrs.iter() {
            if attr.path().is_ident("plugin_def") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("call") {
                        meta.input.parse::<Token![=]>()?;
                        call = meta.input.parse::<Ident>().ok();
                        plugin = true;
                    }
                    Ok(())
                })?;
            }

            if attr.path().is_ident("parse_def") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("rename") {
                        meta.input.parse::<Token![=]>()?;
                        reality_rename = meta.input.parse::<LitStr>().ok();
                    }

                    if meta.path.is_ident("group") {
                        meta.input.parse::<Token![=]>()?;
                        group = meta.input.parse::<LitStr>().ok();
                    }

                    if meta.path.is_ident("replace") {
                        meta.input.parse::<Token![=]>()?;
                        replace = meta.input.parse::<Type>().ok();
                    }

                    Ok(())
                })?;
            }

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

                    if meta.path.is_ident("replace") {
                        meta.input.parse::<Token![=]>()?;
                        replace = meta.input.parse::<Type>().ok();
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
                let fields = data
                    .variants
                    .clone()
                    .into_pairs()
                    .map(|pair| {
                        let variant = pair.into_value();
                        variants.push(variant.clone());
                        variant
                    })
                    .flat_map(|v| {
                        let variant_name = v.ident.clone();
                        v.fields
                            .iter()
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

        if let Some(lifetime) = derive_input
            .generics
            .lifetimes()
            .find(|l| l.lifetime.ident != format_ident!("static"))
        {
            Err(input.error(format!("Struct must be `'static`, therefore may not contain any fields w/ generic lifetimes. Please remove `'{}`", lifetime.lifetime.ident)))
        } else {
            Ok(Self {
                span: input.span(),
                name,
                group,
                generics: derive_input.generics,
                fields,
                rename: reality_rename,
                on_load: reality_on_load,
                on_unload: reality_on_unload,
                on_completed: reality_on_completed,
                plugin,
                ext,
                call,
                replace,
                vis: derive_input.vis,
            })
        }
    }
}

impl StructData {
    fn iter_parse_fields(&self) -> impl Iterator<Item = &StructField> {
        self.fields
            .iter()
            .filter(|_| self.replace.is_none())
            .filter(|f| f.is_parse)
    }

    fn iter_virtual_fields(&self) -> impl Iterator<Item = &StructField> {
        self.fields
            .iter()
            .filter(|_| self.replace.is_none())
            .filter(|f| f.is_virtual)
    }

    fn iter_decorated_fields(&self) -> impl Iterator<Item = &StructField> {
        self.iter_virtual_fields().filter(|f| f.is_decorated)
    }

    pub(crate) fn virtual_plugin(&self) -> TokenStream {
        let ident = &self.name;
        let virtual_ident = format_ident!("Virtual{}", ident);
        let original = self.generics.clone();
        let (impl_generics, ty_generics, where_clause) = original.split_for_impl();

        let field_helpers_impl = self.iter_virtual_fields().map(|f| {
            let name = &f.name;
            let field_ident = f.field_name_lit_str();
            let ty = f.field_ty();
            let absolute_ty = &f.ty;
            let offset = &f.offset;

            // Callback to use
            let get_fn = f.render_get_fn();
            let get_mut_fn = f.render_get_mut_fn();
            let get_ref_helper_fn_ident = format_ident!("__get_field_offset_{}_ref", offset);
            let get_mut_helper_fn_ident = format_ident!("__get_field_offset_{}_mut", offset);
            let set_helper_fn_ident = format_ident!("__set_field_offset_{}", offset);
            let push_helper_fn_ident = format_ident!("__push_field_offset_{}", offset);
            let insert_entry_helper_fn_ident = format_ident!("__insert_entry_field_offset_{}", offset);
            let take_helper_fn_ident = format_ident!("__take_field_offset_{}", offset);
            let encode_helper_fn_ident = format_ident!("__encode_field_offset_{}", offset);
            let decode_apply_helper_fn_ident = format_ident!("__decode_apply_field_offset_{}", offset);
            let filter_packet_helper_fn_ident = format_ident!("__filter_packet_field_offset_{}", offset);

            let push_helper_impl = f.vec_of.as_ref().map(|f| {
                quote_spanned!(f.span()=>
                    fn #push_helper_fn_ident(&mut self, value: #ty) -> bool {
                        self.#name.push(value);
                        true
                    }
                )
            }).unwrap_or(
                quote!(
                    fn #push_helper_fn_ident(&mut self, _: #ty) -> bool {
                        // no-op
                        false
                    }
            ));

            let insert_entry_helper_impl = f.map_of.as_ref().map(|f| {
                quote_spanned!(f.span()=>
                    fn #insert_entry_helper_fn_ident(&mut self, key: impl Into<String>, value: #ty) -> bool {
                        self.#name.insert(key.into(), value).is_none()
                    }
                )
            }).unwrap_or(
                quote!(
                    fn #insert_entry_helper_fn_ident(&mut self, _: impl Into<String>, _: #ty) -> bool {
                        // no-op;
                        false
                    }
            ));


            let set_helper = f.variant.as_ref().map(|(variant, enum_ty)| {
                quote_spanned!(f.span=>
                    let changed = if let #enum_ty::#variant { #name, .. } = &self {
                        #name != &value
                    } else {
                        false
                    };

                    if let #enum_ty::#variant { #name, .. } = self {
                        *#name = value;
                        changed
                    } else {
                        false
                    }
                )
            }).unwrap_or(quote_spanned!(f.span=>
                let changed = self.#name != value;
                self.#name = value;
                changed
            ));

            let take_helper = f.variant.as_ref().map(|(variant, enum_ty)| {
                quote_spanned!(f.span=>
                    if let #enum_ty::#variant { #name, .. } = self {
                        #name
                    } else {
                        unreachable!("Generated code is incorrect")
                    }
                )
            }).unwrap_or(quote_spanned!(f.span=> self.#name));

            let encode_helper_impl = quote_spanned!(f.span=>
                fn #encode_helper_fn_ident(vp: #virtual_ident) -> FieldPacket {
                    let mut packet =  <Self as OnParseField<#offset>>::empty_packet();
                    vp.#name.view_value(|v| {
                        let mut current = <Self as OnParseField<#offset>>::into_packet(v.to_owned());
                        current.owner_name = std::any::type_name::<#ident #ty_generics>().to_string();
                        current.field_name = <Self as runir::prelude::Field<#offset>>::field_name().to_string();
                        packet = current.into_wire::<#absolute_ty>()
                    });
                    packet
                }
            );


            let set_helper_impl = f.variant.as_ref().map(|(variant, enum_ty)| {
                quote_spanned!(f.span=>
                    let changed = if let #enum_ty::#variant { #name, .. } = &owner {
                        #name != v
                    } else {
                        false
                    };

                    if let #enum_ty::#variant { #name, .. } = &owner {
                        *v = #name.to_owned();
                        changed
                    } else {
                        false
                    }
                )
            }).unwrap_or(quote_spanned!(f.span=>
                let changed = v != &owner.#name;
                *v = owner.#name.to_owned();
                changed
            ));

            let decode_helper_impl = quote_spanned!(f.span=>
                /// Decode and apply a field packet to a virtual plugin,
                /// 
                /// Returns a field reference if the decoded value was applied succesfully.
                /// 
                /// To apply successfully: 
                /// - The current owner state must return true when set_field is called on the packet
                /// - The value must not be equal to the current value
                /// - The owner was modified successfully
                /// 
                fn #decode_apply_helper_fn_ident(mut vp: #virtual_ident, fp: FieldPacket) -> anyhow::Result<FieldRef<Self, #ty, #absolute_ty>> {
                    let mut owner = vp.current();

                    if owner.set_field(fp.into_field_owned()) {
                        let applied = vp.#name.edit_value(|_, v| {
                            #set_helper_impl
                        });

                        if applied {
                            vp.#name.pending();
                            Ok(vp.#name)
                        } else {
                            Err(anyhow::anyhow!("No changes were applied, edit value returned false"))
                        }
                    } else {
                        Err(anyhow::anyhow!("No changes were applied, set field returned false"))
                    }
                }
            );

            let name_lit = f.field_name_lit_str();
            let filter_packet_helper_impl = quote_spanned!(f.span=>
                fn #filter_packet_helper_fn_ident(vp: &#virtual_ident, fp: &FieldPacket) -> anyhow::Result<FieldRef<Self, #ty, #absolute_ty>> {
                    if fp.field_offset == #offset && fp.field_name == #name_lit {
                        Ok(vp.#name.clone())
                    } else {
                        Err(anyhow::anyhow!("Does not match"))
                    }
                }
            );

            quote_spanned! {f.span=>
                fn #get_ref_helper_fn_ident(&self) -> (&str, &#absolute_ty) {
                    (#field_ident, #get_fn)
                }

                fn #get_mut_helper_fn_ident(&mut self) -> (&str, &mut #absolute_ty) {
                    (#field_ident, #get_mut_fn)
                }

                fn #set_helper_fn_ident(&mut self, value: #absolute_ty) -> bool {
                    #set_helper
                }

                fn #take_helper_fn_ident(self) -> #absolute_ty {
                    #take_helper
                }

                #push_helper_impl
                #insert_entry_helper_impl
                #encode_helper_impl
                #decode_helper_impl
                #filter_packet_helper_impl
            }
        });

        let vtable_field_helpers_impl = self.iter_virtual_fields().map(|f| {
            let ty = f.field_ty();
            let absolute_ty = &f.ty;
            let offset = &f.offset;

            let get_ref_helper_fn_ident = format_ident!("__get_field_offset_{}_ref", offset);
            let get_mut_helper_fn_ident = format_ident!("__get_field_offset_{}_mut", offset);
            let set_helper_fn_ident = format_ident!("__set_field_offset_{}", offset);
            let push_helper_fn_ident = format_ident!("__push_field_offset_{}", offset);
            let insert_entry_helper_fn_ident = format_ident!("__insert_entry_field_offset_{}", offset);
            let take_helper_fn_ident = format_ident!("__take_field_offset_{}", offset);
            let vtable_helper_fn_ident = format_ident!("__field_offset_{}_vtable", offset);
            let encode_helper_fn_ident = format_ident!("__encode_field_offset_{}", offset);
            let decode_apply_helper_fn_ident = format_ident!("__decode_apply_field_offset_{}", offset);
            let filter_packet_helper_fn_ident = format_ident!("__filter_packet_field_offset_{}", offset);

            quote_spanned! {f.span=>
                fn #vtable_helper_fn_ident() -> &'static FieldVTable<#ident, #ty, #absolute_ty>
                #where_clause
                {
                    static #vtable_helper_fn_ident: std::sync::OnceLock<FieldVTable<#ident, #ty, #absolute_ty>> = std::sync::OnceLock::new();

                    #vtable_helper_fn_ident.get_or_init(|| FieldVTable::new(
                        Self::#get_ref_helper_fn_ident,
                        Self::#get_mut_helper_fn_ident,
                        Self::#set_helper_fn_ident,
                        Self::#push_helper_fn_ident,
                        Self::#insert_entry_helper_fn_ident,
                        Self::#take_helper_fn_ident,
                        Self::#encode_helper_fn_ident,
                        Self::#decode_apply_helper_fn_ident,
                        Self::#filter_packet_helper_fn_ident,
                    ))
                }
            }
        });

        let vtable_field_impl = self.iter_virtual_fields().map(|f| {
            let name = &f.name;
            let ty = f.field_ty();
            let absolute_ty = &f.ty;

            quote_spanned! {f.span=>
                pub #name: FieldRef<#ident #ty_generics, #ty, #absolute_ty>,
            }
        });

        let vtable_field_new_impl = self.iter_virtual_fields().map(|f| {
            let name = &f.name;
            let offset = f.offset;
            let vtable_helper_fn_ident = format_ident!("__field_offset_{}_vtable", offset);
            quote_spanned! {f.span=>
                #name: FieldRef::new(
                    owner.clone(),
                    #ident::#vtable_helper_fn_ident(),
                ),
            }
        });

        let on_read_fields = self
            .iter_virtual_fields()
            .map(|f| {
                let offset = &f.offset;
                let name = &f.name;
                // let ty = f.field_ty();

                quote_spanned!(f.span=>
                    impl #impl_generics OnReadField<#offset> for #ident #ty_generics #where_clause {
                        #[inline]
                        fn read(virt: &Self::Virtual) -> &FieldRef<Self, Self::ParseType, Self::ProjectedType> {
                            &virt.#name
                        }
                    }
                )
            });

        let on_write_fields = self
            .iter_virtual_fields()
            .map(|f| {
                let offset = &f.offset;
                let name = &f.name;

                quote_spanned!(f.span=>
                    impl #impl_generics OnWriteField<#offset> for #ident #ty_generics #where_clause {
                        #[inline]
                        fn write(virt: &mut Self::Virtual) -> &mut FieldRef<Self, Self::ParseType, Self::ProjectedType> {
                            &mut virt.#name
                        }
                    }
                )
            });

        let virt_vis = &self.vis;

        let virtual_ref = quote_spanned!(self.span=>
            /// Virtual interface over plugin,
            ///
            #[derive(Reality)]
            #[reality(replace = #ident)]
            #virt_vis struct #virtual_ident {
                owner: std::sync::Arc<tokio::sync::watch::Sender<#ident>>,
                #(#vtable_field_impl)*
            }

            impl FieldRefController for #virtual_ident {
                type Owner = #ident;

                fn listen_raw(&self) -> tokio::sync::watch::Receiver<Self::Owner> {
                    self.owner.subscribe()
                }

                fn send_raw(&self) -> std::sync::Arc<tokio::sync::watch::Sender<Self::Owner>> {
                    self.owner.clone()
                }

                fn current(&self) -> Self::Owner {
                    self.owner.subscribe().borrow().to_owned()
                }
            }

            impl #impl_generics From<#ident #ty_generics> for #virtual_ident #where_clause  {
                fn from(init: #ident) -> #virtual_ident {
                    let (owner, rx) = tokio::sync::watch::channel(init);
                    let owner = std::sync::Arc::new(owner);
                    Self {
                        owner: owner.clone(),
                        #(#vtable_field_new_impl)*
                    }
                }
            }

            impl NewFn for #virtual_ident {
                type Inner = #ident;

                fn new(value: Self::Inner) -> Self {
                    #virtual_ident::from(value)
                }
            }

            impl ToOwned for #virtual_ident {
                type Owned = #ident;

                fn to_owned(&self) -> Self::Owned {
                    self.current()
                }
            }

            impl std::borrow::Borrow<#virtual_ident> for #ident {
                fn borrow(&self) -> &#virtual_ident {
                    unreachable!("This wouldn't make sense since the virtual type is less current than the actual type")
                }
            }
        );

        //
        // ^ -- TODO -- Create a local action that gets the latest, creates a new hosted resource, publishes, starts, and then collects
        //

        quote_spanned!(self.span=>
            impl #impl_generics #ident #ty_generics #where_clause {
                #(#field_helpers_impl)*

                #(#vtable_field_helpers_impl)*
            }

            #(#on_read_fields)*
            #(#on_write_fields)*

            #virtual_ref
        )
    }

    /// Implements visit traits,
    ///
    pub(crate) fn visit_trait(
        &self,
        impl_generics: &ImplGenerics<'_>,
        ty_generics: &TypeGenerics<'_>,
        where_clause: &Option<&WhereClause>,
    ) -> TokenStream {
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
                // let ty = f.field_ty();
                let offset = &f.offset;
                quote_spanned!(f.span=>
                    <Self as OnParseField<#offset>>::get_field(self)
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
                // let ty = f.field_ty();
                let offset = &f.offset;
                let field_name_lit = f.field_name_lit_str();
                quote_spanned!(f.span=>
                    (#field_name_lit, #offset) => { *<Self as OnParseField<#offset>>::get_field_mut(self).value = field.value; true }
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

        let fields = self.fields.iter().fold(HashMap::new(), |mut acc, f| {
            let key = (&f.ty, f.field_ty());

            if !acc.contains_key(&key) {
                acc.insert(key, vec![f.clone()]);
            } else if let Some(list) = acc.get_mut(&key) {
                list.push(f.clone());
            }

            acc
        });

        let virtual_visit_impls = fields.iter().map(|((absolute_ty, ty), fields)| {
            let owner = &self.name;

            let packet_routes = fields.iter().filter(|f| f.is_virtual).map(|f| {
                let offset = &f.offset;

                quote_spanned!(f.span=>
                    routes.route::<#offset>()
                )
            });

            let packet_routes_mut = fields.iter().filter(|f| f.is_virtual).map(|f| {
                let offset = &f.offset;

                quote_spanned!(f.span=>
                    visit(<Self as OnWriteField::<#offset>>::write(virt));
                )
            });

            quote!(
                impl #impl_generics VisitVirtual<#ty, #absolute_ty> for #owner #ty_generics #where_clause {
                    fn visit_fields<'a>(routes: &'a PacketRoutes<Self>) -> Vec<&'a FieldRef<Self, #ty, #absolute_ty>> {
                        vec![
                            #(#packet_routes),*
                        ]
                    }
                }

                impl #impl_generics VisitVirtualMut<#ty, #absolute_ty> for #owner #ty_generics #where_clause {
                    fn visit_fields_mut(virt: &mut Self::Virtual, mut visit: impl FnMut(&mut FieldRef<Self, #ty, #absolute_ty>)) {
                        #(#packet_routes_mut)*
                    }
                }
            )
        });

        quote_spanned!(self.span=>
            #(#visit_impls)*

            #(#virtual_visit_impls)*
        )
    }

    /// Returns token stream of generated AttributeType trait
    ///
    pub(crate) fn attribute_type_trait(self) -> TokenStream {
        let ident = &self.name;
        let original = self.generics.clone();
        let (impl_generics, ty_generics, where_clause) = original.split_for_impl();
        let ty_generics = ty_generics.clone();

        let visit_impl = self.visit_trait(&impl_generics, &ty_generics, &where_clause);

        let symbol = self.attr_symbol(ident);
        // let fields = self.fields.clone();
        let fields = self.iter_parse_fields().enumerate().map(|(offset, f)| {
            // let ty = &f.field_ty();
            if let Some(_) = f.attribute_type.as_ref() {
                quote_spanned! {f.span=>
                    parser.add_parseable_attribute_type_field::<#offset, Self>();
                }
            } else if f.ext {
                quote_spanned! {f.span=>
                    parser.add_parseable_extension_type_field::<#offset, Self>();
                }
            } else {
                let comment = LitStr::new(
                    format!("Parsing field `{}`", f.name).as_str(),
                    Span::call_site(),
                );
                quote_spanned! {f.span=>
                    let _ = #comment;
                    parser.add_parseable_field::<#offset, Self>();
                }
            }
        });

        let runir_field_impl = self.iter_virtual_fields().map(|f| {
            let field_ident = f.field_name_lit_str();
            let ty = f.field_ty();
            let absolute_ty = &f.ty;
            let ffi_ty = f.ffi.as_ref().map(|f| {
                quote_spanned!(f.span()=>
                    type FFIType = #f
                )
            }).unwrap_or(quote!(type FFIType = ()));
            
            let offset = &f.offset;

            quote_spanned! {f.span=>
                impl #impl_generics runir::prelude::Field<#offset> for #ident #ty_generics #where_clause {
                    type ParseType = #ty;

                    type ProjectedType = #absolute_ty;

                    #ffi_ty;

                    fn field_name() -> &'static str {
                        #field_ident
                    }
                }
            }
        });

        //  Implementation for fields parsers,
        //
        let fields_on_parse_impl = self.iter_virtual_fields().map(|f| {
            let field_ident = f.field_name_lit_str();
            let ty = f.field_ty();
            let offset = &f.offset;

            // Callback to use
            let callback = f.render_field_parse_callback();
            let get_fn = f.render_get_fn();
            let get_mut_fn = f.render_get_mut_fn();

            let mut_value = f.set_of.as_ref().or(f.map_of.as_ref()).map(|_| quote!(mut));

            quote_spanned! {f.span=>
                impl #impl_generics OnParseField<#offset> for #ident #ty_generics #where_clause {
                    #[allow(unused_variables)]
                    fn on_parse(&mut self, #mut_value value: #ty, _input: &str, _tag: Option<&String>) -> ResourceKey<Property> {
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

        if let Some(replace) = self.replace.as_ref() {
            quote_spanned! {self.span=>
                impl #impl_generics runir::prelude::Recv for #ident #ty_generics #where_clause {
                    fn symbol() -> &'static str {
                        #symbol
                    }
                }

                impl #impl_generics AttributeType<Shared> for #ident #ty_generics #where_clause {
                    fn parse(parser: &mut AttributeParser<Shared>, content: impl AsRef<str>) {
                        <#replace as AttributeType<Shared>>::parse(parser, content)
                    }
                }
            }
        } else {
            let virtual_impl = self.virtual_plugin();

            quote_spanned! {self.span=>
                impl #impl_generics runir::prelude::Recv for #ident #ty_generics #where_clause {
                    fn symbol() -> &'static str {
                        #symbol
                    }
                }

                impl #impl_generics AttributeType<Shared> for #ident #ty_generics #where_clause {
                    fn parse(parser: &mut AttributeParser<Shared>, content: impl AsRef<str>) {
                        let mut enable = parser.parse_attribute::<Self>(content.as_ref());

                        if enable.is_ok() {
                            #(#fields)*
                        }
                    }
                }

                #virtual_impl

                #(#runir_field_impl)*

                #(#fields_on_parse_impl)*

                #visit_impl
            }
        }
    }

    /// Get the attribute ty symbol,
    ///
    fn attr_symbol(&self, ident: &Ident) -> TokenStream {
        let group = self
            .group
            .as_ref()
            .map(|g| quote_spanned!(self.span=> #g))
            .unwrap_or(quote_spanned!(self.span=> std::env!("CARGO_PKG_NAME")));
        let name = self.rename.clone().unwrap_or(LitStr::new(
            ident.to_string().to_lowercase().as_str(),
            self.span,
        ));

        quote_spanned!(self.span=>
            concat!(#group, '.', #name)
        )
    }

    /// Returns token stream of generated AttributeType trait
    ///
    pub(crate) fn object_type_trait(self) -> TokenStream {
        let name = self.name.clone();
        let original = self.generics.clone();
        let (impl_generics, ty_generics, where_clause) = original.split_for_impl();

        let on_load = self
            .on_load
            .clone()
            .map(|p| quote!(#p(parser, storage, rk).await))
            .unwrap_or(quote!(parser));
        let on_unload = self
            .on_unload
            .clone()
            .map(|p| quote!(#p(parser, storage, rk).await))
            .unwrap_or(quote!(parser));
        let on_completed = self
            .on_completed
            .clone()
            .map(|p| quote!(#p(storage)))
            .unwrap_or(quote!(None));

        let ext = self
            .fields
            .iter()
            .filter(|f| f.ext && self.replace.is_none())
            .map(|f| {
                let ty = f.field_ty();
                quote_spanned!(f.span=>
                    _parser.with_object_type::<Thunk<#ty>>();
                )
            });

        let plugins = self
            .fields
            .iter()
            .filter(|f| f.plugin && self.replace.is_none())
            .map(|f| {
                let ty = f.field_ty();
                quote_spanned!(f.span=>
                    #ty::register(host);
                )
            });

        let to_frame = self.iter_virtual_fields().map(|f| {
            let offset = f.offset;
            // let ty = f.field_ty();
            let pty = &f.ty;
            let _name = &f.name;

            f.variant.as_ref().map(|(variant, enum_ty)| {
                quote_spanned!(f.span=>
                    if let #enum_ty::#variant { #_name, .. } = self {
                        {
                            let mut packet = <Self as OnParseField<#offset>>::into_packet(#_name.clone());
                            packet.owner_name = std::any::type_name::<#name #ty_generics>().to_string();
                            packet.field_name = <Self as runir::prelude::Field<#offset>>::field_name().to_string();
                            packet.attribute_hash = Some(key.data);
                            packet.into_wire::<#pty>()
                        }
                    } else {
                        unreachable!("Generated code is incorrect")
                    }
                )
                }).unwrap_or(quote_spanned!(f.span=>
                {
                    let mut packet = <Self as OnParseField<#offset>>::into_packet(self.#_name.clone());
                    packet.owner_name = std::any::type_name::<#name #ty_generics>().to_string();
                    packet.field_name = <Self as runir::prelude::Field<#offset>>::field_name().to_string();
                    packet.attribute_hash = Some(key.data);
                    packet.into_wire::<#pty>()
                }))
        });

        let _s = self.clone();
        let synchronizable = _s.iter_decorated_fields().map(|f| {
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
            } else if f.vecdeq_of.is_some() {
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

        let init_virt_plugin = if self.replace.is_none() {
            let ident = &self.name;
            let virtual_ident = format_ident!("Virtual{}", ident);

            Some(quote_spanned!(self.span=>
                #[async_trait]
                impl CallAsync for #virtual_ident {
                    /// Initialize virtual mode for the plugin,
                    ///
                    async fn call(tc: &mut ThunkContext) -> anyhow::Result<()> {
                        enable_virtual_dependencies::<#ident>(tc).await?;

                        Ok(())
                    }
                }

                impl #impl_generics #name #ty_generics #where_clause {
                    pub fn to_virtual(self) -> #virtual_ident {
                        #virtual_ident::new(self)
                    }
                }
            ))
        } else {
            None
        };

        let plugin = if self.plugin {
            let ident = &self.name;
            let virtual_ident = if let Some(replace) = self.replace.as_ref() {
                format_ident!(
                    "Virtual{}",
                    replace
                        .to_token_stream()
                        .to_string()
                        .trim_start_matches("Virtual")
                )
            } else {
                format_ident!("Virtual{}", ident)
            };

            let route_fields = self.iter_virtual_fields().map(|f| {
                let offset = &f.offset;

                quote_spanned!(f.span=>
                    router.route_one::<#offset>()
                )
            });

            quote!(
            impl #impl_generics Plugin for #name #ty_generics #where_clause  {
                type Virtual = #virtual_ident;

                #[allow(unused_variables)]
                fn sync(&mut self, context: &ThunkContext) {
                    #(#synchronizable)*
                }

                fn listen_one(router: std::sync::Arc<PacketRouter<Self>>) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
                    Box::pin(async move {
                        let _ = tokio::join!(#(#route_fields),*);
                    })
                }
            }

            #init_virt_plugin

            impl #impl_generics #name #ty_generics #where_clause  {
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
                impl #impl_generics #name #ty_generics #where_clause  {
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
                impl #impl_generics CallAsync for #name #ty_generics #where_clause {
                    async fn call(context: &mut ThunkContext) -> anyhow::Result<()> {
                        #c(context).await
                    }
                }
            )
        });

        let unit_from_str = if self.fields.is_empty() {
            quote_spanned!(name.span()=>
                impl #impl_generics FromStr for #name #ty_generics #where_clause {
                    type Err = anyhow::Error;

                    fn from_str(_: &str) -> std::result::Result<Self, Self::Err> {
                        Ok(Self)
                    }
                }
            )
        } else {
            quote!()
        };

        let mut from_shared = None;
        if !self.fields.iter().any(|f| f.variant.is_some()) {
            from_shared = Some(self.clone().from_shared());
        }

        let object_type_trait = quote_spanned!(self.span=>
            #[async_trait(?Send)]
            impl #impl_generics BlockObject for #name #ty_generics #where_clause {
                async fn on_load(parser: AttributeParser<Shared>, storage: AsyncStorageTarget<Shared>, rk: Option<ResourceKey<Attribute>>) -> AttributeParser<Shared> {
                    #on_load
                }

                async fn on_unload(parser: AttributeParser<Shared>, storage: AsyncStorageTarget<Shared>, rk: Option<ResourceKey<Attribute>>) -> AttributeParser<Shared> {
                    #on_unload
                }

                fn on_completed(storage: AsyncStorageTarget<Shared>) -> Option<AsyncStorageTarget<Shared>> {
                    #on_completed
                }
            }

            impl #impl_generics ToFrame for #name #ty_generics #where_clause {
                fn to_frame(&self, key: ResourceKey<Attribute>) -> Frame {
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
            #from_shared
        );

        let mut attribute_type = self.clone().attribute_type_trait();
        attribute_type.extend(object_type_trait);
        attribute_type.extend(self.object_ty_api());
        attribute_type
    }

    fn object_ty_api(self) -> TokenStream {
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let name = &self.name;
        let fields = self.fields.iter().filter(|f| !f.ignore && !f.not_wire && self.replace.is_none()).map(|f| {
            let ty = &f.ty;
            let name = f.field_name_lit_str();
            let offset = f.offset;
            let wire_method = f.wire.as_ref().map(|f| quote_spanned!(f.span()=> #f)).unwrap_or(quote!(into_box));

            quote_spanned!(f.span=>
                (#offset, #name) => {
                    if let Some(value) = value.#wire_method::<#ty>() {
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

    fn from_shared(self) -> TokenStream {
        let (impl_generics, ty_generics, where_clause) = self.generics.split_for_impl();
        let name = &self.name;
        let fields = self.fields.iter().filter(|f| !f.ignore && self.replace.is_none()).map(|f| {
            let name = &f.name;
            let name_lit = f.field_name_lit_str();

            let _final = if f.option_of.is_some() {
                quote!()
            } else {
                quote!(.unwrap_or(self.#name))
            };

            quote_spanned!(f.span=>
                self.#name = value.take_resource(ResourceKey::<Self>::new().branch(#name_lit).transmute()).map(|a| *a)#_final;
            )
        });

        let _fields = self.fields.iter().filter(|f| !f.ignore && self.replace.is_none()).map(|f| {
            let name = &f.name;
            let name_lit = f.field_name_lit_str();

            let _final = if f.option_of.is_some() {
                quote!()
            } else {
                quote!(.unwrap_or_default())
            };

            quote_spanned!(f.span=>
                storage.put_resource(self.#name, ResourceKey::<Self>::new().branch(#name_lit).transmute())
            )
        });

        if !self.fields.is_empty() {
            quote_spanned!(self.span=>
                impl #impl_generics Pack for #name #ty_generics #where_clause {
                    /// Packs the receiver into storage,
                    ///
                    fn pack<S>(self, storage: &mut S)
                    where
                        S: StorageTarget
                    {
                        #(#_fields);*
                    }

                    /// Unpacks self from Shared,
                    ///
                    /// The default value for a field will be used if not stored.
                    ///
                    fn unpack<S>(mut self, mut value: &mut S) -> Self
                    where
                        S: StorageTarget
                    {
                        #(#fields)*
                        self
                    }
                }
            )
        } else {
            quote!()
        }
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
    assert_eq!(
        Some("\"Name\"".to_string()),
        field.rename.map(|r| r.to_token_stream().to_string())
    );
    assert_eq!("name", field.name.to_string().as_str());
    assert_eq!("String", field.ty.to_token_stream().to_string().as_str());
}
