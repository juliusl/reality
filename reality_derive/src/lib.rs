mod struct_data;
mod struct_field;

use struct_data::StructData;
use syn::parse_macro_input;

#[proc_macro_derive(AttributeType, attributes(reality))]
pub fn derive_attribute_type(_item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let struct_data = parse_macro_input!(_item as StructData);

    struct_data.attribute_type_trait().into()   
}