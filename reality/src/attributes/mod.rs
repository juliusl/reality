mod attribute_type;

pub use attribute_type::AttributeType;
pub use attribute_type::AttributeTypeParser;
pub use attribute_type::Callback;
pub use attribute_type::CallbackMut;
pub use attribute_type::Handler;

mod storage_target;
use reality_derive::AttributeType;
pub use storage_target::prelude::*;

mod container;
pub use container::Container;

mod parser;
pub use parser::AttributeParser;

use self::attribute_type::OnParseField;

#[derive(AttributeType)]
pub struct Test<T: Send + Sync + 'static> {
    /// Name for test,
    /// 
    name: String,
    /// Value 
    /// 
    #[reality(ignore)]
    _value: T,
}
