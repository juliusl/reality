#[doc(hidden)]
#[macro_use]
pub mod macros;
pub use macros::*;

mod attributes;
pub use attributes::AttributeParser;
pub use attributes::AttributeType;
pub use attributes::CustomAttribute;
pub use attributes::StorageTarget;
pub use attributes::Simple as SimpleStorageTarget;

cfg_specs! {
    pub use attributes::specs::*;
}

mod attribute;
pub use attribute::Attribute;

mod value;
pub use value::Value;
