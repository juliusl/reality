mod struct_data;
mod struct_field;
mod enum_data;

use enum_data::EnumData;
use quote::quote_spanned;
use struct_data::StructData;
use syn::{parse_macro_input, DeriveInput};

/// Derives the AttributeType as well as field parsers,
///
#[proc_macro_derive(AttributeType, attributes(reality))]
pub fn derive_attribute_type(_item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let struct_data = parse_macro_input!(_item as StructData);

    struct_data.attribute_type_trait().into()
}

/// Derives Reality object includes several implementations,
///
#[proc_macro_derive(Reality, attributes(reality))]
pub fn derive_object_type(_item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let struct_data = parse_macro_input!(_item as StructData);

    struct_data.object_type_trait().into()
}
/// Derives Reality object includes several implementations,
///
#[proc_macro_derive(RealityEnum, attributes(reality))]
pub fn derive_flags(_item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let enum_data = parse_macro_input!(_item as EnumData);

    enum_data.render().into()
}

#[proc_macro_derive(RealityTest, attributes(reality))]
pub fn derive_reality_test(_item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let struct_data = parse_macro_input!(_item as DeriveInput);

    quote_spanned!(struct_data.ident.span()=>
    ).into()
}